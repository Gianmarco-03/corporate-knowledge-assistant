# S-02 — Parser, OCR, isolamento e locator · RISULTATI (indice ed Esito complessivo)

**Modifica:** Claude (gsd-executor 01-13) · 2026-10-24 · indice di S-02 ed Esito complessivo (Task 3 del piano 01-13)
**Modifica:** Claude (gsd-executor 01-13 paddle) · 2026-10-25 · PP-OCRv6 identificato ma non valutabile a 2 GiB

## Indice

| Parte | RISULTATI | Evidenze | Hardware |
|---|---|---|---|
| Isolamento del parser, casi ostili, budget (01-07, 01-18, gap-closure 1.4 e 1.5) | [RISULTATI-isolamento.md](RISULTATI-isolamento.md) | [`results/isolation/`](results/isolation/) | [`results/isolation/hw.json`](results/isolation/hw.json) |
| Estrazione PDFium e Docling, pagine-tabella (01-08) | [RISULTATI-estrazione.md](RISULTATI-estrazione.md) | [`results/extract/`](results/extract/) | [`results/extract/hw.json`](results/extract/hw.json) |
| Locator di citazione (01-09) | [RISULTATI-locator.md](RISULTATI-locator.md) | [`results/locator/`](results/locator/) | [`results/locator/hw.json`](results/locator/hw.json) |
| OCR, TTQ-parse, prova ostile sul render (01-13) | [RISULTATI-ocr.md](RISULTATI-ocr.md) | [`results/ocr/`](results/ocr/) | [`results/ocr/hw.json`](results/ocr/hw.json) |

## Esito

Decisione proposta, registrata in [ADR-0004](../../docs/adr/0004-parser-ocr-isolamento.md) (Stato `Proposed`):

- **Parser.** PDFium (`chromium/8100`, binario non-V8 con hash fissato) per testo, pagine e rettangoli di parola dei PDF digitali; Docling (`docling-serve` 1.37.0) solo sulle pagine-tabella scelte dall'euristica di 01-08, su GPU quando c'è (2,0 s per pagina a tempo di client contro 4,0 s su CPU, stessa qualità), come sidecar senza egress ricreato per documento e dopo ogni timeout o disconnessione. PDFium non è decisivo per l'ordine di lettura (CER dominato dall'ordine dei blocchi) né per la geometria fine di parola (E-3 77,0% contro 95%, box del font di `loose_bounds()`): l'ordine va ricostruito sopra PDFium o preso da Docling.
- **OCR.** Tesseract 5.5.0 da CLI con `tessdata_best` ita+eng, solo come job `ocr_render` + `ocr_tesseract` del lanciatore: CER aggregato 3,16% sulle buone (soglia 3%: **non raggiunta**, per l'ordine dei blocchi di due pagine, non per i caratteri) e 2,74% sulle degradate; bbox fedeli al controllo analitico (8 pass su 8, p95 ≤ 0,84 pt sulle buone e ≤ 2,66 pt sulle degradate); circa 2,4-3,0 s per pagina a 300 DPI per job. **Decisione provvisoria:** Tesseract resta l'OCR di riferimento, ma la regola «vince nettamente» contro lo sfidante PP-OCRv6 non è ancora applicabile, perché lo sfidante non è stato misurato (pacchetti pip in attesa dell'approvazione dell'utente, checkpoint del Task 1). La riga «OCR» dell'ADR resta aperta finché PP-OCRv6 non è misurato o l'utente non lo esclude. *Aggiornamento del 2026-10-25:* pacchetti approvati e immagine costruita con le librerie di sistema a versione fissata; PP-OCRv6 è identificato (pesi e modelli `PP-OCRv6_medium_det`/`_rec` ancorati) ma **non valutabile** sul servizio da 2 GiB: la detection fallisce per un difetto oneDNN/PIR di paddlepaddle 3.3.1 e, senza oneDNN, per OOM (RISULTATI-ocr.md, «Anomalie»).
- **Isolamento.** Processo separato per ogni documento e per ogni pagina OCR (01-07: allowlist chiusa, argv senza shell, `RLIMIT_*`, gruppo di processi, subreaper, `oom_score_adj` 1000, budget aggregato sul cgroup del worker), contenimento verificato su 26 casi ostili su 26 di 01-18, sui 5 casi del render (15 righe su 15 con padre vivo e `leftover_procs` 0) e sul percorso OOM (M11). Limiti calibrati con la formula di 01-07 v6: `parse` prenotazione 848.777 KiB (gap-closure 1.4), `ocr_render` 1.111.596 KiB più quota, `ocr_tesseract` 823.248 KiB per job (con il discendente). Concorrenza ammessa nel worker da 2 GiB: **2 job `parse`** (M10 di 01-18 con la prenotazione di allora, 803.057 KiB, e batch reale della 1.4 con 2 job `ok` con i limiti nuovi, D-40), **2 job `ocr_tesseract`** (12 batch su 12 conformi col metodo di M10), 1 job `ocr_render`. Il dimensionamento per altri tagli di memoria è **estrapolato** (D-28). Windows Job Objects fuori scope: il parser gira solo in container Linux.
- **Tetto di decompressione.** Prescan nel figlio prima di PDFium: 64 MiB per stream, 512 MiB di somma reale, rapporto 100 oltre 16 MiB, catena di filtri ≤ 2, `Width × Height × componenti` ≤ 64 MiB sulle dimensioni dichiarate **e** su quelle codificate di DCT (SOF) e JPX (SIZ), gap-closure 1.5, provato anche su `ocr_render` (9 righe su 9). Debiti dichiarati: immagini inline (`BI … ID … EI`) non esaminate dal prescan (fase 3); il tetto conta un byte per campione (RISK-14).
- **Limite di pagine.** 2.000 per documento (`pages-10k` → `ResourceLimit`).
- **Locator.** Due varianti (ADR-0005): PDF = pagina + rettangoli nello spazio utente; DOCX e Markdown = percorso di sezione + intervallo di caratteri sul testo canonico `cka-text-v1`.
- **TTQ-parse** (percorso composto reale, RISULTATI-ocr.md «TTQ-parse»): p95 20,2 s per il PDF digitale da 25 pagine (n utile 20, soglia 5 s) e 73,1 s per quello da 100 pagine (n utile 10, soglia 20 s): non raggiunto. Docling sulle pagine-tabella selezionate (13 su 25, circa 15,3 s) costa da solo tre volte la quota; prescan + PDFium costano 1,1 s, l'OCR delle 2 pagine bianche 2,5 s.

### Alternative scartate, con i numeri

- **MuPDF** (AGPL) e **Poppler** (GPL): escluse per licenza (RICERCA Area B §1).
- **Parsing in processo** (PDFium nel processo del servizio): overhead del processo separato 2,4% (M9, sotto la soglia del 10%) contro un modo in-process che fallisce in modo intermittente a concorrenza 8 (`FormatError` 11 su 20) e che non contiene crash né OOM.
- **Docling su tutte le pagine:** 2-4 s per pagina; sul documento da 25 pagine l'euristica ne manda già 13 a Docling.
- **Baseline PDFium a celle** al posto di Docling per le tabelle: F1 0,011.
- **Pagine DOCX via LibreOffice:** circa 4 s per documento e CVE 2026 nel motore di import (ADR-0005).
- **PP-OCRv6:** non scartato né adottato; non misurato in attesa dell'approvazione dei pacchetti. *Aggiornamento del 2026-10-25:* non valutabile a 2 GiB su CPU (oneDNN/PIR, OOM senza oneDNN); configurazione per una nuova misura da decidere con l'utente.

## Impatto su porte (S-07)

- `DocumentParser` restituisce per i PDF, pagina per pagina, testo e rettangoli di parola in punti PDF con origine in basso a sinistra (più box della pagina e rotazione), le tabelle come blocchi con pagina, rettangolo e celle `(riga, colonna, testo)`; per le pagine senza testo le parole OCR con le stesse coordinate (trasformazione px → pt registrata); per DOCX e Markdown i blocchi di `cka-text-v1`.
- Errori tipizzati: `Encrypted`, `DecompressionLimit`, `Timeout`, `ResourceLimit`, `OutputLimit`, `Crashed{signal}`, `Corrupt`, `Unsupported`; stato `Quarantined` con motivi fissi (`javascript`, `launch`, …). Nessun esito ostile arriva come `ok` con contenuto attivo; un `ok` del render su un JBIG2 rotto è possibile (T-09 aperto).
- Locator a due varianti (ADR-0005), multi-span con al più due span.
- Parsing e OCR fuori processo, con prenotazione nel budget del worker: la porta è asincrona e la concorrenza la decide il budget, non il chiamante.

## Impatto threat model

- **T-06** (tipo falso): `magic-mismatch` e `docm-macro` → `Unsupported` (`results/isolation/hostile.csv`).
- **T-07** (zip bomb e XML bomb): `docx-zip-bomb` → `DecompressionLimit`, `docx-xml-bomb` → `Corrupt` (`hostile.csv`); lettura DOCX solo nel figlio, senza DTD (RISULTATI-locator.md).
- **T-09** (exploit del parser, loop, OOM): bomb PDF negli stream, object stream, catene di filtri, allegati → `DecompressionLimit`; `pages-10k` → `ResourceLimit`; immagini DCT e JPX con dimensioni codificate enormi → `DecompressionLimit` su `parse`, `extract_pdf` e `ocr_render` (`results/isolation/`, `results/ocr/hostile-render.csv`); OOM del cgroup sul figlio (M11, `oom.json`). **Residuo JBIG2 (D-34): aperto** — `jbig2-malformed` dà `ok` sul render in 3 righe su 3 (`results/ocr/hostile-render.csv`), contenimento intatto; `deep-nesting` dà `ok` sul render in 3 righe su 3 (la render non visita `/Extra`). M8 di 01-18 resta com'è (24 casi su 26), questo piano non la ricalcola. Debiti: immagini inline non esaminate dal prescan (fase 3), un byte per campione nel tetto (RISK-14).
- **T-10** (PDF attivo): `/JavaScript` anche in object stream compresso e `/Launch` → quarantena; allegati contati e mai estratti (`hostile.csv`); il viewer riceve coordinate già trasformate (ADR-0005).

## Destinazione

- [ADR-0004](../../docs/adr/0004-parser-ocr-isolamento.md), Stato `Proposed`.
- Riga «OCR/parser» di `DEPENDENCIES.md`: da Aperta a Decisa per PDFium, Docling e l'isolamento; la parte OCR resta Proposta (Tesseract) finché lo sfidante non è misurato. La scrive il consolidamento (piano 01-17).
