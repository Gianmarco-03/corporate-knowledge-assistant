<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_1, piano PLAN-010-02. Trascrizione: Claude (Anthropic), 2026-10-18. -->

# Audit dei rischi — PLAN-010-02 — Harness infra, segreti e gate GPU

**Modifica:** gpt · 2026-10-18 · risposta indipendente a risk_audit_1.

**Esito: REVISE.** L'harness deve essere riconciliato con i comandi dei consumatori prima di chiudere compose e runner. La verifica GPU isolata non prova selezione dei servizi, accesso ai volumi o preparazione offline.

## Identità e revisione effettivamente letta

- Repository: Gianmarco-03/corporate-knowledge-assistant.
- Branch dichiarato: task/PLAN-010-02.
- base_sha = head_sha = 180d63713a6216f905ee3f51845bd0feeb9a23ce.
- [PLAN v1 esaminato](https://github.com/Gianmarco-03/corporate-knowledge-assistant/blob/180d63713a6216f905ee3f51845bd0feeb9a23ce/.planning/phases/01-spike-tecnici/01-02-PLAN.md).
- SHA-256 del PLAN ricalcolato sul contenuto UTF-8 recuperato: `df929915b0e04887220827e20a62bfe0a9e859f68cf866c798856608c52eeab4`; coincide esattamente con la richiesta.
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

Profili, avvio progressivo, worktree, rete interna/loopback, segreti e UID, volumi e immagini, errori di bootstrap, GPU condivisa, tracciabilità delle sessioni. Il rischio runner con tutte le chiavi è già dichiarato T-01-02-07 nel gate: non lo ripresento come scoperta nuova né lo accetto nuovamente per l'utente.

Database, migrazioni e dati legacy: nello stato corrente non c'è uno schema di prodotto eseguito né dati legacy documentati da preservare; dove il PLAN introduce persistenza, sono stati esaminati i contratti futuri e le finestre di failure. Non invento una migrazione o un rollback di produzione. Precisione: rilevanti byte, offset/coordinate, unità temporali e percentili secondo il piano; non c'è logica monetaria/decimal da auditare. Compatibilità e UX: valutate nei seam con i consumatori, nei codici/esiti e nelle citazioni, non in schermate di prodotto ancora assenti.

## Rischi e ricerche nel formato AgentBridge

Gli ID RISK e RESEARCH sono locali a risk_audit_1; non sommare finding correlati di audit diversi come incidenti distinti. evidence.files può includere percorsi futuri prescritti dal PLAN: non afferma che tali file esistano già. I numeri di riga in evidence.details appartengono al documento citato come primo file.

```yaml
reviewed:
  plan_version: 1
  plan_sha256: "df929915b0e04887220827e20a62bfe0a9e859f68cf866c798856608c52eeab4"
  git_head_sha: "180d63713a6216f905ee3f51845bd0feeb9a23ce"
verdict: REVISE
summary: {"critical":0,"high":1,"medium":3,"low":0,"info":0}
risks:
  - id: "RISK-001"
    severity: "HIGH"
    likelihood: "HIGH"
    category: "runtime_dependencies"
    title: "I profili non esprimono una scelta esclusiva fra GPU, CPU e parti S-02"
    scenario: "Il compose chiuso assegna più profili allo stesso servizio: tei-embed e tei-rerank includono gpu; docling e docling-cpu includono s02; worker e paddle sono parte di s02. I Dockerfile worker e paddle nascono solo nei piani 07 e 13."
    trigger: "La slice 08 avvia i profili s02 e gpu per misurare Docling, prima che esista paddle.Dockerfile."
    failure: "Con la semantica ordinaria OR dei profili possono avviarsi servizi non richiesti o non ancora costruibili, oppure TEI e Docling occupano insieme i 6 GB e la misura non rappresenta il singolo motore."
    impact: "Blocco dell'onda o falsi fallimenti per VRAM; il gate GPU di wave 0 non esercita questa combinazione."
    affected: ["spikes/compose.yaml","spikes/results/gpu-check.txt","PLAN-010-08","PLAN-010-13"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-02-PLAN.md",".planning/phases/01-spike-tecnici/01-08-PLAN.md",".planning/phases/01-spike-tecnici/01-WIRING.md"],"details":"Riga 27: tei-embed per D-13/D-14, tei-rerank per D-21, docling GPU e CPU per D-07, worker isolato per D-08, paddle per D-09, runner Linux) con porte solo su 127.0.0.1 e segreti solo da env\"     - \"Nessun segreto né endpoint lette"}
    mitigation: "Definire nella 02 comandi per nomi di servizio espliciti, dipendenze e set di servizi attesi per ogni sessione; non usare i profili come AND. Fare smoke test delle combinazioni progressive prima di chiudere il YAML. La semantica della versione Compose usata resta da confermare con la ricerca collegata."
    test_hint: "Con soli artefatti di wave 0 presenti e poi con quelli della 08, risolvere il compose e verificare che partire col set Docling non richieda paddle o TEI. Controllare container effettivi e VRAM prima della misura."
    test_required: true
  - id: "RISK-002"
    severity: "MEDIUM"
    likelihood: "HIGH"
    category: "filesystem_permissions"
    title: "Runner non root senza prova di scrittura sui volumi Linux"
    scenario: "Il runner usa UID 10001 e monta volumi nuovi per /target, registry e /work/tmp; il piano non definisce inizializzazione dei proprietari di questi mount."
    trigger: "Prima esecuzione con volumi vuoti su Docker Desktop."
    failure: "Una build riuscita dell'immagine non dimostra che cargo, tempfile e il writer dei risultati possano scrivere nei mount a runtime."
    impact: "Tutte le slice possono fermarsi per PermissionDenied; aggirare il difetto eseguendo da root altera l'ambiente della misura."
    affected: ["spikes/docker/runner.Dockerfile","spikes/compose.yaml","spikes/tools/hw-manifest.sh"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-02-PLAN.md"],"details":"Riga 90: utente non root `10001` per l'esecuzione; nessun segreto nell'immagine.  `spikes/.env.example` con le sezioni e chiavi di WIRING §5 W6, valori vuoti per ogni segreto e per ogni URL interno (con l'esempio di forma in una "}
    mitigation: "Prevedere inizializzazione esplicita dei volumi con ownership 10001 e uno smoke test dal runner definitivo; registrare permessi e UID, senza allargare i privilegi del worker."
    test_hint: "Su volumi nuovi, come UID 10001 creare un tempfile in /work/tmp, scrivere in target e registry, creare un CSV/hw.json sotto /src e poi rilanciare dopo ricreazione del container."
    test_required: true
  - id: "RISK-003"
    severity: "MEDIUM"
    likelihood: "MEDIUM"
    category: "external_services"
    title: "Avvio Docling offline non provato prima della chiusura dell'harness"
    scenario: "Docling è solo sulla rete parse internal; RESEARCH §2 riconosce che potrebbero mancare pesi e propone un primo avvio con rete normale, mentre il YAML diventa chiuso dopo la 02."
    trigger: "Il primo lavoro chiede un modello non già incluso nella precisa immagine CPU/GPU."
    failure: "Il servizio non diventa operativo o la conversione fallisce tentando un download. Il piano non possiede un preload riproducibile né un percorso di bootstrap conforme al YAML chiuso."
    impact: "La 08 non può confrontare CPU/GPU; download/cache differenti possono falsare i tempi."
    affected: ["spikes/compose.yaml","spikes/docker/runner.Dockerfile","PLAN-010-08"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-02-PLAN.md",".planning/phases/01-spike-tecnici/01-RESEARCH.md"],"details":"Riga 86: docling `quay.io/docling-project/docling-serve-cu128:v1.37.0` e docling-cpu `quay.io/docling-project/docling-serve-cpu:v1.37.0` (tag CPU [NV] RESEARCH §11.7: se il pull fallisce, usare il tag CPU esistente più vicino e a RESEARCH §2: pesi inclusi [S], se mancanti primo avvio con rete normale [NV]."}
    mitigation: "Definire ora una preparazione dei pesi con manifest/versioni e cache montata, seguita da prova con rete interna; se l'immagine li include già, dimostrarlo con un lavoro reale e cache pulita. Non riaprire la rete durante la misura."
    test_hint: "Con cache vuota e rete parse isolata, convertire una pagina-tabella con entrambe le immagini esatte; una richiesta di rete esterna deve dare un fallimento diagnostico, non una misura valida."
    test_required: true
  - id: "RISK-004"
    severity: "MEDIUM"
    likelihood: "MEDIUM"
    category: "concurrency"
    title: "Nome Compose e risorse globali condivise senza prenotazione della sessione"
    scenario: "Tutti i worktree usano name: cka-spikes e gli stessi volumi e nomi dei container. W20 controlla docker ps e poi avvia il lavoro, senza acquisire una prenotazione atomica."
    trigger: "Due esecutori superano insieme il controllo o un piano esegue down mentre l'altro ha avviato un servizio."
    failure: "Le verifiche non impediscono una race fra check e avvio; l'ambiente di una misura può essere sostituito o arrestato da un altro worktree."
    impact: "Risultati non attribuibili a una singola revisione, interruzioni e attese indefinite."
    affected: ["spikes/compose.yaml","spikes/tools/gpu-check.ps1","spikes/tools/hw-manifest.sh",".planning/phases/01-spike-tecnici/01-WIRING.md"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-02-PLAN.md",".planning/phases/01-spike-tecnici/01-WIRING.md"],"details":"Riga 34: name: cka-spikes\"     - path: \"spikes/tools/gpu-check.ps1\"       provides: \"gate GPU di wave 0\"     - path: \"spikes/results/gpu-check.txt\"       provides: \"esito del gate GPU\"       contains: \"VERDETTO:\"     - path: \"spi"}
    mitigation: "Usare una prenotazione host della sessione di misura con proprietario/run_id e cleanup che tocchi solo i container posseduti; attribuire al manifest progetto Compose e immagine effettivamente usati. Le onde restano valide, la prenotazione ne rende verificabile l'esclusione."
    test_hint: "Avviare due richieste di sessione insieme: una sola acquisisce la risorsa; interrompere il proprietario e verificare rilascio e cleanup senza down dell'ambiente altrui."
    test_required: true
external_research: 
  - id: "RESEARCH-001"
    linked_risk: "RISK-001"
    question: "Con la versione Docker Compose realmente installata su Windows 11/Docker Desktop, un servizio con profiles [s02,gpu] è abilitato dall'unione oppure dall'intersezione? Verificare con fonti ufficiali e una matrice config --services/up con servizi espliciti la combinazione dei nove servizi di PLAN-010-02, inclusi worker/paddle i cui Dockerfile ancora non esistono, e la selezione esclusiva CPU/GPU senza avviare TEI per errore."
    blocking: true
  - id: "RESEARCH-002"
    linked_risk: "RISK-003"
    question: "Le immagini docling-serve-cu128:v1.37.0 e docling-serve-cpu:v1.37.0 includono tutti i pesi necessari a table_mode=accurate, do_ocr=false? Quali artifact/revisioni, directory cache e procedura ufficiale di preload permettono conversione senza egress fin dal primo job? Separare presenza del tag da disponibilità offline dei modelli."
    blocking: false
```

## Condizioni per una nuova valutazione

Correggere o dimostrare la mitigazione dei rischi HIGH con le verifiche indicate, riconciliare i contratti con i piani proprietari senza modifiche silenziose ai file chiusi e aggiornare versione/impronta del PLAN quando cambia. Le ricerche blocking devono essere chiuse con fonti e versione prima di attestare la rispettiva garanzia. I MEDIUM richiedono mitigazione o decisione esplicita nella sede prevista dal progetto; questo audit non concede accettazioni.

Verdetto finale: **REVISE**