# ADR-0005 — Locator di citazione: PDF a pagina e rettangoli, DOCX e Markdown a sezione e intervallo di caratteri

- Stato: Proposed
- Data: 2026-10-22
- Decisori: utente (D-10, D-24 e il paragrafo *Storia* di D-10 in `01-CONTEXT.md`; ratifica nel consolidamento, piano 01-17); proposta di Claude (gsd-executor 01-09) dallo spike S-02 / A-10
- Sostituisce: nessuno

**Modifica:** Claude (gsd-executor 01-09) · 2026-10-22 · ADR proposto con le misure dello spike A-10 (piano 01-09)

## Contesto

Ogni affermazione di una risposta cita uno o più chunk, e il viewer in-app deve portare l'utente al punto esatto del documento. VIEW-01 (U-03) chiede la pagina per i PDF e l'evidenziazione quando ci sono coordinate. Per DOCX e Markdown il criterio 2 della fase 1 lasciava aperta la definizione di «pagina».

Vincoli e forze:

- **Un DOCX non salva le pagine.** La paginazione la calcola chi lo impagina (Word, LibreOffice), e cambia con font, stampante e versione. Ottenerla vuol dire convertire il DOCX in PDF con LibreOffice: circa 4 s per documento e CVE del 2026 nel motore di import (RICERCA Area B §3). Un Markdown non ha pagine affatto.
- **Il citation mapping è deterministico** (D-20, ADR-0002): l'LLM cita solo `chunk_id`; pagina, rettangoli o sezione e offset li aggiunge il sistema dal database. Il locator deve quindi essere calcolabile e ricontrollabile senza modelli.
- **Minacce che hanno guidato la scelta** (01-RICERCA-DISCUSSIONE §3):
  - **T-07**, zip bomb e XML bomb nella lettura del DOCX: la lettura avviene solo nel figlio isolato, dopo il prescan dei limiti d'archivio e senza DTD né entità;
  - **T-10**, PDF attivo servito al viewer: il viewer mostra DOCX e Markdown come testo, mai come documento renderizzato, e per i PDF riceve solo coordinate già trasformate;
  - **T-17**, citation spoofing: il locator è deterministico, legato alla versione del documento e della proiezione, e si ricontrolla prima di restituire testo.

## Opzioni considerate

1. **Locator a due varianti** (D-10): PDF = pagina + rettangoli; DOCX e Markdown = percorso di sezione + intervallo di caratteri su un testo canonico versionato.
   - Vantaggi: nessuna impaginazione da inventare; il locator è calcolabile dal testo; lo stesso documento in Markdown e in DOCX dà lo stesso locator (misurato, vedi Evidenze); il viewer DOCX/MD è un componente di testo semplice.
   - Svantaggi: il viewer DOCX/MD non mostra l'aspetto originale del documento; la proiezione testuale va fissata e versionata, e ogni sua modifica invalida i locator salvati.
   - Rischi: una differenza introdotta da chi produce il DOCX (pandoc trasforma gli apostrofi in U+2019) cambia l'hash del testo di sezione fra i due formati.
2. **Pagine DOCX via LibreOffice** (conversione DOCX → PDF e poi locator PDF).
   - Vantaggi: un solo tipo di locator; il viewer mostra pagine.
   - Svantaggi: circa 4 s per documento sul TTQ, un motore di import complesso con CVE del 2026 nel percorso dei documenti non fidati, pagine che dipendono dai font installati. Scartata senza misura nuova: D-10 è già una decisione dell'utente sui numeri di RICERCA Area B §3.
3. **Numerazione artificiale di pagina per il Markdown** (una «pagina» ogni N caratteri o a ogni `\newpage`).
   - Svantaggi: una pagina che non esiste nel documento confonde l'utente e non ha un equivalente nel DOCX; `\newpage` è un artificio dei documenti sintetici.
4. **Unità degli offset**: byte UTF-8, code unit UTF-16 o Unicode scalar value.
   - Byte: dipendono dalla codifica, un offset può cadere dentro un carattere.
   - Code unit UTF-16: comode per JavaScript, ma un offset può cadere dentro una coppia surrogata e l'unità dipende dal linguaggio del viewer.
   - Scalar value: indipendenti dal linguaggio, coincidono con `char` di Rust e con `char_length` di PostgreSQL su UTF-8; il viewer converte con una regola fissa.
5. **`section_path` di soli titoli** (senza ordinale).
   - Svantaggi: due sezioni sorelle con lo stesso titolo («Risultati» ripetuto) danno lo stesso percorso e il locator diventa ambiguo.

## Decisione

**Opzione 1, nella forma multi-span decisa dall'utente** (*Storia* di D-10, 2026-10-21): un locator è una lista di al più **due** span, cioè al più un confine attraversato, perché un chunk non spezzi un paragrafo o una tabella a cavallo di pagina o di sezione:

```text
Locator =
  | Pdf  { spans: [ { page, rects: [[x0, y0, x1, y1], …] } ] }                 // 1 ≤ len ≤ 2
  | Text { projection: "cka-text-v1", doc_sha256,
           spans: [ { section_path: [ { title, ordinal } ], char_range: { start, end },
                      section_text_sha256 } ] }                                // 1 ≤ len ≤ 2
```

Lo spike ha misurato la forma a **span singolo**, che è il caso particolare `len = 1`: il round-trip, gli ordinali, il legame alla versione e la trasformazione dei rettangoli valgono span per span, e un locator a due span è valido se e solo se lo sono entrambi i suoi span e sono consecutivi nel documento (pagina successiva, o sezione successiva in ordine di documento). La validazione della consecutività non è stata misurata qui: entra nei test della fase che implementa i contratti.

Regole fissate dallo spike:

- **Testo canonico `cka-text-v1`** (definizione completa in `spikes/s02-parser/RISULTATI-locator.md`, «Definizioni»): CRLF e CR → `\n`, NFC, nessun ripiegamento tipografico; esclusi front matter e stili `Title`/`Subtitle`/`Author`/`Date`, interruzioni di pagina, immagini, note, testo cancellato e codici di campo; titoli ATX o `Heading1..6`, fuori dal testo di sezione; testo di sezione = blocchi propri uniti da `\n`, sottosezioni escluse; tabelle con righe `\n` e celle `\t`; enfasi tolta, link ridotti al testo; costrutti non coperti elencati in `unsupported`, mai scartati in silenzio.
- **Ordinale fra sorelle omonime**: ogni passo del percorso è `{title, ordinal}`, con `ordinal` 1-based fra le sorelle con lo stesso titolo canonico sotto lo stesso genitore; inserire una sorella con titolo diverso non cambia gli ordinali.
- **Offset**: Unicode scalar value, intervallo semiaperto `[start, end)`, sul testo canonico della sezione.
- **Legame alla versione**: ogni locator `Text` porta `projection`, `doc_sha256` dei byte del sorgente e `section_text_sha256`; la risoluzione rifiuta (`ProjectionMismatch`, `VersionMismatch`, `SectionHashMismatch`, `SectionNotFound`, `RangeOutOfBounds`) prima di restituire testo. Una regola di normalizzazione cambiata senza cambiare `projection` fa fallire il round-trip, non aggiorna in silenzio i risultati (RISK-001 di risk_audit_12).
- **Conversione UTF-16 lato viewer**: `utf16_start` = somma di `len_utf16` dei primi `start` scalar (1 nel BMP, 2 da U+10000), lo stesso per `end`; il viewer usa `String.prototype.slice` con gli offset convertiti. La conversione non spezza mai una coppia surrogata.
- **Rettangoli PDF**: si salvano nello spazio utente della pagina (punti, origine in basso a sinistra, prima di `/Rotate`) e il sistema li trasforma per il viewer con una funzione pura verso la viewport non scalata con origine in alto a sinistra, come `convertToViewportRectangle` di PDF.js a scala 1: traslazione nell'origine della CropBox, ribaltamento dell'asse y, rotazione oraria della pagina, estremi normalizzati.
- **Confine di esecuzione**: DOCX e Markdown si leggono solo nel figlio isolato del lanciatore di 01-07 (`op: locate`), con `check_docx` prima dell'XML, letture con tetto e parser XML a eventi senza DTD né entità non predefinite; le relazioni esterne del DOCX vanno in quarantena e non si risolvono mai.

## Conseguenze

- Positive: il criterio 2 della fase è chiuso, senza LibreOffice nel percorso dei documenti; il locator è ricontrollabile in modo deterministico (T-17) e lo stesso per MD e DOCX equivalenti; il viewer DOCX/MD non renderizza contenuti attivi (T-10).
- Negative: il viewer DOCX/MD non mostra pagine né l'aspetto originale; una modifica di `cka-text-v1` è una **nuova versione di proiezione** (`cka-text-v2`) e richiede di ricalcolare i locator salvati; l'hash di sezione fra MD e DOCX diverge quando il produttore del DOCX cambia la tipografia (pandoc), e lo spike non normalizza questa differenza.
- Follow-up:
  - S-07 (piano 01-16) e la Fondazione portano il tipo `Locator` multi-span nei contratti condivisi, con al più due span e la validazione della consecutività;
  - la trasformazione dei rettangoli resta lato sistema e si riusa nel viewer PDF (VIEW-01);
  - DEPENDENCIES.md e threat model li aggiorna solo il piano 01-17 (righe proposte in `RISULTATI-locator.md`, «Esito»).

## Evidenze

Misure dello spike (sessione `01-09-20261022t202549z-6312`, i7-13700K, worker con PDFium `chromium/8100`), tutte con DOCX e Markdown letti solo dal figlio `op: locate`:

- round-trip esatto 23/23 job su 12 campioni (11 MD + DOCX, 1 solo MD), `section_path` uguale all'atteso scritto a mano 23/23, sha256 del sorgente uguale al manifest 23/23;
- concordanza MD/DOCX su 11 coppie: percorso 11/11, intervallo 11/11, hash del testo di sezione 8/11 (le 3 divergenze sono apostrofi U+0027 → U+2019 introdotti da pandoc nel DOCX);
- conversione scalar → UTF-16: 10/10 vettori uguali alla fixture scritta a mano, fixture confermata da `Array.from` e `String.prototype.slice` di Node;
- rettangoli: trasformazione pura 8/8 e bbox del tracciato estratta da PDFium 8/8, errore massimo 0,000 pt (soglia 0,5 pt); i bounds di PDFium sono nello spazio utente non ruotato;
- job negativo con `excerpt_file` fuori dalle radici: `job_error`/`PathNotAllowed`, nessun file letto.

- Esiti contro le soglie, anomalie e decisione proposta: [`spikes/s02-parser/RISULTATI-locator.md`](../../spikes/s02-parser/RISULTATI-locator.md).
- Manifest hardware della sessione: [`spikes/s02-parser/results/locator/hw.json`](../../spikes/s02-parser/results/locator/hw.json).
- Conversione degli offset verificata contro la fixture a mano: [`spikes/s02-parser/results/locator/offset_vectors.json`](../../spikes/s02-parser/results/locator/offset_vectors.json), oracolo [`locator-gen/offset_vectors.expected.json`](../../spikes/s02-parser/locator-gen/offset_vectors.expected.json).
- Round-trip DOCX/MD: [`results/locator/locator_samples.json`](../../spikes/s02-parser/results/locator/locator_samples.json), [`results/locator/roundtrip.csv`](../../spikes/s02-parser/results/locator/roundtrip.csv).
- Rettangoli: [`results/locator/rect_transform.csv`](../../spikes/s02-parser/results/locator/rect_transform.csv), oracolo [`locator-gen/rect_cases.expected.json`](../../spikes/s02-parser/locator-gen/rect_cases.expected.json).
