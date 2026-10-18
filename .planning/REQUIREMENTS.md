# Requirements: Corporate Knowledge Assistant (CKA)

**Modifica:** Claude (gsd-roadmapper) · 2026-10-18 · creazione da `.planning/intel/requirements.md` (30 requisiti) + 7 requisiti derivati dalle milestone e dai documenti di qualità
**Modifica:** Claude (gsd-roadmapper) · 2026-10-18 · traccia riallineata alla roadmap rivista per U-04 (15 fasi: Spike, Fondazione, due fasi Provider, consumer 5-15)

**Defined:** 2026-10-18
**Core Value:** Un utente autenticato ottiene risposte supportate da citazioni verificabili, solo dai documenti che è autorizzato a vedere, oppure un rifiuto motivato.

Convenzione: tra parentesi l'ID originale (FR/NFR/BR) e la fonte. I requisiti marcati **[derivato]** non esistono come requisito nei PRD: sono stati introdotti dal roadmapper per coprire milestone (deliverables/checklist.md) e documenti di qualità, e vanno confermati dall'utente.

## v1 Requirements

### Identità e accesso (AUTH)

- [ ] **AUTH-01**: L'utente accede con email e password; credenziali corrette emettono token, gli errori non rivelano dettagli. (FR-001, docs/product/requirements.md)
- [ ] **AUTH-02**: L'`admin` assegna i ruoli `admin`, `manager`, `employee`, `guest`. (FR-002)
- [ ] **AUTH-03**: Le password sono salvate solo come hash Argon2id versionato; nessuna cifratura reversibile. (NFR-002)

### Documenti e ingestion (DOC)

- [ ] **DOC-01**: Upload di PDF, DOCX e Markdown validi produce un job di ingestion osservabile fino a `queryable` o `failed`. (FR-003)
- [ ] **DOC-02**: Un job riuscito pubblica atomicamente una versione queryable; le query vedono la versione precedente o la nuova, mai un indice parziale. (FR-004)
- [ ] **DOC-03**: Le nuove versioni di un documento non interrompono le query sulla versione pubblicata. (FR-011)
- [ ] **DOC-04**: Il Time To Queryable è misurato su hardware noto rispetto ai target p95 iniziali del README. (NFR-007, C-31)

### Fonti online (ONL)

- [ ] **ONL-01**: Un connettore scopre in modo incrementale nuovi documenti e li propone in quarantena. (FR-010)
- [ ] **ONL-02**: Ogni documento scoperto online entra in quarantena; nessuna auto-approvazione. (BR-001, C-25)
- [ ] **ONL-03**: Un documento online non è considerato attendibile solo perché proviene da un URL valido: provenance, licenza, checksum e validazione del file sono verificati in modo deterministico. (README, Principi)

### Sicurezza e RBAC (SEC)

- [ ] **SEC-01**: Il retrieval avviene solo su documenti e chunk accessibili all'utente. (FR-005)
- [ ] **SEC-02**: Deny-by-default: nessun chunk negato esce dal Document Service (né verso reranker, JEV, LLM, cache o log). (NFR-001)
- [ ] **SEC-03**: Autorizzazione, provenance e mapping delle citazioni sono deterministici. (BR-003)
- [ ] **SEC-04**: Il fallimento di JEV o del LLM non bypassa autorizzazione o policy. (NFR-009)
- [ ] **SEC-05**: **[derivato]** Una matrice di test ruolo × classificazione documento, con test negativi, dimostra l'assenza di leakage in risposte, citazioni, viewer e log. (docs/quality/test-plan.md, C-26)

### Query e risposta (QRY)

- [ ] **QRY-01**: Ogni claim verificabile della risposta contiene una citazione valida. (FR-006)
- [ ] **QRY-02**: Evidenza insufficiente o contraddittoria produce un rifiuto esplicito. (FR-008)
- [ ] **QRY-03**: Una risposta non supportata viene rifiutata. (BR-005)
- [ ] **QRY-04**: Le query citano una versione immutabile del documento. (BR-006)
- [ ] **QRY-05**: JEV produce segnali; il codice conserva la decisione finale. (BR-002)

### Viewer (VIEW)

- [ ] **VIEW-01**: La citazione apre nel viewer in-app la versione e la pagina corrette (obbligatorio); il passaggio citato è evidenziato quando il parser fornisce coordinate, altrimenti fallback alla pagina (PDF scannerizzati, DOCX). (FR-007 + README MVP punto 6; **decisione utente 2026-10-18**, risolve il WARNING di INGEST-CONFLICTS.md)
- [ ] **VIEW-02**: Il viewer esegue un nuovo controllo di accesso a ogni apertura. (BR-004)

### Audit e tracciabilità (AUD)

- [ ] **AUD-01**: Login, upload, ACL, ingestion, query, rifiuti e deny sono tracciati. (FR-009)
- [ ] **AUD-02**: Un utente autorizzato consulta gli eventi di audit secondo ruolo e filtri. (README MVP punto 7, `GET /api/v1/audit-events`)
- [ ] **AUD-03**: `request_id`, trace e versione del documento sono presenti in risposta e audit. (NFR-005)

### Piattaforma e architettura (PLAT)

- [ ] **PLAT-01**: I provider (parser, storage, embedding, vector index, JEV, LLM, audit, cache) stanno dietro porte Rust; i tipi SDK restano confinati negli adapter. (NFR-006, D-0001-e)
- [ ] **PLAT-02**: Nessun servizio legge direttamente le tabelle di un altro; migrazioni e credenziali per servizio. (BR-007)
- [ ] **PLAT-03**: I contratti tra servizi sono versionati e testati (contract test in CI). (README, Principi)
- [ ] **PLAT-04**: Fan-out e worker sono bounded, con backpressure (`429` + `Retry-After`), deadline e cancellazione. (NFR-003, C-06, C-07, C-10)
- [ ] **PLAT-05**: Il retry di upload, job ed eventi non duplica effetti. (NFR-004)
- [ ] **PLAT-06**: **[derivato]** L'ambiente locale si avvia da un clone pulito con Docker Compose (PostgreSQL + `pgvector`, tre servizi, frontend) e la CI GitHub Actions verifica ogni push. (U-01, istruzioni orchestratore)
- [ ] **PLAT-07**: **[derivato]** Il threat model è compilato (asset, attori, minacce prioritarie, mitigazioni mappate ai requisiti), chiudendo la lacuna M0. (docs/security/threat-model.md, M0)

### Evaluation (EVAL)

- [ ] **EVAL-01**: L'evaluation corpus è versionato, immutabile per release e separato dal knowledge corpus dinamico. (NFR-008, D-0002-a)
- [ ] **EVAL-02**: **[derivato]** Una suite di evaluation riproducibile misura recall@k, faithfulness, citation correctness, answer relevance, refusal accuracy, leakage rate, latenza e costo/query, registrando le versioni di corpus, modelli e JEV effettivo. (docs/quality/llm-evaluation.md, M3, Q-07)

### Osservabilità (OBS)

- [ ] **OBS-01**: **[derivato]** Una richiesta è tracciabile attraverso i tre servizi con una trace distribuita, e le metriche di saturazione di pool, code e fan-out sono consultabili in locale. (M3, C-06, DEPENDENCIES tracing+OTel)

### Consegna (DLV)

- [ ] **DLV-01**: **[derivato]** La demo MVP end-to-end (login → upload → queryable → domanda → risposta citata → viewer → audit) è riproducibile da un clone pulito seguendo un runbook e uno script di demo. (M4, successo lato sviluppatore)
- [ ] **DLV-02**: **[derivato]** Un case study documenta problema, architettura, decisioni, risultati di evaluation e limiti noti. (M4)

## v2 Requirements

Rinviati; tracciati ma fuori dalla roadmap corrente.

- **IMG-01**: Triage JEV di pagine e immagini (sperimentale, non bloccante per l'happy path — ADR-0002).
- **CACHE-01**: Cache distribuita (Redis) per retrieval/embedding di query, solo dopo misura di hit rate e deployment multi-replica (A-07).
- **CONV-01**: Conversazioni multi-turno (entità `Conversation`), salvo che Q-05 le porti nell'MVP in Fase 8.

## Out of Scope

| Funzionalità | Motivo |
|--------------|--------|
| MFA / secondo fattore | Escluso dall'MVP (ADR-0001 D-0001-f) |
| Kubernetes, service mesh | Sproporzionati per un MVP locale (DEPENDENCIES.md) |
| Message broker dedicato | Outbox su PostgreSQL sufficiente senza requisito misurato |
| Database vettoriale separato | Prima il benchmark di `pgvector` |
| Framework agentico | Pipeline deterministica |
| Sidecar JEV / quarto servizio | Integrazione HTTPS diretta (ADR-0002, DEPENDENCIES.md) |
| Plugin dinamici a runtime | Open-Closed via trait stabili |
| Auto-approvazione documenti online | Disabilitata nell'MVP (C-25) |
| Compatibilità Podman | Superata dalla decisione U-01 |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| PLAT-07 | Phase 1 | Pending |
| PLAT-02 | Phase 2 | Pending |
| PLAT-03 | Phase 2 | Pending |
| PLAT-06 | Phase 2 | Pending |
| PLAT-01 | Phase 4 | Pending |
| AUTH-01 | Phase 5 | Pending |
| AUTH-02 | Phase 5 | Pending |
| AUTH-03 | Phase 5 | Pending |
| DOC-01 | Phase 6 | Pending |
| DOC-02 | Phase 6 | Pending |
| PLAT-05 | Phase 6 | Pending |
| SEC-01 | Phase 7 | Pending |
| SEC-02 | Phase 7 | Pending |
| QRY-01 | Phase 8 | Pending |
| QRY-02 | Phase 8 | Pending |
| QRY-03 | Phase 8 | Pending |
| QRY-04 | Phase 8 | Pending |
| QRY-05 | Phase 8 | Pending |
| VIEW-01 | Phase 9 | Pending |
| VIEW-02 | Phase 9 | Pending |
| SEC-03 | Phase 10 | Pending |
| SEC-04 | Phase 10 | Pending |
| SEC-05 | Phase 10 | Pending |
| AUD-01 | Phase 11 | Pending |
| AUD-02 | Phase 11 | Pending |
| AUD-03 | Phase 11 | Pending |
| DOC-03 | Phase 12 | Pending |
| ONL-01 | Phase 12 | Pending |
| ONL-02 | Phase 12 | Pending |
| ONL-03 | Phase 12 | Pending |
| EVAL-01 | Phase 13 | Pending |
| EVAL-02 | Phase 13 | Pending |
| PLAT-04 | Phase 14 | Pending |
| DOC-04 | Phase 14 | Pending |
| OBS-01 | Phase 14 | Pending |
| DLV-01 | Phase 15 | Pending |
| DLV-02 | Phase 15 | Pending |

**Coverage:**
- v1 requirements: 37 total (30 dai documenti + 7 derivati)
- Mapped to phases: 37
- Unmapped: 0 ✓

Mappatura dagli ID dell'intel (`REQ-{slug}`): login-email-password→AUTH-01, role-management→AUTH-02, password-argon2id→AUTH-03, document-upload→DOC-01, indexing-atomic-publish→DOC-02, document-update-no-disruption→DOC-03, time-to-queryable→DOC-04, online-discovery→ONL-01, online-quarantine→ONL-02, untrusted-online-source→ONL-03, authorized-query→SEC-01, deny-by-default→SEC-02, deterministic-security→SEC-03, safe-degradation→SEC-04, cited-answer→QRY-01, refusal→QRY-02, unsupported-refusal→QRY-03, immutable-cited-version→QRY-04, jev-signals-only→QRY-05, in-app-viewer→VIEW-01, viewer-access-recheck→VIEW-02, audit-logging→AUD-01, audit-consultation→AUD-02, traceability→AUD-03, ports-extensibility→PLAT-01, no-cross-service-tables→PLAT-02, contract-versioning-tests→PLAT-03, bounded-concurrency→PLAT-04, idempotency→PLAT-05, evaluation-corpus-separation→EVAL-01.

---
*Requirements defined: 2026-10-18*
*Last updated: 2026-10-18 after revisione della roadmap per U-04*
