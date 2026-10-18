<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_4, piano PLAN-010-08. Trascrizione: Claude (Anthropic), 2026-10-18. -->

# Audit dei rischi — PLAN-010-08 — Estrazione PDF e Docling CPU/GPU

**Modifica:** gpt · 2026-10-18 · risposta indipendente a risk_audit_4.

**Esito: REVISE.** Va resa eseguibile l'invocazione isolata, dimostrato il confine del ramo Docling e reso indipendente il punteggio della selezione/allineamento.

## Identità e revisione effettivamente letta

- Repository: Gianmarco-03/corporate-knowledge-assistant.
- Branch dichiarato: task/PLAN-010-08.
- base_sha = head_sha = 180d63713a6216f905ee3f51845bd0feeb9a23ce.
- [PLAN v1 esaminato](https://github.com/Gianmarco-03/corporate-knowledge-assistant/blob/180d63713a6216f905ee3f51845bd0feeb9a23ce/.planning/phases/01-spike-tecnici/01-08-PLAN.md).
- SHA-256 del PLAN ricalcolato sul contenuto UTF-8 recuperato: `8a8735b5a8dbfda45124d53ae0e50bde5ae595981b6e42935e278f62312f67b7`; coincide esattamente con la richiesta.
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

Pagine/coordinate e ordine di lettura, matching e denominatori delle metriche, annotazioni indipendenti e guardrail, selezione delle tabelle, isolamento Docling, timeout, cache/warmup e dipendenze dei servizi CPU/GPU. D-07 e D-11 non vengono ridiscusse; i limiti del campione vanno riportati come tali.

Database, migrazioni e dati legacy: nello stato corrente non c'è uno schema di prodotto eseguito né dati legacy documentati da preservare; dove il PLAN introduce persistenza, sono stati esaminati i contratti futuri e le finestre di failure. Non invento una migrazione o un rollback di produzione. Precisione: rilevanti byte, offset/coordinate, unità temporali e percentili secondo il piano; non c'è logica monetaria/decimal da auditare. Compatibilità e UX: valutate nei seam con i consumatori, nei codici/esiti e nelle citazioni, non in schermate di prodotto ancora assenti.

## Rischi e ricerche nel formato AgentBridge

Gli ID RISK e RESEARCH sono locali a risk_audit_4; non sommare finding correlati di audit diversi come incidenti distinti. evidence.files può includere percorsi futuri prescritti dal PLAN: non afferma che tali file esistano già. I numeri di riga in evidence.details appartengono al documento citato come primo file.

```yaml
reviewed:
  plan_version: 1
  plan_sha256: "8a8735b5a8dbfda45124d53ae0e50bde5ae595981b6e42935e278f62312f67b7"
  git_head_sha: "180d63713a6216f905ee3f51845bd0feeb9a23ce"
verdict: REVISE
summary: {"critical":0,"high":2,"medium":2,"low":0,"info":0}
risks:
  - id: "RISK-001"
    severity: "HIGH"
    likelihood: "HIGH"
    category: "interface_integration"
    title: "extract pdf non è invocabile dal protocollo CLI promesso"
    scenario: "Task 1 richiede un bin extract con sottocomando pdf e opzioni input/out; il contratto della 07 espone solo parse-parent run --child extract --input ... --out ..., senza passthrough dei sottocomandi."
    trigger: "Prima estrazione via worker usando esattamente il comando prescritto."
    failure: "La CLI del figlio e quella del lanciatore non hanno una mappatura definita. Il piano vieta di modificare il lanciatore e non prevede un wrapper compatibile."
    impact: "La misura dei PDF digitali può fermarsi prima di esercitare l'isolamento."
    affected: ["spikes/s02-parser/src/bin/extract/main.rs","spikes/s02-parser/src/bin/parse-parent.rs"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-08-PLAN.md",".planning/phases/01-spike-tecnici/01-07-PLAN.md",".planning/phases/01-spike-tecnici/01-WIRING.md"],"details":"Riga 98: Sottocomando `score` (fuori dal figlio, sui JSON prodotti): CER con distanza di Levenshtein su testo normalizzato come in `parser-it.json` (NFC con il crate `unicode-normalization`, già dichiarato nel piano 01-01); IoU p"}
    mitigation: "Allineare il contratto upstream nella 07 e adottarlo qui; in alternativa definire esplicitamente un entry point del figlio compatibile con input/out, separato dai comandi di score. Verificare il processo realmente lanciato, non solo cargo check."
    test_hint: "Eseguire un PDF minimo via parse-parent e verificare ChildOutcome ok più JSON pagina; invocazione errata deve fallire prima della misura con diagnostica separata dal fallimento del parser."
    test_required: true
  - id: "RISK-002"
    severity: "HIGH"
    likelihood: "MEDIUM"
    category: "security_isolation"
    title: "Docling riceve il PDF completo fuori dal supervisore per documento"
    scenario: "docling-client nel runner invia il PDF a un servizio persistente con page_range e document_timeout=120. I limiti mem/numero pagine sono globali di servizio; il piano non richiede un prescan riuscito come condizione dell'invio né un ciclo di vita per richiesta."
    trigger: "PDF contenente struttura ostile fuori dalle pagine scelte, conversione bloccata o più richieste che condividono il worker del servizio."
    failure: "page_range non dimostra che solo quelle pagine siano lette; il processo può trattenere lavoro dopo timeout HTTP o influenzare il documento successivo. Il worker PDFium isolato non contiene il processo Docling."
    impact: "D-08 non è dimostrata per il ramo tabelle e la misura del secondo documento può includere lavoro residuo del primo."
    affected: ["spikes/s02-parser/src/bin/docling-client/main.rs","spikes/compose.yaml","spikes/s02-parser/results/extract/docling.csv"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-08-PLAN.md",".planning/phases/01-spike-tecnici/01-02-PLAN.md",".planning/phases/01-spike-tecnici/01-CONTEXT.md"],"details":"Riga 100: Bin `docling-client` (eseguito nel runner, rete `parse`): `POST {DOCLING_URL|DOCLING_CPU_URL}/v1/convert/file` multipart con `to_formats=json`, `do_ocr=false`, `table_mode=accurate`, `do_table_structure=true`, `page_rang"}
    mitigation: "Esplicitare validazione/quarantena prima dell'invio, timeout lato chiamante e prova di arresto effettivo del lavoro lato servizio; predisporre riavvio controllato fra job ostili o isolamento per job compatibile col sidecar. Confermare nella ricerca collegata quali garanzie offra il servizio pinnato."
    test_hint: "Inviare PDF con problema fuori da page_range e una conversione che eccede il tempo; dopo il fallimento verificare processo, CPU/memoria e successo di una successiva pagina benigna entro budget."
    test_required: true
  - id: "RISK-003"
    severity: "MEDIUM"
    likelihood: "HIGH"
    category: "evaluation_alignment"
    title: "IoU parola per parola senza contratto di abbinamento può nascondere omissioni"
    scenario: "PDFium ricostruisce parole dai segmenti e lo scorer confronta parole TAT-DQA; tokenizzazione, ordine, numeri ripetuti e rotazioni possono divergere. Non è definito come abbinare e contare le parole senza match."
    trigger: "Pagina a colonne, token spezzati o termini numerici ripetuti."
    failure: "Abbinare per indice o testo non univoco oppure calcolare la quota IoU solo sulle parole matched può produrre errori o un punteggio alto mentre interi passaggi sono persi."
    impact: "Scelta di parser/coordinate non sostenuta dalla metrica; highlight sbagliati per le citazioni."
    affected: ["spikes/s02-parser/src/bin/extract/score.rs","spikes/s02-parser/results/extract/bbox_iou.csv","spikes/s02-parser/RISULTATI-estrazione.md"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-08-PLAN.md"],"details":"Riga 98: IoU parola per parola contro TAT-DQA dopo la conversione dichiarata delle coordinate (TAT-DQA usa pixel origine in alto a sinistra: trasformazione scritta in `score.rs` e in RISULTATI); F1 di cella; recall delle regioni "}
    mitigation: "Fissare allineamento, normalizzazione e sistema di coordinate prima della misura; contare tutti gli attesi nel denominatore, omissioni come miss e match univoci. Pubblicare esempi di allineamento e i casi non valutabili separatamente."
    test_hint: "Output sintetico con una colonna mancante e dieci numeri identici in posizioni diverse: il punteggio deve penalizzare le omissioni e non riutilizzare la stessa parola più volte."
    test_required: true
  - id: "RISK-004"
    severity: "MEDIUM"
    likelihood: "MEDIUM"
    category: "measurement_validity"
    title: "Il campione per la selezione delle tabelle può essere lo stesso usato per tarare l'euristica"
    scenario: "Densità dei numeri e allineamento x non hanno parametri congelati; la recall ≥0,95 è richiesta sul documento da 25 pagine di cui è già disponibile l'elenco manuale delle pagine-tabella."
    trigger: "Si aggiustano le soglie dell'euristica finché la recall passa su quel documento."
    failure: "Il test misura adattamento al campione visto, non l'affidabilità della selezione su documenti successivi. Una pagina saltata da questo filtro non raggiunge Docling."
    impact: "Prestazioni apparentemente compatibili col budget grazie a un filtro che perde tabelle reali fuori campione."
    affected: ["spikes/s02-parser/src/bin/extract/tables.rs","spikes/s02-parser/results/extract/table_pages.csv","docs/adr/0004-parser-ocr-isolamento.md"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-08-PLAN.md"],"details":"Riga 98: euristica PDFium per le pagine-tabella (densità di token numerici e allineamento di colonne per coordinate x) e raggruppamento a celle per la baseline.  Bin `docling-client` (eseguito nel runner, rete `parse`): `POST {DO"}
    mitigation: "Separare pagine di taratura e holdout con layout diversi e congelare i parametri prima di aprire gli attesi del secondo set. Dichiarare la copertura limitata della misura e conservare un fallback per pagine incerte."
    test_hint: "Holdout con tabelle senza bordi, poche cifre e testo a colonne non tabellare; misurare recall e precisione separatamente da quelle del documento di taratura."
    test_required: true
external_research: 
  - id: "RESEARCH-001"
    linked_risk: "RISK-002"
    question: "Per docling-serve v1.37.0 verificare da fonte primaria l'isolamento del job, la semantica di document_timeout e della disconnessione del client, la cancellazione effettiva del lavoro e se page_range evita la lettura di strutture/pagine esterne all'intervallo. Indicare le configurazioni necessarie per confine per documento o riavvio sicuro su timeout, senza reinterpretare un timeout HTTP come kill del parser."
    blocking: true
```

## Condizioni per una nuova valutazione

Correggere o dimostrare la mitigazione dei rischi HIGH con le verifiche indicate, riconciliare i contratti con i piani proprietari senza modifiche silenziose ai file chiusi e aggiornare versione/impronta del PLAN quando cambia. Le ricerche blocking devono essere chiuse con fonti e versione prima di attestare la rispettiva garanzia. I MEDIUM richiedono mitigazione o decisione esplicita nella sede prevista dal progetto; questo audit non concede accettazioni.

Verdetto finale: **REVISE**