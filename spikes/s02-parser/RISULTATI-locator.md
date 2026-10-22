# S-02 / A-10 — Locator di citazione per PDF, DOCX e Markdown · RISULTATI

**Modifica:** Claude (gsd-executor 01-09) · 2026-10-22 · domanda, definizioni e soglie scritte prima di ogni misura (Task 1 del piano 01-09)

## Domanda e decisione da chiudere

Spike S-02, parte locator (assunzione A-10; decisioni D-10 e D-24 di `01-CONTEXT.md`; criterio 2 della fase: «definizione di pagina per DOCX e Markdown nel viewer»). Domande:

1. Un locator a due varianti, **PDF = pagina + rettangoli** e **DOCX/Markdown = percorso di sezione + intervallo di caratteri sul testo canonico**, ritrova in modo esatto il testo citato, e lo stesso documento in Markdown e in DOCX dà lo stesso locator?
2. La conversione degli offset verso il viewer JavaScript (code unit UTF-16) è specificata e verificata contro un oracolo indipendente dal codice misurato?
3. La trasformazione dei rettangoli PDF dallo spazio utente alla viewport del viewer regge pagine ruotate e CropBox diverso da MediaBox, e PDFium restituisce la bbox di un tracciato vettoriale nello spazio atteso?

D-10 ha già scelto la forma a due varianti (niente pagine per DOCX e Markdown); lo spike misura se la forma regge e fissa le regole che la rendono riproducibile. La decisione si registra in ADR-0005.

## Definizioni

**Testo canonico `cka-text-v1`.** È la proiezione testuale su cui vivono gli offset. I lettori Markdown e DOCX producono la stessa sequenza di blocchi; le sezioni, il testo di sezione e gli offset li costruisce un solo modulo (`src/bin/locator/canon.rs`). Regole:

- **Ingresso:** CRLF e CR diventano `\n`; NFC sul testo finale di ogni blocco; nessun ripiegamento tipografico (virgolette curve, trattini, spazio non separabile restano come sono: una differenza introdotta da pandoc si misura come anomalia, non si normalizza via).
- **Esclusi:** front matter YAML del Markdown e, simmetricamente, i paragrafi DOCX con stile `Title`, `Subtitle`, `Author`, `Date`; righe `\newpage` del Markdown e interruzioni di pagina DOCX (`w:br w:type="page"`, `w:lastRenderedPageBreak`); immagini; note a piè di pagina (riferimenti e definizioni); `w:delText` e `w:instrText`; HTML grezzo del Markdown, registrato in `unsupported`. Un blocco che resta vuoto dopo le esclusioni (per esempio un paragrafo DOCX con la sola interruzione di pagina) non esiste.
- **Titoli:** ATX `#`..`######` fuori dai blocchi di codice; nel DOCX i paragrafi con stile `Heading1`..`Heading6`. Il titolo è il suo testo inline, senza spazi esterni, e non fa parte del testo della sezione.
- **Testo di sezione:** i blocchi fra il titolo e il titolo successivo di qualunque livello (le sottosezioni sono sezioni a sé), uniti da `\n`. Una sezione con il solo titolo esiste e ha testo vuoto. Blocchi: paragrafo (righe interne unite da uno spazio, sequenze di spazi ASCII compresse in uno, estremi rimossi); voce di elenco (marcatore `-`, `*`, `+`, `N.` rimosso; la numerazione DOCX non è testo); blocco di codice (contenuto verbatim, recinti esclusi; righe DOCX separate da `w:br` diventano `\n`); tabella (righe unite da `\n`, celle da `\t`, contenuto di cella come un paragrafo; riga separatrice `|---|` esclusa, pipe esterne e padding rimossi).
- **Inline:** marcatori di enfasi `*`, `_`, `**`, `__`, `~~` e backtick rimossi; link `[testo](url)` ridotto a `testo`, immagini escluse; escape `\x` → `x`. DOCX: concatenazione dei `w:t` di tutti i run del paragrafo (i run spezzati non lasciano separatori), `w:tab` → `\t`, `w:br` non di pagina → `\n`, `w:ins` incluso; la destinazione degli hyperlink non entra mai nel testo.
- **Costrutti non coperti:** finiscono nell'elenco `unsupported` del payload, nessun errore silenzioso.

**Locator `Text`.** `section_path` è una lista di passi `{title, ordinal}` dalla radice: `ordinal` è la posizione 1-based fra le sorelle con lo stesso titolo canonico sotto lo stesso genitore (titoli ripetuti restano distinti; una sorella con titolo diverso inserita fra due omonime non cambia gli ordinali). `char_range` è `{start, end}` in **Unicode scalar value** (il `char` di Rust), intervallo semiaperto `[start, end)`, sul testo canonico della sezione: mai byte né code unit UTF-16. È un'unità indipendente dal linguaggio e coincide con `char_length` di PostgreSQL su UTF-8. Il locator porta sempre `projection: "cka-text-v1"`, `doc_sha256` (sha256 dei byte del sorgente) e `section_text_sha256` (sha256 del testo canonico della sezione): `resolve` rifiuta con `ProjectionMismatch`, `VersionMismatch` o `SectionHashMismatch` un locator che non combacia, invece di restituire testo.

**Conversione per il viewer.** `utf16_start` è la somma delle lunghezze UTF-16 (1 per uno scalar del BMP, 2 per U+10000..U+10FFFF) dei primi `start` scalar; lo stesso per `end`. In JavaScript `Array.from(text)` indicizza per scalar e `String.prototype.slice` vuole gli offset convertiti.

**Locator `Pdf`.** Pagina 1-based e rettangoli `[x0, y0, x1, y1]` in punti. Il sistema li salva nello spazio utente PDF (origine in basso a sinistra, prima di `/Rotate`) e li trasforma per il viewer con una funzione pura (`src/bin/locator/pdfrect.rs`) verso la viewport non scalata con origine in alto a sinistra, come `convertToViewportRectangle` di PDF.js a scala 1: traslazione nell'origine della CropBox, ribaltamento dell'asse y, rotazione oraria della pagina, estremi normalizzati.

## Soglie (scritte PRIMA della misura)

Le soglie sotto si fissano in questo commit, prima di qualunque file in `results/locator/`, e non si ritoccano sui risultati: una correzione va in «Anomalie e deviazioni», con motivo.

| Id | Metrica | Soglia | Fonte |
|----|---------|--------|-------|
| L-1 | Round-trip esatto: testo riestratto dal locator identico all'excerpt atteso (entrambi in `cka-text-v1`), una sola occorrenza per campione | 100% di almeno 10 campioni DOCX/MD | D-10, RESEARCH §8.1 |
| L-2 | Per i campioni con DOCX: stesso `section_path` (titoli e ordinali) fra versione MD e DOCX | 100% | D-10, D-24 |
| L-3 | Per i campioni con DOCX: stesso `char_range` fra MD e DOCX | 100% | D-10 |
| L-4 | Per i campioni con DOCX: stesso `section_text_sha256` fra MD e DOCX | 100% | D-10, D-20 |
| L-5 | `section_path` calcolato uguale a quello atteso scritto a mano | 100% dei campioni | D-24 |
| L-6 | Conversione scalar → UTF-16 del bin (`got_utf16_range`, `got_text`) uguale alla fixture `locator-gen/offset_vectors.expected.json`; la fixture stessa confermata dalla semantica JavaScript (`Array.from`, `String.prototype.slice`) | 100% dei vettori | D-24 |
| L-7 | Trasformazione pura dei rettangoli (stadio `transform`) uguale a `expected_viewport_rect` sugli 8 casi numerici (`/Rotate` 0/90/180/270 × CropBox uguale o spostata) | 100%, tolleranza 0,5 pt | RICERCA M7, VIEW-01 |
| L-8 | Bbox del tracciato vettoriale estratta da PDFium uguale al rettangolo disegnato e, trasformata, a `expected_viewport_rect` (stadio `extract_path`) | 100% degli 8 casi, tolleranza 0,5 pt | RICERCA M7 |
| L-9 | Bbox dei glifi della stringa marcatore (stadio `extract_text`) | informativa, nessuna soglia | RICERCA M7 |
| L-10 | Parsing DOCX/MD solo via job `op: locate` del contratto di 01-07, esito `ok` su tutti i documenti dei campioni; il job negativo `neg-path-<run_tag>` dà `job_error`/`PathNotAllowed` | 100% | D-05, D-08 |

**Provenienza dei valori attesi.** Gli excerpt e i `section_path` attesi di `locator-gen/expected_samples.json` sono di classe `estratto-sorgente`: copiati verbatim dal Markdown sintetico applicando a mano le regole di `cka-text-v1`, con il percorso dai titoli ATX che li precedono. I rettangoli attesi (`locator-gen/rect_cases.expected.json`) e i vettori di offset (`locator-gen/offset_vectors.expected.json`) sono di classe `derivazione-documentata`: svolgimento a mano nel campo `derivation`, scritti dall'agente `test-write` e committati prima del codice e dei risultati. Nessun valore atteso è prodotto dal bin sotto test né ricalcolato da `gen_pages.py`. Sono fixture meccaniche su documenti **sintetici**, non oracoli di valutazione del prodotto né voci dell'evaluation corpus: per questo non passano dal double-check GPT del piano 01-06 (decisione del coordinatore, round 1 dei gate).

**Bounds di PDFium, nota prima della misura ([NV] di RESEARCH §11.5).** Il piano 01-08 tratta i bounds di PDFium come spazio utente della pagina, prima di `/Rotate` (`src/bin/extract/pdf.rs` li ruota con la propria `Frame`), e i suoi test sulla rotazione lo confermano per i char box. Ipotesi di questa misura: anche i bounds di un oggetto tracciato sono nello spazio utente non ruotato; lo stadio `extract_path` confronta prima i bounds grezzi con `user_rect`, poi li trasforma una sola volta. Se risultassero già ruotati, lo stadio lo mostra e la trasformazione non si applica due volte.
