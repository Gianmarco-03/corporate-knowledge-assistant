# Vincoli — intel sintetizzata

Generato da gsd-doc-synthesizer il 2026-10-18 (MODE: new).
Fonti SPEC: docs/architecture/interfaces-and-concurrency.md, docs/architecture/architecture.md, docs/data/corpus-strategy.md, DEPENDENCIES.md. Un vincolo (C-31) proviene dal PRD README.md perché ha natura SPEC (target prestazionali).
Tipi: api-contract | schema | nfr | protocol. Le scelte tecnologiche (Decisa/Proposta/Aperta) sono in decisions.md.
Dove un vincolo SPEC tocca una decisione LOCKED, la ADR prevale (annotato).

---

## Contratti e API — source: docs/architecture/interfaces-and-concurrency.md

Nota della fonte: i nomi sono concettuali; le firme Rust definitive verranno validate con uno spike.

### C-01 Regole dei contratti
- source: docs/architecture/interfaces-and-concurrency.md
- type: protocol
- contenuto:
  - UUID opachi, orari UTC RFC 3339, API sotto `/api/v1`.
  - Operazioni mutanti con idempotency key o identificatore stabile equivalente.
  - Propagazione di `request_id` e trace context.
  - Errori con `code`, `message`, `request_id` e dettagli non sensibili.
  - Nessun tipo di database o SDK esterno attraversa una porta di dominio.

### C-02 HTTP — Identity & Access
- source: docs/architecture/interfaces-and-concurrency.md
- type: api-contract
- contenuto:
  - POST `/api/v1/auth/login` — verifica credenziali ed emette token
  - POST `/api/v1/auth/refresh` — ruota il refresh token
  - POST `/api/v1/auth/logout` — revoca la sessione
  - GET `/api/v1/auth/me` — identità e ruoli correnti
  - POST `/api/v1/users` — crea utente, solo admin
  - PATCH `/api/v1/users/{id}/roles` — aggiorna ruoli, solo admin
  - GET `/.well-known/jwks.json` — chiavi pubbliche di verifica
- nota: JWKS e refresh presuppongono JWT asimmetrici, che in DEPENDENCIES.md sono PROPOSTA (vedi INFO in INGEST-CONFLICTS.md).

### C-03 HTTP — Document & Indexing
- source: docs/architecture/interfaces-and-concurrency.md
- type: api-contract
- contenuto:
  - POST `/api/v1/documents` — upload streaming; restituisce `document_id` e `job_id`
  - GET `/api/v1/documents` — elenca soltanto documenti visibili
  - GET `/api/v1/documents/{id}` — metadati e versione corrente
  - GET `/api/v1/documents/{id}/content` — contenuto autorizzato per il viewer
  - DELETE `/api/v1/documents/{id}` — tombstone/cancellazione amministrativa
  - GET `/api/v1/ingestion-jobs/{id}` — stato e fasi dell'ingestion
  - POST `/internal/v1/retrieval/search` — candidati autorizzati per RAG Service
  - POST `/internal/v1/documents/{id}/authorize` — ricontrollo prima della risposta

### C-04 HTTP — RAG & Answer
- source: docs/architecture/interfaces-and-concurrency.md
- type: api-contract
- contenuto:
  - POST `/api/v1/queries` — query con deadline esplicita
  - GET `/api/v1/queries/{id}` — risultato e citazioni
  - GET `/api/v1/audit-events` — audit secondo ruolo e filtri
  - POST `/internal/v1/evaluations` — avvia evaluation versionata

### C-05 Porte Rust interne
- source: docs/architecture/interfaces-and-concurrency.md
- type: api-contract (interno)
- contenuto (firme concettuali):
  - `DocumentSource`: `discover(cursor) -> DiscoveryPage`, `fetch(candidate) -> DocumentStream`
  - `DocumentStore`: `put(stream, metadata) -> ObjectRef`, `get(object, range) -> DocumentStream`
  - `DocumentParser`: `parse(object) -> ParsedDocument`
  - `Chunker`: `chunk(document) -> Vec<ChunkDraft>` (sincrono)
  - `Embedder`: `embed_batch(chunks) -> Vec<Embedding>`
  - `VectorIndex`: `publish_version(version)`, `search(AuthorizedSearch) -> Vec<CandidateChunk>`
  - `DecisionEngine`: `validate_query`, `rank`, `assess_evidence`, `validate_citations`
  - `AnswerGenerator`: `generate(GroundedAnswerRequest) -> StructuredAnswer`
  - `AccessPolicy`: `authorize(subject, action, resource) -> AuthorizationDecision`
  - `AuditSink`: `append(event)`
  - `Cache`: `get`, `put(key, value, ttl)`, `invalidate(scope)`
  - `DecisionEngine` avrà adapter JEV, fake deterministico e baseline. Autorizzazione e provenance NON appartengono a questa porta (coerente con D-0002-d, LOCKED).

---

## Concorrenza, dati ed eventi — source: docs/architecture/interfaces-and-concurrency.md

### C-06 Modello di concorrenza e upload/ingestion
- source: docs/architecture/interfaces-and-concurrency.md
- type: nfr
- contenuto:
  - Async per I/O; pool limitati per lavoro CPU-bound. Ogni fan-out ha deadline globale, limite, cancellazione, budget di retry e metriche di saturazione.
  - Upload in streaming; mai l'intero file in memoria.
  - Job restituito dopo persistenza sicura dell'originale e dell'outbox.
  - Worker con lease e `FOR UPDATE SKIP LOCKED` o equivalente.
  - Pool distinti per parsing, OCR, embedding e scrittura DB.
  - Parsing/OCR CPU-bound tramite `spawn_blocking` o processi isolati (entro il Document Service: ADR-0001 conseguenze, LOCKED).
  - Embedding in batch bounded con backpressure.
  - Lock logico su `(source, external_id, version)`.
  - Checksum e unique constraint per retry idempotenti.

### C-07 Concorrenza della query RAG
- source: docs/architecture/interfaces-and-concurrency.md
- type: nfr
- contenuto:
  1. Retrieval autorizzato con deadline interna.
  2. Reranking JEV bounded; mai fan-out illimitato per chunk.
  3. Cancellazione dei task residui dopo timeout.
  4. Una sola generazione per tentativo; rigenerazione esplicita e limitata.
  5. Citation validation concorrente con semaforo.
  6. Ricontrollo autorizzativo prima di citazioni e link.
  - Baseline sperimentale: top-20 dal retrieval, max 8 decisioni JEV concorrenti, deadline complessiva configurabile (da calibrare).

### C-08 Database ed eventi
- source: docs/architecture/interfaces-and-concurrency.md
- type: protocol
- contenuto:
  - Pool separato per servizio, sotto il massimo del server.
  - Transazioni brevi; nessuna chiamata esterna dentro una transazione.
  - Version column per aggiornamenti concorrenti di metadati e ACL.
  - Outbox nella stessa transazione del cambiamento di dominio.
  - Consumer at-least-once, `event_id` univoco e handler idempotente.

### C-09 Race condition da testare
- source: docs/architecture/interfaces-and-concurrency.md
- type: nfr
- contenuto (scenario -> comportamento richiesto):
  - Due upload identici -> un solo contenuto fisico; job idempotenti o versioni esplicite.
  - Cancellazione durante ingestion -> job cancellato e versione non pubblicata.
  - Nuova versione durante query -> snapshot coerente e versione citata.
  - Revoca ruolo durante query -> ricontrollo prima della risposta.
  - Documento rimosso dopo risposta -> il viewer applica nuovamente l'autorizzazione.
  - Retry evento audit -> nessun duplicato grazie a `event_id`.
  - Timeout JEV -> fallback/rifiuto; mai bypass della sicurezza.

### C-10 Backpressure e degradazione
- source: docs/architecture/interfaces-and-concurrency.md
- type: nfr
- contenuto:
  - `429` e `Retry-After` quando le code superano le soglie.
  - Circuit breaker per JEV, embedding e LLM.
  - Il guasto JEV non abilita azioni negate.
  - Viewer e autenticazione restano indipendenti dal generative LLM (e da JEV: ADR-0002 conseguenze).

---

## Architettura — source: docs/architecture/architecture.md

### C-11 Driver architetturali
- source: docs/architecture/architecture.md
- type: nfr
- contenuto:
  - RBAC deny-by-default e assenza di leakage verso JEV, LLM, cache e log.
  - Citazioni riproducibili fino a versione, pagina e posizione.
  - Ingestion incrementale da upload e fonti online.
  - Estensibilità tramite porte e adapter.
  - Job lunghi asincroni, osservabili e idempotenti.
  - Massimo tre microservizi di dominio significativi (coerente con D-0001-a).

### C-12 Vista dei container
- source: docs/architecture/architecture.md
- type: nfr
- contenuto: Browser React/TS -> reverse proxy/routing -> Identity / Document / RAG services -> un cluster PostgreSQL con ownership separata. Document Service -> object storage (aperto) ed embedding provider. RAG Service -> JEV API e generative LLM. Reverse proxy, frontend, worker e infrastruttura non sono bounded context aggiuntivi.

### C-13 Responsabilità Identity & Access Service
- source: docs/architecture/architecture.md
- type: nfr
- contenuto: Utenti, hash Argon2id, ruoli, autenticazione, token firmati, chiavi pubbliche, audit di autenticazione. Nessun MFA nell'MVP. Gli altri servizi verificano localmente firma, issuer, audience e scadenza e applicano le proprie policy di risorsa.

### C-14 Responsabilità Document & Indexing Service
- source: docs/architecture/architecture.md
- type: nfr
- contenuto: Upload, fonti online, quarantena, validazione deterministica, parsing/OCR, chunking, embedding, ACL, versionamento, vector index, retrieval autorizzato, job e contenuto per il viewer. Il worker è una modalità di esecuzione dello stesso servizio.

### C-15 Responsabilità RAG & Answer Service
- source: docs/architecture/architecture.md
- type: nfr
- contenuto: Query, `DecisionEngine`, candidati autorizzati, reranking, evidence gate, generazione strutturata, citation validation, rifiuto, conversazioni, audit query ed evaluation.

### C-16 Flusso di ingestion
- source: docs/architecture/architecture.md
- type: protocol
- contenuto: upload/discovery -> quarantine -> deterministic validation -> provenance and policy check -> parse/OCR -> chunk + positional metadata -> batch embedding -> transactional publish of a document version -> queryable. Pubblicazione atomica: le query vedono la versione precedente o la nuova, mai un indice parziale.
- nota: per l'upload manuale la quarantena qui è indicata anche come primo passo; docs/data/corpus-strategy.md prescrive per l'upload manuale "pubblicazione dopo validazione e ACL esplicita" e quarantena obbligatoria solo per la scoperta online. Compatibili (vedi context.md, domande aperte).

### C-17 Flusso di query
- source: docs/architecture/architecture.md
- type: protocol
- contenuto: authenticate -> semantic query validation (JEV) -> authorized candidate retrieval -> bounded parallel rerank -> evidence sufficiency / contradiction gate -> structured answer generation -> bounded parallel citation validation -> authorization re-check -> response or refusal -> audit.

### C-18 Dati e ownership
- source: docs/architecture/architecture.md
- type: schema
- contenuto:
  - Un cluster PostgreSQL nell'MVP; credenziali e schema/database distinti per servizio.
  - Nessuna join cross-service.
  - `pgvector` candidato per chunk ed embedding del Document Service.
  - Eventi tramite transactional outbox e consumer idempotenti.
  - Consistenza forte nel servizio; consistenza eventuale per proiezioni e audit aggregato.

### C-19 Estensibilità
- source: docs/architecture/architecture.md
- type: nfr
- contenuto: Adapter per parser, embedding, JEV, LLM, storage e cache sostituibili senza modificare i casi d'uso. Open-Closed non implica plugin dinamici nell'MVP: implica dipendenze verso trait stabili e testabili.

---

## Corpus — source: docs/data/corpus-strategy.md

### C-20 Knowledge corpus vs evaluation corpus
- source: docs/data/corpus-strategy.md
- type: schema
- contenuto:
  - Knowledge corpus: visibile secondo ACL; cambia tramite upload e fonti online; ogni documento versionato; la nuova versione sostituisce la pubblicata solo dopo indicizzazione completa.
  - Evaluation corpus: versionato e immutabile per release; contiene domande, risposte attese, evidenze, pagine e profilo utente; non ampliato automaticamente dal crawler; ogni modifica produce una nuova versione del benchmark.
  - Dimensione iniziale: 10-30 documenti selezionati che coprano testo, tabelle, grafici, versioni e livelli di accesso (anche README.md).

### C-21 Pipeline di aggiornamento online
- source: docs/data/corpus-strategy.md
- type: protocol
- contenuto: source connector -> discovered candidate -> quarantine -> checksum/deduplication -> provenance and license policy -> deterministic file validation -> JEV semantic triage -> approval policy -> ingestion -> atomic publication.

### C-22 Ruolo di JEV nel triage dei documenti
- source: docs/data/corpus-strategy.md
- type: protocol
- contenuto:
  - JEV può: classificare tipo e pertinenza del documento; segnalare nuova versione o candidato irrilevante; rilevare segnali di contenuto sospetto o prompt injection; valutare pagine/immagini in modalità sperimentale; assegnare priorità alla revisione.
  - JEV non deve decidere da solo: autenticità e licenza della fonte; autorizzazioni e classificazione finale; assenza di malware; deduplicazione esatta; pubblicazione di contenuti riservati.
- nota: le esclusioni coincidono con D-0002-d (LOCKED). Il triage semantico dei documenti estende l'elenco degli usi di D-0002-c senza contraddirlo (INFO in INGEST-CONFLICTS.md).

### C-23 Metadati di provenance
- source: docs/data/corpus-strategy.md
- type: schema
- contenuto: ogni `DocumentSource` espone discovery incrementale tramite cursor e fetch dell'originale; primo connettore su fonte ufficiale e stabile (API, RSS o manifest preferiti allo scraping HTML). Metadati minimi:
  ```json
  {
    "source_id": "official-source",
    "external_id": "stable-provider-id",
    "canonical_url": "https://example.org/document.pdf",
    "discovered_at": "RFC-3339",
    "published_at": "RFC-3339",
    "content_sha256": "...",
    "retrieval_etag": "...",
    "license": "...",
    "provenance_status": "verified"
  }
  ```

### C-24 Dataset iniziali
- source: docs/data/corpus-strategy.md
- type: nfr
- contenuto: TAT-DQA candidato per evaluation e documenti visualmente ricchi; runtime corpus da un piccolo sottoinsieme poi esteso con documenti online; FinanceBench benchmark complementare dopo verifica delle condizioni d'uso; DocLayNet per test di parsing e layout.

### C-25 Politica MVP di pubblicazione
- source: docs/data/corpus-strategy.md
- type: protocol
- contenuto:
  - Upload manuale: pubblicazione dopo validazione e ACL esplicita.
  - Scoperta online: quarantena obbligatoria.
  - Auto-approvazione: disabilitata inizialmente.
  - Fonte fidata: aggiornamento solo dopo provenance, checksum, tipo file e policy.
  - Documento fallito: diagnostica conservata, mai queryable.
  - Cancellazione: tombstone e rimozione asincrona da indice e storage.

### C-26 Dataset di sicurezza sintetico
- source: docs/data/corpus-strategy.md
- type: nfr
- contenuto: documenti con ACL diverse, informazioni presenti solo in contenuti vietati, versioni in conflitto, domande senza risposta, prompt injection, metadati falsi, PDF corrotti/cifrati, scansioni di bassa qualità.

---

## Dipendenze — source: DEPENDENCIES.md

### C-27 Integrazione JEV
- source: DEPENDENCIES.md
- type: protocol
- contenuto: HTTPS diretto a `POST /v1/systemone` tramite adapter Rust; nessun SDK Rust community; contratto HTTP/OpenAPI ufficiale (https://docs.typesafe.ai/api, https://api.typesafe.ai/openapi.json); modello alias configurabile, inizialmente `jev-latest`; immagini sperimentali; timeout stretto, retry solo transitori entro deadline globale; fallback rifiuto/baseline locale/risposta degradata secondo policy; test con fake locale del `DecisionEngine`, nessuna API reale nei test unitari. Gli SDK ufficiali (TypeScript, Python) non sono usati: un sidecar aggiungerebbe un quarto servizio.

### C-28 Policy di caching
- source: DEPENDENCIES.md
- type: nfr
- contenuto: il codice dipende dalla porta `Cache`; primo adapter `NoOpCache`; Redis solo dopo misura. Nell'MVP non cacheare risposte complete o contenuti autorizzati senza includere nella chiave identità, versione della policy e versioni dei documenti. Usi futuri: JWKS e configurazioni non sensibili, embedding di query normalizzate, risultati di retrieval (chiave con identità/policy/versioni), rate limiting distribuito, stato effimero di job e lock distribuiti.

### C-29 Persistenza e isolamento
- source: DEPENDENCIES.md
- type: schema
- contenuto: un singolo cluster non significa database condiviso: ogni servizio usa credenziali e schema/database propri e comunica con gli altri tramite contratti. Migrazioni SQL per servizio. Audit/outbox su tabelle append-only con `event_id` univoco (PROPOSTA).

### C-30 Provider AI dietro porte
- source: DEPENDENCIES.md
- type: nfr
- contenuto: ogni provider (embedding, LLM, OCR/parser, reranker) implementa una porta interna; i tipi del provider non attraversano il dominio.

---

## Prestazioni — source: README.md (PRD, contenuto di natura SPEC)

### C-31 Target iniziali Time To Queryable (p95)
- source: README.md
- type: nfr
- contenuto:
  - Upload acknowledgment: <= 500 ms
  - PDF digitale, 25 pagine, max 10 MB: <= 15 s
  - PDF digitale, 100 pagine: <= 60 s
  - PDF scannerizzato, 25 pagine: <= 45 s
  - Stato di job fallito visibile: <= 5 s
  - I target saranno riesaminati dopo una baseline su hardware documentato.
- correlato: NFR-007 (requirements.md). Vedi INFO in INGEST-CONFLICTS.md su upload ack vs persistenza prima della risposta (C-06).

---

## Riepilogo per tipo

- api-contract: 4 (C-02, C-03, C-04, C-05)
- schema: 4 (C-18, C-20, C-23, C-29)
- nfr: 15 (C-06, C-07, C-09, C-10, C-11, C-12, C-13, C-14, C-15, C-19, C-24, C-26, C-28, C-30, C-31)
- protocol: 8 (C-01, C-08, C-16, C-17, C-21, C-22, C-25, C-27)
- Totale: 31
