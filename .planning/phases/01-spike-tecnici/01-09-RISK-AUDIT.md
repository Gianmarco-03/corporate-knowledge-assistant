<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_5, piano PLAN-010-09. Trascrizione: Claude (Anthropic), 2026-10-18. -->

# Audit dei rischi — PLAN-010-09 — Locator DOCX/Markdown/PDF

**Modifica:** gpt · 2026-10-18 · risposta indipendente a risk_audit_5.

**Esito: REVISE.** La CLI va riconciliata e il locator va reso non ambiguo su testo canonico, sezioni ripetute e unità Unicode; il test geometrico deve separare trasformazione da bbox del font.

## Identità e revisione effettivamente letta

- Repository: Gianmarco-03/corporate-knowledge-assistant.
- Branch dichiarato: task/PLAN-010-09.
- base_sha = head_sha = 180d63713a6216f905ee3f51845bd0feeb9a23ce.
- [PLAN v1 esaminato](https://github.com/Gianmarco-03/corporate-knowledge-assistant/blob/180d63713a6216f905ee3f51845bd0feeb9a23ce/.planning/phases/01-spike-tecnici/01-09-PLAN.md).
- SHA-256 del PLAN ricalcolato sul contenuto UTF-8 recuperato: `ec9b96648bac1b5927f4e5dce3a9a0570ac9e151fa985e4abbb54041edf700b0`; coincide esattamente con la richiesta.
- Il connettore GitHub è stato usato con lo SHA completo, senza sostituirlo con il default branch.

L'albero Git completo restituito non era troncato: allo SHA indicato **non esistono spikes/, Cargo.toml radice, codice o test di prodotto implementati**. Esistono documenti e scheletro con .gitkeep. L'assenza degli spike futuri è prevista: non è un finding. I rischi sotto riguardano l'esecuzione futura del PLAN e i contratti che promette, non difetti runtime di codice già scritto.

## Evidenza e limiti

Letti CONTEXT, RESEARCH, WIRING e GATE della fase, il PLAN in oggetto e i piani confinanti rilevanti. Ulteriori riscontri: regole correnti in CLAUDE.md e docs/architecture/interfaces-and-concurrency.md; stub di data-model/api-contract/threat-model; configurazione .gitignore e regole dei confini dei crate. Non ho avviato container, installato pacchetti, eseguito parser/file ostili o chiamato provider a pagamento. I test indicati sono verifiche richieste, **non test già passati**.

Provenienza:
- **repository**: presenza/assenza dei file, configurazione e regole attuali allo SHA sopra;
- **plan**: istruzioni e interfacce prescritte dai documenti citati in evidence, con righe del PLAN dove applicabile;
- **reasoning**: scenario condizionale e conseguenze riportati per ciascun rischio; non sono osservazioni di un incidente reale;
- **external_required**: questioni puntuali in external_research. Le etichette [V]/[NV] della ricerca di fase sono attribuzioni del documento: qui non diventano verifiche web nuove.

Il gate 01-GATE.md è stato letto, inclusi i finding già corretti. I suoi esiti non sostituiscono questa verifica indipendente dei failure mode. Le decisioni D-01..D-22 restano vincolanti; nessun rischio viene accettato per conto dell'utente.

## Copertura

Identità della sezione/versione, parsing DOCX limitato, testo canonico, serializzazione offset Rust→browser, precisione e trasformazioni PDF, indipendenza degli attesi e differenze MD/DOCX. D-10 resta a due varianti senza pagine artificiali; la deroga del gate per fixture sintetiche non è contestata.

Database, migrazioni e dati legacy: nello stato corrente non c'è uno schema di prodotto eseguito né dati legacy documentati da preservare; dove il PLAN introduce persistenza, sono stati esaminati i contratti futuri e le finestre di failure. Non invento una migrazione o un rollback di produzione. Precisione: rilevanti byte, offset/coordinate, unità temporali e percentili secondo il piano; non c'è logica monetaria/decimal da auditare. Compatibilità e UX: valutate nei seam con i consumatori, nei codici/esiti e nelle citazioni, non in schermate di prodotto ancora assenti.

## Rischi e ricerche nel formato AgentBridge

Gli ID RISK e RESEARCH sono locali a risk_audit_5; non sommare finding correlati di audit diversi come incidenti distinti. evidence.files può includere percorsi futuri prescritti dal PLAN: non afferma che tali file esistano già. I numeri di riga in evidence.details appartengono al documento citato come primo file.

```yaml
reviewed:
  plan_version: 1
  plan_sha256: "ec9b96648bac1b5927f4e5dce3a9a0570ac9e151fa985e4abbb54041edf700b0"
  git_head_sha: "180d63713a6216f905ee3f51845bd0feeb9a23ce"
verdict: REVISE
summary: {"critical":0,"high":1,"medium":4,"low":0,"info":0}
risks:
  - id: "RISK-001"
    severity: "HIGH"
    likelihood: "HIGH"
    category: "interface_integration"
    title: "Il lanciatore non inoltra excerpt e sottocomando al bin locator"
    scenario: "locator richiede locate --input --excerpt-file oppure roundtrip/rects con altri argomenti. La CLI upstream della 07 ammette solo child/input/out e rimane chiusa per questa slice."
    trigger: "Task 2 esegue locate e roundtrip nel worker usando il vincolo parse-parent."
    failure: "Non esiste una forma di comando prescritta che trasferisca l'intera richiesta al figlio isolato. Eseguire il bin direttamente aggirerebbe il confine promesso."
    impact: "Lo spike può non partire oppure misurare un percorso diverso da quello isolato dichiarato nell'ADR."
    affected: ["spikes/s02-parser/src/bin/locator/main.rs","spikes/s02-parser/src/bin/parse-parent.rs"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-09-PLAN.md",".planning/phases/01-spike-tecnici/01-07-PLAN.md",".planning/phases/01-spike-tecnici/01-WIRING.md"],"details":"Riga 92: `main.rs`: sottocomandi `locate --input <md|docx> --excerpt-file <f>` (stampa `ChildOutcome` con i locator trovati), `roundtrip --samples <json>`, `rects --pdf <f> --expected <json>` (estrae con PDFium la bbox della stri"}
    mitigation: "Allineare il protocollo della 07 con forwarding tipizzato o argomenti dopo --, e documentare qui le invocazioni complete. Verificare che excerpt e out siano percorsi consentiti e che i file di risultato siano prodotti dal job corretto."
    test_hint: "Usare locate con excerpt-file dal percorso con spazi, roundtrip e rects tramite il padre; controllare che ogni operazione resti soggetta a limiti e produca il proprio output."
    test_required: true
  - id: "RISK-002"
    severity: "MEDIUM"
    likelihood: "HIGH"
    category: "citation_identity"
    title: "Titoli ripetuti rendono section_path e offset non univoci"
    scenario: "Il locator Text contiene il vettore dei titoli verbatim e un intervallo nella sezione. Nei documenti sono possibili due sezioni sorelle con lo stesso titolo e lo stesso excerpt."
    trigger: "Due sezioni chiamate Risultati sotto lo stesso percorso, con il medesimo intervallo locale."
    failure: "Entrambe producono lo stesso locator; il viewer non può distinguere l'origine. Un round-trip che confronta soltanto il testo identico può risultare verde."
    impact: "Una citazione può evidenziare la sezione errata pur riportando il testo atteso."
    affected: ["spikes/s02-parser/src/bin/locator/md.rs","spikes/s02-parser/src/bin/locator/docx.rs","docs/adr/0005-locator-citazione.md","PLAN-010-16"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-09-PLAN.md"],"details":"Riga 89: section_path` = vettore dei titoli dalla radice, testo di sezione normalizzato, `char_range` = offset in caratteri Unicode (non byte) nel testo della sezione. - `docx.rs`: `s02_parser::prescan::check_docx` prima di tutto"}
    mitigation: "Mantenere D-10 aggiungendo un discriminante deterministico per occorrenza/blocco o una regola di unicità verificabile, sempre legata alla versione del documento. Il test deve confrontare identità del blocco, non soltanto stringa riestratta."
    test_hint: "Due sezioni sorelle omonime con testi identici e due versioni del documento: i locator devono distinguere i blocchi e non risolversi sulla versione sbagliata."
    test_required: true
  - id: "RISK-003"
    severity: "MEDIUM"
    likelihood: "HIGH"
    category: "unicode_serialization"
    title: "Caratteri Unicode non definisce l'unità che userà il viewer JavaScript"
    scenario: "Il Rust usa intervalli in caratteri Unicode nel testo NFC, mentre il viewer React/JS futuro può indicizzare stringhe in code unit UTF-16. I campioni prescritti contengono accenti ma non caratteri non BMP."
    trigger: "Emoji o altri caratteri non BMP prima o dentro il passaggio; testo con accenti combinanti."
    failure: "Lo stesso start/end seleziona un intervallo diverso se il browser usa slice direttamente. Il round-trip interamente Rust non scopre il disallineamento."
    impact: "Evidenziazione spostata e contratto serializzato ambiguo per S-07/Fondazione."
    affected: ["spikes/s02-parser/src/bin/locator/md.rs","spikes/s02-parser/src/bin/locator/docx.rs","docs/adr/0005-locator-citazione.md","spikes/s07-ports/src/lib.rs"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-09-PLAN.md"],"details":"Riga 89: offset in caratteri Unicode (non byte) nel testo della sezione. - `docx.rs`: `s02_parser::prescan::check_docx` prima di tutto; lettura in streaming di `word/document.xml` (via `zip` con `take`, parser `quick-xml` a event"}
    mitigation: "Specificare Unicode scalar values o altra unità precisa, intervallo half-open, normalizzazione e gestione degli a capo; definire come convertirli per il viewer. Aggiungere vettori di contratto consumabili da Rust e JavaScript."
    test_hint: "Round-trip su A😀e più accento combinante e newline CRLF; applicare gli offset con l'algoritmo del viewer e verificare che il testo selezionato coincida col Rust dopo la normalizzazione fissata."
    test_required: true
  - id: "RISK-004"
    severity: "MEDIUM"
    likelihood: "HIGH"
    category: "text_canonicalization"
    title: "Il testo canonico DOCX/MD non è definito oltre NFC e newline"
    scenario: "Il piano impone uguale excerpt e percorso anche per campioni dentro tabelle, ma il Markdown contiene markup, separatori e front matter; il DOCX contiene run, celle, tab, paragrafi e stili generati da pandoc."
    trigger: "Excerpt su più celle, testo con grassetto/link, tab o paragrafi con più run."
    failure: "I due parser possono produrre testi di sezione diversi pur rappresentando lo stesso documento; offset e round-trip non sono confrontabili. I soli campioni selezionati dentro una cella possono nascondere il difetto."
    impact: "ADR-0005 trasferisce una rappresentazione non riproducibile ai chunk e al viewer."
    affected: ["spikes/s02-parser/src/bin/locator/docx.rs","spikes/s02-parser/src/bin/locator/md.rs","spikes/s02-parser/locator-gen/expected_samples.json"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-09-PLAN.md",".planning/phases/01-spike-tecnici/01-03-PLAN.md"],"details":"Riga 89: testo di sezione normalizzato, `char_range` = offset in caratteri Unicode (non byte) nel testo della sezione. - `docx.rs`: `s02_parser::prescan::check_docx` prima di tutto; lettura in streaming di `word/document.xml` (vi"}
    mitigation: "Definire una proiezione testuale canonica per heading, markup, celle, spazi e paragrafi, conservandone versione e regole; scrivere gli attesi indipendenti prima della misura, inclusi separatori e offset."
    test_hint: "Stesso documento MD/DOCX con tabella multi-cella, titolo con enfasi/link e testo spezzato in run; confrontare il testo canonico completo e gli offset, non solo l'excerpt trovato."
    test_required: true
  - id: "RISK-005"
    severity: "MEDIUM"
    likelihood: "MEDIUM"
    category: "geometry_oracle"
    title: "Posizione nota del marcatore e bbox estratta non sono automaticamente lo stesso rettangolo"
    scenario: "gen_pages.py calcola un rettangolo analitico, mentre rects estrae la bbox della stringa via PDFium. Il piano non fissa se l'atteso rappresenti area disegnata, advance dei glifi, ink bounds o loose bounds."
    trigger: "Font/bearing differenti o stringa marcatore la cui bbox non coincide con il box progettato."
    failure: "Una trasformazione corretta può fallire la tolleranza 0,5 pt per una differenza nell'input geometrico; correggere la trasformazione sull'output atteso può introdurre un errore reale."
    impact: "Misura della normalizzazione PDF ambigua, con rischio di highlight sistematicamente spostato."
    affected: ["spikes/s02-parser/locator-gen/gen_pages.py","spikes/s02-parser/src/bin/locator/pdfrect.rs","spikes/s02-parser/results/locator/rect_transform.csv"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-09-PLAN.md"],"details":"Riga 93: rettangolo atteso calcolato analiticamente (derivazione scritta nello script e riportata in RISULTATI: è un valore geometrico, non un output della pipeline). Output in `spikes/s02-parser/results/raw/locator/` (ignorato)."}
    mitigation: "Separare test della trasformazione pura su rettangoli numerici noti dal test di estrazione della bbox; dichiarare quale bbox si confronta, font/metriche e spazio di partenza. Verificare se PDFium ha già applicato parte della rotazione per evitare doppia trasformazione."
    test_hint: "Eseguire le otto combinazioni su un rettangolo numerico asimmetrico, poi separatamente sul testo; un errore di bbox del font non deve far cambiare la matrice di trasformazione."
    test_required: true
external_research: []
```

## Condizioni per una nuova valutazione

Correggere o dimostrare la mitigazione dei rischi HIGH con le verifiche indicate, riconciliare i contratti con i piani proprietari senza modifiche silenziose ai file chiusi e aggiornare versione/impronta del PLAN quando cambia. Le ricerche blocking devono essere chiuse con fonti e versione prima di attestare la rispettiva garanzia. I MEDIUM richiedono mitigazione o decisione esplicita nella sede prevista dal progetto; questo audit non concede accettazioni.

Verdetto finale: **REVISE**