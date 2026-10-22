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

*(compilato nel Task 2)*

## Risultati

*(compilato nel Task 2)*

## Anomalie e deviazioni

*(compilato nel Task 2)*

## Esito parziale

*(compilato nel Task 2)*
