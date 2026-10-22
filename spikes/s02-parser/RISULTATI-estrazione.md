# S-02 — Estrazione da PDF digitale: testo, coordinate, tabelle e Docling · RISULTATI

**Modifica:** Claude (gsd-executor 01-08) · 2026-10-22 · domanda, soglie e contratti di abbinamento scritti prima di ogni misura (Task 1 del piano 01-08)

## Domanda e decisione da chiudere

Spike S-02, parte estrazione (assunzione A-05; decisioni D-07, D-08, D-11 di `01-CONTEXT.md`). Domande:

1. PDFium (`pdfium-render` 0.9.4, build `chromium/8100`), lanciato solo come figlio isolato `op: extract_pdf` del lanciatore di 01-07, estrae dai PDF digitali testo, numero di pagina e rettangoli delle parole con la qualità che serve alle citazioni verificabili?
2. Docling (`docling-serve` v1.37.0), usato solo sulle pagine con tabelle, ricostruisce le celle meglio di un raggruppamento per coordinate di PDFium, e con quali tempi su CPU e su GPU RTX 3050?
3. Un'euristica PDFium sceglie le pagine-tabella da mandare a Docling senza perderne, misurata su pagine mai viste in taratura?
4. Che cosa resta del lavoro di Docling dopo un timeout o una disconnessione del client, e quanto costa ricrearlo a ogni documento?

I numeri servono all'ADR-0004 (piano 01-13), che sceglie il parser; questo file non decide.

## Soglie (scritte PRIMA della misura)

Le soglie sotto e i contratti della sezione successiva sono fissati in questo commit, prima di qualunque file in `results/extract/`, e non si ritoccano sui risultati: una correzione va in «Anomalie e deviazioni», con motivo.

| Id | Metrica | Soglia | Fonte |
|----|---------|--------|-------|
| E-1 | CER del testo PDFium su TAT-DQA (EN), aggregato sulle 20 pagine: Σ distanze / Σ caratteri attesi | ≤ 1% | RICERCA M2, D-11 |
| E-2 | CER del testo PDFium sull'oracolo IT ratificato (P-001…P-006), aggregato come E-1; le pagine a due colonne misurano anche l'ordine di lettura | ≤ 1% | RICERCA M2, D-07 |
| E-3 | Quota di parole attese TAT-DQA con match e IoU ≥ 0,5, sul denominatore di tutte le parole attese valutabili delle 20 pagine (Σ su pagine) | ≥ 95% | RICERCA M3 |
| E-4 | F1 di cella di Docling sulle 4 tabelle IT (P-007…P-010), micro-aggregata (Σ celle abbinate, Σ attese, Σ predette), per ciascun dispositivo | ≥ 0,90 | RICERCA M5, D-07 |
| E-5 | F1 di cella della baseline PDFium (raggruppamento per coordinate) sulle stesse 4 tabelle | riportata, nessuna soglia | D-07 |
| E-6 | Recall delle regioni tabella di Docling sulle pagine DocLayNet con almeno una tabella (entrambi gli split: Docling non si tara) | ≥ 0,90 | RICERCA M5 |
| E-7 | Recall dell'euristica pagine-tabella sull'**holdout** (pagine DocLayNet di holdout + documento IT da 25 pagine, cumulata: Σ pagine-tabella selezionate / Σ pagine-tabella vere); le pagine `uncertain` contano come selezionate | ≥ 0,95 | D-07, D-29 |
| E-8 | Precisione dell'euristica sull'holdout; recall e precisione sulla taratura | riportate, mai usate come esito | D-29 |
| E-9 | Docling GPU, secondi per pagina-tabella (richiesta di una pagina con `page_range`), p95 nearest-rank sulle ripetizioni utili | ≤ 1,0 s | RICERCA §4 |
| E-10 | Docling CPU, stessa metrica, confrontata col budget di parse del README (25 pagine digitali ≤ 15 s di TTQ p95) | riportata (RICERCA: ≈ 3,1 s/pagina) | README, RICERCA |
| E-11 | VRAM di picco di Docling GPU (campionamento `nvidia-smi` ogni 500 ms, picco meno il valore a riposo prima dell'avvio del servizio) | registrata, nessuna soglia | RESEARCH §10 |
| E-12 | Docling dopo timeout o disconnessione: servizio riportato a riposo solo dal riavvio (stop + up); dopo il riavvio una pagina benigna riesce entro la soglia p95 del suo dispositivo (GPU: E-9; CPU: p95 misurato in E-10) | `next_benign_status=ok` e tempo ≤ p95 del dispositivo | deepsearch_2 |

Regole comuni:

- **Oracoli pendenti.** Si usano solo le voci di `spikes/data/corpus/oracles/parser-it.json` con i tre assi verdi (`claude` concorde, `gpt` `CONFIRMED`, `user` `ratified`); le altre si escludono e si contano qui.
- **Ripetizioni Docling.** Tre richieste per pagina; la prima si scarta (riscaldamento del modello sul documento); n utile = 2 per pagina. Prima della prima misura di ogni servizio c'è una richiesta di riscaldamento su una pagina benigna di riferimento, esclusa.
- **Percentile.** p95 nearest-rank: valore in posizione ⌈0,95 · n⌉ della lista ordinata.
- **Esiti non `ok`.** Un job `extract_pdf` non `ok` resta nel denominatore di E-1…E-3 con CER 1 e parole tutte `missed`, e si riporta; non si ripete in silenzio.

## Contratti di abbinamento (scritti PRIMA della misura)

Implementati in `src/bin/extract/score.rs` e `src/bin/extract/tables.rs`, con test unitari.

### Coordinate

- Tutto è in punti PDF, origine in basso a sinistra, nel `MediaBox` della pagina dopo la rotazione dichiarata (`/Rotate`). PDFium dà i rettangoli dei caratteri (`loose_bounds()`, box del font sull'intero avanzamento) nello spazio utente non ruotato; il figlio li porta nello spazio della pagina ruotata: con `MediaBox` `[x0, y0, x1, y1]`, rotazione 0 → `(x − x0, y − y0)`; 90 → `(y − y0, x1 − x)`; 180 → `(x1 − x, y1 − y)`; 270 → `(y1 − y, x − x0)`. `width` e `height` del JSON di pagina sono quelli della pagina ruotata.
- TAT-DQA (pixel, origine in alto a sinistra, sull'immagine annotata `W_px × H_px` = `pages[i].bbox` del JSON del dataset) → `x_pt = x_px · W_pt / W_px`, `y_pt = H_pt − y_px · H_pt / H_px`, con `W_pt × H_pt` della pagina PDF ruotata. DocLayNet (pixel COCO 1025 × 1025, origine in alto a sinistra) usa la stessa trasformazione con l'immagine `coco_width × coco_height` e la pagina PDF.
- Le pagine con `CropBox` ≠ `MediaBox` o rotazione ≠ 0 si trattano con la stessa trasformazione e si marcano in «Anomalie».

### Normalizzazione

- **Token** (parole e celle): Unicode NFC e spazi rimossi ai bordi, nient'altro (la punteggiatura resta: `1.234` e `1234` sono diversi).
- **Testo per il CER**, sui due lati: NFC, ogni sequenza di spazi bianchi (a capo compresi) collassata in uno spazio, bordi rimossi. Distanza di Levenshtein sui caratteri Unicode; CER = distanza / caratteri attesi.
- **Lato predetto dell'oracolo IT**, in più, le regole meccaniche dell'oracolo (`ORACOLI.md`): legature tipografiche U+FB00…U+FB06 scritte come lettere separate; segno meno U+2212 → `-`; trattino morbido U+00AD rimosso; trattino di sillabazione a fine riga (`-` fra una lettera e un a capo seguito da una lettera minuscola) unito. Esclusioni meccaniche: righe la cui mezzeria cade nel 7% superiore o inferiore dell'altezza della pagina (testatina e piè di pagina con il numero di pagina). Il testo dentro grafici e fotografie **non** si esclude meccanicamente: resta nella predizione e nel CER. Per questo, accanto al CER, si riporta la scomposizione delle operazioni di modifica (sostituzioni S, cancellazioni D, inserzioni I) e la diagnostica (S + D) / N, che non entra nell'esito.
- **Testo atteso TAT-DQA**: parole dei blocchi in ordine di `order`, nell'ordine di `word_list`, unite da uno spazio. **Testo predetto PDFium**: il testo della pagina di PDFium (`text`, campo del JSON di pagina), stessa normalizzazione.
- **Celle**: normalizzazione dei token più spazi interni collassati in uno e segno meno U+2212 → `-` (regola dell'oracolo delle tabelle), su atteso e predetto.

### Parole (IoU)

- Parole predette: caratteri di PDFium nell'ordine del testo della pagina; una parola è una sequenza massimale di caratteri non bianchi (gli spazi e gli a capo, anche generati da PDFium, separano); bbox = unione dei bbox dei caratteri.
- Per pagina, coppie candidate = parola attesa e parola predetta con testo normalizzato uguale e IoU > 0. Abbinamento uno a uno greedy per IoU decrescente (a parità, distanza dei centri crescente, poi indici crescenti): ogni parola predetta si usa al più una volta.
- Seconda passata solo per le attese rimaste libere, nell'ordine delle attese: unione di 2 o 3 parole predette libere, consecutive nell'ordine di lettura e sulla stessa riga (intervalli verticali sovrapposti per almeno metà dell'altezza minore), la cui concatenazione è uguale al testo atteso e il cui bbox unione ha IoU > 0 con l'atteso → match `merged` col bbox unione (prima la più corta, a parità l'IoU più alto).
- Attese senza match = `missed` con IoU 0, nel denominatore. Predette non usate = `spurious`. Attese non valutabili (testo vuoto dopo la normalizzazione, bbox degenere o non interamente dentro la pagina) = `unscorable`, escluse dal denominatore ma contate e riportate.
- `iou_mean` = media dell'IoU su tutte le attese valutabili (miss = 0); `share_iou_ge_05` = attese con match (`matched` o `merged`) e IoU ≥ 0,5 / attese valutabili. Senza attese valutabili la pagina riporta 0 e si segnala.

### Celle (F1)

- Cella = `(riga, colonna, testo normalizzato)`; abbinamento uno a uno per uguaglianza esatta della tripla (multinsieme: celle identiche ripetute si abbinano una volta ciascuna). Precision = abbinate / predette; recall = abbinate / attese; F1 armonica (0 se entrambe 0).
- Tabella predetta abbinata alla tabella attesa della stessa pagina con IoU dei bbox massimo, uno a uno (greedy per IoU decrescente). Una tabella attesa senza predetta conta tutte le sue celle come miss, con precision 0.
- **Oracolo IT senza bbox**: le tavole P-007…P-010 non hanno rettangolo; la tavola attesa si abbina, fra le predette della stessa pagina, a quella con più celle abbinate (a parità la prima nell'ordine dell'output), e le altre predette della pagina non si contano. Si confrontano solo le celle del corpo (`row` ≥ 1 dell'oracolo); le intestazioni (`headers`) non entrano nell'F1.
- **Indici delle celle predette**: Docling dà indici 0-based di riga e colonna (`start_row_offset_idx`, `start_col_offset_idx`) e marca le intestazioni di colonna; le righe con sole celle d'intestazione si tolgono, le altre si rinumerano da 1 dall'alto, le colonne valgono indice + 1. Una cella su più righe o colonne vale nella sua posizione iniziale. La baseline PDFium usa le righe di testo della pagina con almeno 3 gruppi di parole (gruppo = parole consecutive con spazio orizzontale < `cell_gap_pt` del file dei parametri), riga = ordine dall'alto fra queste, colonna = ordine del gruppo nella riga.

### Regioni tabella (DocLayNet)

- Regioni attese = annotazioni di categoria `Table` (id 9) con `precedence` 0 della pagina, convertite in punti PDF. Regioni predette = rettangoli delle tabelle di Docling (`prov[].bbox`, convertiti in punti PDF con origine in basso a sinistra secondo il `coord_origin` dichiarato).
- Abbinamento uno a uno greedy per IoU decrescente, match se IoU ≥ 0,5; recall = regioni attese abbinate / regioni attese, cumulata sulle pagine.

### Euristica pagine-tabella

- Parametri solo nel file `--params` (`results/extract/table_heuristic_params.json`), nessun default nel codice: pesi, espressione dei token numerici, tolleranza di allineamento, banda di incertezza. Punteggio di pagina in [0, 1] da densità di token numerici e allineamento delle colonne per coordinate x.
- Banda: punteggio ≥ `select_from` → `heuristic`; `uncertain_from` ≤ punteggio < `select_from` → `heuristic+uncertain` (mandata comunque a Docling); sotto → non selezionata.
- Taratura solo sulle pagine `tune` di `spikes/data/s02/splits.json`, senza aprire gli attesi dell'holdout né l'elenco pagine-tabella dell'oracolo IT. Regola di taratura fissata qui: `select_from` = punteggio minimo fra le pagine-tabella vere di taratura; `uncertain_from` = 0,5 × `select_from`. Il file dei parametri si committa da solo, prima di `table_pages.csv`.
- Recall e precisione per documento; recall 1 con zero pagine-tabella vere, precisione 1 con zero selezionate. L'esito E-7 usa solo le righe `split=holdout`, cumulate.

### Esempi

`extract score` scrive in `results/extract/iou_examples.json` 10 esempi di allineamento per la lettura umana: due per tipo (`matched`, `merged`, `missed`, `spurious`, `unscorable`) quando esistono, scelti in ordine di documento e pagina.

## Setup

**Modifica:** Claude (gsd-executor 01-08) · 2026-10-22 · Setup, Risultati, Anomalie ed Esito parziale (Task 2 del piano 01-08)

- **Hardware:** [`results/extract/hw.json`](results/extract/hw.json) (sha256 `dc30535c…161980c`): i7-13700K 16 core / 24 thread, 32 GB, RTX 3050 6 GB (driver 610.88, CUDA 13.3), Docker 29.8.2 su WSL2 (16 CPU e 19,5 GiB a Docker Desktop), rustc 1.95.0. Il file è normalizzato allo schema di `common::manifest` (vedi Anomalie), con l'originale in `results/raw/extract-hw-raw.json`.
- **Immagini:** worker `cka-spikes-worker@sha256:8ed6d8c6…3dd2d657` (ricostruita da questo piano con il bin `extract`, Dockerfile di 01-07 invariato); `docling-serve-cu128@sha256:2aa6fc68…dfc76d` e `docling-serve-cpu@sha256:6eda9c52…d18a3`. `GET /version` del servizio: docling-serve 1.37.0, docling 2.138.0, docling-core 2.101.1, docling-parse 7.22.3, docling-ibm-models 4.0.3. PDFium `chromium/8100` nel worker.
- **Dati:** `spikes/data/s02/manifest.json` (84 voci, licenze in `DATASET.md`), `splits.json` (20 + 20 per documento), corpus comune con l'oracolo `parser-it.json` (11 voci, tutte con i tre assi verdi: nessun oracolo escluso). Derivati per le prove di confine, prodotti con `qpdf --deterministic-id` nel runner da `der-it-01-relazione-annuale-2025-p1-25`: `single-p12.pdf` (la pagina-tabella 12, 1 pagina), `big400.pdf` (il documento ripetuto 16 volte, 400 pagine, 2,75 MB), `outside400.pdf` (la pagina 12 seguita dalle prime 399 di `big400`, 400 pagine).
- **Prova d'invocazione** ([`invocation.txt`](results/extract/invocation.txt)), nel worker con `parse-parent run --job <file>`: il job `extract_pdf` valido dà `RunReport` con `status=ok`, `pages=1` e `page-0001.json` nell'`out_dir` (496 parole, 2.659 caratteri); lo stesso job con `params: {"dpi": 300}` dà `job_error` `InvalidParams` con exit 2; un input `/etc/passwd` dà `PathNotAllowed` (`input_outside_roots`) con exit 2. Ogni prova ha il proprio `job_id`.
- **Limiti** ([`limits-extract_pdf.json`](results/extract/limits-extract_pdf.json)): `parse-parent calibrate --op extract_pdf` su 70 PDF (i 63 di `data/s02`, i 4 documenti IT del corpus, i 3 derivati), formula v6 di 01-07: 66 ok, 4 rifiuti (Anomalie); max `VmPeak` 758.252 kB, max `VmHWM` 313.352 kB, massimo scritto 44,7 MB (il documento da 400 pagine). Profilo `extract_pdf`: prenotazione 962.617.344 B (3 × `VmHWM`), `RLIMIT_AS` 1.879.048.192 B (tetto del budget del worker), quota 89.421.160 B, CPU 60 s, parete 120 s, 64 file aperti.
- **Estrazione:** 70 job in `results/raw/jobs/extract.jsonl` (`job_id` = `<item>-re08s1rn5`), `parse-parent batch --concurrency 4 --limits /results/extract/limits-extract_pdf.json`: 66 `ok`, 4 `failed` `Unsupported` (gli stessi della calibrazione), nessun `job_error`; il budget ha ammesso al più 2 job insieme. PDFium estrae 1 pagina in circa 45 ms, il documento da 228 pagine in 2,7 s e quello da 400 in 3,1 s di parete del job.
- **Docling:** `docling-bench.sh measure` e `probes` dentro `session.sh with 01-08`, prima `docling` (GPU) poi `docling-cpu`, mai insieme (`session.sh ps --assert-only` a ogni avvio); riscaldamento su una pagina TAT-DQA esclusa; 3 richieste per pagina, la prima scartata; servizio ricreato (`stop` + `up`) a ogni documento. Sessioni: misure e prove GPU (ripetizione 1) `01-08-20261022t172432z-31a3`, misure e prove CPU (ripetizione 1) `01-08-20261022t174356z-978a`, prove di confine ripetizione 2 su GPU e poi CPU `01-08-20261022t180815z-6da7` (colonna `run`).
- **Comandi per rigenerare** (da `spikes/`, con `CKA_RUN_ID` della sessione): `bash data/s02/fetch.sh`; `bash data/corpus/fetch.sh` fuori sessione; nel runner `qpdf` per i derivati come sopra; `docker compose … build worker`; `session.sh up "$CKA_RUN_ID" worker`; `session.sh exec "$CKA_RUN_ID" worker -- parse-parent calibrate --op extract_pdf --inputs /results/raw/extract-cal --out /results/extract/limits-extract_pdf.json --csv /results/raw/extract-calcsv/calibration-extract_pdf.csv --run-tag <tag>`; nel runner `extract jobs --manifest /src/data/s02/manifest.json --corpus-doc <4 documenti IT> --input /results/raw/derived/<3 derivati> --run-id r<tag>`; `session.sh exec … parse-parent batch --jobs /results/raw/jobs/extract.jsonl --concurrency 4 --limits /results/extract/limits-extract_pdf.json > results/raw/extract/runs.jsonl`; `docling-bench.sh hash-inputs`; nel runner `extract score text …`, `extract select-tables tune …` (commit dei parametri), `extract select-tables measure …`; `docling-bench.sh measure gpu|cpu <pagine>` e `probes gpu|cpu`; nel runner `extract score tables … --docling gpu=… --docling cpu=…`.

## Risultati

### Testo e coordinate di PDFium

| Id | Misura | n | Valore | Soglia | Esito |
|----|--------|---|--------|--------|-------|
| E-1 | CER TAT-DQA aggregato | 20 pagine, 67.735 caratteri attesi | **9,48%** (mediana per pagina 0,14%; 15 pagine su 20 ≤ 1%) | ≤ 1% | **non raggiunta** |
| E-2 | CER oracolo IT aggregato | 6 pagine, 15.581 caratteri | **21,58%** | ≤ 1% | **non raggiunta** |
| E-3 | Parole TAT-DQA con match e IoU ≥ 0,5 | 9.759 parole valutabili | **77,0%** (7.514) | ≥ 95% | **non raggiunta** |

CSV: [`text_cer.csv`](results/extract/text_cer.csv), [`bbox_iou.csv`](results/extract/bbox_iou.csv), esempi in [`iou_examples.json`](results/extract/iou_examples.json).

**E-1.** Il CER aggregato è fatto da due pagine: `tatdqa-018ba3b4…` (43,2%) e `tatdqa-09a6000c…` (66,9%), le stesse che hanno 364 e 537 parole attese non valutabili (rettangoli di larghezza zero nell'annotazione). Delle altre 18, 15 stanno entro l’1% (7 a zero) e 3 fra 1,1% e 10,8%.

**E-2, per pagina** (S, D, I = sostituzioni, cancellazioni, inserzioni; (S + D)/N è diagnostica, non esito):

| Oracolo | Pagina | Caratteri | CER | S | D | I | (S + D)/N | Due colonne |
|---------|--------|-----------|-----|---|---|---|-----------|-------------|
| P-001 | pub-it-01 p. 11 | 2.602 | 1,27% | 0 | 0 | 33 | 0,00% | no |
| P-002 | pub-it-01 p. 12 | 2.615 | 74,57% | 0 | 958 | 992 | 36,63% | no |
| P-003 | pub-it-01 p. 13 | 3.432 | 1,02% | 1 | 0 | 34 | 0,03% | no |
| P-004 | pub-it-03 p. 5 | 2.127 | 0,75% | 0 | 7 | 9 | 0,33% | sì |
| P-005 | pub-it-03 p. 6 | 2.549 | 40,96% | 0 | 514 | 530 | 20,16% | sì |
| P-006 | pub-it-03 p. 12 | 2.256 | 12,59% | 0 | 135 | 149 | 5,98% | sì |

Nelle pagine con CER alto le cancellazioni e le inserzioni quasi si pareggiano e le sostituzioni sono zero: PDFium ha tutti i caratteri, ma in un altro ordine (blocchi interi spostati: la Tavola 1.1 e la nota di P-002, i riquadri di P-005 e P-006). Le inserzioni residue delle pagine buone (33-34 caratteri) sono testo che l'oracolo esclude e le esclusioni meccaniche non tolgono (titoli correnti fuori dalla banda del 7%).

**E-3.** Su 9.759 parole attese valutabili, 9.405 hanno un match di testo uno a uno (96,4%), 354 sono `missed`, nessuna `merged`; 1.374 predette sono `spurious`, 901 attese `unscorable`. La quota sotto soglia viene quindi dalla geometria, non dal testo: `loose_bounds()` dà il box del font (da discendente ad ascendente), le annotazioni TAT-DQA un box più stretto, e su parole corte l'IoU scende sotto 0,5. `iou_mean` medio per pagina 0,666. La pagina `tatdqa-04aea9b8…` ha CropBox diverso dal MediaBox (marcata, vedi Anomalie): quota 24,3%; senza di lei E-3 varrebbe 80,5% (diagnostica, non esito).

### Euristica pagine-tabella

Parametri congelati in [`table_heuristic_params.json`](results/extract/table_heuristic_params.json) (commit `5d2d65e`, prima di `table_pages.csv`): `select_from` 0,3057, `uncertain_from` 0,1528, tarati su 16 pagine di taratura estratte (le 4 NSANY non hanno job ok). CSV: [`table_pages.csv`](results/extract/table_pages.csv).

| Id | Split | Pagine-tabella vere | Selezionate (con incerte) | Colpite | Recall | Precisione | Soglia | Esito |
|----|-------|---------------------|---------------------------|---------|--------|------------|--------|-------|
| E-7 | holdout (10 DocLayNet + 1 IT) | 11 | 23 | 10 | **0,909** | 0,435 | recall ≥ 0,95 | **non raggiunta** |
| E-8 | taratura | 10 | 11 | 8 | 0,800 | 0,727 | riportata | — |

Nell'holdout l'unica pagina-tabella persa è `dln-nyse-hrl-2004-p19`, la tabella con meno cifre del campione (quota di cifre 0,015): il punteggio sta sotto `uncertain_from`. Le altre 9 tabelle DocLayNet di holdout, comprese le senza bordi e le altre 4 con poche cifre, sono selezionate. Nel documento IT da 25 pagine l'euristica prende la sola pagina-tabella vera (12) ma anche 12 negative (le pagine 5-7 di indice e avvertenze, che l'oracolo segnala come negative difficili, e 9 pagine di testo con molte cifre): precisione 1/13. In taratura le 2 pagine-tabella perse sono le NSANY non estratte.

### Docling: celle, regioni e tempi

CSV: [`tables.csv`](results/extract/tables.csv), [`docling.csv`](results/extract/docling.csv).

| Id | Misura | GPU | CPU | Soglia | Esito |
|----|--------|-----|-----|--------|-------|
| E-4 | F1 di cella micro sulle 4 tavole IT | **0,580** (140 abbinate su 237 attese, 246 predette) | **0,580** | ≥ 0,90 | **non raggiunta** |
| E-5 | F1 di cella della baseline PDFium | 0,011 | — | riportata | — |
| E-6 | Recall delle regioni tabella DocLayNet (18 pagine con tabella, 42 regioni) | **0,595** (25) | **0,595** | ≥ 0,90 | **non raggiunta** |
| E-9 | Secondi per pagina-tabella, p95 (n = 78 richieste utili) | **2,054 s** (p50 2,022) | — | ≤ 1,0 s | **non raggiunta** |
| E-10 | Stessa metrica su CPU | — | **8,016 s** (p50 4,015) | riportata | — |
| E-11 | VRAM di picco sopra il riposo | **1.569 MiB** (per documento 1.157-1.569) | — | registrata | — |

Per tavola (identico su GPU e CPU): P-007 0,990, P-008 0,063, P-009 0,054, P-010 1,000. Su P-008 e P-009 Docling non marca come intestazione la seconda riga d'intestazione (`PAESI` / `VOCI` con anni e trimestri): per il contratto quella riga diventa la riga 1 del corpo e tutte le righe successive slittano di uno. Diagnostica a posteriori, non esito: con lo spostamento di una riga P-008 sale a 0,844 e P-009 a 0,966, e l'F1 micro a 0,965. La baseline PDFium non arriva a nessuna tavola: le righe con almeno 3 gruppi comprendono intestazioni e titoli, e la colonna 1 dell'oracolo (etichetta su più parole) si spezza.

Regioni: 15 pagine su 18 hanno recall 1; le altre tre sono `nyse-smfg-2011-p70` (10 regioni attese, piccole tabelle affiancate, 2 predette, 0 abbinate), `asx-sto-2004-p59` (1 su 4) e `nyse-chk-2010-p46` (4 tabelle di quotazioni nel riquadro laterale, una sola regione predetta).

Tempi: il tempo del client è quantizzato a multipli di circa 2 s (GPU quasi tutte le richieste a 2,02 s, CPU a 4,0, 8,0 e 16,0 s): l'endpoint sincrono di docling-serve attende il risultato a intervalli. Il `processing_time` dichiarato dal servizio nella risposta (diagnostica) vale p50 0,725 s e p95 1,700 s su GPU, p50 2,233 s e p95 7,572 s su CPU. La prima richiesta per pagina (scartata) non è più lenta delle altre (p50 2,023 s GPU, 4,017 s CPU): il riscaldamento per documento basta. Ricreare il servizio (stop, up, attesa di `/version`, riscaldamento) costa p50 19,1 s su GPU (29 riavvii, massimo 21,7 s) e 20,8 s su CPU (massimo 26,4 s). Tutti i 234 esiti delle richieste di misura sono `ok`; nessuna riga Docling riguarda input senza `RunReport` ok (le 2 pagine-tabella NSANY non estratte sono escluse dall'elenco).

Confronto col budget di parse: il documento IT da 25 pagine manda a Docling 13 pagine; a tempo di client sono circa 26 s su GPU e oltre 52 s su CPU, contro un TTQ p95 di 15 s per l'intero documento.

### Confine di Docling

CSV: [`docling_isolation.csv`](results/extract/docling_isolation.csv); tempi delle prove e dei riavvii in `results/raw/docling/<dispositivo>/probes.tsv`, marcatore in `state-marker.tsv`. Due ripetizioni per dispositivo; i numeri sotto sono ripetizione 1 / ripetizione 2.

| Prova | Dispositivo | Esito del client | CPU del container 0 s → 10 s | Memoria 0 s → 10 s (MiB) | Picco cgroup (MiB) | Dopo il riavvio |
|-------|-------------|------------------|-------------------------------|--------------------------|--------------------|-----------------|
| `timeout` (400 pagine, `document_timeout` 5) | GPU | `error` dopo 10,1 s (r2), exit 1 | 0,29% → 0,17% / 0,16% → 0,35% | 4.003 → 4.008 / 2.586 → 2.588 | 4.977 / 4.959 | pagina benigna `ok` in 2,02 s |
| `timeout` | CPU | `error` dopo 8,1 s, exit 1 | 0,11% → 0,11% / 0,10% → 0,16% | 4.618 → 4.618 / 4.131 → 4.130 | 4.790 / 4.941 | `ok` in 4,02 s |
| `disconnect` (chiusura dopo 3 s) | GPU | `client_abort`, exit 3 | 243% → 234% / 233% → 210% | 4.859 → 6.073 / 4.919 → 6.237 | 6.205 / 6.322 | `ok` in 2,01 s |
| `disconnect` | CPU | `client_abort`, exit 3 | 641% → 833% / 613% → 862% | 4.496 → 5.152 / 4.618 → 4.860 | 5.177 / 5.194 | `ok` in 4,02 s |
| `outside_range` pagina da sola (`page_range` 1-1) | GPU | `ok` in 2,02 s; `processing_time` 0,970 s (r2) | ≈ 0% | 1.709 / 1.741 | 1.744 / 1.747 | `ok` |
| `outside_range` pagina + 399 (`page_range` 1-1) | GPU | `ok` in 2,03 s; `processing_time` 1,098 s (r2) | ≈ 0% | 1.715 / 1.725 | 1.755 / 1.764 | `ok` |
| `outside_range` pagina da sola | CPU | `ok` in 4,02 s; `processing_time` 3,678 s (r2) | ≈ 0% | 1.054 / 1.060 | 1.240 / 1.246 | `ok` |
| `outside_range` pagina + 399 | CPU | `ok` in 4,03 s; `processing_time` 3,560 s (r2) | ≈ 0% | 1.077 / 1.078 | 1.274 / 1.279 | `ok` |
| `state_marker` (TEST-4) | GPU e CPU | marcatore assente nel container ricreato | — | — | — | `ok` |

- **Timeout.** Con `document_timeout` 5 il servizio risponde con un errore dopo 8-10 s e il container torna a riposo (CPU sotto l'1% a 0 e 10 s): il lavoro si ferma, ma la memoria resta occupata (2,6-4,6 GB) finché il container non si ricrea.
- **Disconnessione.** La chiusura della connessione **non** ferma il lavoro: 10 s dopo il container usa ancora da 2 a 8,6 CPU e la memoria cresce (fino a +1,3 GB). Solo il riavvio lo riporta a riposo.
- **`page_range`.** Con `page_range` 1-1 una pagina dentro un PDF da 400 costa quasi come la pagina da sola: +0,13 s di `processing_time` su GPU, −0,12 s su CPU, +11-34 MiB di picco del cgroup; la risposta contiene la sola pagina 1. Docling quindi non elabora le pagine fuori intervallo; resta il costo di aprire e indicizzare il PDF intero, piccolo su questo file da 2,8 MB.
- **Stato fra documenti (TEST-4, RISK-001 di risk_audit_11).** Dopo un documento si scrive un marcatore in `/tmp` e nella home del servizio, poi `stop` + `up`: il container ricreato ha un id diverso, nessuno dei due marcatori, 4 processi come prima, e un solo mount, il volume `docling-models` in sola lettura (`/models`, `RW=false`). Nessuna cache o volume scrivibile sopravvive al riavvio, nei limiti di quanto si osserva da dentro il container.
- **E-12.** Dopo ogni timeout, disconnessione e prova `page_range` il servizio ricreato risponde alla pagina benigna con `ok` in 2,01-2,02 s su GPU e 4,01-4,02 s su CPU, sotto il p95 del dispositivo (2,054 s e 8,016 s): **raggiunta**.

## Anomalie e deviazioni

- **[NV] RESEARCH §11.5, chiuso.** `loose_bounds()` di `pdfium-render` 0.9.4 dà i rettangoli nello spazio utente non ruotato, e `width()`/`height()` di PDFium misurano la pagina sul CropBox: il figlio porta i rettangoli nella pagina ruotata e prende larghezza e altezza dal MediaBox ruotato (correzione `6d36335`, prima di ogni misura registrata). Una pagina TAT-DQA (`tatdqa-04aea9b8…`) ha CropBox `[4,5 9 607,5 783]` su MediaBox `[0 0 612 792]`: trattata con la stessa trasformazione come da contratto e marcata (`crop_differs=true`); l'immagine annotata di TAT-DQA segue il CropBox, quindi la sua quota IoU (24,3%) misura soprattutto lo scarto fra i due riquadri. Nessuna pagina ruotata nel campione.
- **[NV] RESEARCH §11.8, chiuso.** Nell'OpenAPI del servizio (`/openapi.json`, schema `Body_process_file_v1_convert_file_post`) `page_range` è un array di due interi (`minItems` = `maxItems` = 2, pagine da 1); nel multipart si invia come due campi omonimi e la risposta contiene solo le pagine richieste, con la numerazione originale (`page_no` 17 per `page_range` 17-17). Il fallback `/v1/convert/source` non è servito. Campi del `DoclingDocument` usati: `tables[].prov[0].page_no`, `prov[0].bbox` (`l`, `t`, `r`, `b`, `coord_origin` `BOTTOMLEFT`), `data.table_cells[]` con `start_row_offset_idx`, `start_col_offset_idx`, `column_header`, `text`, `pages.<n>.size`.
- **Prescan su PDF benigni.** Le 4 pagine DocLayNet del documento `OTC_NSANY_2004` (2 con tabella, 2 a colonne, tutte nello split di taratura) sono rifiutate dal prescan v6 di 01-07 con `Unsupported` / `image_filter_on_non_image`, sia in calibrazione sia nel batch. Restano nei denominatori: in taratura non fissano la banda (correzione `3918aa8`) e contano come pagine-tabella perse; non vanno a Docling perché non hanno `RunReport` ok. È un falso positivo del prescan su un file benigno, da portare a 01-07/ADR-0004, non corretto qui (lanciatore chiuso).
- **Oracoli.** Nessun oracolo pendente: le 11 voci di `parser-it.json` hanno i tre assi verdi.
- **Annotazioni non valutabili.** 901 parole TAT-DQA `unscorable`, quasi tutte con rettangolo di larghezza zero (`[0, y, 0, y']`) in due pagine (`tatdqa-018ba3b4…` 364, `tatdqa-09a6000c…` 537): escluse dal denominatore di E-3 come da contratto. Le stesse due pagine fanno il CER aggregato di E-1.
- **Contratto delle celle e intestazioni.** Il contratto fissato prima toglie solo le righe tutte marcate come intestazione; Docling non marca la seconda riga d'intestazione di P-008 e P-009, e l'F1 di quelle tavole crolla per uno slittamento di riga. Il numero d'esito resta quello del contratto (0,580); la cifra con lo spostamento (0,965) è una diagnostica calcolata dopo aver visto i risultati e non sostituisce l'esito.
- **Copertura limitata dell'holdout.** 11 pagine-tabella vere: una sola pagina persa porta la recall da 1 a 0,909. La pagina persa è la tabella con meno cifre, cioè proprio la tipologia che l'holdout doveva contenere.
- **Tempi di Docling quantizzati.** L'endpoint sincrono `/v1/convert/file` restituisce il risultato a intervalli di circa 2 s: il tempo del client (quello della soglia E-9) non scende sotto 2 s per pagina anche quando il servizio dichiara meno di 1 s di elaborazione. Il `processing_time` della risposta si riporta come diagnostica.
- **Difetti dello strumento corretti durante il Task 2** (file di questo piano, commit separati, nessuna misura registrata prima delle correzioni): scrittura non bufferizzata del JSON di pagina, da 2 a 29 s per pagina sul bind mount (`3918aa8`); larghezza e altezza della pagina dal CropBox (`6d36335`); `docling-bench.sh` che non passava `--extract-jobs` all'attesa del servizio (`1097fd2`), che consumava lo stdin nel ciclo delle pagine e misurava solo la prima pagina di ogni documento (`dfa11b2`), che non registrava il tempo delle prove (`bd501f5`) né il JSON delle prove `page_range` (`a16b99b`). Le righe di una prima corsa GPU interrotta per il difetto dello stdin (3 documenti, una pagina ciascuno) sono state scartate e la corsa rifatta per intero; non compaiono in `docling.csv`. Per gli ultimi due difetti le prove di confine hanno una seconda ripetizione: la prima (run 1) non ha tempo della prova né `processing_time`, ed è tenuta nel CSV.
- **Deviazione (a): `docling-client --extract-jobs`.** Il `RunReport` di 01-07 non porta `input`: gli input ammessi si ricavano dal JSONL dei job (`job_id` → `input`, solo `op: extract_pdf`) unito a `--extract-runs`. Lo sha256 «stesso input» non lo verifica il client (nessuna dipendenza `sha2` nel crate, `Cargo.toml` fuori dai file del piano): lo verifica `docling-bench.sh` con `sha256sum` contro `inputs.sha256`, registrato subito dopo l'estrazione.
- **Deviazione (a): job da file nella prova d'invocazione.** Il piano chiedeva `parse-parent run --job -`; `session.sh exec` non inoltra lo stdin, quindi i tre job stanno in file sotto `/results/raw/jobs/`.
- **Deviazione (a): download per membro d'archivio.** Gli archivi di TAT-DQA e DocLayNet superano il tetto di 100 MiB per file: `fetch.sh` legge i soli membri con richieste HTTP Range (ognuna sotto il tetto) e verifica lo sha256 sul membro. Il DNS di sistema non risolveva l'host di DocLayNet: `fetch.sh` accetta un risolutore DNS-over-HTTPS facoltativo.
- **Deviazione (b), segnalazione: `hw-manifest.sh` / `common::manifest`.** Lo stesso difetto già registrato da 01-04, 01-05, 01-07, 01-11 e 01-12: `common::csv::Writer` rifiuta l'`hw.json` grezzo («invalid type: map, expected a string»). `hw.json` è normalizzato come in 01-07 (campi oggetto portati a stringa, dettagli in `*_detail`); nessun file chiuso toccato.
- **`hw.json` della sessione del worker.** È scritto nella sessione di estrazione (run_id `01-08-20261022t171317z-d936`); le sessioni Docling dello stesso pomeriggio usano la stessa macchina e le immagini Docling elencate nello stesso file.
- **Corpus comune fuori sessione.** `data/corpus/fetch.sh` (01-03) dentro `session.sh with` falliva sui download (`curl` riceve percorsi non convertiti con la conversione MSYS spenta); lanciato fuori sessione, ha prenotato da sé la finestra come `01-03`.
- **Rischio residuo MEDIUM di risk_audit_11 (RISK-001).** Il marcatore di stato, i processi e i mount sono controllati fra due documenti consecutivi su entrambi i dispositivi: nessuno stato osservabile passa. Resta fuori dalla prova quanto non si vede da dentro il container (per esempio lo stato del driver GPU dell'host).

## Esito parziale

- **PDFium per testo, pagine e coordinate: confermato per completezza e posizione, non per l'ordine di lettura né per la geometria di parola.** Sulle pagine TAT-DQA ben annotate il testo è quasi esatto (15 pagine su 20 con CER ≤ 1%) e il 96,4% delle parole ha il proprio match uno a uno; il numero di pagina e il rettangolo di ogni parola ci sono sempre. Le tre soglie E-1, E-2, E-3 non sono raggiunte: il CER è dominato dall'ordine dei blocchi (tavole, note, riquadri, colonne), e l'IoU dal box del font di `loose_bounds()` contro il box più stretto delle annotazioni. Per le citazioni conta che ogni parola abbia pagina e rettangolo; l'ordine di lettura va ricostruito sopra PDFium o preso da Docling, e la tolleranza sulla geometria va decisa nell'ADR-0004 (01-13 confronta già OCR e PDFium con questa riserva).
- **Docling GPU contro CPU sulle pagine-tabella.** Stessa qualità (stesse celle e regioni sui due dispositivi). Su GPU 2,0 s per pagina a tempo di client (0,7 s di elaborazione dichiarata), su CPU 4,0 s (2,2 s); la soglia di 1,0 s su GPU non è raggiunta per la granularità dell'endpoint sincrono. Le celle sono buone dove le intestazioni sono riconosciute (P-007 0,99, P-010 1,00) e crollano per uno slittamento di riga dove non lo sono; le regioni DocLayNet sono trovate su 15 pagine su 18, mancano su pagine con molte tabelle piccole o affiancate. VRAM di picco 1,6 GB sopra il riposo: entra nei 6 GB della RTX 3050 accanto al desktop. La baseline PDFium a celle non è un'alternativa (F1 0,011).
- **Euristica di selezione.** Recall di holdout 0,909 contro 0,800 di taratura (quest'ultima abbassata dalle pagine NSANY non estratte), precisione 0,435 in holdout: perde la tabella con meno cifre e manda a Docling molte pagine di testo numerico e d'indice. Con una sola tabella persa su 11 la soglia di 0,95 non è raggiunta.
- **Confine di Docling.** `document_timeout` ferma il lavoro (il container torna a riposo) ma non libera la memoria; la disconnessione del client **non** ferma il lavoro, che continua con più CPU e più memoria finché il container non si ricrea. `page_range` evita l'elaborazione delle pagine fuori intervallo (costo aggiuntivo di 0,1 s e 11-34 MiB su un PDF da 400 pagine). Ricreare il servizio costa circa 19 s su GPU e 21 s su CPU per documento; dopo ogni riavvio la pagina benigna successiva riesce nei tempi del dispositivo. Il riavvio per documento e dopo ogni fallimento resta necessario: nessuna di queste prove autorizza a toglierlo, e lo decide l'ADR-0004 sui numeri.
- **Dati per D-27.** La condizione di D-27 («`page_range` non riduce il costo») non si osserva su questo PDF: la pagina dentro 400 costa come la pagina da sola, a meno di 0,13 s e 34 MiB. Il costo che pesa è un altro: circa 20 s di riavvio per documento, 2-4 s per pagina-tabella, 12 pagine non tabellari su 13 mandate a Docling nel documento IT. La scelta fra le opzioni di «Deferred Ideas» resta all'utente.
- **Impatto sulle porte.** `DocumentParser` restituisce per i PDF, pagina per pagina, il testo e i rettangoli delle parole in punti PDF con origine in basso a sinistra (più il box della pagina e la rotazione), e le tabelle come blocchi strutturati con pagina, rettangolo e celle `(riga, colonna, testo)`; l'ordine di lettura non può essere quello del flusso di PDFium. Le pagine rifiutate dal prescan sono esiti tipizzati, non pagine vuote.
- **Impatto sul threat model.** Docling come sidecar senza egress sulla rete interna `parse`, raggiunto solo da PDF che hanno superato il prescan nel figlio `extract_pdf` (verificato da `docling-client` e dallo sha256), ricreato per documento e dopo ogni timeout, disconnessione o errore: un suo crash o un lavoro residuo non tocca il worker, e nessuno stato scrivibile sopravvive al riavvio. Il lavoro residuo dopo una disconnessione è dimostrato (T-01-08-03): il riavvio non è un'ottimizzazione, è la mitigazione. Minacce con evidenza: T-01-08-01 → `invocation.txt`, `limits-extract_pdf.json`; T-01-08-02 → `docling.csv` (solo input con `RunReport` ok); T-01-08-03 → `docling_isolation.csv`; T-01-08-04 → `fetch.sh` e `manifest.json`; T-01-08-05 → commit dei contratti, dello split e dei parametri prima delle misure.
- **Destinazione.** ADR-0004 (piano 01-13), riga «OCR/parser» di DEPENDENCIES.md tramite 01-17.
