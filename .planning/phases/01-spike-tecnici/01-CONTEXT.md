# Phase 1: Spike tecnici - Context

**Gathered:** 2026-10-18
**Status:** Ready for planning
**Criticità:** critica — proposta da ROADMAP (parsing di file non fidati e upload fissano il design dell'ingestion), ratificata con questo CONTEXT. In `plan`: `implementation-risk-audit` + `test-design` di GPT via AgentBridge.

<domain>
## Phase Boundary

La fase chiude, con misure documentate su hardware noto, le decisioni tecniche che fissano porte e provider (upload, storage degli originali, parser/OCR, indice vettoriale, embedding, JEV, LLM, reranker baseline, cancellazione HTTP) e compila il threat model. Produce misure, ADR e aggiornamenti di `DEPENDENCIES.md`; il codice degli spike è usa-e-getta in `spikes/`, fuori dal workspace di prodotto e dalla CI. Non produce codice di prodotto.

Hardware degli spike (rilevato 2026-10-18): Intel i7-13700K 16C/24T, 32 GB RAM, NVIDIA RTX 3050 6 GB VRAM, Windows 11, Docker Desktop (backend WSL2).

Ricerca a supporto: `01-RICERCA-DISCUSSIONE.md` (Area A upload/storage/threat, Area B parser/OCR, Area C pgvector/embedding, Area D JEV/LLM/reranker). Ogni decisione sotto cita l'area del brief che la fonda; il brief porta URL e data di accesso.

</domain>

<decisions>
## Implementation Decisions

### Upload e storage (S-01, A-02)
- **D-01:** Originali in **object storage S3-compatibile con Garage** (AGPLv3, stabile, senza versioning: le versioni stanno nel dominio), client `object_store`, dietro la porta `DocumentStore`. Fonte: Area A §2 (MinIO CE archiviato 2026-04-25, nessuna immagine da ottobre 2025; RustFS GA solo dal 2026-09-16). Decisione utente: S3 da subito per realismo, scartato il default PG `bytea`.
- **D-02:** Coerenza oggetto/transazione: **chiave dell'oggetto = sha256 del contenuto**, PUT prima della transazione; poi transazione breve con metadati + outbox; job di GC elimina gli oggetti senza riga in DB più vecchi di N ore (N deciso nel plan). La risposta all'upload è **identica** che l'oggetto esistesse già o no: nessun side-channel della deduplica. Fonte: Area A §2 (2-phase PUT + GC orfani) e §3 (threat side-channel dedup).
- **D-03:** Upload in streaming su **tempfile**, validazione completa (magic bytes, struttura DOCX, limiti zip, sha256 calcolato in streaming) **prima** di toccare storage e DB. Fonte: Area A §1 (axum 0.8.9 `Multipart` + `field.chunk()`, `field.bytes()` escluso; limite default Multipart 2 MB da alzare). Su Docker Desktop il tempfile sta su volume Linux, non bind NTFS.
- **D-04:** **Limite rigido 25 MB** (413 oltre), applicato a tre livelli (`RequestBodyLimitLayer`, `DefaultBodyLimit` sulla sola route di upload, contatore manuale). Il target TTQ resta definito su 10 MB. Fonte: Area A §1; README (target TTQ).
- **D-05:** Difese file = **ASVS L1 + regole economiche**: limiti sugli archivi DOCX (anti zip-bomb), quarantena dei PDF con `/JavaScript`, `/Launch` (anche quando l'oggetto è nascosto in un object stream compresso). I PDF cifrati si ammettono solo se si aprono con la password utente vuota (cifratura dei soli permessi) e si analizzano sul contenuto decifrato; quelli che richiedono una password si rifiutano (`Encrypted`). Gli allegati incorporati (`/EmbeddedFile`) si contano e non si estraggono mai: non sono un motivo di quarantena, ma i loro stream restano soggetti agli stessi tetti di filtri e decompressione. Antivirus/CDR (ASVS L2) rinviati con un ADR. Fonte: Area A §3 (ASVS 5.0 V5, OWASP File Upload Cheat Sheet); emendata dalla decisione utente del 2026-10-21; piano 01-07 v4, commit `66be9b6` sul branch `task/PLAN-010-07`.
  - *Storia (2026-10-21, decisione utente):* la prima stesura diceva «rifiuto dei PDF cifrati, quarantena dei PDF con `/JavaScript`, `/Launch`, `/EmbeddedFile`». Nello smoke del prescan nel worker queste regole rifiutavano PDF benigni del corpus, fra cui uno cifrato solo per i permessi e uno con un allegato; l'utente le ha superate nel ripasso del piano 01-07 (v4, commit `66be9b6` sul branch `task/PLAN-010-07`).
- **D-06:** Lo spike S-01 misura **tre adapter** con le stesse misure: Garage, PG `bytea` a chunk da 1 MiB (baseline per motivare la scelta nell'ADR) e adapter nullo (solo streaming + hash). Fonte: Area A §5.

### Parser, OCR e pagine (S-02, A-05, A-10)
- **D-07:** **PDFium** (`pdfium-render` 0.9.4, MIT/Apache) per testo, bbox e numero di pagina su tutte le pagine; **Docling** (MIT) come worker esterno **solo sulle pagine con tabelle** (conti economici, stati patrimoniali). Lo spike misura Docling su CPU e su GPU RTX 3050. Fonte: Area B §1 (Docling 3,1 s/pagina CPU ≈ 78 s per 25 pagine contro target 15 s; PDFium senza struttura tabellare; MuPDF/PyMuPDF scartati AGPL, Poppler GPL).
- **D-08:** **Isolamento in processo separato già nell'MVP**: un processo figlio per documento con limite di memoria (cgroup/`--memory` nel container, Job Object su Windows), timeout rigido, tetto ai byte decompressi. Fonte: Area B §4 (CVE PDFium 2026 fino a CVSS 8.8; PDFium non thread-safe, mutex globale in pdfium-render; OWASP sandbox). Coerente con ADR-0001: il processo isolato è una modalità di esecuzione del Document Service.
  - *Precisazione (2026-10-21, decisione utente):* nel budget aggregato dei figli la prenotazione di memoria di un job vale 3 × `VmHWM` (memoria residente di picco misurata sugli input benigni); `RLIMIT_AS` resta per ogni figlio come tetto rigido, calcolato su `VmPeak`. Fonte: decisione utente del 2026-10-21; piano 01-07 v4, commit `66be9b6` sul branch `task/PLAN-010-07`.
  - *Storia:* fino al piano 01-07 v3 la prenotazione si calcolava su `RLIMIT_AS` = 3 × `VmPeak`. Su un PDF da una pagina valeva circa 1,8 GiB, perché PDFium riserva molto spazio virtuale all'avvio, e nel budget da 2 GiB entrava un solo job alla volta.
- **D-09:** **OCR = Tesseract 5.5.3 da CLI** con `tessdata_best` `ita`+`eng`, output TSV con bbox per parola, nello stesso modello di processo isolato. **PP-OCRv5** misurato come sfidante: si cambia solo se vince nettamente. Fonte: Area B §2 (leptess fermo al 2023; ocrs early preview).
- **D-10:** **Locator di citazione a due varianti**: PDF = pagina + rettangoli; DOCX/Markdown = percorso di sezione + intervallo di caratteri (niente pagine per DOCX/MD). Il viewer mostra DOCX/MD come testo con evidenziazione. Fonte: Area B §3 (DOCX non salva pagine; LibreOffice con CVE 2026 e ~4 s/doc). Coerente con VIEW-01 (U-03: pagina obbligatoria per PDF, highlight quando ci sono coordinate).
  - *Storia (2026-10-21, discuss fase 3, decisione utente):* il locator diventa **multi-span con al massimo un confine**: `Pdf { spans: [{ page, rects }] }` e `Text { spans: [{ section_path, char_range }] }`, al massimo 2 elementi, perché un chunk non spezzi un paragrafo o una tabella a cavallo di pagina o di sezione. La forma a una pagina resta scritta sopra come prima stesura. La modifica entra nel brief di execute di 01-09 (ADR-0005) e di 01-16 → `.planning/phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md` D-10.
- **D-11:** Dataset di misura del parser: **TAT-DQA** (CC BY 4.0, bbox per parola), **DocLayNet** (CDLA-Permissive), **FinanceBench** (CC BY-NC 4.0: ammesso, progetto portfolio non commerciale), più documenti italiani pubblici con oracolo fatto a mano (nessun dataset IT esistente). Fonte: Area B §5.

### Indice ed embedding (S-03, A-03)
- **D-12:** **Ricerca esatta nell'MVP** (nessun indice ANN) con predicato di autorizzazione nella stessa query SQL; lo spike misura la **soglia** (numero di chunk/latenza) oltre cui passare a HNSW con `hnsw.iterative_scan`. Fonte: Area C §1 (HNSW + WHERE = post-filter, perdita di recall silenziosa con filtri selettivi; iterative scan limitato a `max_scan_tuples` 20.000). Pin pgvector ≥ 0.8.4 (fix corruzione HNSW in vacuum); immagine da verificare su Docker Hub.
- **D-13:** **Embedding locale su GPU**, servito da un **container TEI (Text Embeddings Inference) con CUDA** via passthrough GPU di Docker Desktop/WSL2, dietro la porta `Embedder` con adapter HTTP. Lo spike verifica **per primo** il passthrough GPU. Fonte: Area C §2. Decisione utente dopo valutazione dei rischi CPU (stima 20–50 s di solo embedding per 25 pagine con un modello da 0,6 B in fp32).
- **D-14:** Modello: **Qwen3-Embedding-0.6B** (Apache 2.0, 1024 dim, 32K contesto) default; **bge-m3** (MIT) e **multilingual-e5-large-instruct** (MIT) sfidanti, misurati su 60–100 query etichettate IT/EN/cross-lingual. Esclusi Jina v3/v4 (licenza). Fonte: Area C §2 (nessun benchmark su testi finanziari italiani: misura in-domain obbligatoria).
  - *Storia (2026-10-23, decisione utente, D-38):* Qwen3-Embedding-0.6B era il default; la misura di 01-10 lo supera con bge-m3, che diventa il default.
- **D-15:** **Ricerca ibrida**: `tsvector` + fusione RRF (k=60) come baseline; `pg_textsearch` (BM25) solo se lo spike mostra un guadagno misurato. Il filtro ACL sta in **entrambe** le parti della query. Fonte: Area C §1 (numeri e sigle finanziarie favoriscono il match lessicale; pg_textsearch richiede immagine derivata).

### JEV, LLM e citazioni (S-04/A-09, S-05, A-04, A-06)
- **D-16:** **Corpus pubblico/sintetico**: relazioni annuali pubbliche + documenti sintetici classificati per simulare la riservatezza. Invio a LLM e JEV cloud ammesso; il RBAC resta reale e testato (nessun chunk **non autorizzato** raggiunge JEV/LLM). Fonte: Area D §6 Q1 (JEV ospitato negli USA, retention «as long as necessary», ZDR solo enterprise; Anthropic 30 giorni).
- **D-17:** **JEV pinnato a `jev-1.13.0`** in configurazione (non `jev-latest`); ogni cambio di versione è deliberato e rilancia la calibrazione delle soglie; la versione risolta (`response.model`) si registra in ogni run di evaluation (chiude anche la parte tecnica di Q-07). Fonte: Area D §1. Da aggiornare in `DEPENDENCIES.md`.
- **D-18:** **LLM generativo via API** (nessun LLM locale): **Claude Haiku 5.5** (`claude-haiku-5-5`) per le domande semplici, **Claude Sonnet 5.5** (`claude-sonnet-5-5`) per quelle complesse. La **complessità la decide JEV** dentro `validate_query` (il `QueryDecision` porta un campo di complessità), coerente con il ruolo di «classificazione della query» (README, ADR-0002). Client `reqwest` diretto (nessun SDK Rust ufficiale). Fonte: Area D §4 (Haiku ≈ $0.0008/query; 8B locale al limite su 6 GB). Decisione utente.
- **D-19:** **Fallback se JEV non risponde**: generazione con **Sonnet 5.5**; la risposta segue poi la policy di degradazione (senza validazione JEV delle citazioni: degradata o rifiutata secondo policy, mai un controllo di sicurezza bypassato). Decisione utente.
- **D-20:** **Risposta in JSON tramite structured output** dell'LLM, citazioni **per affermazione**: schema indicativo `{answer, claims: [{text, citations: [chunk_id]}], refusal?}`. L'LLM cita solo `chunk_id` forniti nel contesto; versione, pagina, rettangoli o sezione+offset li aggiunge il sistema in modo deterministico dal DB; un `chunk_id` non fornito o non autorizzato fa scartare il claim; JEV valida claim–citazione. Niente Citations native di Claude (incompatibili con structured output: errore 400). Fonte: Area D §4; ADR-0002 (citation mapping deterministico, validazione semantica JEV). Decisione utente.
- **D-21:** **Reranker baseline**: no-rerank + **bge-reranker-v2-m3** (Apache-2.0) su GPU in container TEI, entrambi misurati contro JEV `rank`. jina-reranker-v2 escluso (licenza NC). Fonte: Area D §5; JEV per il reranking resta LOCKED (ADR-0002).
- **D-22:** Spike JEV e cancellazione come da Area D §7: forma della chiamata `rank` (una request per candidato con domande Noul vs state unico vs Choice), concorrenza 4/8/16 con conteggio dei 429, risoluzione degli alias, cancellazione con server mock h1/h2 e con JEV reale (lavoro addebitato dopo il drop?), deadline globale (`timeout_at` + `JoinSet`/`Semaphore`). Fonte: Area D §2–§3.

### Decisioni dopo l'audit GPT v1 (utente, 2026-10-19)

- **D-09 emendata:** lo sfidante OCR è **PP-OCRv6** (PaddleOCR 3.7.0, 2026-06-11; default `PP-OCRv6_medium`, 50 lingue), selezionato con flag esplicito e pesi identificati; PP-OCRv5 è un riferimento secondario opzionale, misurato accanto a v6 e senza effetto sulla decisione; se v6 non è selezionabile in modo esplicito (o i modelli risolti non sono della famiglia v6) la misura PaddleOCR si ferma e la scelta torna all'utente. La regola «si cambia solo se vince nettamente» su Tesseract resta. Fonti: [PaddleOCR 3.x, uso della pipeline OCR](https://www.paddleocr.ai/latest/en/version3.x/pipeline_usage/OCR.html), [release v3.7.0](https://github.com/PaddlePaddle/PaddleOCR/releases), consultate 2026-10-19. *Storia:* la prima stesura vincolava PP-OCRv5 (Area B §2); deepsearch_1 documentava solo v5/v4/v3 per `ocr_version`.
- **D-23:** nello spike S-01 il carico si genera con `curl` + `xargs -P` invece di `oha`, che non varia il corpo delle richieste (deviazione da RESEARCH §3, ratificata).
- **D-24:** proposte ratificate per ADR-0005 (locator): offset in Unicode scalar su testo NFC, conversione UTF-16 nel viewer; `section_path` con ordinale fra sorelle omonime; testo di sezione = solo testo proprio, sottosezioni escluse; proiezione canonica versionata `cka-text-v1` (ogni modifica = nuova versione).
- **D-25:** i PDF con stream LZW/RunLength o filtri ignoti si rifiutano (`Unsupported`). Gli object stream non si rifiutano per numero: il tetto è sulla somma reale dei byte decompressi, object stream compresi, che non supera 512 MiB (oltre, `DecompressionLimit`). Sui PDF cifrati ammessi vale in più una quota per stream di min(64 MiB, ⌊512 MiB / n⌋), con n il numero di object stream. È un progetto dimostrativo: il limite **va dichiarato nella documentazione ufficiale** (ADR-0004 e documentazione utente/README del prodotto, sezione limiti noti). Fonte della regola sugli object stream: decisione utente del 2026-10-21; piano 01-07 v4, commit `66be9b6` sul branch `task/PLAN-010-07`.
  - *Storia (2026-10-21, decisione utente):* la prima stesura diceva «i PDF con stream LZW/RunLength o filtri ignoti, o con object stream oltre il limite, si rifiutano (`Unsupported`/`DecompressionLimit`)». Il limite era sul numero di object stream, con la loro espansione stimata come conteggio × 64 MiB. Nello smoke nel worker rifiutava PDF benigni con 12-312 object stream per file.
- **D-26:** tolleranze del controllo analitico delle bbox (0,5 px / 1 pt / 3 pt) e pin `pip==24.3.1` / `pip-tools==7.4.1` accettati per lo spike; da rifinire in produzione.
- **D-27 (rinviata a dopo la misura):** se `docling_isolation.csv` mostra che `page_range` non riduce il costo, si sceglie fra le opzioni in «Deferred Ideas» (Docling e pagine-tabella).

- **D-28:** memoria del worker resta **2 GiB** (compose di 01-02 invariato): 01-13 misura e riporta il `max_admitted` reale per concorrenza richiesta e il dimensionamento in ADR-0004 si dichiara estrapolato da quel dato.
- **D-29:** DocLayNet in 01-08 passa da 20 a **40 pagine**, split per documento 20 di taratura + 20 di verifica (holdout); l'esito si giudica solo sull'holdout.
- **D-30:** il piano 01-07 si divide: 01-07 (contratto del job, lanciatore, prescan, figli, calibrazione dei limiti) e il nuovo 01-18 (casi ostili, budget aggregato, overhead, `RISULTATI-isolamento.md`), così 01-08 parte senza attendere le misure ostili. Respinta la divisione della fase in 1.1/1.2 (i decimali sono riservati alla gap-closure; nuovo audit di tutti i piani; conflitti `proprieta-file` sui file condivisi degli spike).
- **D-31 (decisione utente 2026-10-19, completata il 2026-10-22):** spesa Anthropic di 01-12 dimezzata: Haiku su tutte le 30 query, Sonnet solo sulle `complex` e `no_answer` (3 ripetizioni), fallback su 15 query a seme fisso, `S06_MAX_COST_USD` = 2; injection e JEV invariati. La regola di routing diventa: Haiku resta sulle `simple` se sulle `complex` sta entro 0,05 da Sonnet nel tasso di claim supportati e se sulle `simple` raggiunge da solo ≥ 0,85; altrimenti la scelta torna all'utente. 01-12 si esegue subito, in parallelo all'attesa degli audit GPT di 01-07 e 01-18, invece che in onda 7.
- **D-32 (decisione utente 2026-10-22):** la regola di routing di D-31 è fallita su entrambe le condizioni (Haiku 0,6852 contro Sonnet 0,7931 sulle `complex`; Haiku 0,7241 sulle `simple`, sotto 0,85). Prima di scegliere il modello delle `simple` si misura Sonnet sulle stesse `simple` (Task 3 di 01-12, CSV separato, Soglie e CSV precedenti invariati, stesso tetto di 2 USD); la scelta resta all'utente, ADR-0011 `Proposed` fino ad allora.
- **D-33 (decisione utente 2026-10-22):** Sonnet 5.5 è il modello generativo anche delle query `simple` (quindi di tutte): sulle 13 `simple` della misura supplementare Sonnet 1,0000 di claim supportati contro 0,7241 di Haiku, costo ≈ 0,0037 USD per risposta (≈ 19,5× Haiku). JEV continua a restituire la complessità in `validate_query`, che oggi non cambia il modello. Emenda D-18 (Haiku per le semplici) → ADR-0011.
  - *Storia:* D-18 diceva Haiku per le domande semplici e Sonnet per le complesse; superata dalle misure di 01-12 (D-31, D-32).
- **D-34 (decisione utente 2026-10-22):** M8 di 01-18 resta dichiarata «non raggiunta» (24/26 casi): `deep-nesting` e `jbig2-malformed` danno `ok` invece di `Corrupt`/`Crashed`, con contenimento intatto (padre vivo, 0 processi residui). Gli attesi del manifest non si ritoccano. Motivo dichiarato: il figlio estrae solo il testo e non decodifica le immagini, quindi il decoder JBIG2 non viene esercitato. Il caso JBIG2 si riprova in 01-13, dove le pagine si rasterizzano per l'OCR: richiede un ripasso del piano 01-13 prima della sua onda.
- **D-35 (decisione utente 2026-10-22):** i due difetti dei driver di misura di 01-07 trovati da 01-18 (`budget` lascia `/work/selftest-input` sul percorso d'errore; `overhead` in modo `in-process-mutex` si interrompe su `FormatError` e tronca il CSV a 6/20) si correggono nella gap-closure 1.4 (`riapre_perimetro: [1]`), in parallelo all'onda 6; il CSV `overhead` completo serve a M9.
- **D-36 (decisione utente 2026-10-22):** D-27 si chiude con l'opzione 4 («lasciare com'è»): la condizione non si è verificata, perché `page_range` riduce il costo (una pagina in un PDF da 400 costa +0,13 s di processing su GPU e −0,12 s su CPU). Resta un **punto aperto per l'ADR di ingestion**: il riavvio di Docling per documento (≈ 19 s su GPU, ≈ 21 s su CPU) e la precisione dell'euristica pagine-tabella (0,435 sull'holdout; nel documento IT da 25 pagine 13 inviate a Docling, 1 tabella vera) portano a ≈ 26 s su GPU contro un TTQ di 15 s. Fonte: `spikes/s02-parser/RISULTATI-estrazione.md`.
- **D-37 (decisione utente 2026-10-22):** il prescan v6 di 01-07 rifiuta come `Unsupported` / `image_filter_on_non_image` 4 pagine DocLayNet benigne di `OTC_NSANY_2004` (`spikes/s02-parser/src/prescan.rs:684-686`). È un falso positivo e si corregge nella gap-closure 1.4, allargata con un secondo piano (01.4-02) sul prescan: il caso benigno passa, i casi ostili restano respinti, la calibrazione dei limiti si rifà. Rilevato da 01-08 (`RISULTATI-estrazione.md`, «Prescan su PDF benigni»).
- **D-38 (decisione utente 2026-10-23):** il modello di embedding è **bge-m3** (MIT), servito da TEI CUDA, e non più Qwen3-Embedding-0.6B. Nella misura di 01-10 (74 query IT/EN/cross-lingual ratificate, 60 valutabili) ha nDCG@10 0,723, contro 0,660 di Qwen3 (senza istruzione) e 0,618 di e5. Il vantaggio supera il margine di parità di 0,02 e bge-m3 passa tutte le soglie M-1..M-8: query p95 12 ms, 25 pagine in 1,02 s, VRAM 1.453 MiB, leak 0. Qwen3 è anche 2,2× più lento in servizio. Emenda D-14 → ADR-0007 (`Proposed` fino a verify). Fonte: `spikes/s03-pgvector/RISULTATI-embedding.md`.
- **D-39 (decisione utente 2026-10-23):** il tetto `max_image_bytes` del prescan si calcola su `/Width × /Height` dichiarati, mentre il decoder JPEG legge le dimensioni dal marker SOF: un DCT che dichiara 1×1 e contiene un JPEG enorme lo aggira (RISK-16, TEST-14 di test_design_13). Si chiude **prima di 01-13** con la gap-closure 1.5 (`riapre_perimetro: [1]`): il prescan legge il SOF dei DCT e lo confronta con le dimensioni dichiarate, con un caso ostile SOF-divergente e la prova anche sul percorso `ocr_render`. 01-13 aspetta la 1.5; il suo TEST-14 si considera coperto dalle prove della 1.5.
  - *Storia (2026-10-23):* la copertura di TEST-14 si divide. La 1.5 copre il prescan e il percorso del lanciatore su `parse` ed `extract_pdf`. 01-13 copre `ocr_render`, che su main non esiste ancora, facendo passare la fixture della 1.5 nella sua prova del render ostile: è un obbligo scritto in `01.5-CONTRATTO.md`. Dettaglio in D-05 di `01.5-CONTEXT.md`.
- **D-40 (decisione utente 2026-10-23):** TEST-15 di 01-13, parte parse: M10 è stata misurata con la prenotazione 803057 KiB, mentre i limiti attuali danno 848777 KiB (D-12: non si rifà). 01-13 non blocca su questo. ADR-0004 dichiara che la concorrenza parse viene da M10 con la prenotazione vecchia e dal batch reale della 1.4 (2 job ok con i limiti nuovi). TEST-15 resta attivo per la parte OCR.
- **D-41 (decisione utente 2026-10-24):** la regola `upload-in-memory` di `skills/security-scanner/security_scan.py` controlla una parola (`.read_to_end(`) al posto della proprietà che ci interessa, cioè che ogni lettura di dati non fidati abbia un tetto. Sbaglia in due direzioni: segnala HIGH un `read_to_end` già limitato da `.take(` (falso positivo della fase 1.5) e non vede `io::copy` verso un buffer senza `take` (falso negativo). Scelta l'opzione B: la regola non segnala un `read_to_end` con `.take(` nella stessa istruzione, e una regola nuova segnala `io::copy` verso un `Vec` senza `take`. Scartate A (lasciarla com'è: il falso negativo resta e premia chi evita la parola) e C (eccezione annotata: è un bypass, vietato). Nella stessa correzione il prescan torna alla forma del piano, `take` + `read_to_end` (`spikes/s02-parser/src/prescan.rs`, ramo FlateDecode di `image_prefix`), senza il commento di giustificazione. Si applica in `/verify 1`, in una gap-closure decimale perché `prescan.rs` è un file chiuso; non blocca 01-13. La garanzia sostanziale resta dei test ostili e della review sul diff.

### Claude's Discretion
- **S-07 (firme Rust candidate di tutte le porte)**: le scrive lo spike, incluse le variazioni emerse qui — `Embedder` deve coprire anche l'embedding della query (oggi `embed_batch(&[ChunkDraft])` non basta); `DecisionEngine::validate_query` restituisce la complessità (D-18); il locator a due varianti (D-10) entra nei tipi di `crates/contracts`.
- N ore del GC degli oggetti orfani (D-02), soglie di pass/fail di dettaglio degli spike, budget VRAM fra TEI embedding, TEI reranker e Docling (6 GB condivisi: lo spike misura la convivenza).
- Corpus degli spike: scelta delle relazioni annuali pubbliche IT/EN e costruzione dei documenti sintetici classificati.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Ricerca di fase
- `.planning/phases/01-spike-tecnici/01-RICERCA-DISCUSSIONE.md` — brief a 4 aree con fonti (URL, data di accesso), piani di misura degli spike, file di test ostili.

### Decisioni di progetto
- `docs/adr/0001-system-boundaries-and-stack.md` — tre servizi, trait-porta, worker parte del Document Service.
- `docs/adr/0002-incremental-corpus-and-jev.md` — ruoli ammessi e vietati di JEV.
- `DEPENDENCIES.md` — stato Decisa/Proposta/Aperta; da aggiornare con gli esiti (A-02, A-03, A-04, A-05, A-06, A-09, A-10, JEV pin).
- `.planning/PROJECT.md` — decisioni bloccate e U-01..U-04.
- `.planning/ROADMAP.md` §Phase 1 e §Strategia di consegna — criteri di successo e perimetro file (`spikes/**`, nuovi ADR, `DEPENDENCIES.md`, `docs/security/threat-model.md`).

### Architettura e contratti
- `docs/architecture/interfaces-and-concurrency.md` — porte Rust, modello di concorrenza, spike elencati, race condition.
- `docs/architecture/architecture.md` — flussi di ingestion e query.
- `docs/data/corpus-strategy.md` — corpus, dataset di evaluation.
- `docs/security/threat-model.md` — da compilare (PLAT-07); input: Area A §3 (28 minacce candidate T-01..T-28).

### Requisiti
- `.planning/REQUIREMENTS.md` — PLAT-07 (threat model) e i requisiti che gli spike sbloccano (DOC-*, SEC-*, QRY-*, VIEW-*).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Nessun codice di prodotto: il repo contiene solo documenti e lo scheletro di directory (`.gitkeep`).

### Established Patterns
- Spike in `spikes/` fuori dal workspace di prodotto e dalla CI; nessun file di prodotto li importa (ROADMAP Phase 1, criterio 5).
- Gate e hook del loop già attivi: `crate-boundary-check` e `cargo` risultano «non applicabile» finché non esiste `Cargo.toml` alla radice; gli spike non devono creare un `Cargo.toml` alla radice del repo.

### Integration Points
- Gli esiti diventano input della Fase 2 (Fondazione) e delle Fasi 3–4 (Provider): firme delle porte (S-07), scelte di storage, parser, embedding, LLM.

</code_context>

<specifics>
## Specific Ideas

- Routing per complessità delegato a JEV (Haiku per il semplice, Sonnet per il complesso), con Sonnet come fallback se JEV non risponde.
- GPU RTX 3050 dedicata a embedding (TEI), reranker baseline e Docling; nessun LLM locale.
- La scelta S3/Garage è motivata anche dal valore portfolio; la baseline PG nello spike serve all'ADR e al case study.

</specifics>

<deferred>
## Deferred Ideas

- **Docling e pagine-tabella (D-27), opzioni da valutare dopo la misura di 01-08:**
  1. *Sotto-PDF nel figlio isolato:* nuova op `pdf_subset` nell'allowlist di 01-07: il figlio copia nel proprio `out_dir` le sole pagine-tabella (PDFium `FPDF_ImportPages` o lopdf), e Docling riceve solo quelle. Docling legge meno; resta il parsing PDF lato Docling, ma su un file ridotto e già passato dal prescan.
  2. *Immagini di pagina:* il figlio rasterizza le pagine-tabella (stessa via di `ocr_render`) e a Docling si inviano PNG. Docling non apre mai il PDF non fidato (superficie d'attacco minima), ma perde lo strato di testo e lavora come OCR + TableFormer: va misurato l'effetto su IoU e celle.
  3. *Un PDF per pagina:* una richiesta per pagina-tabella (sotto-PDF di una pagina), con timeout e riavvio per pagina e parallelismo naturale; costo di avvio moltiplicato per pagina.
  4. *Lasciare com'è:* se il costo fuori `page_range` risulta trascurabile.
  Le opzioni 1–3 cambiano il contratto del lanciatore (01-07) e quindi passano dalla fase che ne è proprietaria.

- Antivirus (ClamAV) e CDR per PDF/DOCX (ASVS L2): ADR di rinvio in questa fase, valutazione in Fase 14 (hardening).
- HNSW + `iterative_scan`: si attiva oltre la soglia misurata dallo spike (Fase 14 o quando il corpus la supera).
- `pg_textsearch` BM25: solo se lo spike mostra un guadagno misurato.

</deferred>

---

*Phase: 01-spike-tecnici*
*Context gathered: 2026-10-18*

**Modifica:** Claude (Anthropic) · 2026-10-18 · discuss 1 (decisioni utente)
**Modifica:** Claude (Anthropic) · 2026-10-19 · D-09 emendata (PP-OCRv6), D-23..D-27 e opzioni Docling/pagine-tabella (decisioni utente dopo l'audit GPT v1)
**Modifica:** Claude (Anthropic) · 2026-10-19 · D-28 (worker 2 GiB), D-29 (DocLayNet 40 pagine), D-30 (divisione di 01-07)
**Modifica:** Claude (Anthropic) · 2026-10-19 · D-09 emendata: ruolo di PP-OCRv5 allineato a 01-13 v3
**Modifica:** Claude (Anthropic) · 2026-10-21 · D-10: paragrafo *Storia* sul locator multi-span (decisione utente, discuss fase 3)
**Modifica:** Claude (Anthropic) · 2026-10-21 · allineamento alle regole del prescan e della calibrazione decise dall'utente (ripasso 01-07 v4)
**Modifica:** Claude (Anthropic) · 2026-10-22 · D-31 (costi e regola di routing di 01-12, anticipo dell'esecuzione)
**Modifica:** Claude (Anthropic) · 2026-10-22 · D-32 (misura supplementare di Sonnet sulle simple in 01-12)
**Modifica:** Claude (Anthropic) · 2026-10-22 · D-33 (Sonnet anche sulle simple, emenda D-18)
**Modifica:** Claude (Anthropic) · 2026-10-22 · D-34 (M8 dichiarata, JBIG2 a 01-13) e D-35 (gap-closure 1.4 sui driver di 01-07)
**Modifica:** Claude (Anthropic) · 2026-10-22 · D-36 (D-27 chiusa con l'opzione 4, punto aperto per l'ADR di ingestion) e D-37 (falso positivo del prescan nella gap-closure 1.4)
**Modifica:** Claude (Anthropic) · 2026-10-23 · D-38 (bge-m3 come modello di embedding, emenda D-14)
**Modifica:** Claude (Anthropic) · 2026-10-23 · D-39 (gap-closure 1.5 sul SOF JPEG prima di 01-13) e D-40 (M10 parse dichiarata in ADR-0004)
**Modifica:** Claude (Anthropic) · 2026-10-24 · D-41 (regola upload-in-memory dello scanner: opzione B, applicazione in /verify 1)
