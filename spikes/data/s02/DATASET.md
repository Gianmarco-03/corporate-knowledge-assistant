# Dataset di misura di S-02 (estrazione da PDF digitale)

**Modifica:** Claude (gsd-executor 01-08) · 2026-10-22 · scelta delle pagine, split taratura/holdout, licenze e attribuzioni (D-11, D-29)

Questa cartella registra i sottoinsiemi di tre dataset pubblici che il piano 01-08 usa per misurare l'estrazione di PDFium e di Docling. I file non si ridistribuiscono: `fetch.sh` li scarica in `raw/` (ignorata da git) e ne verifica lo sha256 contro `manifest.json`. Le annotazioni sono ground truth pubblicato dagli autori dei dataset, indipendente dalla pipeline del progetto; gli unici oracoli derivati dal progetto sono quelli ratificati nel piano 01-06 (`spikes/data/corpus/oracles/parser-it.json`), che stanno nel corpus comune.

## Licenze e attribuzioni

| Dataset | Licenza | Attribuzione richiesta | Sorgente usata |
|---------|---------|------------------------|----------------|
| TAT-DQA | CC BY 4.0 | Zhu F., Lei W., Feng F., Wang C., Zhang H., Chua T.-S., «Towards Complex Document Understanding By Discrete Reasoning», ACM MM 2022; NExT++, National University of Singapore | `tatdqa_docs_dev.zip` del repository Hugging Face `next-tat/TAT-DQA`, revisione `bc46208b22f0bfe47152c98fd0938d238ce868c2` |
| DocLayNet | CDLA-Permissive-1.0 | Pfitzmann B., Auer C., Dolfi M., Nassar A. S., Staar P., «DocLayNet: A Large Human-Annotated Dataset for Document-Layout Segmentation», KDD 2022; IBM Research | `DocLayNet_core.zip` (annotazioni COCO, `COCO/test.json`) e `DocLayNet_extra.zip` (PDF di pagina, `PDF/<hash>.pdf`) dell'object storage IBM, versione 1.0.0 |
| FinanceBench | CC BY-NC 4.0 | Islam P., Kannappan A., Kiela D., Qian R., Scherrer N., Vidgen B., «FinanceBench: A New Benchmark for Financial Question Answering», 2023; Patronus AI | cartella `pdfs/` del repository GitHub `patronus-ai/financebench`, commit `cc39aeb4afdf33909ee1412188bf89035950c2eb` |

FinanceBench è CC BY-NC: ammesso perché il progetto è un portfolio non commerciale (D-11). Le modifiche fatte ai dati sono solo di selezione (pagine e documenti estratti dagli archivi), nessuna alterazione dei file.

## Come si scaricano

Gli archivi di TAT-DQA (155 MB per il solo dev) e di DocLayNet (30 GB e 8 GB) superano il tetto di 100 MiB per file del piano. `fetch.sh` legge con richieste HTTP Range la directory centrale di ogni archivio zip e poi i soli byte dei membri elencati: ogni richiesta resta sotto il tetto, e lo sha256 si verifica sul membro estratto. FinanceBench si scarica per file intero. Il 2026-10-22 il DNS di sistema non risolveva l'host dell'object storage di DocLayNet; `fetch.sh` accetta per questo un risolutore DNS-over-HTTPS facoltativo (`CKA_FETCH_DOH_URL`, solo https).

## Scelta delle pagine

**TAT-DQA (20 pagine, bbox per parola).** Dallo split `dev`, i primi 20 documenti di una sola pagina in ordine di identificativo (`uid` esadecimale crescente). Tutti hanno font incorporati, cioè testo digitale. Il JSON del documento porta per ogni pagina il riquadro dell'immagine annotata (`pages[].bbox`, pixel a 150 dpi) e, per blocco, parole e rettangoli in pixel con origine in alto a sinistra.

**DocLayNet (40 pagine, regioni di layout).** Dallo split `test`, categoria `financial_reports` (32 documenti, 1.755 pagine). Ogni pagina è classificata dalle sole annotazioni COCO con `precedence` 0: `T` se ha almeno una regione `Table`; altrimenti `C` se ha due regioni `Text` affiancate (intervalli orizzontali disgiunti, verticali sovrapposti), cioè testo a colonne; altrimenti `O`. I documenti si ordinano per sha256 del nome e si assegnano a turno a taratura e holdout, saltando quelli con meno di due pagine `T` o meno di due pagine non `T`, fino a 5 documenti per split. Per documento si prendono 4 pagine: 2 `T` e 2 non `T` (prima le `C`), ciascun gruppo in ordine di sha256 di `<documento>:<pagina>`. Per l'holdout, fra le prime 6 pagine `T` in quell'ordine, la prima con quota di cifre fra i caratteri delle celle PDF dentro le regioni tabella sotto 0,25 (dal JSON di `DocLayNet_extra.zip`) prende il primo posto: così l'holdout contiene di proposito tabelle con poche cifre.

Risultato (`splits.json`, 20 + 20 pagine):

- **Taratura:** `0000006951-20-000048_20201211x10k`, `NASDAQ_SHEN_2003`, `NYSE_SMFG_2011`, `NASDAQ_ATRI_2003`, `OTC_NSANY_2004`; 10 pagine con tabella, 10 senza (5 di testo a colonne).
- **Holdout:** `NYSE_HRL_2004`, `TSX_KMP_2013`, `ASX_STO_2004`, `NYSE_CHK_2010`, `ASX_MRM_2000`; 10 pagine con tabella (5 con quota di cifre sotto 0,25: HRL p. 28 e p. 19, KMP p. 86, STO p. 83, MRM p. 50) e 10 di testo a colonne non tabellare. Le tabelle dell'holdout sono senza bordi: verificato a vista sulle immagini di pagina di HRL p. 28, STO p. 83 e CHK p. 46 (solo filetti orizzontali o una riga verticale, nessuna griglia).

Il documento italiano da 25 pagine dell'oracolo (`der-it-01-relazione-annuale-2025-p1-25`, corpus comune, oracolo P-011) è holdout per costruzione.

**La scelta delle pagine e degli split non usa `select-tables` né i suoi punteggi:** lo split è fissato in questo commit, prima che l'euristica esista come misura, su criteri di documento (ordine per hash del nome) e di tipologia di pagina (annotazioni degli autori). Nessun documento compare in entrambi gli split (`extract select-tables` lo verifica con `validate_splits` prima di ogni calcolo).

**FinanceBench (3 PDF, misure di lunghezza).** Un comunicato trimestrale (`MCDONALDS_2023Q1_EARNINGS`), un 10-Q (`APPLE_2023Q3_10Q`, 29 pagine) e un 10-K (`AMAZON_2020_10K`, 80 pagine), scelti fra i più leggeri di ciascun tipo. Non hanno annotazioni di pagina usate qui: servono al tempo e alla memoria dell'estrazione su documenti lunghi.

## Copertura limitata

Il campione è piccolo e va letto come tale: 20 pagine TAT-DQA di 20 documenti in inglese, 40 pagine DocLayNet di 10 documenti finanziari in inglese (annual report 2000-2021), 3 documenti FinanceBench, più le pagine italiane dell'oracolo. La recall di holdout dell'euristica si misura su 11 pagine-tabella (10 DocLayNet e 1 del documento italiano): una sola pagina persa la porta sotto la soglia di 0,95. Le tabelle DocLayNet hanno regioni ma non celle: l'F1 di cella si misura solo sulle 4 tavole italiane.
