# Roadmap: Corporate Knowledge Assistant (CKA)

**Modifica:** Claude (gsd-roadmapper) · 2026-10-18 · creazione della roadmap da ingest (12 fasi, 37/37 requisiti)
**Modifica:** Claude (gsd-roadmapper) · 2026-10-18 · revisione per la decisione utente U-04 "provider-first, insiemi di file chiusi": la vecchia Fase 1 si divide in Spike (1), Fondazione (2) e due fasi Provider (3, 4); le fasi consumer passano a 5-15 (15 fasi, 37/37 requisiti)

## Overview

Il percorso chiude prima le scelte tecniche con spike misurati (Fase 1), poi costruisce per intero le parti comuni (Fase 2: workspace, composition root, config, pool DB, errori, telemetria, router, migrazioni, CI, Compose, hook vuoti pre-registrati per ogni fase successiva) e tutti i provider esterni dietro le loro porte Rust, completi di fake e contract test (Fasi 3 e 4). Solo dopo arrivano le fasi consumer, che usano i provider tramite le porte senza toccarne i file: il vertical slice M1 (identità, upload e indicizzazione, retrieval autorizzato, risposta citata, viewer), poi M2 (RBAC end-to-end e guardrail, audit, corpus incrementale con fonti online in quarantena), M3 (evaluation, osservabilità e hardening) e M4 (demo e case study). Ogni fase indica la criticità: le fasi critiche ricevono l'audit dei rischi GPT nel loop.

## Strategia di consegna

Decisione utente U-04 (2026-10-18): provider-first e insiemi di file chiusi per fase. Gli insiemi chiusi riguardano solo i file di codice e configurazione: i documenti Markdown si possono modificare da qualsiasi fase (decisione utente 2026-10-18).

**Ordine dei verbi del loop (diverso dall'ordine numerico di esecuzione):**

1. `discuss` + `plan` della **Fase 1 (Spike)**, poi `execute` + `verify` della Fase 1 **in parallelo** al `discuss` di tutte le altre fasi, 2-15 (decisione utente 2026-10-18, emendamento di U-04: gli spike sono il percorso critico e il loro perimetro è già fissato dal CONTEXT della Fase 1). Misure e ADR, codice usa-e-getta fuori dall'albero di prodotto. Una misura richiesta da un discuss successivo e non coperta dagli spike diventa un piano aggiuntivo della Fase 1 o uno spike nella fase interessata.
2. Le decisioni di prodotto che vincolano i provider (es. Q-02 classificazione, Q-04 ownership dell'audit, Q-05 multi-turno, Q-07 versione JEV, prima fonte online) si chiudono nei discuss 2-15 prima che i provider siano pianificati. Il discuss delle Fasi 2-4 si riapre solo se le misure dello spike ne cambiano le premesse.
3. `plan` delle **Fasi 2, 3, 4** (Fondazione e Provider): fissano porte, DTO, hook e perimetri nei rispettivi `0X-CONTRATTO.md`.
4. `plan` delle **fasi consumer 5-15**, ciascuna contro i contratti e le trait-porta delle Fasi 2-4, mai contro codice provvisorio.
5. **Riconciliazione dei piani provider**: ogni operazione di porta usata da un piano consumer esiste nel piano provider, e ogni operazione provider ha almeno una fase consumer pianificata che la usa (nessuna operazione speculativa). Un delta riapre il piano provider (`plan_version` +1, re-gate) e aggiorna la tabella "Tracciabilità provider → consumer".
6. `execute` + `verify` in ordine numerico: 2 → 3 → 4 → 5 → … → 15 (le fasi 3 e 4 sono indipendenti tra loro e possono procedere in parallelo).

**Regola degli insiemi di file chiusi:**

- Ogni fase dichiara nel proprio `0X-CONTRATTO.md` l'insieme chiuso dei file che possiede (percorsi o glob). Le fasi successive non li modificano.
- Gli hook pre-registrati dalla Fase 2 per la fase N (punti di montaggio delle rotte, slot del composition root, voci del route registry di `apps/web`, moduli vuoti) appartengono alla fase N, che li riempie; nessun'altra fase li tocca.
- **Whitelist** dei file condivisi, modificabili solo nel modo indicato:
  - append-only: nuovi file di migrazione nella fascia di numerazione della fase (convenzione in `.planning/WIRING-MAP.md`); sezioni `[dependencies]` dei `Cargo.toml` e `dependencies` di `apps/web/package.json`; `.env.example`; `.planning/WIRING-MAP.md`; una sezione per fase in `docs/architecture/data-model.md`, `docs/architecture/api-contract.md` e nella sezione comandi di `CLAUDE.md`;
  - rigenerati: `Cargo.lock`, `apps/web/package-lock.json`, documenti OpenAPI generati e client TypeScript generato;
  - valori (non schema) dei file di configurazione dei parametri, per la calibrazione della Fase 14.
- Un difetto trovato in un file chiuso di una fase precedente si corregge con una fase decimale di gap-closure (es. 7.1) che riapre esplicitamente quel perimetro, mai con una modifica silenziosa dentro un'altra fase.

## Phases

**Phase Numbering:**

- Integer phases (1, 2, 3): lavoro pianificato della milestone
- Decimal phases (2.1, 2.2): inserimenti urgenti o gap-closure su un perimetro chiuso (marcati INSERTED)

- [ ] **Phase 1: Spike tecnici** - Chiude con misure documentate le decisioni aperte che bloccano porte e provider; threat model compilato (M0, critica)
- [ ] **Phase 2: Fondazione** - Workspace, composition root, config, pool DB, errori, telemetria, router, migrazioni, shell di apps/web, Compose, CI, hook vuoti per tutte le fasi (M0/M1, normale)
- [ ] **Phase 3: Provider di ingestion e indice** - DocumentStore, DocumentParser con OCR, Chunker, Embedder, VectorIndex, DocumentSource completi, con fake e contract test (M1, critica)
- [ ] **Phase 4: Provider di decisione, risposta e trasversali** - DecisionEngine JEV con fake e baseline, AnswerGenerator, AuditSink, NoOpCache, con contract test (M1, critica)
- [ ] **Phase 5: Identità e accesso** - Login email/password, Argon2id, JWT/JWKS, gestione ruoli da admin (M1, critica)
- [ ] **Phase 6: Upload e indicizzazione** - Upload in streaming, job osservabile, pipeline di ingestion sulle porte, pubblicazione atomica idempotente (M1, critica)
- [ ] **Phase 7: Retrieval autorizzato** - Ricerca deny-by-default dentro il Document Service con AccessPolicy deterministica (M1, critica)
- [ ] **Phase 8: Risposta citata e rifiuto** - Flusso query RAG sulle porte DecisionEngine e AnswerGenerator, citazioni a versione immutabile, rifiuto motivato (M1, normale)
- [ ] **Phase 9: Viewer in-app delle citazioni** - La citazione apre versione e pagina, con evidenziazione quando ci sono coordinate, e ricontrollo di accesso (M1, critica)
- [ ] **Phase 10: RBAC end-to-end e guardrail** - Matrice ruolo × classificazione anti-leakage, revoca durante query, fault injection e fake ostile (M2, critica)
- [ ] **Phase 11: Audit end-to-end e tracciabilità** - Audit aggregato, consultazione per ruolo, request_id/trace/versione (M2, critica)
- [ ] **Phase 12: Corpus incrementale e scoperta online** - Nuove versioni senza interruzioni, connettore online, quarantena e approvazione (M2, critica)
- [ ] **Phase 13: Suite di evaluation** - Evaluation corpus versionato e metriche RAG riproducibili con oracoli indipendenti (M3, normale)
- [ ] **Phase 14: Osservabilità, prestazioni e hardening** - Trace distribuita, Time To Queryable misurato, backpressure e race condition (M3, normale)
- [ ] **Phase 15: Deployment, demo e case study** - Demo end-to-end riproducibile da clone pulito e case study (M4, normale)

## Phase Details

### Phase 1: Spike tecnici

**Goal**: Le decisioni tecniche che fissano porte e provider sono chiuse con misure documentate, e il threat model guida le scelte su upload e parsing di file non fidati
**Milestone**: M0 (chiusura lacune)
**Criticità**: critica (parsing di file non fidati e upload: S-01, S-02, A-05, A-10 fissano il design dell'ingestion)
**Depends on**: Nothing (first phase)
**Requirements**: PLAT-07
**Success Criteria** (what must be TRUE):

  1. S-01: un multipart in streaming su Axum è misurato con file da 10 MB e oltre; la memoria di picco resta costante rispetto alla dimensione del file, e l'esito è registrato con le misure.
  2. S-02, A-05, A-10: i candidati parser/OCR sono misurati su PDF digitali, PDF scannerizzati, DOCX e Markdown (testo, tabelle, numero di pagina, coordinate dei passaggi) e su file ostili (corrotti, cifrati, decompression bomb); sono decisi il parser, l'isolamento in-process o in processo separato e la definizione di "pagina" per DOCX e Markdown nel viewer.
  3. S-03 e A-03: `pgvector` è misurato sul corpus iniziale con il modello di embedding scelto (recall, latenza, inserimento durante le query, ricerca con predicato di autorizzazione nella stessa query); indice e metrica sono decisi.
  4. S-04, A-09, S-05, A-04, A-06: la forma della chiamata JEV (batch o concorrente per candidato) è misurata, la cancellazione verso provider HTTP è verificata, e sono scelti LLM generativo e baseline di reranking; A-02 (storage degli originali) è deciso; S-07 produce le firme Rust candidate di tutte le porte.
  5. Ogni esito è registrato in un ADR o in `DEPENDENCIES.md` con le misure e l'hardware usato; il threat model è compilato con mitigazioni mappate ai requisiti; il codice degli spike sta in `spikes/`, fuori dal workspace di prodotto e dalla CI, e nessun file di prodotto lo importa.

**Decisioni da chiudere nel discuss**: A-02, A-03, A-04, A-05, A-06, A-09, A-10; S-01, S-02, S-03, S-04, S-05, S-07; Proposte `pgvector` e reqwest+Rustls; corpus e hardware degli spike
**Perimetro file**: `spikes/**` (usa-e-getta), nuovi ADR in `docs/adr/`, `DEPENDENCIES.md`, `docs/security/threat-model.md`
**Plans**: TBD

### Phase 01.5: Gap-closure prescan: tetto delle immagini DCT sul SOF del JPEG (INSERTED)

**Goal:** [Urgent work - to be planned]
**Requirements**: TBD
**Depends on:** Phase 01
**Plans:** 0 plans

Plans:

- [ ] TBD (run /gsd:plan-phase 01.5 to break down)

### Phase 01.4: Gap-closure driver di misura di 01-07: pulizia di budget sul percorso d'errore e overhead in-process completo (INSERTED)

**Goal:** [Urgent work - to be planned]
**Requirements**: TBD
**Depends on:** Phase 1
**Plans:** 2/2 plans complete

Plans:

- [x] TBD (run /gsd:plan-phase 01.4 to break down) (completed 2026-10-23)

### Phase 01.3: Gap-closure compose.yaml: /work del worker scrivibile dall'utente 10001 (INSERTED)

**Goal:** [Urgent work - to be planned]
**Requirements**: TBD
**Depends on:** Phase 1
**Plans:** 9/18 plans executed

Plans:

- [x] TBD (run /gsd-plan-phase 01.3 to break down) (completed 2026-10-21)

### Phase 01.2: Gap-closure hist.rs: il Recorder delle latenze non satura sopra 2 ms (INSERTED)

**Goal:** [Urgent work - to be planned]
**Requirements**: TBD
**Depends on:** Phase 1
**Plans:** 1/1 plans complete

Plans:

- [x] TBD (run /gsd-plan-phase 01.2 to break down) (completed 2026-10-21)

### Phase 01.1: Gap-closure session.sh: segreti di spikes.env prevalgono su env host (INSERTED)

**Goal:** [Urgent work - to be planned]
**Requirements**: TBD
**Depends on:** Phase 1
**Plans:** 6/18 plans executed

Plans:

- [x] TBD (run /gsd-plan-phase 01.1 to break down) (completed 2026-10-21)

### Phase 2: Fondazione

**Goal**: Lo sviluppatore avvia l'intero sistema scheletro da un clone pulito con un solo comando, la CI verifica ogni push, e ogni fase successiva trova già pronti i propri punti di innesto vuoti, senza dover modificare file comuni
**Milestone**: M0/M1 (prerequisiti)
**Criticità**: normale
**Depends on**: Phase 1
**Requirements**: PLAT-02, PLAT-03, PLAT-06
**Success Criteria** (what must be TRUE):

  1. Da un clone pulito, `docker compose up` avvia PostgreSQL con `pgvector` (e lo storage degli originali se A-02 lo richiede), i tre servizi Rust con health check verdi e il frontend Vite raggiungibile nel browser; le immagini sono non-root e senza segreti.
  2. La CI GitHub Actions esegue a ogni push fmt/clippy/test del workspace Cargo (3 servizi + `crates/contracts` + `crates/telemetry`), lint/tsc/vitest di `apps/web` e almeno un contract test OpenAPI per servizio, ed è verde.
  3. Ogni servizio applica solo le proprie migrazioni con credenziali proprie; un test dimostra che l'utente DB di un servizio non può leggere le tabelle di un altro.
  4. Ogni servizio parte dal proprio composition root con config tipizzata, pool DB limitato, modello di errore `code`/`message`/`request_id`, propagazione di `request_id` e trace context, telemetria e layer di backpressure configurabile: una rotta di prova restituisce `request_id` ed errori nel formato comune.
  5. Ogni fase 3-15 ha i propri hook vuoti pre-registrati (montaggio rotte per servizio, slot dei provider nel composition root, fascia di migrazioni, voce nel route registry di `apps/web`), elencati in `.planning/WIRING-MAP.md`; un controllo di perimetro conferma che riempirli non richiede modifiche a file di Fondazione fuori whitelist.

**Decisioni da chiudere nel discuss**: A-01 (tooling OpenAPI); conferma delle Proposte di runtime (Tokio, axum/tower, sqlx, serde, thiserror/anyhow, tracing+OpenTelemetry, Vite, React Router, TanStack Query, client OpenAPI generato, Vitest, Playwright); collocazione delle primitive trasversali (idempotency key, outbox append-only, worker con lease): raccomandata in Fondazione
**Perimetro file**: `Cargo.toml` di workspace, `crates/contracts/**`, `crates/telemetry/**`, `services/*/src/{main,config,db,error,router}*` e cartelle degli hook vuoti, `apps/web` (shell, route registry, stile di base), `infra/`, `docker-compose*.yml`, `.github/workflows/**`
**Plans**: TBD
**UI hint**: yes

### Phase 3: Provider di ingestion e indice

**Goal**: Ogni provider del lato ingestion e indice funziona per intero dietro la propria porta, con un fake e una suite di contract test comune, così che le fasi consumer lo usino senza toccarne il codice
**Milestone**: M1 (prerequisiti)
**Criticità**: critica (il parser elabora input non fidati; il connettore online raggiunge la rete)
**Depends on**: Phase 2
**Requirements**: Nessuno (fase provider: abilita le fasi consumer, vedi "Tracciabilità provider → consumer")
**Success Criteria** (what must be TRUE):

  1. Ogni operazione di `DocumentStore` (`put`, `get` con range), `DocumentParser` (`parse`), `Chunker` (`chunk`), `Embedder`, `VectorIndex` (`publish_version`, `search`) e `DocumentSource` (`discover`, `fetch`) supera la stessa suite di contract test sia sull'adapter reale sia sul fake.
  2. Il parser, sui file ostili dello spike (corrotti, cifrati, oltre i limiti, decompression bomb), termina con un errore tipizzato entro limiti di tempo e memoria senza far cadere il processo ospite; sui file validi restituisce testo con pagina e, quando disponibili, coordinate.
  3. `VectorIndex.search` applica il predicato di autorizzazione ricevuto dal chiamante nella query stessa (nessun post-filtro), e `publish_version` interrotto a metà non lascia nulla di visibile.
  4. `DocumentSource` raggiunge solo host in allowlist (protezione SSRF) e rispetta limiti di dimensione e tempo; `DocumentStore` e `Embedder` lavorano in streaming e in batch bounded.
  5. La tabella "Tracciabilità provider → consumer" lega ogni operazione ad almeno una fase consumer pianificata, e `crate-boundary-check` conferma che nessun tipo SDK o DB attraversa una porta.

**Decisioni da chiudere nel discuss**: conferma delle firme S-07 per le porte di questa fase (in particolare l'embedding della query per la Fase 7); forma del predicato di autorizzazione di `VectorIndex` (da Q-02); vincoli di `DocumentSource` dalla prima fonte scelta nel discuss della Fase 12
**Perimetro file**: moduli `ports` e `adapters` delle sei porte nel Document Service, slot provider pre-registrati per la Fase 3, relativi contract test e fake
**Plans**: TBD

### Phase 4: Provider di decisione, risposta e trasversali

**Goal**: Il decision layer JEV, il generatore di risposte, l'audit sink e la cache funzionano per intero dietro le rispettive porte, con fake deterministici e baseline, e nessun contenuto oltre quello passato dal chiamante attraversa il confine verso JEV o LLM
**Milestone**: M1 (prerequisiti)
**Criticità**: critica (confine JEV/LLM della regola no-leak)
**Depends on**: Phase 2
**Requirements**: PLAT-01
**Success Criteria** (what must be TRUE):

  1. Le quattro operazioni di `DecisionEngine` (`validate_query`, `rank`, `assess_evidence`, `validate_citations`) superano la stessa suite di contract test sull'adapter JEV (`POST /v1/systemone`, senza SDK), sul fake deterministico e sulla baseline scelta in A-06; l'adapter restituisce solo segnali e registra la versione JEV effettiva (Q-07).
  2. Gli adapter JEV e LLM applicano deadline, cancellazione (S-05), concorrenza limitata (A-09), budget di retry e circuit breaker: con timeout, errori 5xx o risposte malformate restituiscono un errore tipizzato, mai un segnale positivo.
  3. Un test dimostra che gli adapter inviano solo i dati ricevuti come argomento e che log e telemetria non contengono testo di chunk né prompt.
  4. `AnswerGenerator.generate` produce una risposta strutturata (claim con riferimenti ai chunk forniti) validata contro schema; un output malformato è un errore tipizzato.
  5. `AuditSink.append` è append-only e idempotente per `event_id`; `NoOpCache` rispetta il contratto di `Cache` (`get`, `put`, `invalidate`); `crate-boundary-check` su tutti gli adapter delle Fasi 3 e 4 conferma PLAT-01, e ogni operazione è nella tabella di tracciabilità.

**Decisioni da chiudere nel discuss**: conferma delle firme S-07 per le porte di questa fase; Proposta NoOpCache; Proposta audit/outbox append-only per `AuditSink` (vincolata da Q-04, chiusa nel discuss della Fase 11); campi della risposta strutturata (vincolati da Q-05)
**Perimetro file**: moduli `ports` e `adapters` di `DecisionEngine`, `AnswerGenerator`, `Cache` nel RAG Service e di `AuditSink` nei servizi indicati da Q-04, slot provider pre-registrati per la Fase 4, relativi contract test, fake e baseline
**Plans**: TBD

### Phase 5: Identità e accesso

**Goal**: Un utente creato dall'admin accede all'applicazione con email e password, e gli altri servizi riconoscono la sua identità e i suoi ruoli senza chiamare l'Identity Service
**Milestone**: M1
**Criticità**: critica (identità e autenticazione)
**Depends on**: Phase 2, Phase 4
**Requirements**: AUTH-01, AUTH-02, AUTH-03
**Success Criteria** (what must be TRUE):

  1. Un utente creato dall'admin accede dalla pagina di login web con email e password e vede la propria identità e i propri ruoli (`GET /api/v1/auth/me`).
  2. Credenziali errate e utente inesistente producono lo stesso errore generico, senza rivelare quale dato è sbagliato; login riusciti e falliti sono emessi tramite `AuditSink`.
  3. L'admin assegna e revoca i ruoli `admin`, `manager`, `employee`, `guest`; un non-admin che ci prova riceve un deny.
  4. Le password esistono solo come hash Argon2id con parametri versionati; il refresh ruota il token, il logout revoca la sessione.
  5. Document e RAG Service verificano localmente firma, issuer, audience e scadenza del token tramite JWKS, negli hook di autenticazione pre-registrati, e rifiutano un token alterato o scaduto.

**Decisioni da chiudere nel discuss**: conferma della Proposta "JWT firmati asimmetricamente" (se rifiutata, rivedere C-02); storage dei token nel browser (vincolo `security-scanner`); forma di propagazione dell'identità tra servizi (usata dalla Fase 7)
**Perimetro file**: moduli auth/users dell'Identity Service, hook di verifica token in Document e RAG Service, feature auth di `apps/web`, fascia migrazioni della Fase 5
**Plans**: TBD
**UI hint**: yes

### Phase 6: Upload e indicizzazione

**Goal**: Un utente autorizzato carica un documento con ACL esplicita e lo vede diventare queryable, con una versione pubblicata atomicamente e chunk che conservano pagina e posizione, usando i provider della Fase 3 solo tramite le porte
**Milestone**: M1
**Criticità**: critica (ingestion: upload, orchestrazione del parsing di file non fidati)
**Depends on**: Phase 3, Phase 5
**Requirements**: DOC-01, DOC-02, PLAT-05
**Success Criteria** (what must be TRUE):

  1. Un utente autorizzato carica PDF, DOCX o Markdown dalla UI con ACL esplicita, riceve subito un job e vede le fasi dell'ingestion fino a `queryable` oppure `failed` con diagnostica.
  2. Un file non valido (tipo non ammesso, oltre 10 MB, PDF corrotto o cifrato) è rifiutato o termina in `failed`, e non diventa mai queryable.
  3. Un job riuscito pubblica la versione in una sola transazione: un test che interrompe il worker a metà non lascia mai un indice parziale visibile; la pipeline lavora per versione, così che una nuova versione di un documento esistente segua lo stesso percorso.
  4. Ripetere lo stesso upload o rieseguire un job o un evento non duplica contenuto fisico, chunk né versioni.
  5. I chunk pubblicati conservano documento, versione, pagina e, quando il parser le fornisce, le coordinate del passaggio; upload, modifiche di ACL e fasi di ingestion sono emessi tramite `AuditSink`.

**Decisioni da chiudere nel discuss**: Q-01 (permessi per ruolo), Q-02 (schema di classificazione; vincola il predicato di `VectorIndex` nella Fase 3), Q-06 (l'upload manuale passa da una quarantena transitoria?)
**Perimetro file**: moduli upload, job e pipeline di ingestion del Document Service, worker, feature upload di `apps/web`, fascia migrazioni della Fase 6
**Plans**: TBD
**UI hint**: yes

### Phase 7: Retrieval autorizzato

**Goal**: Il RAG Service ottiene dal Document Service solo candidati che l'utente può vedere, e nessun chunk negato esce mai dal Document Service
**Milestone**: M1
**Criticità**: critica (RBAC + retrieval autorizzato)
**Depends on**: Phase 6
**Requirements**: SEC-01, SEC-02
**Success Criteria** (what must be TRUE):

  1. Una ricerca per conto di un utente restituisce solo chunk di versioni pubblicate di documenti accessibili al suo ruolo; un utente senza permessi riceve zero risultati, senza indizi sull'esistenza dei documenti negati.
  2. L'`AccessPolicy` deterministica costruisce il predicato che `VectorIndex.search` applica nella query stessa, dentro il Document Service: test negativi dimostrano che nessun chunk negato compare in risposta, log o telemetria.
  3. `POST /internal/v1/retrieval/search` senza identità utente valida propagata risponde deny (deny-by-default), e il deny è emesso tramite `AuditSink`.
  4. `POST /internal/v1/documents/{id}/authorize` restituisce la decisione corrente, che riflette subito una revoca di ACL.

**Decisioni da chiudere nel discuss**: parametri di retrieval (top-k, deadline interna) entro l'esito di S-03
**Perimetro file**: `AccessPolicy` e rotte `/internal/v1` di retrieval e authorize del Document Service, fascia migrazioni della Fase 7
**Plans**: TBD

### Phase 8: Risposta citata e rifiuto

**Goal**: L'utente pone una domanda e riceve una risposta in cui ogni claim è citato a una versione immutabile, oppure un rifiuto motivato
**Milestone**: M1
**Criticità**: normale
**Depends on**: Phase 4, Phase 7
**Requirements**: QRY-01, QRY-02, QRY-03, QRY-04, QRY-05
**Success Criteria** (what must be TRUE):

  1. L'utente pone una domanda dalla UI e riceve una risposta in cui ogni claim verificabile ha una citazione a documento, versione e pagina.
  2. Una domanda senza evidenza nei documenti autorizzati, o con evidenze contraddittorie, produce un rifiuto esplicito con motivazione, non una risposta.
  3. Il risultato salvato (`GET /api/v1/queries/{id}`) continua a puntare alla versione citata anche dopo la pubblicazione di una versione nuova.
  4. Con il fake `DecisionEngine` e con l'adapter JEV reale, i segnali (validazione query, rerank, sufficienza, validazione claim–citazione) influenzano il flusso, ma la decisione finale e il mapping citazione→chunk sono presi dal codice in modo deterministico.
  5. Ogni query ha una deadline esplicita e un ricontrollo di accesso prima di citazioni e link; con deadline scaduta o JEV/LLM in errore si applica la policy di fallback decisa (rifiuto o baseline), mai una risposta non validata; query e rifiuti sono emessi tramite `AuditSink`.

**Decisioni da chiudere nel discuss**: Q-05 (conversazioni multi-turno nell'MVP?); policy di fallback per guasto JEV/LLM (rifiuto, baseline locale o risposta degradata); uso della cache nel percorso query (`NoOpCache`)
**Perimetro file**: moduli query, evidence gate e citazioni del RAG Service, rotte `/api/v1/queries`, feature query di `apps/web`, fascia migrazioni della Fase 8
**Plans**: TBD
**UI hint**: yes

### Phase 9: Viewer in-app delle citazioni

**Goal**: Cliccando una citazione l'utente vede il documento alla versione e alla pagina citate, con il passaggio evidenziato quando possibile, solo se è ancora autorizzato (chiusura del vertical slice M1)
**Milestone**: M1
**Criticità**: critica (ricontrollo autorizzativo sul contenuto servito)
**Depends on**: Phase 8
**Requirements**: VIEW-01, VIEW-02
**Success Criteria** (what must be TRUE):

  1. Cliccando una citazione, il viewer in-app apre il documento alla versione e alla pagina citate.
  2. Se il parser ha fornito coordinate, il passaggio citato è evidenziato; altrimenti (PDF scannerizzati, DOCX) il viewer apre la pagina corretta e indica che l'evidenziazione non è disponibile.
  3. Se l'accesso è stato revocato o il documento rimosso dopo la risposta, aprire la citazione produce un deny e il contenuto non viene servito.
  4. Il viewer funziona anche con JEV e LLM non raggiungibili.
  5. Un test E2E Playwright percorre login → upload → queryable → domanda → risposta citata → viewer alla pagina citata.

**Decisioni da chiudere nel discuss**: conferma della Proposta PDF.js; resa di DOCX e Markdown entro la definizione di pagina decisa in S-02
**Perimetro file**: rotta `GET /api/v1/documents/{id}/content` del Document Service, feature viewer di `apps/web`, test E2E del vertical slice
**Plans**: TBD
**UI hint**: yes

### Phase 10: RBAC end-to-end e guardrail

**Goal**: È dimostrato, con test automatici, che nessun ruolo vede contenuti oltre la propria classificazione in nessun canale, e che guasti o input ostili non aggirano i controlli
**Milestone**: M2
**Criticità**: critica (RBAC + retrieval autorizzato)
**Depends on**: Phase 9
**Requirements**: SEC-03, SEC-04, SEC-05
**Success Criteria** (what must be TRUE):

  1. La matrice ruolo × classificazione gira in CI: per ogni combinazione, test positivi e negativi provano che nessun contenuto vietato compare in risposte, citazioni, viewer, log, cache o prompt verso JEV/LLM.
  2. Revocando un ruolo durante una query in corso, il ricontrollo prima della risposta rimuove le citazioni non più autorizzate o rifiuta.
  3. Con JEV o LLM in timeout o errore (fault injection end-to-end), il sistema rifiuta o degrada secondo la policy della Fase 8, con circuit breaker osservabile, senza mai saltare autorizzazione o policy.
  4. Sostituendo JEV con un fake ostile, autorizzazione, provenance e mapping delle citazioni producono le stesse decisioni: sono deterministici.
  5. I documenti con prompt injection del dataset di sicurezza sintetico non alterano le decisioni di accesso né fanno uscire contenuti negati.

**Decisioni da chiudere nel discuss**: composizione del dataset di sicurezza sintetico (C-26)
**Perimetro file**: suite di sicurezza (matrice, fault injection, fake ostile) e dataset sintetico; un difetto trovato in file di fasi precedenti apre una fase decimale di gap-closure
**Plans**: TBD

### Phase 11: Audit end-to-end e tracciabilità

**Goal**: Ogni azione rilevante per la sicurezza lascia una traccia ricostruibile, e un utente autorizzato può consultarla dall'applicazione
**Milestone**: M2
**Criticità**: critica (eventi di autenticazione e deny; consultazione filtrata per ruolo)
**Depends on**: Phase 10
**Requirements**: AUD-01, AUD-02, AUD-03
**Success Criteria** (what must be TRUE):

  1. Un test end-to-end verifica che login riusciti e falliti, upload, modifiche di ACL e ruoli, fasi di ingestion, query, rifiuti e deny, emessi dalle fasi 5-9 tramite `AuditSink`, arrivano nell'audit aggregato append-only.
  2. Un utente autorizzato consulta gli eventi dalla UI filtrando per tipo, utente e intervallo; un utente non autorizzato riceve deny.
  3. Ogni risposta e ogni evento di audit riportano `request_id`, trace id e versione del documento coinvolto, e permettono di ricostruire la catena query → evidenze → risposta.
  4. Il retry di un evento non produce duplicati (`event_id` univoco) e l'audit aggregato converge anche dopo il riavvio di un consumer.
  5. Gli eventi di audit non contengono password, token né contenuto di chunk negati.

**Decisioni da chiudere nel discuss**: Q-03 (chi consulta l'audit e con quali filtri), Q-04 (ownership dell'audit aggregato; vincola `AuditSink` nella Fase 4)
**Perimetro file**: proiezione dell'audit aggregato e rotta `GET /api/v1/audit-events` del RAG Service, feature audit di `apps/web`, fascia migrazioni della Fase 11
**Plans**: TBD
**UI hint**: yes

### Phase 12: Corpus incrementale e scoperta online

**Goal**: La knowledge base si aggiorna con nuove versioni e documenti scoperti online senza interrompere le query e senza pubblicare nulla che non abbia superato quarantena e approvazione
**Milestone**: M2
**Criticità**: critica (ingestion da fonti online non fidate)
**Depends on**: Phase 11
**Requirements**: DOC-03, ONL-01, ONL-02, ONL-03
**Success Criteria** (what must be TRUE):

  1. Caricando una nuova versione di un documento pubblicato, le query continuano a rispondere sulla versione precedente finché la nuova non è pubblicata atomicamente; dopo, citano la nuova.
  2. Il connettore, tramite `DocumentSource`, scopre in modo incrementale nuovi documenti da una fonte ufficiale (API, RSS o manifest), che entrano in quarantena con metadati di provenance completi (URL canonico, checksum, licenza, stato di provenance).
  3. Nessun documento scoperto online diventa queryable senza checksum/deduplicazione, policy di provenance e licenza, validazione del file e approvazione esplicita dalla UI di revisione; il triage JEV vi appare solo come segnale consultivo.
  4. Un candidato con URL valido ma metadati falsi, contenuto sospetto o file malformato resta in quarantena o viene rifiutato, con diagnostica visibile.

**Decisioni da chiudere nel discuss**: scelta della prima fonte ufficiale e della sua licenza (vincola `DocumentSource` nella Fase 3); emendamento di ADR-0002 per il triage JEV dei documenti (consigliato da INGEST-CONFLICTS.md)
**Perimetro file**: moduli di quarantena, provenance, revisione e nuove versioni del Document Service, scheduler del connettore, feature di revisione di `apps/web`, fascia migrazioni della Fase 12
**Plans**: TBD
**UI hint**: yes

### Phase 13: Suite di evaluation

**Goal**: La qualità di retrieval, risposte, citazioni e rifiuti è misurata con una suite riproducibile su un evaluation corpus versionato, i cui valori attesi provengono da fonti indipendenti dalla pipeline
**Milestone**: M3
**Criticità**: normale
**Depends on**: Phase 12
**Requirements**: EVAL-01, EVAL-02
**Success Criteria** (what must be TRUE):

  1. L'evaluation corpus è versionato e immutabile per release, separato dal knowledge corpus: i documenti scoperti online non vi entrano mai e ogni modifica produce una nuova versione del benchmark.
  2. Un comando (o `POST /internal/v1/evaluations`) esegue la suite e produce un report versionato con recall@k, faithfulness, citation correctness, answer relevance, refusal accuracy, leakage rate, latenza e costo/query.
  3. Ogni run registra versione del corpus, configurazione, modelli LLM/embedding e la versione JEV effettiva risolta dall'alias, così che una regressione sia attribuibile.
  4. Ogni valore atteso ha superato il triplo guardrail (`eval-oracle-check`: Claude, double-check GPT, utente); le soglie sono definite e la CI fallisce su regressione.
  5. Il report confronta il reranking JEV con la baseline di `DecisionEngine` scelta in A-06 e documenta la calibrazione delle soglie JEV.

**Decisioni da chiudere nel discuss**: Q-07 (registrazione della versione JEV effettiva; vincola l'adapter JEV nella Fase 4); soglie delle metriche; uso di TAT-DQA e verifica delle condizioni d'uso di FinanceBench
**Perimetro file**: `evaluation/**`, rotta `/internal/v1/evaluations` del RAG Service, job CI di evaluation
**Plans**: TBD

### Phase 14: Osservabilità, prestazioni e hardening

**Goal**: Il comportamento del sistema sotto carico è visibile e misurato, e i limiti di concorrenza reggono senza degradare la sicurezza
**Milestone**: M3
**Criticità**: normale
**Depends on**: Phase 13
**Requirements**: PLAT-04, DOC-04, OBS-01
**Success Criteria** (what must be TRUE):

  1. Una richiesta che attraversa i tre servizi appare come un'unica trace in locale, con metriche di saturazione di pool, code e fan-out.
  2. Il Time To Queryable è misurato su hardware documentato per i profili del README (upload ack, PDF 25 e 100 pagine, scannerizzato 25 pagine, job fallito visibile) con report p95, e i target sono riesaminati.
  3. Oltre la soglia di coda, i servizi rispondono `429` con `Retry-After`; ogni fan-out ha limite, deadline, cancellazione e budget di retry verificati da test, con parametri calibrati.
  4. Le race condition elencate in interfaces-and-concurrency.md (upload identici, cancellazione durante ingestion, nuova versione durante query, revoca durante query, documento rimosso, retry audit, timeout JEV) hanno test automatici verdi.
  5. La decisione sulla cache (A-07) è presa su misure; se serve una cache reale, va in una fase provider decimale con invalidazione testata (S-06), altrimenti resta `NoOpCache` e Redis rimane v2.

**Decisioni da chiudere nel discuss**: A-07 (NoOpCache vs Redis), S-06 (invalidazione cache, solo se introdotta); hardware di riferimento per il benchmark
**Perimetro file**: override Compose di osservabilità, dashboard e benchmark, suite delle race condition, valori di calibrazione nei file di configurazione (whitelist)
**Plans**: TBD

### Phase 15: Deployment, demo e case study

**Goal**: Chiunque cloni il repository può riprodurre la demo MVP end-to-end, e il progetto è presentabile come case study portfolio
**Milestone**: M4
**Criticità**: normale
**Depends on**: Phase 14
**Requirements**: DLV-01, DLV-02
**Success Criteria** (what must be TRUE):

  1. Da un clone pulito, seguendo il runbook, la demo gira con Docker Compose: login → upload → queryable → domanda → risposta citata → viewer alla pagina citata → audit.
  2. Script e dataset di demo sono versionati; la suite di evaluation e i test RBAC anti-leakage sono verdi sull'artefatto usato per la demo.
  3. Sull'artefatto di demo è verificato che le immagini sono non-root, con health check e filesystem read-only dove possibile, e che nessun segreto è presente in immagini o file Compose versionati.
  4. Il case study descrive problema, architettura, decisioni (ADR), risultati di evaluation e limiti noti.

**Perimetro file**: runbook, script e dataset di demo, case study in `deliverables/`
**Plans**: TBD

## Tracciabilità provider → consumer

Ogni operazione di porta implementata nelle Fasi 3-4 è usata da almeno una fase consumer pianificata. La tabella si riallinea nella riconciliazione (passo 5 della strategia).

| Porta (fase) | Operazione | Fasi consumer |
|--------------|------------|---------------|
| DocumentStore (3) | `put` | 6 (upload), 12 (fetch online in quarantena) |
| DocumentStore (3) | `get` (con range) | 6 (worker di parsing), 9 (contenuto del viewer) |
| DocumentParser (3) | `parse` | 6, 12 |
| Chunker (3) | `chunk` | 6, 12 |
| Embedder (3) | `embed_batch` | 6, 12 |
| Embedder (3) | embedding della query (firma da S-07) | 7 |
| VectorIndex (3) | `publish_version` | 6, 12 (nuova versione) |
| VectorIndex (3) | `search` (con predicato di autorizzazione) | 7 |
| DocumentSource (3) | `discover`, `fetch` | 12 |
| DecisionEngine (4) | `validate_query`, `rank`, `assess_evidence`, `validate_citations` (adapter JEV) | 8, 13 |
| DecisionEngine (4) | fake deterministico | 8 (test unitari), 10 (fake ostile) |
| DecisionEngine (4) | baseline | 8 (fallback), 13 (confronto A-06) |
| AnswerGenerator (4) | `generate` | 8, 13 |
| AuditSink (4) | `append` | 5, 6, 7, 8, 9, 11, 12 |
| Cache (4) | `get`, `put` (`NoOpCache`) | 8 (percorso query), 10 (canale cache nella matrice) |
| Cache (4) | `invalidate` | 12 (pubblicazione nuova versione), 14 (decisione A-07) |

Non sono provider esterni e restano nella fase consumer: `AccessPolicy` (deterministica, Fase 7), hashing Argon2id e firma JWT (Fase 5).

## Decisioni aperte per fase

Le decisioni restano aperte: ciascuna va chiusa nel `discuss` della fase indicata. Con la strategia U-04 tutti i discuss precedono la pianificazione dei provider, quindi una decisione chiusa in una fase consumer può vincolare un provider (colonna "Vincola").

| Voce | Argomento | Fase | Vincola |
|------|-----------|------|---------|
| A-01 | Tooling OpenAPI (`utoipa` vs contract-first) | 2 | — |
| A-02 | Storage originali (object storage S3-compatibile vs `bytea`) | 1 | 2, 3 |
| A-03 | Embedding model | 1 | 3 |
| A-04 | LLM generativo | 1 | 4 |
| A-05 | OCR/parser | 1 | 3 |
| A-06 | Baseline di reranking di confronto/fallback | 1 | 4 |
| A-07 | Caching (`NoOpCache` vs Redis) | 14 | — |
| A-08 | Container | chiusa (U-01) | — |
| A-09 | Batching JEV | 1 (spike S-04) | 4 |
| A-10 | Parser in-process vs isolato (sempre nel Document Service) | 1 | 3 |
| S-01, S-02, S-03, S-04, S-05 | Spike di upload, parser/OCR, `pgvector`, JEV, cancellazione HTTP | 1 | 2, 3, 4 |
| S-07 | Firme Rust delle porte | 1 (candidate), conferma in 3 e 4 | 3, 4 |
| S-06 | Invalidazione cache (se introdotta) | 14 | — |
| Q-01 | Permessi per ruolo | 6 | 7, 10 |
| Q-02 | Schema di classificazione dei documenti | 6 | 3, 7, 10 |
| Q-03 | Chi consulta l'audit | 11 | — |
| Q-04 | Ownership dell'audit aggregato | 11 | 4 |
| Q-05 | Conversazioni multi-turno nell'MVP | 8 | 4 |
| Q-06 | Quarantena per l'upload manuale | 6 | — |
| Q-07 | Versione JEV effettiva nell'evaluation | 13 | 4 |
| Proposte di runtime | Tokio, axum/tower, sqlx, serde, thiserror/anyhow, tracing+OTel, Vite, React Router, TanStack Query, client OpenAPI, Vitest, Playwright | 2 | — |
| Proposte `pgvector`, reqwest+Rustls | Indice vettoriale e client HTTP | 1 | 3, 4 |
| Proposte NoOpCache, audit/outbox append-only | Cache e audit | 4 (outbox anche in 2) | — |
| Proposta JWT asimmetrici | Token, JWKS, propagazione dell'identità | 5 | 7 |
| Proposta PDF.js | Viewer | 9 | — |
| Policy di fallback JEV/LLM | Rifiuto, baseline o degradazione | 8 | 4 |
| Prima fonte online | Fonte ufficiale e licenza | 12 | 3 |

## Progress

**Execution Order:**
Discuss di tutte le fasi, poi Spike, poi plan di Fondazione e Provider, plan dei consumer, riconciliazione (vedi "Strategia di consegna"). L'esecuzione segue l'ordine numerico 1 → 2 → 3 → … → 15 (3 e 4 parallelizzabili; le fasi decimali si inseriscono tra gli interi).

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Spike tecnici | 13/18 | In Progress|  |
| 2. Fondazione | 0/TBD | Not started | - |
| 3. Provider di ingestion e indice | 0/TBD | Not started | - |
| 4. Provider di decisione, risposta e trasversali | 0/TBD | Not started | - |
| 5. Identità e accesso | 0/TBD | Not started | - |
| 6. Upload e indicizzazione | 0/TBD | Not started | - |
| 7. Retrieval autorizzato | 0/TBD | Not started | - |
| 8. Risposta citata e rifiuto | 0/TBD | Not started | - |
| 9. Viewer in-app delle citazioni | 0/TBD | Not started | - |
| 10. RBAC end-to-end e guardrail | 0/TBD | Not started | - |
| 11. Audit end-to-end e tracciabilità | 0/TBD | Not started | - |
| 12. Corpus incrementale e scoperta online | 0/TBD | Not started | - |
| 13. Suite di evaluation | 0/TBD | Not started | - |
| 14. Osservabilità, prestazioni e hardening | 0/TBD | Not started | - |
| 15. Deployment, demo e case study | 0/TBD | Not started | - |
