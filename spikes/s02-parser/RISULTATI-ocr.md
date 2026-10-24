# S-02 — OCR delle scansioni, TTQ-parse e prova ostile sul render · RISULTATI

**Modifica:** Claude (gsd-executor 01-13) · 2026-10-24 · domanda e soglie scritte prima di ogni misura (Task 2 del piano 01-13)

## Domanda e decisione da chiudere

Spike S-02, parte OCR e sintesi (decisioni D-07, D-09 emendata, D-28, D-34 di `01-CONTEXT.md`; criterio 2 della fase: PDF scannerizzati). Domande:

1. Tesseract da CLI (5.5.0, `tessdata_best` ita+eng, TSV con bbox per parola), lanciato solo come job `ocr_render` + `ocr_tesseract` del lanciatore di 01-07, regge in CER, secondi per pagina e fedeltà delle bbox su scansioni buone e su una variante degradata dichiarata? Lo sfidante PP-OCRv6 (PaddleOCR 3.7.0) vince nettamente secondo la regola scritta qui sotto?
2. Quanti job `ocr_tesseract` ammette il budget del worker da 2 GiB, e i batch restano conformi col metodo di M10 di 01-18?
3. Il TTQ-parse del percorso composto reale (prescan + PDFium via `extract_pdf`, Docling sulle pagine-tabella, `ocr_render` + `ocr_tesseract` sulle pagine senza testo) sta nella quota di 5 s del target di 15 s?
4. Il decoder JBIG2 di PDFium, che l'op `parse` non esercita (M8 di 01-18), contiene `jbig2-malformed` sul percorso `ocr_render`? E il render rifiuta col prescan le immagini DCT e JPX della gap-closure 1.5?

Le risposte chiudono, con i tre RISULTATI già scritti, la decisione su parser, OCR, isolamento e tetto di decompressione in ADR-0004.

## Soglie (scritte PRIMA della misura)

Fonti: RICERCA-DISCUSSIONE M1 e M6, `01-13-PLAN.md` (Task 2), Claude's Discretion. Immutabili dopo questo commit: una correzione va in «Anomalie e deviazioni».

### OCR Tesseract

- **CER** ≤ 3% sulle scansioni buone (300 DPI) delle pagine IT dell'oracolo; sulle degradate il CER si riporta senza soglia.
- **Variante degradata dichiarata** (parametri immutabili): render a 200 DPI, rumore gaussiano `-attenuate 0.5 +noise Gaussian`, rotazione di 1,5° attorno al centro a tela invariata (`-distort SRT 1.5`), JPEG qualità 60, riletto come PNG a 200 DPI.
- **Oracoli:** CER solo sulle voci di `parser-it.json` con i tre assi verdi (`claude` valorizzato, `gpt` = `CONFIRMED`, `user` = `ratified`), normalizzazione di 01-08 (NFC, spazi bianchi collassati, bordi rimossi). Riga «oracoli parser-it pendenti esclusi e contati» con il numero in «Anomalie».

### Fedeltà delle bbox

Due riferimenti con classe di provenienza dichiarata (vocabolario di `skills/eval-oracle-check`; fixture meccaniche dello spike, non voci dell'evaluation corpus).

- **Controllo analitico (`reference=analytic`)**, giudicato sempre. Pagine di `scan-gen/gen_geom.py` (generatore di questo piano, sola standard library) con parole maiuscole e cifre senza discendenti in Helvetica standard non incorporata, a corpi 10, 14 e 24 pt e posizioni note (inclusi i quattro angoli della pagina). Rettangolo d'inchiostro atteso in punti PDF calcolato dalle metriche AFM di Helvetica (Adobe Core14): `x0 = x + s·llx(primo)/1000`, `x1 = x + s·(Σ avanzamenti fino al penultimo + urx(ultimo))/1000`, `y0 = y_base + s·min lly/1000`, `y1 = y_base + s·max ury/1000`. Classe `derivazione-documentata`, `derivation_ref` = `spikes/s02-parser/scan-gen/gen_geom.py` + questo file §Setup. Due stadi:
  1. `map` puro, senza OCR: rettangoli attesi portati in pixel con la trasformazione diretta registrata e riportati in punti con l'inversa → errore massimo agli angoli ≤ 0,5 px equivalenti su ogni caso;
  2. OCR Tesseract sulle stesse pagine renderizzate: errore p95 agli angoli ≤ 1 pt sulle buone e ≤ 3 pt sulle degradate; IoU mediana riportata.

  Limite dichiarato: PDFium disegna Helvetica non incorporata con un font sostitutivo i cui avanzamenti seguono l'AFM ma il cui inchiostro può scostarsi di frazioni di punto; per questo la tolleranza dello stadio 2 è 1 pt e non 0,5.
- **Riferimento PDFium (`reference=pdfium`)**, condizionato a 01-08. Parole OCR ricondotte in punti con la trasformazione inversa, contro i rettangoli di parola di `extract_pdf` sul PDF digitale. Classe `derivazione-documentata`, riferimento «PDFium validato in 01-08 contro TAT-DQA», `derivation_ref` = `spikes/s02-parser/RISULTATI-estrazione.md` + `spikes/s02-parser/results/extract/bbox_iou.csv`. Soglia: IoU mediana ≥ 0,5 e errore p95 sugli angoli ≤ 3 pt, **giudicata solo se 01-08 ha superato la propria soglia IoU** (quota di parole con IoU ≥ 0,5 ≥ 95% su TAT-DQA); altrimenti la riga è «non valutabile» (`verdict=non_valutabile`): i numeri si riportano, mai come PASS. Differenza sistematica dichiarata prima della misura: `extract_pdf` usa `loose_bounds()` (box del font, da discendente ad ascendente, su tutto l'avanzamento), Tesseract il box d'inchiostro; l'IoU OCR-vs-PDFium è quindi strutturalmente < 1 anche con una mappatura perfetta e lo scarto verticale è un bias, non un errore della mappatura. Come plafond si riporta accanto l'IoU di PDFium contro TAT-DQA (`iou_mean`, `share_iou_ge_05` di `bbox_iou.csv`).
- **Metrica:** coppie OCR–attese abbinate per testo normalizzato uguale, uno a uno, per IoU decrescente; errore agli angoli = per parola la massima distanza fra i quattro angoli corrispondenti, p95 nearest-rank sulle parole abbinate della riga.

### Throughput e conformità dei batch

- **25 pagine scansionate ≤ 30 s** con concorrenza richiesta 8 (`OMP_THREAD_LIMIT=1` dal padre), giudicate con il `max_admitted` reale riportato accanto. Il worker resta a 2 GiB (D-28): se il budget ammette meno di 8 processi la soglia si giudica sul tempo misurato, il dato si riporta com'è e il dimensionamento in ADR-0004 si dichiara **estrapolato**; nessun aumento della memoria del worker.
- **Conformità di un batch** (metodo di M10 di 01-18, `RISULTATI-isolamento.md` §Soglie, riusato per D-28): padre vivo ed exit di `parse-parent batch` 0; riga `batch_summary` presente; `memory.max` del cgroup limitato e pari a 2 GiB; `oom_kill_delta` del cgroup = 0; nessun `job_error`; `leftover_procs` = 0 su ogni job; rapporto prenotazione/`peak_rss_kb` ≥ 1 su ogni job ammesso, con la prenotazione di 01-07 v6 calcolata sul file `limits-ocr_tesseract.json` corrente (2 × `memory_reservation_bytes` + `out_quota_bytes`, un discendente). Colonna `m10_pass`. Concorrenza OCR pubblicata = il più grande N i cui batch sono tutti conformi, limitato dal `max_admitted` misurato; vale per `ocr_tesseract`, per i carichi misurati e per il worker da 2 GiB, e va ripetuta se cambiano workload, memoria o limiti. La concorrenza di `ocr_render` non si pubblica.
- **Picco del discendente:** il figlio `ocr tesseract` riporta come `vm_hwm_kb` e `vm_peak_kb` il massimo fra il proprio valore e quello del processo `tesseract` (campionato su `/proc/<pid>/status` durante l'esecuzione), così la calibrazione e il rapporto prenotazione/picco vedono la memoria del discendente e non solo quella del figlio.

### Prova ostile sul render (D-34 e obbligo del contratto della 1.5)

Attesi copiati letterali dai manifest (nessun atteso nuovo), classe `derivazione-documentata`.

- Da `spikes/data/hostile/manifest.json` di 01-18 (`derivation_ref` = manifest di 01-18 + `01-CONTEXT.md` D-34): `jbig2-malformed` → `failed` con `kind` ∈ `["Corrupt","Crashed"]`; `deep-nesting` → `failed` con `kind` ∈ `["Corrupt","Crashed"]`.
- Da `spikes/data/hostile/sof/manifest.json` della 1.5 (`derivation_ref` = manifest della 1.5 + `01.5-CONTRATTO.md` «Obbligo per 01-13»): `dct-sof-huge-image` → `failed` `DecompressionLimit` con detail che inizia per `dct_sof over_cap`; `jpx-siz-huge-image` e `jpx-siz-huge-jp2` → `failed` `DecompressionLimit` con detail che inizia per `jpx_siz over_cap`.
- Una riga passa se valgono tutte: esito secondo la regola di `hostile_verdict` di 01-07 (status uguale, `kind` nell'array, `oom_adj_set` = `true`; per i casi della 1.5 anche il prefisso del detail), padre vivo (riga `RunReport` su stdout di `parse-parent run`, processo uscito da sé e non per segnale, worker ancora in esecuzione a `session.sh ps --assert-only worker`) e `leftover_procs` = 0. Tre ripetizioni per caso, nessuna scartata; a fine batch `/work` del worker vuoto.
- Esito per il residuo T-09 JBIG2: «chiuso» solo se 3 righe su 3 di `jbig2-malformed` passano; con un `ok` il decoder è stato esercitato senza un errore visibile al figlio e il residuo resta «aperto» con il numero; con un contenimento violato (padre morto o `leftover_procs` > 0) l'esito è «contenimento fallito», anomalia bloccante per ADR-0004. Il contenimento si giudica sempre, anche con esito `ok`. Nessuna euristica sull'immagine renderizzata trasforma un `ok` in `failed`: l'esito è quello che il figlio emette. Regola del figlio scritta qui: un esito del prescan si emette com'è; un errore restituito da PDFium al caricamento o alla render di una pagina → `failed/Corrupt` con detail `pdfium_load` o `pdfium_render` e il nome della variante d'errore; un crash lo classifica il padre (`Crashed{signal}`).
- `deep-nesting`: l'annidamento sta in un oggetto citato solo da `/Extra` del Catalog e la render potrebbe non visitarlo; un `ok` si riporta com'è, senza conclusioni oltre la misura.

### Sfidante PP-OCRv6

- **«Vince nettamente»** (D-09 emendata): CER relativo migliore di almeno il 30% e di almeno 1 punto assoluto su buone e degradate, con tempo per pagina ≤ 1,5 × Tesseract sullo stesso hardware (CPU, stessi 8 core e 2 GiB). Uno sfidante i cui pesi non sono identificati (file + SHA-256 e modello risolto) è «non identificato» ed escluso dalla regola. Solo v6 può far sostituire Tesseract; PP-OCRv5, se misurato, si riporta accanto senza effetto sulla decisione.
- Con i pacchetti pip non approvati dall'utente, o con `v6.selection.json` `selectable=false`, lo sfidante non si misura e l'anomalia si riporta.

### TTQ-parse

- p95 del documento digitale da 25 pagine ≤ 5 s e del documento da 100 pagine ≤ 20 s (M1), tempo di parete esterno della sequenza reale nella stessa run (prescan + PDFium via `extract_pdf`, Docling sulle pagine-tabella, `ocr_render` + `ocr_tesseract` sulle pagine senza testo), container già avviati; p95 nearest-rank sui totali per run (mai somma di percentili), n utile dichiarato; il PDF misto (pagine immagine) misurato e riportato senza soglia propria. Scarto > 10% fra `wall_ms` e somma delle componenti → «Anomalie».
