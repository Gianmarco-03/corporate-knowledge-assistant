<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_6, piano PLAN-010-13. Trascrizione: Claude (Anthropic), 2026-10-18. -->

# Audit dei rischi — PLAN-010-13 — OCR, PP-OCR e TTQ-parse

**Modifica:** gpt · 2026-10-18 · risposta indipendente a risk_audit_6.

**Esito: REVISE.** Il percorso OCR isolato e le concorrenze devono essere eseguibili entro budget; la misura composta e le bbox degradate richiedono una provenienza completa. Il lock pip non identifica i pesi.

## Identità e revisione effettivamente letta

- Repository: Gianmarco-03/corporate-knowledge-assistant.
- Branch dichiarato: task/PLAN-010-13.
- base_sha = head_sha = 180d63713a6216f905ee3f51845bd0feeb9a23ce.
- [PLAN v1 esaminato](https://github.com/Gianmarco-03/corporate-knowledge-assistant/blob/180d63713a6216f905ee3f51845bd0feeb9a23ce/.planning/phases/01-spike-tecnici/01-13-PLAN.md).
- SHA-256 del PLAN ricalcolato sul contenuto UTF-8 recuperato: `f7ece26d9a2e323ab5f767148f98515f1a6517bd72592506d01a394bf75711b5`; coincide esattamente con la richiesta.
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

Rendering, OCR e discendenti, budget aggregati, precisione geometrica, identificazione modelli, download/warmup, CER e campioni esclusi, composizione delle latenze e n per i percentili, aggiornamento ADR. La correzione del gate che impone hash alle dipendenze pip è presente: non la riporto come mancante; non viene eseguita alcuna installazione né richiesta la ratifica dei pacchetti in questo audit.

Database, migrazioni e dati legacy: nello stato corrente non c'è uno schema di prodotto eseguito né dati legacy documentati da preservare; dove il PLAN introduce persistenza, sono stati esaminati i contratti futuri e le finestre di failure. Non invento una migrazione o un rollback di produzione. Precisione: rilevanti byte, offset/coordinate, unità temporali e percentili secondo il piano; non c'è logica monetaria/decimal da auditare. Compatibilità e UX: valutate nei seam con i consumatori, nei codici/esiti e nelle citazioni, non in schermate di prodotto ancora assenti.

## Rischi e ricerche nel formato AgentBridge

Gli ID RISK e RESEARCH sono locali a risk_audit_6; non sommare finding correlati di audit diversi come incidenti distinti. evidence.files può includere percorsi futuri prescritti dal PLAN: non afferma che tali file esistano già. I numeri di riga in evidence.details appartengono al documento citato come primo file.

```yaml
reviewed:
  plan_version: 1
  plan_sha256: "f7ece26d9a2e323ab5f767148f98515f1a6517bd72592506d01a394bf75711b5"
  git_head_sha: "180d63713a6216f905ee3f51845bd0feeb9a23ce"
verdict: REVISE
summary: {"critical":0,"high":3,"medium":2,"low":0,"info":0}
risks:
  - id: "RISK-001"
    severity: "HIGH"
    likelihood: "HIGH"
    category: "interface_integration"
    title: "OCR e render richiedono opzioni non previste dal lanciatore chiuso"
    scenario: "Il bin ocr ha render --dpi e tesseract --image; il contratto parse-parent della 07 espone solo --child, --input e --out. Questa slice non può modificare il padre."
    trigger: "Si eseguono le scansioni e Tesseract nel modello isolato obbligatorio."
    failure: "Non è definito il trasferimento del sottocomando, del DPI o del percorso immagine al figlio; invocarlo direttamente cambierebbe il percorso sottoposto a misura."
    impact: "Il benchmark OCR non è eseguibile o non verifica il modello di isolamento promesso."
    affected: ["spikes/s02-parser/src/bin/ocr/main.rs","spikes/s02-parser/scan-gen/make_scans.sh","spikes/s02-parser/src/bin/parse-parent.rs"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-13-PLAN.md",".planning/phases/01-spike-tecnici/01-07-PLAN.md",".planning/phases/01-spike-tecnici/01-WIRING.md"],"details":"Riga 121: Bin `ocr` (cartella `src/bin/ocr/`, nessuna modifica a `lib.rs`), eseguito come figlio di `parse-parent run --child ocr`: sottocomando `render --input <pdf> --dpi 300 --out <dir>` (PDFium `render_with_config` con `scale_"}
    mitigation: "Riconciliare prima il protocollo upstream della 07 e fissare comandi completi per render, tesseract e misure parallele; ogni Tesseract deve restare nel gruppo e nei limiti del job."
    test_hint: "Da input PDF fino al TSV passando dal padre, verificare inoltro DPI, input/out, classificazione dell'errore e assenza di processi residui."
    test_required: true
  - id: "RISK-002"
    severity: "HIGH"
    likelihood: "HIGH"
    category: "aggregate_resource_budget"
    title: "Otto o sedici processi OCR non sono dimensionati sul cgroup da 2 GiB"
    scenario: "Il throughput usa 1/4/8/16 figli isolati. Il limite memory=2g è dell'intero worker; /work è tmpfs da 2g e ogni figlio eredita un limite AS individuale calibrato altrove."
    trigger: "Rendering a 300 DPI di molte pagine o più istanze Tesseract con ita+eng contemporanee."
    failure: "Il consumo aggregato e le immagini temporanee superano la riserva senza violare il limite del singolo processo; il kernel può terminare il job o il padre."
    impact: "Confronto di throughput/OCR falsato e promessa di contenimento inapplicabile alle concorrenze prescritte."
    affected: ["spikes/s02-parser/src/bin/ocr/main.rs","spikes/s02-parser/results/ocr/ocr_throughput.csv","spikes/compose.yaml"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-13-PLAN.md",".planning/phases/01-spike-tecnici/01-02-PLAN.md",".planning/phases/01-spike-tecnici/01-07-PLAN.md"],"details":"Riga 147: 1, 4, 8, 16 processi paralleli (figli isolati); PaddleOCR (se approvato) con `paddleocr ocr -i <png> --lang it --ocr_version PP-OCRv5 --use_doc_orientation_classify False --use_doc_unwarping False --use_textline_orientat"}
    mitigation: "Misurare prima memoria e scratch per pagina, fissare admission control e budget aggregato, ripulire per pagina e registrare le concorrenze non sostenibili come fallimenti. Non aumentare in silenzio memoria o usare runner non isolato per far passare il target."
    test_hint: "Pagini grandi alla massima dimensione ammessa e 8/16 figli; verificare memoria totale, scratch, eventi OOM, padre vivo e ricostruibilità del risultato su container fresco."
    test_required: true
  - id: "RISK-003"
    severity: "HIGH"
    likelihood: "MEDIUM"
    category: "latency_measurement"
    title: "La somma di tempi da CSV non dimostra il TTQ-parse del percorso composto"
    scenario: "Il PLAN richiede componenti nella stessa sessione, ma descrive ttq come lettore di CSV e richiama anche i tempi di estrazione/Docling prodotti dalla 08; non prescrive una correlazione run_id, input e insieme delle pagine fra questi dati e le nuove misure OCR."
    trigger: "Cache calda per un componente e fredda per un altro, o somma di tempi aggregati anziché della stessa elaborazione."
    failure: "Un total_ms può escludere prescan, rendering, scrittura/trasferimento o setup, oppure sommare percentili non componibili. Quattro run utili non sostengono da sole una stima stabile del p95."
    impact: "Approvazione della quota di 5 s/20 s senza evidenza del lavoro effettivamente necessario; la scelta parser trasferita all'ADR può non rispettare il budget."
    affected: ["spikes/s02-parser/src/bin/ocr/main.rs","spikes/s02-parser/results/ocr/ttq_parse.csv","docs/adr/0004-parser-ocr-isolamento.md"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-13-PLAN.md"],"details":"Riga 121: compone i tempi letti dai CSV e dalle misure della sessione.  `spikes/docker/paddle.Dockerfile` (solo dopo il Task 1): `FROM python:3.12-slim`, `COPY docker/paddle-requirements.lock /tmp/requirements.lock` (contesto di b"}
    mitigation: "Mantenere la composizione dichiarata ma fissare run_id/input hash/insieme di pagine comuni, confini e inclusione di tutti i costi; sommare osservazioni della medesima run, poi calcolare il percentile. Confrontare il totale con un cronometro esterno della sequenza e dichiarare warm/cold e numerosità insufficiente."
    test_hint: "Ritardo artificiale in prescan, render e trasporto deve comparire nel totale; non consentire componenti di run_id diversi. Provare un PDF misto oltre ai digitali e riportare n delle osservazioni utili."
    test_required: true
  - id: "RISK-004"
    severity: "MEDIUM"
    likelihood: "HIGH"
    category: "ocr_provenance"
    title: "La variante degradata richiede una trasformazione inversa oltre al cambio DPI"
    scenario: "Le scansioni degradate sono a 200 DPI, ruotate di 1,5 gradi e ricodificate; il comando Tesseract indica --dpi 300 e tsv.rs prevede soltanto px→pt e inversione Y."
    trigger: "Una bbox TSV della variante degradata deve tornare al PDF originale."
    failure: "Senza DPI effettivo, dimensioni e trasformazione della degradazione, i rettangoli non mappano il testo sorgente. Il CER può restare buono e non rilevare l'errore."
    impact: "Citazioni OCR con bbox plausibili ma sbagliate, trasferite come capacità del parser a S-07."
    affected: ["spikes/s02-parser/scan-gen/make_scans.sh","spikes/s02-parser/src/bin/ocr/tsv.rs","spikes/s02-parser/RISULTATI-ocr.md"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-13-PLAN.md"],"details":"Riga 119: 200 DPI, rumore gaussiano, rotazione 1,5°, JPEG qualità 60) e un PDF misto (pagine di testo + pagine immagine) con `qpdf`; output in `spikes/s02-parser/results/raw/scans/` (ignorato), sha256 annotati in RISULTATI §Setup."}
    mitigation: "Salvare per immagine la trasformazione completa render/degrado e il DPI effettivo; convertire i quattro vertici con l'inversa e dichiarare unità/origine. Allineare il DPI comunicato a Tesseract alla variante o giustificare esplicitamente il diverso valore."
    test_hint: "Testo in coordinate note prima di render/degrado; ricondurre le bbox good/degraded al medesimo rettangolo PDF con tolleranza dichiarata, anche agli angoli della pagina."
    test_required: true
  - id: "RISK-005"
    severity: "MEDIUM"
    likelihood: "HIGH"
    category: "model_reproducibility"
    title: "Il lock pip non congela i pesi né dimostra la variante PP-OCRv6 misurata"
    scenario: "La variante v5 è selezionata per flag, mentre quella chiamata v6 omette --ocr_version e scarica i modelli al primo avvio. Il lock con hash copre i pacchetti Python, non gli artefatti modello risolti o la cache."
    trigger: "Default effettivo diverso da quello atteso, primo download aggiornato o cache preesistente."
    failure: "Una riga version=PP-OCRv6 può etichettare un modello diverso senza prova della versione realmente caricata; il confronto CPU incorpora condizioni di cache differenti."
    impact: "Regola vince nettamente applicata a motori non identificati in modo riproducibile."
    affected: ["spikes/docker/paddle.Dockerfile","spikes/docker/paddle-requirements.lock","spikes/s02-parser/results/ocr/ocr.csv"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-13-PLAN.md"],"details":"Riga 147: la stessa riga senza `--ocr_version` con `--save_path /results/raw/ocr/paddle-v6` (output grezzi sotto `results/raw/`, già ignorato da `**/results/raw/`) (default PP-OCRv6_medium, deviazione 2 di RESEARCH §0, misura vinc"}
    mitigation: "Registrare nome/revisione/hash dei pesi effettivi e selezione risolta dal runtime, fissare cache/preload e separare download/warmup dalla misura. Se il default non coincide, segnare lo sfidante non misurato, non rinominarlo v6."
    test_hint: "Due avvii con cache pulita devono riportare gli stessi artifact hash e configurazione; un default diverso deve produrre mismatch esplicito. Confermare i parametri esatti con la ricerca collegata."
    test_required: true
external_research: 
  - id: "RESEARCH-001"
    linked_risk: "RISK-005"
    question: "In paddleocr 3.7.0 con paddlepaddle 3.3.1, quali nomi/configurazioni e artifact effettivi vengono scelti da paddleocr ocr --lang it senza --ocr_version e con --ocr_version PP-OCRv5? Verificare da documentazione/sorgente versionati come fissare esplicitamente v5/v6, registrare hash/revisione dei pesi e disabilitare aggiornamenti/download durante il benchmark; non assumere v6 dal solo nome della cartella."
    blocking: false
```

## Condizioni per una nuova valutazione

Correggere o dimostrare la mitigazione dei rischi HIGH con le verifiche indicate, riconciliare i contratti con i piani proprietari senza modifiche silenziose ai file chiusi e aggiornare versione/impronta del PLAN quando cambia. Le ricerche blocking devono essere chiuse con fonti e versione prima di attestare la rispettiva garanzia. I MEDIUM richiedono mitigazione o decisione esplicita nella sede prevista dal progetto; questo audit non concede accettazioni.

Verdetto finale: **REVISE**