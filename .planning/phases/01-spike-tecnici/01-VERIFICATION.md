---
phase: 01-spike-tecnici
verified: 2026-10-26T14:20:00Z
status: gaps_found
score: 105/110 verità verificate (5 criteri del ROADMAP più 100 must-have dei 18 PLAN; 4 per override; 5 verità ingegneristiche della banda ECC fallite, instradate alla gap-closure 1.6)
overrides_applied: 4
overrides:
  - must_have: "01-06: ogni oracolo ha il triplo guardrail completo, con un agente Claude separato dall'esecutore"
    reason: "Il derivatore è la stessa sessione gsd-executor di 01-06, distinta dall'orchestratore e dagli esecutori degli spike; double-check GPT e ratifica utente completi su 74 query e 11 oracoli del parser"
    accepted_by: "utente"
    accepted_at: "2026-10-21T00:00:00Z"
  - must_have: "01-09: lo stesso documento in MD e DOCX produce lo stesso percorso di sezione, lo stesso intervallo e lo stesso hash del testo canonico di sezione"
    reason: "Percorso 11/11 e intervallo 11/11; hash 8/11 per gli apostrofi U+2019 introdotti da pandoc nel DOCX. Un locator si risolve sempre sul file da cui è calcolato (doc_sha256); deviazione scritta in ADR-0005:80 e ratificata"
    accepted_by: "utente (ratifica-tutti, piano 01-17)"
    accepted_at: "2026-10-26T00:00:00Z"
  - must_have: "01-11: nessun passaggio non visibile al ruolo viene inviato a JEV, controllo deterministico prima di ogni richiesta e conteggio come hard gate"
    reason: "La guardia assert_visible esiste ed è provata dai test unitari; il contatore unauthorized_sent è zero per costruzione. Residuo accettato in 01-REVIEW.md §D; il prodotto conta sul corpo serializzato (threat model T-14)"
    accepted_by: "utente (instradamento della banda ECC)"
    accepted_at: "2026-10-26T00:00:00Z"
  - must_have: "01-12: simple → Haiku, complex → Sonnet"
    reason: "La regola D-31 è fallita sulle misure (0,6852 contro 0,7431 e 0,7241 contro 0,85, ricalcolati dal verifier su llm.csv); con D-33 Sonnet è il modello unico. La Choice di complessità JEV resta misurata (25/30)"
    accepted_by: "utente (D-33, ratificata con ADR-0011)"
    accepted_at: "2026-10-22T00:00:00Z"
gaps:
  - truth: "Lo scanner di sicurezza rileva tutte le forme che dichiara di coprire"
    status: failed
    reason: "La banda ECC (01-REVIEW.md §B.1) ha trovato regole che non scattano su forme reali: upload in memoria sensibile alla posizione di .take( e cieco a io::copy verso Vec (D-41), segreti con nomi e prefissi non coperti, file testuali non scansionati, fixture ed esclusioni senza allowlist, potatura troppo larga, file illeggibili ignorati, log su più righe, handler Axum senza argomenti"
    artifacts:
      - path: "skills/security-scanner/security_scan.py"
        issue: "regole upload-in-memory, segreti, no-leak e identità incomplete; exit code senza percorsi ambiguo"
      - path: "spikes/s02-parser/src/prescan.rs"
        issue: "la rilettura Flate usa io::copy al posto di take + read_to_end proprio per non far scattare la regola (857-867, commento 5239f0e)"
    missing:
      - "Correzione D-41: regola upload-in-memory indipendente dalla posizione di .take(, nuova regola su io::copy verso Vec, test sulle forme; prescan riportato a take + read_to_end"
      - "Regole dei segreti estese a *_SECRET_KEY, *_ACCESS_KEY, *_TOKEN e ai prefissi noti (sk-ant-, GK…)"
      - "Scansione dei soli segreti su .txt, .csv, .pem, .key"
      - "Allowlist versionata per il marcatore .security-scan-fixture e gate sulle directory escluse"
      - "Potatura limitata a target, node_modules e .git"
      - "File illeggibile come finding bloccante"
      - "Macro di log su più righe normalizzate prima delle regole no-leak"
      - "Handler Axum senza estrattore d'identità segnalati anche senza argomenti"
      - "Exit code senza percorsi chiarito"
  - truth: "Gli hook git applicano ai commit gli stessi controlli dello scanner, spike compresi, senza bloccare i commit di soli dati"
    status: failed
    reason: "01-REVIEW.md §B.2: i controlli Rust non guardano spikes/Cargo.toml, le regole dei segreti divergono da quelle dello scanner, e un commit di soli dati viene bloccato da security-scope-empty"
    artifacts:
      - path: ".githooks/controlli-comuni.sh"
        issue: "nessun controllo Rust sul workspace degli spike, regole dei segreti proprie, falso blocco security-scope-empty"
    missing:
      - "Controlli Rust anche su spikes/Cargo.toml, oppure dichiarazione esplicita che gli spike non sono coperti"
      - "Stesse regole dei segreti dello scanner"
      - "Commit di soli dati non più bloccati da security-scope-empty"
  - truth: "Il runner e i servizi del compose degli spike girano con privilegi minimi, segreti solo dove servono e pesi a revisione fissata"
    status: failed
    reason: "01-REVIEW.md §B.3, confermato sul file: il servizio runner (compose.yaml:219 e seguenti) non ha cap_drop, no-new-privileges né pids_limit e riceve tutti i segreti (GARAGE_SECRET_KEY, TYPESAFE_API_KEY, …); paddle (:203) ha cap_drop ma non read_only né pids_limit; docling-models scarica con --all (:175); le revisioni dei pesi hanno default mutabile ${EMBED_REVISION:-main} (:78) e ${RERANK_REVISION:-main} (:101)"
    artifacts:
      - path: "spikes/compose.yaml"
        issue: "runner senza hardening e con tutti i segreti; paddle senza read_only e pids_limit; --all a :175; revisioni con default main a :78 e :101"
    missing:
      - "cap_drop: [ALL], no-new-privileges e pids_limit sul runner"
      - "Segreti passati solo ai job che li usano"
      - "read_only e pids_limit su paddle"
      - "Download dei modelli Docling senza --all (riga 175)"
      - "Revisioni dei pesi obbligatorie: ${EMBED_REVISION:?} e ${RERANK_REVISION:?}"
  - truth: "Ogni driver di misura esce con errore quando una componente della misura non riesce"
    status: failed
    reason: "01-REVIEW.md §B.4: quattro driver possono produrre un CSV o un esito verde con una componente fallita. La verifica sui dati ha mostrato che nessuna misura ratificata ne è alterata (run_ttq.sh: non determinabile ma non alterabile), quindi il difetto è dello strumento, non dei numeri degli ADR"
    artifacts:
      - path: "spikes/s02-parser/ocr-run/run_ttq.sh"
        issue: "considera riuscito l'OCR senza contare i rapporti ok (211-217)"
      - path: "spikes/s02-parser/src/bin/ocr/ttq.rs"
        issue: "ttq-merge non richiede N rapporti ok pari alle pagine OCR attese"
      - path: "spikes/s04-jev/vram.sh"
        issue: "un carico con rc diverso da 0 viene solo loggato (213-219)"
      - path: "spikes/s02-parser/measure/budget_real.py"
        issue: "M10 senza confronto dell'insieme dei job_id (259-269)"
      - path: "spikes/s02-parser/hostile-gen/gen_hostile.py"
        issue: "fill_manifest non esce 1 su mismatch"
      - path: "spikes/s02-parser/hostile-gen/gen_sof.py"
        issue: "stessa funzione fill_manifest, da allineare"
    missing:
      - "run_ttq.sh e ttq-merge richiedono N rapporti ok pari alle pagine OCR attese"
      - "vram.sh esce 1 se un carico fallisce"
      - "budget_real.py confronta l'insieme dei job_id"
      - "fill_manifest esce 1 sul mismatch"
  - truth: "I test degli spike sono stabili e i TODO noti della fase sono chiusi"
    status: failed
    reason: "01-REVIEW.md §B.5: test d'isolamento di s02 instabili sotto carico, cartelle tmp dei test non pulite, inventario dei modelli Docling non allineato, calibrazione di BudgetExceeded aperta, quattro incoerenze di testo annotate"
    artifacts:
      - path: "spikes/s02-parser/tests/"
        issue: "test d'isolamento flaky sotto carico; directory tmp non rimosse"
      - path: "spikes/results/docling-models.txt"
        issue: "elenca modelli rimossi"
      - path: "spikes/s02-parser/src/bin/parse-parent.rs"
        issue: "calibrazione di BudgetExceeded (599-610)"
      - path: "spikes/s02-parser/src/isolate.rs"
        issue: "calibrazione di BudgetExceeded (120-125); nota incoerente a :534"
      - path: "spikes/s02-parser/hostile-gen/gen_hostile.py"
        issue: "timestamp a :577"
      - path: "spikes/data/corpus/oracles/ORACOLI.md"
        issue: "anomalia a :39"
    missing:
      - "Test d'isolamento di s02 stabili sotto carico"
      - "Pulizia delle directory tmp dei test di s02"
      - "results/docling-models.txt allineato ai modelli rimossi"
      - "Calibrazione di BudgetExceeded (parse-parent.rs:599-610, isolate.rs:120-125)"
      - "Incoerenze di testo: nota di isolate.rs:534, timestamp di gen_hostile.py:577, anomalia di ORACOLI.md:39, maiuscole miste negli stati di overhead"
deferred:
  - truth: "01-04 TEST-4: il semaforo delle validazioni non è mai arrivato a saturazione (picco 1 su 2)"
    addressed_in: "Phase 6"
    evidence: "Goal della fase 6: un utente autorizzato carica un documento e lo vede diventare queryable (upload di prodotto con admission control, ADR-0003)"
  - truth: "01-04 TEST-6: diagnosi dei 500 di Garage sui PUT concorrenti della stessa chiave"
    addressed_in: "Phase 3"
    evidence: "Goal della fase 3: ogni provider del lato ingestion funziona per intero dietro la propria porta con contract test; threat model T-37, test previsto in fase 3"
  - truth: "01-07 TEST-4: percorsi oom_adj_failed e subreaper_unavailable letti nel codice, non eseguiti"
    addressed_in: "Phase 3"
    evidence: "Provider DocumentParser di prodotto; vincolo §C.1 del threat model (isolamento per job con cgroup.kill)"
  - truth: "01-07: regola PDFium sul caso proprietario vuota e utente non vuota (Encrypted/pdfium_password, fail-closed)"
    addressed_in: "Phase 3"
    evidence: "DocumentParser e prescan di prodotto; la voce resta non esercitata in RISULTATI-isolamento.md:191 e :218"
  - truth: "01-08 TEST-4: stato di Docling fuori dal container non osservabile (MEDIUM residuo di risk_audit_11)"
    addressed_in: "Phase 3"
    evidence: "Adapter Docling dietro DocumentParser, fase 3"
  - truth: "01-11 TEST-16 e 01-12 TEST-11: rilevamento delle istruzioni ostili e lettura qualitativa delle risposte di injection"
    addressed_in: "Phase 13"
    evidence: "Goal della fase 13 (suite di evaluation su corpus versionato); threat model T-16, test previsto nelle fasi 8 e 13"
  - truth: "01-14 TEST-1, TEST-2, TEST-4, TEST-8 e fusion_mismatch: test db di leak e fusione eseguiti solo dall'esecutore"
    addressed_in: "Phase 7"
    evidence: "Goal della fase 7: nessun chunk negato esce dal Document Service; threat model T-13, controllo negativo e di non vacuità nel contract test di VectorIndex e retrieve_authorized"
  - truth: "01-14 TEST-7, TEST-11, TEST-12 e 01-03 RISK-11: selettività dei ruoli e ACL sintetica (0,0048 solo nei log)"
    addressed_in: "Phase 10"
    evidence: "Goal della fase 10: dimostrato con test automatici che nessun ruolo vede oltre la propria classificazione; matrice ruolo × classificazione di T-13"
  - truth: "01-16: adapter HTTP reale di EvidenceRetriever e collocazione di query_text in RetrievalSearchRequest"
    addressed_in: "Phase 7"
    evidence: "Goal della fase 7 (RAG Service ottiene dal Document Service solo candidati visibili); vincolo §C.4 del threat model; 01-CONTRATTO.md §6 collocazione per servizio"
  - truth: "01-16: Debug che stampa il Locator (Block, IndexedChunk, IndexVersionDraft) e Debug derivato di CacheValue ed Embedding"
    addressed_in: "Phase 2"
    evidence: "Vincolo §C.3 del threat model (fasi 2 e 4): Debug manuale su tutti i tipi che portano dati nella trascrizione in crates/contracts"
  - truth: "01-18 TEST-14 e TEST-3: sei voci del contratto non esercitate e annidamento profondo senza piano"
    addressed_in: "Phase 3"
    evidence: "Rischio residuo «Decoder JBIG2, annidamento profondo e immagini inline» del threat model, responsabile fase 3"
human_verification:
  - test: "Riratificare docs/security/threat-model.md nella versione riscritta in /verify 1"
    expected: "L'utente accetta la nuova colonna Evidenza/Test di T-13, T-14, T-16, T-17, T-24, T-28 e T-29 (T-14, T-16, T-17, T-28 e T-29 passano a «da implementare»), il limite del contenimento di T-09 (setsid), le due voci nuove del rischio residuo (T-14 e T-09) e i nove vincoli per le fasi di prodotto; conferma anche i residui con responsabile l'utente: T-31 (dati presso JEV e Anthropic), T-33 (addebito JEV delle richieste cancellate, console non determinabile) e T-39 (antivirus e CDR)"
    why_human: "La versione ratificata il 2026-10-26 è stata modificata dopo la ratifica per decisione dell'utente («Riscrivi l'evidenza»); una ratifica è un atto dell'utente"
  - test: "Confermare le soglie mancate e accettate con la ratifica degli ADR"
    expected: "Conferma esplicita di: estrazione PDFium E-1..E-3 (CER 9,48% e 21,58% contro 1%, IoU 77,0% contro 95%) ed E-4, E-6, E-7, E-9 di Docling (ADR-0004); TTQ-parse circa 20 s contro la quota di 5 s (ttq_parse.csv); CER e throughput OCR (34,5-34,9 s per 25 pagine con 2 job); M8 a 24 casi su 26 (deep-nesting e jbig2-malformed danno ok, contenimento intatto); L-4 a 8/11 (ADR-0005); latenza del reranker TEI p95 360 ms contro 300 (ADR-0009); memoria di Garage costante solo con parti da 2 MiB e una in volo (ADR-0003, condizione 1)"
    why_human: "Il SUMMARY di 01-17 dichiara che il bridge prova la corrispondenza fra stati e ratifica, non l'accettabilità delle soglie mancate: è un giudizio dell'utente"
  - test: "Accettare le evidenze attestate solo da rapporti o da log grezzi non più disponibili, oppure chiederne la ripetizione"
    expected: "Decisione voce per voce o in blocco su: 01-02 TEST-1, TEST-2, TEST-4 (set di avvio, reclaim, smoke del runner); 01-03 TEST-4 (ramo exit 1 di fetch.sh solo statico); 01-04 TEST-2 (pdfgen con qpdf); 01-05 TEST-10 (p99 h2 dai grezzi); 01-08 TEST-1, TEST-3 e costo del riavvio Docling (19,1 e 20,8 s); 01-09 L-10 (job negativo) e lo stato precedente della correzione upload-in-memory; 01-10 query irraggiungibili, TEI CPU, seconda esportazione, copie di /info; 01-11 TEST-15 (varianza per candidato); 01-13 TEST-2, TEST-3, TEST-4, TEST-5, TEST-7, TEST-10 (configurazione e pesi PP-OCRv6 risolti, autocontrollo del ritardo); 01-14 numero di query in carico (2.679) e confronto coseno/prodotto interno; 01-15 legame misura-codice della sessione VRAM e TEST-9 (due prove di vram.sh); 01-18 TEST-4, TEST-13, TEST-18, TEST-19 (detail di validazione, exit 2, preflight) e cargo test --workspace nel runner (manca addr2line)"
    why_human: "I file di results/raw sono ignorati da git e i worktree sono stati rimossi: il verifier non può rileggerli; i CSV versionati che sostengono gli ADR sono stati invece ricontrollati"
  - test: "Leggere i nove documenti sintetici di spikes/data/corpus/synthetic (01-03 TEST-10)"
    expected: "Persone citate solo per carica, società fittizie, nessun dato reale"
    why_human: "Il test design la assegna a una lettura umana, mai fatta né dall'esecutore né dal bridge"
  - test: "Controllo a mano del campione di oracoli (01-06 TEST-3): 10 query su PDF e 2 pagine di parser-it.json confrontate carattere per carattere con i PDF di raw/, con il seme scritto nel test design"
    expected: "Valori attesi ed estratti coincidono con il documento sorgente"
    why_human: "Un controllo automatico userebbe un estrattore di testo, cioè una pipeline: l'oracolo deve restare indipendente"
  - test: "Confermare l'approvazione dei pacchetti pip di PaddleOCR (01-13 TEST-1) e dei 34 crate di 01-01"
    expected: "L'utente conferma le approvazioni date in conversazione (lock dbb615b prima della prima build 7c85634; «Approvato» del 2026-10-19 sui 34 crate)"
    why_human: "Le approvazioni sono risposte in conversazione, riportate nei RISULTATI e nei commit, senza un artefatto che le provi"
---

# Fase 1: Spike tecnici — Rapporto di verifica

**Obiettivo della fase:** le decisioni tecniche che fissano porte e provider sono chiuse con misure documentate, e il threat model guida le scelte su upload e parsing di file non fidati.
**Verificato:** 2026-10-26
**Status:** gaps_found
**Ri-verifica:** no, verifica iniziale (HEAD `b22a1d2`)

## Esito in breve

Le decisioni della fase sono chiuse e reggono sul codice e sui dati. Gli undici ADR 0003-0013 sono `Accepted`, ognuno con le evidenze e il `hw.json` della propria cartella di risultati (ADR-0012 dichiara di non averne perché è una verifica di sola compilazione, ADR-0013 non ha misure). Tutti i file di `spikes/` citati dagli ADR, da `DEPENDENCIES.md` e dal threat model esistono e sono tracciati; l'unica eccezione è `spikes/s01-upload/results/raw/`, che ADR-0003:71 dichiara non versionata. Ho ricontrollato sui CSV i numeri che sostengono i cinque criteri del ROADMAP, e coincidono con quelli scritti negli ADR e nel threat model. Il codice degli spike è fuori dal prodotto: non c'è un `Cargo.toml` alla radice, `.github/workflows/` contiene solo `.gitkeep` e nessun file sotto `apps`, `crates`, `services`, `infra`, `tests` o `evaluation` nomina `spikes`.

Tutti i 105 must-have (5 criteri e 100 verità dei PLAN) risultano verificati, quattro per override, cioè per deviazioni già accettate dall'utente. Lo status è `gaps_found` perché la banda ECC di /verify 1 (`01-REVIEW.md`) ha trovato cinque gruppi di difetti negli strumenti della fase. L'utente li ha instradati alla gap-closure 1.6 prima della chiusura (§B). Li registro come cinque verità ingegneristiche fallite, una per gruppo, con i file. Quei difetti non alterano nessuna misura ratificata: lo ha provato la verifica sui dati della banda ECC, e i numeri che ho ricontrollato lo confermano. Dopo la 1.6 restano sei punti di UAT, il primo dei quali è la riratifica del threat model.

## Criteri del ROADMAP

| # | Criterio | Status | Prova |
|---|---|---|---|
| 1 | S-01: multipart in streaming misurato con file da 10 MB e oltre, memoria di picco costante, esito registrato | VERIFICATO (con condizione) | dhat del puro streaming: `max_bytes` 1.486.181 / 2.334.796 / 2.564.162 a 1, 10 e 25 MiB (`s01-upload/results/dhat_*.json`). `VmHWM` del percorso completo con `garage-p2k1`: 23.280-27.892 KiB fra 1 e 25 MiB (`mem_by_size.csv`, sessione `…303e`). Con la configurazione di default Garage cresce di 42,6 e 48,1 MiB, e per questo ADR-0003 la esclude (condizione 1, `RISULTATI.md:253`, `:263`). 413 a 26.214.401 byte: `correctness.csv` 20/20 `pass` |
| 2 | S-02, A-05, A-10: parser/OCR misurati su PDF digitali, scansionati, DOCX, Markdown e file ostili; decisi parser, isolamento e definizione di «pagina» | VERIFICATO | Digitali: `extract/text_cer.csv`, `bbox_iou.csv`, `tables.csv`, `docling.csv`. Scansionati: `ocr/ocr.csv`, con Tesseract 5.5.0 e PP-OCRv6 da 12 righe ciascuno su `good` e `degraded`. DOCX/MD: `locator/roundtrip.csv`, 23/23 `equal`. Ostili: `isolation/hostile.csv`, 26 casi e 78 righe, `parent_alive` true e `leftover_procs` 0 su 78/78, `ms_to_fail` massimo 238; `ocr/hostile-render.csv`. Decisioni: ADR-0004 (PDFium, Docling sulle pagine-tabella, Tesseract, processo figlio) e ADR-0005 (DOCX e MD senza pagine, percorso di sezione e intervallo) |
| 3 | S-03, A-03: pgvector misurato su recall, latenza, inserimento durante le query e predicato ACL nella stessa query; indice e metrica decisi | VERIFICATO | `index/threshold.csv` a 10k, 50k, 100k e 300k chunk × exact, hnsw_vector e hnsw_halfvec; `recall.csv`; `ingest_under_load.csv`; `leak.csv` con 2.067 righe e `denied_in_topk` 0 su tutte; `plans/*.json`; `embedding/models.csv` con tre modelli e `leaked_chunks` 0. ADR-0006 (ricerca esatta, coseno, rivalutazione oltre 50.000 chunk) e ADR-0007 (bge-m3, D-38) |
| 4 | S-04, A-09, S-05, A-04, A-06, A-02, S-07: forma JEV misurata, cancellazione verificata, LLM e baseline di rerank scelti, storage deciso, firme di tutte le porte | VERIFICATO | `jev/forms.csv` con 273 righe (91 per forma a, b e c) e `model_resolved` `jev-1.13.0` su 273/273 (ADR-0008). `s05-cancel/results/cancel.csv` (909 righe campionate su h1, h2 e wiremock) e `deadline.csv` (massimo 2.091 µs su 36 righe), ADR-0010. `s06-llm/results/llm.csv` e `llm-simple-sonnet.csv`, ADR-0011 (Sonnet, D-33). `rerank/rerank.csv` con 887 righe, ADR-0009. ADR-0003 (Garage). `s07-ports/src/lib.rs` con 13 trait (`:578`-`:1813`), 45 test unitari e 5 doctest verdi, rieseguiti oggi sull'host |
| 5 | Esiti in ADR o `DEPENDENCIES.md` con misure e hardware; threat model compilato e mappato; spike fuori dal prodotto e dalla CI | VERIFICATO (riratifica in UAT) | Ogni ADR collega RISULTATI e `hw.json`. `DEPENDENCIES.md:25`, `:28`, `:52`, `:54`, `:67`, `:88`-`:91` sono Decise, con *Storia* a `:33`, `:57`, `:73` e `:93`. Il threat model ha 39 righe; tutti i requisiti citati esistono in `REQUIREMENTS.md`. `test ! -f Cargo.toml` passa; `.github/workflows/` contiene solo `.gitkeep`; grep `spikes` vuoto nei sei alberi di prodotto |

## Verità dei PLAN

### 01-01: harness Rust

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | Workspace isolato con membri espliciti, nessun `Cargo.toml` alla radice | VERIFICATO | `spikes/Cargo.toml:6` con 8 membri; radice senza `Cargo.toml` |
| 2 | Dipendenze pinnate `=x.y.z` e attive nei crate | VERIFICATO | 34 voci `"=` in `[workspace.dependencies]`, nessuna versione non pinnata; 8 manifest con `workspace = true` |
| 3 | `common` con API fissa che rifiuta un CSV senza `hw.json` | VERIFICATO | `common/src/csv.rs:1-20`, `manifest.rs:29` (`ensure_present`), test `csv_create_without_hw_json_fails` (`:94`); 17 test verdi oggi |
| 4 | Crate verificati come legittimi dall'utente | VERIFICATO (conferma in UAT 6) | `01-01-SUMMARY.md:127`: «Approvato» il 2026-10-19 su 34 crate |

### 01-02: compose, sessioni, segreti

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | Gate GPU per primo | VERIFICATO | `spikes/results/gpu-check.txt`: 2026-10-19, run `…0709`, RTX 3050 6144 MiB |
| 2 | Compose con tutti i servizi, porte solo su 127.0.0.1, segreti solo da env | VERIFICATO (hardening del runner al gap 3) | `compose.yaml:35-219`, 10 servizi; porte `127.0.0.1:` a `:44`, `:67`, `:86`, `:107`; Docling su rete interna (`:20`) |
| 3 | Avvio per nome con controllo del set risolto e in esecuzione | VERIFICATO (prove positive in UAT 3) | `tools/session.sh`; correzione del separatore `37c68ee` e `results/session-negative.txt`; la banda ECC ha usato `session.sh with … oneshot` |
| 4 | Prenotazione atomica, run_id validato, pulizia per etichetta | VERIFICATO | `session.sh:42` (`RUN_ID_RE`), `:173-177` (lock `mkdir`), `:175-185` (`down_impl` per etichetta); gap-closure 1.1 `passed` |
| 5 | Runner uid 10001 che scrive sui volumi | VERIFICATO | `RISULTATI-isolamento.md:54-55` (`id -u` 10001 in due sessioni); gap-closure 1.3 `passed` |
| 6 | Docling offline con pesi in un volume read-only e SHA-256 versionati | VERIFICATO (inventario al gap 5) | `spikes/results/docling-models.txt`; `compose.yaml:167-175` |
| 7 | Nessun segreto né endpoint `http://` versionato; pin JEV e modelli nell'esempio | VERIFICATO | `spikes/.env.example`: segreti vuoti, gli URL interni compaiono solo come commento «forma», `JEV_MODEL=jev-1.13.0`, `LLM_MODEL_SIMPLE/COMPLEX`. Info: i default `EMBED_MODEL=Qwen…` e `LLM_MODEL_SIMPLE=claude-haiku-5-5` sono precedenti a D-38 e D-33 |
| 8 | Segreti in `$HOME/.cka-spikes/spikes.env`, passati con `--env-file` | VERIFICATO | `session.sh:121`; 1.1 `passed` |
| 9 | `hw.json` per ogni sessione | VERIFICATO | `hw.json` presente in tutte le cartelle di risultati citate dagli ADR |

### 01-03: corpus

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | Corpus pubblico o sintetico | VERIFICATO | `data/corpus/manifest.json`: 23 documenti con `classification`, `license`, `source_url` |
| 2 | Provenienza registrata, binari pubblici non ridistribuiti | VERIFICATO | campi `sha256`, `license`, `oracle_author`; `spikes/.gitignore:5` (`data/**/raw/`) |
| 3 | Matrice ruolo × classificazione con profilo ≤ 5% e documento trappola | VERIFICATO | `acl.json` (`max_chunk_selectivity` 0,05); `syn-03-verbale-cda-restricted.md`; la misura 0,0048 è rinviata (vedi differiti) |
| 4 | Documenti di prompt injection | VERIFICATO | `synthetic/syn-07-injection-public.md`, `syn-08-injection-internal-en.md`; usati in `s06-llm/results/injection.csv` |

### 01-04: S-01 e A-02

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | Heap dhat costante; `VmHWM` costante per processo nuovo; soglie scritte prima | VERIFICATO (con condizione) | Vedi criterio 1; soglie a `RISULTATI.md:21-36` |
| 2 | 413 oltre 25 MiB senza leggere il corpo; i confini accettati | VERIFICATO | `RISULTATI.md:134`; `correctness.csv` 20/20 |
| 3 | Magic bytes, struttura, limiti DOCX e prefiltro PDF prima dello storage | VERIFICATO | `correctness.csv` (polyglot e mismatch 415, zip bomb e DTD 422, `/J#61vaScript` in quarantena); threat model T-06, T-07, T-10 |
| 4 | Admission control con tetti e ritorno al baseline | VERIFICATO (saturazione del semaforo differita) | `admission.csv`: `status_503` 14, `status_408` 3, FD e byte temporanei al baseline dopo 15 s |
| 5 | Garage, `bytea` e nullo misurati sugli stessi corpi; scelta motivata | VERIFICATO | `adapters.csv`, `storage.csv`, `runs.csv`; ADR-0003 |
| 6 | Chiave sha256, PUT prima della transazione, GC a 24 h, risposta identica con o senza deduplica | VERIFICATO | `gc.csv`, `gc_race.csv`, `dedup_timing.csv` (Garage entro ±T in 3 sessioni su 3); `sink.rs:33` (`trait BlobSink`), `sql/schema.sql` (stato `deleting`) |

### 01-05: S-05

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | Cancellazione su h1/h2 per 10.000 cicli senza leak | VERIFICATO (p99 dai grezzi in UAT 3) | `cancel.csv`: 303 righe ciascuno per mock h1, mock h2 e wiremock h1, campionate ogni 100 cicli; `open_fds` massimo 41 |
| 2 | Uscita entro 10 ms dalla deadline, due varianti | VERIFICATO | `deadline.csv`: 18 righe per la variante a e 18 per la b, `exit_after_deadline_us` massimo 2.091 |
| 3 | 20 richieste JEV cancellate registrate e verificate in console | VERIFICATO (esito «non determinabile», residuo T-33 in UAT 1) | `jev_cancel.csv`: 20 `cancelled`, 5 `control` |
| 4 | reqwest con Rustls validato, ADR-0010 | VERIFICATO | ADR-0010 `Accepted`; `DEPENDENCIES.md:28` |

### 01-06: oracoli

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | 60-100 query etichettate e oracoli del parser IT dal documento sorgente | VERIFICATO | `queries.json`: 80 query (32 IT, 32 EN, 16 cross-lingua, 12 `no_answer`); `parser-it.json` 11 |
| 2 | Triplo guardrail completo | PASSED (override) | 74 query `CONFIRMED` e `ratified`, le 6 `PARTIALLY_CONFIRMED` restano escluse (`ORACOLI.md:85`); parser 11/11. Il derivatore coincide con l'esecutore (`ORACOLI.md:25`) |
| 3 | Profilo di ruolo per query | VERIFICATO | `role_profile` presente su 80/80 |
| 4 | Complessità con regola scritta | VERIFICATO | `complexity` e `complexity_ref` su 80/80 (68 `simple`, 12 `complex`) |

### 01-07: lanciatore isolato

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | Figlio per job con limiti, in un container senza rete, read-only e non root | VERIFICATO | `RISULTATI-isolamento.md:54` (uid 10001), `:59` (container OOM `--network none --read-only --cap-drop ALL`); worker `read_only` (`compose.yaml:188`) |
| 2 | Solo job JSON tipizzati con op in allowlist | VERIFICATO | `RISULTATI-isolamento.md:167-169` (`UnknownOp` exit 2) |
| 3 | Prenotazioni sotto il cgroup, `RLIMIT_AS`, `oom_score_adj` 1000 | VERIFICATO | `isolate.rs:118-125` (`reservation_bytes`); `limits.json`; `oom_adj_set` true su 78/78 righe di `hostile.csv` |
| 4 | Gruppo ucciso e cartella rimossa su ogni uscita | VERIFICATO (limite `setsid` in T-09) | `leftover_procs` 0 su 78/78; `/work` a 0 voci (`RISULTATI-isolamento.md:54`); `oom.json` con `job_dir_removed` true |
| 5 | Tetti di decompressione al caricamento, limite di pagine, esiti `Kind` | VERIFICATO | `hostile.csv`: `objstm-bomb`, `objstm-sum-bomb`, `filter-chain-bomb`, `flate-bomb` → `DecompressionLimit`; `pages-10k` → `ResourceLimit`; DCT e JPX dalla 1.5 |
| 6 | Regole sui PDF cifrati e attivi (D-05 emendata) | VERIFICATO | `encrypted-user` e `encrypted-pubsec` → `Encrypted`; `encrypted-owner-only` → `ok`; `pdf-objstm-hidden-js` → quarantena `javascript`; `pdf-embedded-file` → `ok` |
| 7 | PDFium chromium/8100 e tessdata con hash, Tesseract 5.5.0 | VERIFICATO | `docker/worker.Dockerfile` (`PDFIUM_SHA256`, `TESSDATA_*_SHA256`, `sha256sum -c`); `ocr.csv` versione 5.5.0 |
| 8 | Limiti calibrati in `limits.json`, `calibrate --op` | VERIFICATO | `isolation/limits.json`, `calibration.csv`; `extract/limits-extract_pdf.json`, `ocr/limits-ocr_render.json`, `limits-ocr_tesseract.json` |

### 01-08: estrazione

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | PDFium misurato in CER e IoU con un contratto di abbinamento fissato prima | VERIFICATO (soglie mancate in UAT 2) | `text_cer.csv`, `bbox_iou.csv`, `iou_examples.json` |
| 2 | Docling sulle pagine-tabella CPU e GPU; euristica tarata e congelata prima dell'holdout | VERIFICATO | `docling.csv`, `table_pages.csv`, `table_heuristic_params.json` |
| 3 | TAT-DQA, DocLayNet e FinanceBench con licenza e sha256 | VERIFICATO | `data/s02/manifest.json`: 40 CC BY 4.0, 41 CDLA-Permissive-1.0, 3 CC BY-NC 4.0 |
| 4 | Estrazione solo come job `extract_pdf` con limiti calibrati | VERIFICATO | `extract/invocation.txt`, `limits-extract_pdf.json`; `extract/main.rs` usa `protocol::emit`, `pdf.rs` usa `Rejection::` |
| 5 | Docling solo dopo il prescan, riavvio fra documenti | VERIFICATO (costo del riavvio in UAT 3) | `docling_isolation.csv`; `docling-bench.sh` chiama `session.sh` |

### 01-09: locator

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | Locator a due varianti | VERIFICATO | ADR-0005; `RISULTATI-locator.md:97` |
| 2 | Round-trip esatto su almeno 10 campioni; percorso, intervallo e hash uguali fra MD e DOCX | PASSED (override) | `roundtrip.csv` 23/23 `equal` (12 MD, 11 DOCX); percorso 11/11, intervallo 11/11, hash 8/11 (`RISULTATI-locator.md:72`, `:80`) |
| 3 | Offset scalar, conversione UTF-16 contro una fixture scritta a mano | VERIFICATO | `offset_vectors.json` contro `locator-gen/offset_vectors.expected.json`, 10/10 (L-6) |
| 4 | Trasformazione dei rettangoli al 100% su valori derivati a mano | VERIFICATO | `rect_transform.csv` stadio `transform` 8/8 `pass`, errore 0,000 pt; le 8 righe `extract_text` sono lo stadio diagnostico separato |
| 5 | DOCX e MD solo nel figlio, job `op: locate` | VERIFICATO | `RISULTATI-locator.md:81`; job negativo in UAT 3 |

### 01-10: embedding

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | Tre modelli misurati per lingua | VERIFICATO | `models.csv`: 8 righe per qwen3, bgem3 ed e5 |
| 2 | TEI CUDA misurato dopo il gate GPU | VERIFICATO | `throughput.csv`, `embedding/hw.json` |
| 3 | Effetto dell'istruzione asimmetrica | VERIFICATO | `query_instruction.csv` |
| 4 | Ricerca esatta con ACL nella stessa query | VERIFICATO | `leaked_chunks` 0 su tutte le righe di `models.csv` |
| 5 | ADR-0007 con misure e hardware | VERIFICATO | `Accepted`, `hw.json` a `:66` |

### 01-11: JEV

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | Forme A, B e C con metriche su 3 ripetizioni | VERIFICATO | `forms.csv`, 273 righe |
| 2 | Concorrenza 4/8/16 con 429 e 529; risoluzione degli alias | VERIFICATO | `concurrency.csv`, 9 righe con `status_429` e `status_529` a 0 (il backoff non si è mai attivato, T-24); `aliases.csv`: latest → 1.13.0, preview → 1.14.0 |
| 3 | Pin `jev-1.13.0` con versione risolta in ogni riga | VERIFICATO | 273/273 |
| 4 | Nessun passaggio non visibile a JEV; conteggio come hard gate | PASSED (override) | guardia `s04-jev/src/candidates.rs:153`; test `send_path_blocks_before_network_on_invisible_passage`; il contatore è tautologico (T-14, §D) |

### 01-12: LLM

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | Haiku e Sonnet misurati via API con structured output | VERIFICATO | `llm.csv`, 94 righe (60 Haiku, 34 Sonnet); `llm-simple-sonnet.csv` |
| 2 | Complessità decisa da JEV; simple → Haiku | PASSED (override) | `routing.csv` 25/30 `agree`; D-33 |
| 3 | Senza JEV, Sonnet e risposta degradata, mai validata | VERIFICATO | `fallback.csv`: 15/15 Sonnet, `validated` false, `degraded` true |
| 4 | Solo `chunk_id` forniti; scarto deterministico | VERIFICATO (solo per test unitari, T-17) | test `unknown_citation_id_drops_claim`, `locator_comes_from_context_not_model`; `claims_unknown_id` 0 su `llm.csv`, quindi il ramo non è stato esercitato dal vivo |
| 5 | Nessun passaggio non visibile nel prompt né a JEV | VERIFICATO | guardia `s06-llm/src/context.rs:190`, `:271`; test `context_rejects_invisible_passage_before_prompt`, `jev_calls_guarded_by_assert_visible` |

TEST-21 di 01-12, che chiedeva la rigenerazione coi semi reali, l'ho eseguito io. `s06-llm select --seed 120601 --fallback-seed 120602` su una copia di `queries.json` nello scratchpad produce un file identico byte per byte a `spikes/data/s06/selection.json` (sha256 `f18f7c7e…`). TEST-23 e TEST-28 (script con fixture mai scritti) sono superati da D-33. Ho comunque ricalcolato su `llm.csv` le due condizioni di D-31: Haiku complex 37/54 = 0,6852, Sonnet complex 46/58 = 0,7931, Haiku simple 21/29 = 0,7241. Coincidono con ADR-0011.

### 01-13: OCR, TTQ, ADR-0004

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | Tesseract da CLI via job, CER, s/pagina e bbox su scansioni buone e degradate | VERIFICATO | `ocr.csv` (12 `good` e 12 `degraded`), `bbox_map.csv` |
| 2 | PP-OCRv6 con flag espliciti, pesi registrati, rete negata | VERIFICATO | `ocr.csv` 24 righe PP-OCRv6; `paddle-weights.sha256`; detection ridotta per decisione utente (ADR-0004:37, *Storia*); runtime in UAT 3 |
| 3 | Throughput a N = 1, 4, 8, 16 con `max_admitted` e metodo M10 | VERIFICATO | `ocr_throughput.csv`: `max_admitted` 1/2/2/2, `m10_pass` true su 12/12 |
| 4 | JBIG2 sul percorso `ocr_render` con atteso scritto prima | VERIFICATO (residuo aperto, come ammette il must-have) | `hostile-render.csv`: `jbig2-malformed` e `deep-nesting` danno `ok` 3/3, contenuti; DCT e JPX della 1.5 rifiutati 9/9 |
| 5 | TTQ-parse sul percorso composto reale contro la quota di 5 s | VERIFICATO (strumento al gap 4; soglia in UAT 2) | `ttq_parse.csv`: parete 19,8-20,0 s su 25 pagine; `tesseract_ms` mai a 0 |
| 6 | ADR-0004 riassume i quattro RISULTATI e annota Windows Job Objects | VERIFICATO | `Accepted`; collegamenti `:71-73`; Job Objects `:41`, `:65`; Limiti noti `:76` |
| 7 | Pacchetti pip approvati prima dell'installazione | VERIFICATO (conferma in UAT 6) | `docker/paddle-requirements.lock`; lock `dbb615b` prima della build `7c85634` |

### 01-14: indice

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | Ricerca esatta con ACL a 10k-300k, soglia HNSW misurata | VERIFICATO | `threshold.csv`, 4 scale × 3 metodi; ADR-0006 (50.000) |
| 2 | Ibrida tsvector + RRF con ACL in entrambe le parti | VERIFICATO | `hybrid.csv`; CTE `allowed` in `hybrid.rs`, filtro a `:424` |
| 3 | Leak test a 0 per ruolo, parte e scala | VERIFICATO (controlli negativo e di non vacuità differiti) | `leak.csv`: 2.067 righe, `denied_in_topk` 0, parti sem, lex e hybrid |
| 4 | Inserimento durante le query | VERIFICATO | `ingest_under_load.csv` (`errors` per secondo) |
| 5 | ADR-0006 con EXPLAIN e hardware | VERIFICATO | `results/index/plans/*.json`, `index/hw.json` |

### 01-15: reranker

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | No-rerank, bge-reranker TEI GPU e JEV sugli stessi top-20 | VERIFICATO | `rerank.csv`: 887 righe (tei 443, none 222, jev 222) |
| 2 | Convivenza in 6 GB e budget VRAM | VERIFICATO | `vram.csv`: massimo 5.562 MiB, `oom` false su 593 righe |
| 3 | Visibilità ricontrollata prima di ogni invio | VERIFICATO | test `rerank_blocks_invisible_candidate_before_tei` e `…_before_jev` (`bin/rerank/main.rs`) |
| 4 | JEV resta il rank di prodotto, baseline in ADR-0009 | VERIFICATO | ADR-0009 `Accepted`; `DEPENDENCIES.md:91` |

### 01-16: firme delle porte

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | Firme di 13 porte; il crate compila | VERIFICATO | `lib.rs:578`, `:838`, `:1085`, `:1203`, `:1221`, `:1325`, `:1426`, `:1453`, `:1512`, `:1581`, `:1628`, `:1760`, `:1813`; test verdi oggi |
| 2 | `Reranker` estratto da `DecisionEngine` | VERIFICATO | `:1426`, `:1453`; ADR-0012 |
| 3 | Variazioni misurate incorporate | VERIFICATO | `Embedder` (`:578`), `CallContext` con deadline (ADR-0012:52), `Locator`, `MutationOutcome` |
| 4 | Evidenza sui due lati del confine | VERIFICATO | `EvidenceRetriever` (`:838`), `authorize_response` (`:1005-1043`) |
| 5 | `AuthorizedEvidence` non costruibile fuori dal modulo; `Debug` redatto; niente `Serialize` | VERIFICATO (gli altri tipi con dati sono differiti) | doctest `compile_fail` a `:666`, `:891`, `:903`, `:910` |
| 6 | `CacheKey` con scope, `MutationContext`, `EmbeddingSpace` | VERIFICATO | `compile_fail` `:1677`; test `t6_*` e `t8_*` |
| 7 | Nessun tipo di provider in firma; `Arc<dyn Trait>` | VERIFICATO | grep di `object_store`, `sqlx`, `reqwest`, `pgvector`, `pdfium`, `axum` vuoto in `lib.rs` e `Cargo.toml` |
| 8 | Scelta fra async-trait e boxing in ADR-0012 | VERIFICATO | `lib.rs:512`, ADR-0012 |

### 01-17: consolidamento

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | Threat model compilato con mitigazioni mappate | VERIFICATO (riratifica in UAT 1) | `docs/security/threat-model.md`: T-01..T-39, checklist, rischio residuo, vincoli |
| 2 | Ogni esito in un ADR o in `DEPENDENCIES.md` con misure e hardware | VERIFICATO | Vedi criterio 5 |
| 3 | `DEPENDENCIES.md` riflette le decisioni, con *Storia* | VERIFICATO | Righe e *Storia* citate al criterio 5 |
| 4 | Antivirus e CDR rinviati con un ADR | VERIFICATO | ADR-0013 `Accepted`; T-39 |
| 5 | Spike fuori dal prodotto e dalla CI | VERIFICATO | Vedi criterio 5 |

### 01-18: casi ostili e budget

| # | Must-have | Status | Prova |
|---|---|---|---|
| 1 | Ogni file ostile termina con un esito tipizzato entro 5 s senza far cadere il padre | VERIFICATO | `hostile.csv`: 26 casi, padre vivo su 78/78, massimo 238 ms. M8 sugli esiti attesi dà 24/26, scritto senza ritoccare gli attesi (`RISULTATI-isolamento.md:97`) |
| 2 | Regole di 01-07 v4/v6 misurate in `hostile.csv` | VERIFICATO | Vedi 01-07 #6; `encrypted-object-dropped` → `Corrupt`; `objstm-many` → `ok`; `pdf-embedded-launch` → quarantena |
| 3 | Attesi e soglie committati prima delle misure | VERIFICATO | `4a28697` (soglie e attesi, 18:13) precede `7c63561` (misure, 18:26) del 2026-10-22 |
| 4 | Driver `budget` solo aritmetico | VERIFICATO | `budget.csv`; RISULTATI non ne deduce concorrenze |
| 5 | Protezione reale da `budget-real.csv` | VERIFICATO | 8 righe, benign e hostile × 1/4/8/16, `max_admitted` 1/2/2/2, `m10_pass` 8/8; `budget-real-jobs.csv` con 178 `job_id` univoci (verifica ECC) |
| 6 | OOM esercitato: vittima il figlio, padre vivo | VERIFICATO | `oom.json`: `esito` verificato, `oom_kill_delta` 1, `leftover_procs` 0, `container_exit` 0 |
| 7 | «Validazione di 01-07» con le difformità alla gap-closure | VERIFICATO | `RISULTATI-isolamento.md:210`; difformità chiuse dalla 1.4 (`passed`) |
| 8 | Misure dal lanciatore senza modificarne il codice | VERIFICATO | Nessun commit di 01-18 su `isolate.rs`, `parse-parent.rs` o `prescan.rs` |

**Punteggio:** 105 must-have verificati su 105, di cui 4 per override. Contando le 5 verità ingegneristiche della banda ECC, il totale è 105 su 110.

## Verità ingegneristiche della banda ECC (gap 1.6)

| # | Verità | Status | Prova |
|---|---|---|---|
| E1 | Lo scanner rileva le forme che dichiara | FALLITA | `01-REVIEW.md` §B.1; la rilettura Flate in `prescan.rs:857-867` aggira la regola |
| E2 | Gli hook applicano gli stessi controlli dello scanner | FALLITA | §B.2 |
| E3 | Runner e servizi con privilegi minimi, segreti per job, pesi fissati | FALLITA | §B.3; confermato su `compose.yaml` (runner senza `cap_drop`, `:78`, `:101`, `:175`) |
| E4 | I driver di misura falliscono in modo rumoroso | FALLITA | §B.4; misure ratificate intatte |
| E5 | Test stabili e TODO noti chiusi | FALLITA | §B.5 |

## Collegamenti chiave

`gsd-sdk verify.key-links` dà 30 collegamenti verdi su 33 nei 13 PLAN che sa leggere. Ho controllato a mano i 3 rossi e i 5 PLAN che il tool non legge (01-01, 01-04, 01-06, 01-08, 01-14).

| Collegamento | Esito | Dettaglio |
|---|---|---|
| 01-02 `compose.yaml` → `spikes.env` | CABLATO | Falso negativo del pattern escapato: `${POSTGRES_PASSWORD}` a `compose.yaml:42` e `:233`; `--env-file` a `session.sh:121` |
| 01-09 `locator` → `parse-parent batch` | CABLATO | Il locator consuma i rapporti di `batch` (`batch_summary`, `locator/main.rs:413-422`); comando a `RISULTATI-locator.md:61` |
| 01-18 `budget_real.py` → `parse-parent batch` | CABLATO | `measure/run_budget_real.sh:63` lancia `batch`, `budget_real.py:200` legge `batch_summary` |
| 01-01, 01-04, 01-06, 01-08, 01-14 | CABLATI | `workspace = true` in 8 manifest; `ensure_present`; `trait BlobSink`; `deleting` nello schema; ADR-0003 → RISULTATI; `document_id` in `queries.json`; `protocol::emit`, `Rejection::`, `page_range`, `session.sh`; CTE `allowed`, `s03_pgvector::` |

## Spot-check di comportamento (host, sola lettura)

| Comportamento | Comando | Risultato | Status |
|---|---|---|---|
| Firme delle porte e `common` | `cargo test -p s07-ports -p common --locked --offline` | 17 + 45 test e 5 doctest verdi | PASS |
| Determinismo della selezione di S-06 | `s06-llm --spikes-dir <scratch> select --seed 120601 --fallback-seed 120602` | identico a `data/s06/selection.json` | PASS |
| Regola D-31 | ricalcolo Python su `llm.csv` | 0,6852 / 0,7931 / 0,7241 | PASS |
| Numeri dei criteri | lettura dei CSV di S-01..S-06 | coincidono con ADR e threat model | PASS |
| Stato | `node scripts/lint-stato.cjs` | verde | PASS |

Non ho rieseguito Docker: valgono le catene verdi registrate in `01-REVIEW.md`, compreso `cargo test -p s02-parser` nel runner. Nessun probe `scripts/*/tests/probe-*.sh` esiste né è dichiarato.

## Copertura dei requisiti

| Requisito | Piani | Descrizione | Status | Prova |
|---|---|---|---|---|
| PLAT-07 | tutti i 18 | threat model compilato (asset, attori, minacce, mitigazioni mappate) | SODDISFATTO nel codice, riratifica pendente | `threat-model.md` completo; requisiti citati tutti presenti in `REQUIREMENTS.md`. Resta `Pending` a `REQUIREMENTS.md:109` fino alla chiusura, che non tocca a questa verifica |

`REQUIREMENTS.md` assegna alla fase 1 solo PLAT-07: non ci sono requisiti orfani.

## Anti-pattern

| File | Riga | Pattern | Gravità | Impatto |
|---|---|---|---|---|
| file della fase (`spikes/`, scanner, hook, ADR, threat model) | — | `TBD` / `FIXME` / `XXX` | nessuno | grep vuoto |
| `spikes/data/corpus/oracles/ORACOLI.md` | 105, 111 | TODO-01, TODO-02 | Info | chiusi alla riga `:117` (ratifica del 2026-10-21) |
| `spikes/.env.example` | s03, s06 | default `Qwen3` e `haiku` precedenti a D-38 e D-33 | Info | il file non si usa per le misure; si può allineare nella 1.6 |
| `spikes/s02-parser/src/prescan.rs` | 857-867 | `io::copy` per evitare la regola dello scanner | Avviso | gap E1 |

## Verifica umana richiesta

### 1. Riratifica del threat model
**Test:** rileggere `docs/security/threat-model.md` nella versione di /verify 1.
**Atteso:** accettare l'evidenza riscritta di sette minacce, il limite di T-09, le voci nuove del rischio residuo e i nove vincoli, e confermare i residui con responsabile l'utente (T-31, T-33, T-39).
**Perché umano:** il testo è cambiato dopo la ratifica del 2026-10-26.

### 2. Soglie mancate accettate
**Test:** confermare l'elenco del frontmatter (estrazione PDFium e Docling, TTQ-parse, OCR, M8, L-4, reranker, memoria di Garage).
**Atteso:** conferma esplicita della ratifica «ratifica-tutti».
**Perché umano:** è un giudizio di accettabilità.

### 3. Evidenze solo da rapporto
**Test:** decidere sulle voci elencate nel frontmatter (raw ignorati, worktree rimossi).
**Atteso:** accettazione o richiesta di ripetizione.
**Perché umano:** il verifier non può rileggere file che non esistono più.

### 4. Lettura dei nove sintetici
**Test:** leggere `spikes/data/corpus/synthetic/syn-0N-*.md`.
**Atteso:** nessuna persona o società reale.

### 5. Campione degli oracoli
**Test:** 10 query e 2 pagine di `parser-it.json` confrontate a mano con i PDF.
**Atteso:** coincidenza carattere per carattere.

### 6. Approvazioni date in conversazione
**Test:** confermare l'approvazione dei pacchetti pip di PaddleOCR e dei 34 crate.
**Atteso:** conferma dell'utente.

## Punti human_judgment dei SUMMARY: come sono stati trattati

Gli 80 punti `human_judgment: true` dei 18 SUMMARY ricadono in quattro gruppi.

- **Chiusi da fatti registrati (non vanno in UAT).** Ratifica di ADR-0005, 0008, 0010, 0011 e 0012 (tutti `Accepted` il 2026-10-26). Coerenza interna di ADR-0011, allineato con i paragrafi *Storia* `:31`, `:42` e `:62`. `std::time::Instant` motivato in ADR-0012:52. p95 di Haiku superato da D-33. TEST-21, TEST-23 e TEST-28 di 01-12, rieseguiti o ricalcolati qui sopra. Deviazione del derivatore di 01-06 (override). Proprietà del container a runtime e OOM vero di 01-07, misurati in 01-18 (`RISULTATI-isolamento.md:54`, `:59`, `oom.json`). Misura dei casi ostili di 01-07, fatta in 01-18. `calibrate --op` per le altre op (esistono i tre `limits-*.json`). M10 `parse` non ripetuta (D-40, ADR-0004:49). M9 di 01-18, rimisurata dalla 1.4. Hook di 01-02 TEST-7 e scanner di 01-09 HIGH, che confluiscono nei gap E1 ed E2.
- **Confluiti nell'UAT.** 01-17 (contenuto ratificato), 01-08 E-1..E-9 e 01-09 L-4 nel punto 2; 01-05 addebito JEV nel punto 1; le evidenze solo da rapporto nel punto 3; 01-03 TEST-10 nel punto 4; 01-06 TEST-3 nel punto 5; 01-13 TEST-1 nel punto 6.
- **Differiti a fasi successive.** Undici voci, elencate nel frontmatter `deferred` con la fase e la prova dal ROADMAP.
- **Gap.** Nessun punto human_judgment è un gap: i gap vengono dalla banda ECC.

## Riconciliazione del contratto

| Promessa di `01-CONTRATTO.md` | Codice e documenti | Esito |
|---|---|---|
| ADR 0003-0013 con le decisioni della §6 | tutti presenti e `Accepted`; ADR-0007 sceglie bge-m3 invece del default Qwen3 (D-38), ADR-0011 sceglie Sonnet unico (D-33) | mantenuta, con le deviazioni decise dall'utente |
| `DEPENDENCIES.md` a Decisa con *Storia* | righe e *Storia* presenti | mantenuta |
| Threat model T-01..T-28 più le nuove | T-01..T-39 | mantenuta |
| Limiti noti (D-25) in ADR-0004 e `README.md` | `ADR-0004:76`, `README.md:111` | mantenuta |
| Dimensionamento estrapolato (D-28) | `ADR-0004:57`, `:83` | mantenuta |
| Nessun seam nella WIRING-MAP | `.planning/WIRING-MAP.md:40` e registro `:46` | mantenuta |
| Vincoli verso la Fondazione (`exclude = ["spikes"]`, riallineamento di `interfaces-and-concurrency.md`) | non ancora applicabili: non c'è un workspace di prodotto | passano alla fase 2 come scritto nel contratto |

## Riepilogo dei gap

Il goal della fase è raggiunto: decisioni chiuse con misure e hardware, threat model compilato e mappato, spike isolati dal prodotto. Restano due cose. La prima sono i cinque gruppi di difetti negli strumenti che l'utente ha voluto chiudere prima della fase, cioè scanner, hook, compose del runner, driver di misura, test instabili e TODO noti: sono i gap E1-E5, con file e voci pronti per `/plan 1.6 --gaps`. I file sono quasi tutti sotto `spikes/`, quindi la 1.6 avrà `riapre_perimetro: [1]`. `skills/security-scanner/security_scan.py` e `.githooks/controlli-comuni.sh` vanno verificati contro `scripts/proprieta-file.whitelist.json` prima del piano. La seconda sono i sei punti di UAT, il più importante dei quali è la riratifica del threat model.

---

_Verificato: 2026-10-26_
_Verificatore: Claude (gsd-verifier)_

**Modifica:** Claude (gsd-verifier) · 2026-10-26 · creazione
