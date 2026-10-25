# ADR-0004 — Parser PDFium con Docling sulle pagine-tabella, OCR Tesseract, parsing isolato in processo separato e tetto di decompressione nel prescan

- Stato: Proposed
- Data: 2026-10-24
- Decisori: utente (D-05, D-07, D-08, D-09 emendata, D-28, D-34, D-39, D-40 di `01-CONTEXT.md`; ratifica nel consolidamento, piano 01-17); proposta di Claude (gsd-executor 01-13) dallo spike S-02 (piani 01-07, 01-08, 01-09, 01-13, 01-18, gap-closure 1.4 e 1.5)
- Sostituisce: nessuno

**Modifica:** Claude (gsd-executor 01-13) · 2026-10-24 · ADR proposto con le misure dei quattro RISULTATI di S-02 (piano 01-13)
**Modifica:** Claude (gsd-executor 01-13 paddle) · 2026-10-25 · riga OCR: PP-OCRv6 identificato ma non valutabile a 2 GiB (oneDNN/PIR, OOM); Tesseract provvisorio, stato invariato
**Modifica:** Claude (gsd-executor 01-13 paddle) · 2026-10-25 · riga OCR: PP-OCRv6 misurato con detection ridotta (decisione utente, opzione a, un solo tentativo), non vince nettamente; Tesseract confermato, «(provvisorio)» tolto dal titolo; stato Proposed invariato, ratifica in /verify 1

## Contesto

Il Document & Indexing service riceve PDF, DOCX e Markdown non fidati: caricati dagli utenti o scoperti online e approvati dopo la quarantena. Deve estrarne testo, pagine, coordinate e tabelle per il chunking e il locator delle citazioni (ADR-0005), leggere con l'OCR le pagine scannerizzate e restare in piedi con qualunque file. Il target del README è un TTQ di 15 s per un PDF digitale da 25 pagine, di cui 5 s di parse.

Decisioni dell'utente che vincolano la scelta: D-05 (casi ostili e quarantena), D-07 (parser e tetto di decompressione, TTQ-parse), D-08 (isolamento in processo separato), D-09 emendata (OCR Tesseract da CLI contro lo sfidante PP-OCRv6), D-28 (worker a 2 GiB, dimensionamento estrapolato), D-34 (prova JBIG2 sul render), D-39 (tetto sulle dimensioni codificate, gap-closure 1.5), D-40 (concorrenza `parse` da M10).

Minacce (01-RICERCA-DISCUSSIONE §3, `docs/security/threat-model.md`):

- **T-06**, tipo di file falso (estensione diversa dal contenuto, DOCX con macro);
- **T-07**, zip bomb e XML bomb nei DOCX;
- **T-09**, exploit del parser, loop e OOM: decompression bomb, object stream, catene di filtri, immagini enormi, decoder d'immagine (JBIG2) e annidamento profondo;
- **T-10**, PDF attivo (`/JavaScript`, `/Launch`, allegati).

PDFium ha avuto CVE nel 2026 (RICERCA Area B §1): un suo crash o un suo consumo anomalo non deve toccare il servizio. MuPDF (AGPL) e Poppler (GPL) sono esclusi per licenza.

## Opzioni considerate

1. **Parser.**
   - *PDFium* (binario non-V8 `chromium/8100` con hash fissato) per testo, pagine e rettangoli di parola: quasi esatto sul testo (15 pagine TAT-DQA su 20 con CER ≤ 1%, 96,4% delle parole con il proprio match), sempre con pagina e rettangolo; non decisivo per l'ordine di lettura (E-1 ed E-2 non raggiunte per l'ordine dei blocchi) né per la geometria fine di parola (E-3 77,0% contro 95%, per il box del font di `loose_bounds()`).
   - *Docling* (`docling-serve` 1.37.0) come sidecar sulle sole pagine-tabella: celle buone dove le intestazioni sono riconosciute, 2,0 s per pagina su GPU e 4,0 s su CPU, ricreato per documento; su tutte le pagine costerebbe 2-4 s per pagina.
   - *MuPDF*, *Poppler*: scartati per licenza. *Baseline PDFium a celle* per le tabelle: F1 0,011.
2. **OCR** (D-09 emendata; fonti: [PaddleOCR 3.x, pipeline OCR](https://www.paddleocr.ai/latest/en/version3.x/pipeline_usage/OCR.html) e [release PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR/releases), consultate il 2026-10-19).
   - *Tesseract 5.5.0* da CLI, `tessdata_best` ita+eng, TSV con bbox per parola, come job `ocr_render` + `ocr_tesseract` del lanciatore: CER aggregato 3,16% sulle scansioni buone (soglia 3% non raggiunta per l'ordine dei blocchi di due pagine: le parole ritrovate sono il 92-99%) e 2,74% sulle degradate; bbox entro 0,84 pt (buone) e 2,66 pt (degradate) dal rettangolo d'inchiostro analitico; 2,4-3,0 s per pagina a 300 DPI per job.
   - *PP-OCRv6* (PaddleOCR 3.7.0, `PP-OCRv6_medium`): sfidante principale con la regola «vince nettamente» scritta prima (CER relativo migliore del 30% e di almeno 1 punto su buone e degradate, tempo ≤ 1,5 × Tesseract). Lock pip approvato e librerie di sistema a versione fissata, pesi identificati (`paddle-weights.sha256`, `PP-OCRv6_medium_det` e `PP-OCRv6_medium_rec` risolti in build e confrontati in sessione, `--ocr_version PP-OCRv6 --lang it`). Sul servizio `paddle` da 2 GiB e 8 CPU, con **detection ridotta** (`--enable_mkldnn False --text_det_limit_type max --text_det_limit_side_len 960`, decisione utente del 2026-10-25, un solo tentativo, fissata prima della misura, svantaggio dichiarato sul testo piccolo): 60 invocazioni su 60 riuscite, picco 1,41 GiB; CER aggregato 13,57% sulle buone e 14,19% sulle degradate (contro 3,16% e 2,74% di Tesseract; 0,75-3,21% sulle quattro pagine a testo corrente, 29-51% su due pagine per l'ordine delle righe), 23,3 e 20,8 s per pagina (7,7 e 8,6 × Tesseract), bbox di riga e non di parola (controllo analitico fail 8 su 8). **Non vince nettamente**, né sul CER né sul tempo. PP-OCRv5, riferimento secondario, non misurato.
     *Storia:* fino al 2026-10-25 lo sfidante era «non valutabile su questo hardware»: col comando del piano la detection falliva per un difetto oneDNN/PIR di paddlepaddle 3.3.1 e, senza oneDNN e a piena risoluzione, usciva per OOM a 2 GiB a 300 e a 200 DPI; l'utente ha scelto allora la detection ridotta (RISULTATI-ocr.md, «Domande per l'utente», opzione a).
3. **Isolamento** (D-08).
   - *In-process*: PDFium nel processo del servizio, dietro il mutex di `thread_safe`. Fallisce in modo intermittente a concorrenza 8 (`FormatError` 11 su 20) e non contiene crash né OOM.
   - *Processo separato per documento* (lanciatore di 01-07): allowlist chiusa operazione → bin, argv senza shell, `RLIMIT_AS/CPU/FSIZE/NOFILE/CORE`, gruppo di processi e subreaper, `oom_score_adj` 1000 sul figlio, budget aggregato sul cgroup del worker, quota di output, tmpfs per job. Overhead 2,4% sul TTQ-parse del documento da 25 pagine (M9, soglia 10%).
   - *Windows Job Objects* (variante di D-08): non necessari, perché il parser gira solo in container Linux.

## Decisione

- **Parser:** PDFium per testo, pagine e rettangoli; Docling sidecar solo sulle pagine-tabella scelte dall'euristica di 01-08 (GPU quando c'è), raggiunto solo da PDF che hanno superato il prescan nel figlio `extract_pdf` e ricreato per documento e dopo ogni timeout, disconnessione o errore. L'ordine di lettura si ricostruisce sopra PDFium o si prende da Docling.
- **OCR:** Tesseract 5.5.0 da CLI, solo come job `ocr_render` (render a 300 DPI, PNG con `pHYs`) e `ocr_tesseract` (TSV per parola, coordinate riportate in punti PDF con la trasformazione registrata). Confermato dalla regola «vince nettamente»: lo sfidante PP-OCRv6, misurato con detection ridotta a 2 GiB e 8 CPU, ha CER e tempo per pagina peggiori (Opzioni). La soglia CER ≤ 3% di Tesseract sulle buone resta non raggiunta (3,16%, ordine dei blocchi di due pagine): il debito sull'ordine di lettura vale anche per l'OCR. Lo stato dell'ADR resta *Proposed* fino alla ratifica.
  *Storia:* fino al 2026-10-25 la riga era «OCR (provvisoria)», con la sostituzione aperta perché lo sfidante non girava sul servizio da 2 GiB.
- **Isolamento:** parsing e OCR in processo separato per documento e per pagina OCR, sempre tramite il lanciatore e i limiti calibrati con la formula di 01-07 v6 (prenotazione = 3 × max `VmHWM` benigno × (1 + discendenti) + quota; `RLIMIT_AS` = min(3 × max `VmPeak`, budget)): `parse` 848.777 KiB (gap-closure 1.4), `ocr_render` 1.111.596 KiB + quota 284.487.846 B, `ocr_tesseract` 823.248 KiB per job (con il discendente).
- **Concorrenza nel worker da 2 GiB:** 2 job `parse` insieme (M10 di 01-18 con la prenotazione di allora, 803.057 KiB, e batch reale della gap-closure 1.4 con 2 job `ok` con la prenotazione attuale, 848.777 KiB; M10 non è stata ripetuta, D-12 e D-40; il margine prima che la concorrenza scenda a 1 è dell'8,1% sul `VmHWM` di calibrazione); 2 job `ocr_tesseract` insieme (`max_admitted` 2 misurato, 12 batch su 12 conformi col metodo di M10); 1 job `ocr_render`.
- **Tetto di decompressione** nel prescan del figlio, prima di PDFium: 64 MiB per stream, 512 MiB di somma reale dei byte decompressi, rapporto 100 oltre 16 MiB, catena di filtri ≤ 2, `Width × Height × componenti` ≤ 64 MiB sulle dimensioni dichiarate e su quelle codificate di DCT (SOF) e JPX (SIZ); limite di 2.000 pagine.

## Conseguenze

- **Positive:** un file ostile costa al più un figlio: 26 casi ostili su 26 contenuti su `parse` (01-18), 15 righe su 15 contenute sul render (`jbig2-malformed`, `deep-nesting`, i 3 casi DCT/JPX della 1.5), OOM del cgroup sul figlio con padre vivo (M11). Le immagini con dimensioni codificate enormi sono respinte prima di PDFium su `parse`, `extract_pdf` e `ocr_render`. Errori tipizzati per la porta `DocumentParser` (S-07): `Encrypted`, `DecompressionLimit`, `Timeout`, `ResourceLimit`, `OutputLimit`, `Crashed{signal}`, `Corrupt`, `Unsupported`, stato `Quarantined`.
- **Tesseract 5.5.0 di Debian trixie accettata** (D-09 indica la CLI, la patch è irrilevante); `tessdata_best` e PDFium con hash fissati nell'immagine del worker; il binario PDFium si aggiorna sostituendo release e hash.
- **Pesi OCR:** PaddleOCR non adottato; se lo si riconsidera, pesi precaricati nell'immagine, fissati per SHA-256 (`paddle-weights.sha256`) e modelli risolti registrati e confrontati a ogni sessione (`v6.resolved.json`).
- **Concorrenza OCR limitata dal budget del cgroup del worker a 2 GiB:** 25 pagine scansionate richiedono 34,5-34,9 s con 2 job ammessi (soglia di 30 s con 8 job richiesti non raggiunta). Il dimensionamento per la produzione è **estrapolato** dal `max_admitted` misurato, non misurato: N job `ocr_tesseract` richiedono `memory.max` ≥ 256 + 804·N MiB (3,4 GiB per 4 job, 6,5 GiB per 8), con 8 job le 25 pagine scenderebbero a circa 9-10 s; ipotesi lineari, da misurare se cambia la memoria del worker (D-28). La prenotazione è una stima con margine (rapporto prenotazione/picco ≥ 6 sui job OCR misurati), non un tetto rigido (RISK-001): vale per i carichi misurati.
- **TTQ-parse:** il percorso composto reale non sta nella quota di 5 s per il documento digitale da 25 pagine, perché Docling sulle 13 pagine selezionate dall'euristica costa da solo circa 15 s (numeri in RISULTATI-ocr.md, «TTQ-parse»). La scelta fra le opzioni di «Deferred Ideas» (D-27: meno pagine a Docling, Docling asincrono dopo il primo indice, euristica più precisa) resta all'utente.
- **Debiti dichiarati:**
  - *immagini inline* (`BI … ID … EI` con DCT o JPX dentro i content stream): il prescan non le esamina e il tetto non le copre (D-09 della 1.5, T-01.5-11); la chiusura passa al prescan di prodotto della fase 3;
  - *un byte per campione* (RISK-14, accettato): il tetto conta `W × H × componenti`; un JPEG a 12/16 bit o un JPEG 2000 con più di 8 bit per campione può usare 2-4 volte quella memoria, contenuta comunque da `RLIMIT_AS` del figlio;
  - *residuo T-09 JBIG2: aperto* (D-34): `jbig2-malformed` dà `ok` sul render in 3 righe su 3 col contenimento intatto; il decoder JBIG2 di PDFium gestisce il flusso rotto senza errore visibile e non è dimostrato sicuro;
  - *`deep-nesting`*: `ok` sul render, la render non visita l'oggetto annidato;
  - *`parse-parent calibrate`* rifiuta le op con discendenti (`BudgetExceeded`): i limiti `ocr_tesseract` sono calcolati con la stessa formula da `ocr limits-from-runs`; il difetto del file chiuso di 01-07 va a una gap-closure.
- **Windows Job Objects fuori scope:** il parser gira solo in container Linux (cgroup v2, `RLIMIT_AS`, `oom_score_adj`, subreaper) e lo sviluppo su Windows usa Docker Desktop.
- **Docling sidecar** sulla rete interna `parse`, senza egress e senza porte pubblicate.

## Evidenze

- Indice ed Esito complessivo di S-02: [`spikes/s02-parser/RISULTATI.md`](../../spikes/s02-parser/RISULTATI.md).
- Isolamento e casi ostili: [`RISULTATI-isolamento.md`](../../spikes/s02-parser/RISULTATI-isolamento.md), [`results/isolation/hw.json`](../../spikes/s02-parser/results/isolation/hw.json).
- Estrazione PDFium e Docling: [`RISULTATI-estrazione.md`](../../spikes/s02-parser/RISULTATI-estrazione.md), [`results/extract/hw.json`](../../spikes/s02-parser/results/extract/hw.json).
- Locator: [`RISULTATI-locator.md`](../../spikes/s02-parser/RISULTATI-locator.md), [`results/locator/hw.json`](../../spikes/s02-parser/results/locator/hw.json) e [ADR-0005](0005-locator-citazione.md).
- OCR, throughput, TTQ-parse e prova ostile sul render: [`RISULTATI-ocr.md`](../../spikes/s02-parser/RISULTATI-ocr.md), [`results/ocr/hw.json`](../../spikes/s02-parser/results/ocr/hw.json).
