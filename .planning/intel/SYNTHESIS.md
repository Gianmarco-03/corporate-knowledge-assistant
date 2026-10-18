# SYNTHESIS — punto di ingresso per gsd-roadmapper

Generato da gsd-doc-synthesizer il 2026-10-18. MODE: new. Precedenza: ADR > SPEC > PRD > DOC (nessun override per documento).
Progetto: Corporate Knowledge Assistant, un assistente RAG aziendale con RBAC, citazioni verificabili, viewer in-app, audit, corpus incrementale e decision layer JEV. Stato: definizione dell'architettura, prima del vertical slice.

## Documenti sintetizzati: 15

- ADR (2): docs/adr/0001-system-boundaries-and-stack.md, docs/adr/0002-incremental-corpus-and-jev.md
- SPEC (4): DEPENDENCIES.md, docs/architecture/architecture.md, docs/architecture/interfaces-and-concurrency.md, docs/data/corpus-strategy.md
- PRD (3): docs/product/requirements.md, README.md, docs/product/project-charter.md (stub)
- DOC (6, tutti stub su template): docs/architecture/api-contract.md, docs/architecture/data-model.md, docs/quality/llm-evaluation.md, docs/quality/test-plan.md, docs/security/threat-model.md, deliverables/checklist.md

Grafo dei riferimenti: aciclico, profondità massima 2. Nessun documento UNKNOWN.

## Decisioni (.planning/intel/decisions.md)

- LOCKED: 2 ADR con 11 enunciati (D-0001-a..f, D-0002-a..e)
  - docs/adr/0001-system-boundaries-and-stack.md: 3 servizi; React/TS + Rust; PostgreSQL con ownership per servizio; HTTP/JSON OpenAPI; job asincroni; trait per i provider; niente MFA
  - docs/adr/0002-incremental-corpus-and-jev.md: knowledge corpus ed evaluation corpus separati; quarantena per gli online; ambito di JEV e sue esclusioni; JEV via HTTP dietro DecisionEngine
- DECISA (DEPENDENCIES.md): TS+React, Rust stable, Argon2, PostgreSQL in un cluster, migrazioni per servizio, integrazione JEV via HTTPS diretto
- USER-RATIFIED (2026-10-18): U-01 Docker Desktop + Compose; U-02 metodologia GSD+ECC (solo processo)
- PROPOSTA: Tokio, Axum/tower, serde, sqlx, pgvector, JWT asimmetrici, reqwest+Rustls, thiserror/anyhow, tracing+OTel, Vite, React Router, TanStack Query, PDF.js, client OpenAPI generato, Vitest, Playwright, audit/outbox append-only, NoOpCache
- APERTE: A-01..A-10 (tooling OpenAPI, storage originali, embedding, LLM, OCR/parser, baseline reranker, Redis, container (chiusa), batching JEV, isolamento parser)
- Spike richiesti: S-01..S-07
- Esclusioni MVP: MFA, K8s/mesh, broker, DB vettoriale separato, framework agentico, cache distribuita, sidecar JEV, plugin dinamici, auto-approvazione

## Requisiti (.planning/intel/requirements.md): 30

- Funzionali (12): REQ-login-email-password (FR-001), REQ-role-management (FR-002), REQ-document-upload (FR-003), REQ-indexing-atomic-publish (FR-004), REQ-authorized-query (FR-005), REQ-cited-answer (FR-006), REQ-in-app-viewer (FR-007, VARIANTI CONCORRENTI), REQ-refusal (FR-008), REQ-audit-logging (FR-009), REQ-audit-consultation (README), REQ-online-discovery (FR-010), REQ-document-update-no-disruption (FR-011)
- Non funzionali (11): REQ-deny-by-default (NFR-001), REQ-password-argon2id (NFR-002), REQ-bounded-concurrency (NFR-003), REQ-idempotency (NFR-004), REQ-traceability (NFR-005), REQ-ports-extensibility (NFR-006), REQ-time-to-queryable (NFR-007), REQ-evaluation-corpus-separation (NFR-008), REQ-safe-degradation (NFR-009), REQ-contract-versioning-tests (README), REQ-untrusted-online-source (README)
- Regole di business (7): REQ-online-quarantine (BR-001), REQ-jev-signals-only (BR-002), REQ-deterministic-security (BR-003), REQ-viewer-access-recheck (BR-004), REQ-unsupported-refusal (BR-005), REQ-immutable-cited-version (BR-006), REQ-no-cross-service-tables (BR-007)

## Vincoli (.planning/intel/constraints.md): 31

- api-contract: 4 (endpoint Identity/Document/RAG, porte Rust)
- schema: 4 (ownership dei dati, corpus, metadati di provenance, isolamento della persistenza)
- nfr: 15 (concorrenza, race condition, backpressure, responsabilità dei servizi, dataset, cache, target TTQ p95)
- protocol: 8 (regole dei contratti, DB/eventi, flussi di ingestion e query, pipeline online, triage JEV, policy di pubblicazione, integrazione JEV)

## Contesto (.planning/intel/context.md): 12 topic

Visione, milestone M0-M4 (deliverables/checklist.md, utili come scheletro delle fasi), risorse API, modello dati, metriche di evaluation, strategia di test, priorità del threat model, struttura del repo, ruolo di JEV, decisioni utente, lacune (7 stub su template), domande aperte Q-01..Q-07.

## Conflitti (.planning/INGEST-CONFLICTS.md)

- BLOCKER: 0
- WARNING (varianti concorrenti): 1, REQ-in-app-viewer (FR-007 "pagina con highlight se disponibile" vs README "pagina e passaggio citato")
- INFO (risolti automaticamente o informativi): 10, di cui 3 risolti con ADR > SPEC (triage JEV dei documenti, isolamento del parser nel Document Service, baseline del reranker) e 1 decisione aperta chiusa dall'utente (container)

## Indicazioni per il roadmapper

- Prima del routing va risolto il WARNING su REQ-in-app-viewer.
- Le milestone M0-M4 di deliverables/checklist.md sono lo scheletro naturale delle fasi; M1 (vertical slice upload -> retrieval -> risposta citata) dipende dagli spike S-01, S-03 e S-07 e dalle decisioni A-01..A-05.
- Le domande aperte Q-01 (permessi per ruolo) e Q-02 (classificazione dei documenti) bloccano M2 (RBAC end-to-end) e la matrice di test.
- JEV non è prerequisito per autenticazione, viewer e RBAC (ADR-0002): le fasi possono procedere usando il fake del DecisionEngine.
