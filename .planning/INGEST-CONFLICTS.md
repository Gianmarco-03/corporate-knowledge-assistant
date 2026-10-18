## Conflict Detection Report

Generato da gsd-doc-synthesizer il 2026-10-18 (MODE: new). Precedenza: ADR > SPEC > PRD > DOC. ADR LOCKED: docs/adr/0001-system-boundaries-and-stack.md, docs/adr/0002-incremental-corpus-and-jev.md.
Le voci "Aperta"/"Proposta" di DEPENDENCIES.md sono decisioni aperte, non conflitti: sono elencate in .planning/intel/decisions.md (sezioni 4 e 5).

### BLOCKERS (0)

Nessun blocker.
- Nessuna contraddizione LOCKED vs LOCKED: ADR-0001 e ADR-0002 sono coerenti (source: docs/adr/0001-system-boundaries-and-stack.md, docs/adr/0002-incremental-corpus-and-jev.md).
- Nessun documento UNKNOWN a bassa confidenza: tutti i 15 documenti sono classificati con confidenza high tramite manifest.
- Nessun ciclo nel grafo dei riferimenti: archi interni README.md -> {docs/data/corpus-strategy.md, docs/architecture/architecture.md, docs/architecture/interfaces-and-concurrency.md, DEPENDENCIES.md} e docs/architecture/architecture.md -> docs/architecture/interfaces-and-concurrency.md; profondità massima 2. I riferimenti ai template in 00-shared-templates sono esterni al set ingerito.

### WARNINGS (1)

[WARNING] Varianti di accettazione concorrenti per REQ-in-app-viewer (FR-007)
  Found: docs/product/requirements.md (FR-007) richiede "la citation apre versione e pagina corrette con highlight se disponibile"; README.md (obiettivo MVP, punto 6) richiede di "aprire il documento nell'app alla pagina e al passaggio citato", senza la clausola "se disponibile" e senza citare la versione.
  Impact: Entrambi sono PRD con la stessa precedenza. Con la variante A l'evidenziazione del passaggio è un miglioramento opzionale (fallback a livello di pagina accettabile, ad esempio per PDF scannerizzati o DOCX); con la variante B il posizionamento a livello di passaggio è un criterio di accettazione obbligatorio per l'MVP, e questo vincola parser/OCR (coordinate), chunking con metadati posizionali e viewer PDF.js. La sintesi non può scegliere senza perdere l'intento. Contesto tecnico: docs/architecture/architecture.md richiede "citazioni riproducibili fino a versione, pagina e posizione", il che rende fattibile ma non obbligatoria la variante B.
  → Scegliere una variante (ad esempio "versione e pagina obbligatorie; passaggio evidenziato quando il parser fornisce coordinate, altrimenti fallback alla pagina"), oppure dividere in due requisiti (REQ-viewer-page per l'MVP, REQ-viewer-passage-highlight per M2/M3) prima di passare al roadmapper. Entrambe le varianti sono conservate in .planning/intel/requirements.md.

### INFO (10)

[INFO] Risolto automaticamente: ADR > SPEC sul ruolo di JEV nel triage dei documenti
  Note: docs/data/corpus-strategy.md (SPEC) assegna a JEV un "semantic triage" dei documenti scoperti online (classificare tipo e pertinenza, segnalare nuove versioni o candidati irrilevanti, rilevare segnali di contenuto sospetto o prompt injection, dare priorità alla revisione); README.md (PRD) parla di "triage opzionale di pagine e immagini". docs/adr/0002-incremental-corpus-and-jev.md (LOCKED) elenca come usi di JEV solo query validation, reranking, evidence sufficiency, contradiction detection, citation validation e triage opzionale delle immagini. È un'estensione, non una contraddizione: le esclusioni di corpus-strategy (autenticità/licenza, autorizzazioni/classificazione finale, malware, deduplicazione esatta, pubblicazione di contenuti riservati) coincidono con l'elenco "non usare JEV per" dell'ADR, e BR-002 (docs/product/requirements.md) mantiene JEV come fonte di soli segnali. L'ADR prevale: il triage è solo consultivo e la decisione resta all'approval policy deterministica. Consigliato un emendamento di ADR-0002 per includere esplicitamente il triage dei documenti.

[INFO] Risolto automaticamente: ADR > SPEC sull'isolamento del parser/OCR
  Note: docs/architecture/interfaces-and-concurrency.md (SPEC) prevede parsing/OCR tramite "spawn_blocking o processi isolati" e uno spike "in-process oppure worker isolato". docs/adr/0001-system-boundaries-and-stack.md (LOCKED) stabilisce che "il worker di ingestion resta parte del Document Service". L'ADR prevale: qualsiasi processo o worker isolato è una modalità di esecuzione del Document Service (come conferma docs/architecture/architecture.md), non un quarto servizio né un nuovo bounded context.

[INFO] Risolto automaticamente: ADR > SPEC sul reranker
  Note: DEPENDENCIES.md (SPEC) marca "Reranker baseline" come Aperta (confronto tra JEV, no-rerank e cross-encoder). docs/adr/0002-incremental-corpus-and-jev.md (LOCKED) stabilisce l'uso di JEV per il reranking. L'ADR prevale: JEV è l'adapter di reranking del DecisionEngine; resta aperta solo la scelta della baseline di confronto/fallback (docs/architecture/interfaces-and-concurrency.md prevede adapter JEV, fake deterministico e baseline). Registrata come A-06 in .planning/intel/decisions.md.

[INFO] Decisione aperta sui container chiusa da una decisione dell'utente
  Note: DEPENDENCIES.md ("Container e ambiente locale — decisione aperta"), docs/architecture/architecture.md ("Docker/Compose oppure manifest compatibili anche con Podman") e le conseguenze di docs/adr/0001-system-boundaries-and-stack.md la lasciavano aperta. Decisione utente del 2026-10-18 (comunicata dall'orchestratore): Docker Desktop + Docker Compose. La direzione proposta in DEPENDENCIES.md diventa il riferimento; la compatibilità con Podman non è più un criterio. Registrata come U-01 in decisions.md. Anche la metodologia GSD+ECC (CLAUDE.md) è stata ratificata dall'utente, ma non ha impatto sul prodotto.

[INFO] Il contratto HTTP presuppone JWT, che è ancora Proposta
  Note: docs/architecture/interfaces-and-concurrency.md (SPEC) definisce GET /.well-known/jwks.json e POST /api/v1/auth/refresh, e docs/architecture/architecture.md richiede la verifica locale di firma/issuer/audience/scadenza; DEPENDENCIES.md (stessa precedenza SPEC) marca "JWT firmati asimmetricamente" come Proposta. Non c'è contraddizione: il contratto è coerente con la proposta. Se lo spike la rifiutasse, C-02 andrebbe rivisto.

[INFO] Target Time To Queryable (README) e NFR-007 sono complementari
  Note: docs/product/requirements.md (NFR-007) richiede che il Time To Queryable sia "misurato su hardware noto"; README.md (stessa precedenza PRD) fornisce target p95 iniziali (upload ack <= 500 ms; PDF digitale 25 pagine/10 MB <= 15 s; 100 pagine <= 60 s; scannerizzato 25 pagine <= 45 s; job fallito visibile <= 5 s) "da riesaminare dopo una baseline". Nessuna divergenza di accettazione: i target sono trattati come iniziali e rivedibili (.planning/intel/constraints.md C-31).

[INFO] Tensione tra l'upload ack a 500 ms e la persistenza prima della risposta
  Note: README.md fissa l'upload acknowledgment a p95 <= 500 ms; docs/architecture/interfaces-and-concurrency.md richiede che il job venga restituito solo "dopo persistenza sicura dell'originale e dell'outbox", con upload in streaming fino a 10 MB. Le due richieste sono compatibili se l'ack si misura lato server dalla ricezione completa del body. Da chiarire nel piano di benchmark; non è un conflitto di decisione.

[INFO] Alias jev-latest e riproducibilità dell'evaluation
  Note: DEPENDENCIES.md configura il modello JEV con un alias, inizialmente "jev-latest"; docs/data/corpus-strategy.md richiede di poter attribuire le regressioni anche alla "versione JEV", e ADR-0002 richiede la calibrazione delle soglie su dati etichettati. Le due cose sono compatibili se ogni run di evaluation registra la versione effettiva del modello risolta dall'alias. Registrata come Q-07 in .planning/intel/context.md.

[INFO] I DOC stub rimandano a template esterni (lacune, non conflitti)
  Note: docs/product/project-charter.md, docs/architecture/api-contract.md, docs/architecture/data-model.md, docs/security/threat-model.md, docs/quality/test-plan.md, docs/quality/llm-evaluation.md e deliverables/checklist.md sono stub che rimandano a C:\Users\Gianmarco\Documents\Portfolio projects\00-shared-templates\ (i template esistono). Il loro contenuto parziale è coerente con SPEC e ADR. Le lacune sono elencate in .planning/intel/context.md (topic "Lacune").

[INFO] Domande di prodotto aperte emerse dalla sintesi
  Note: Nessun documento definisce i permessi per ruolo oltre all'assegnazione dei ruoli da parte di admin (FR-002 in docs/product/requirements.md), lo schema di classificazione dei documenti richiesto da docs/quality/test-plan.md, chi può consultare l'audit (README.md), né se le conversazioni (entità Conversation in docs/architecture/data-model.md) fanno parte dell'MVP. Si tratta di lacune, non di contraddizioni. Sono registrate come Q-01..Q-06 in .planning/intel/context.md e vanno risolte durante la pianificazione delle fasi che le toccano.
