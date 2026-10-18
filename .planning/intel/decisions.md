# Decisioni — intel sintetizzata

Generato da gsd-doc-synthesizer il 2026-10-18 (MODE: new). Ogni voce riporta `source:` per la tracciabilità.
Precedenza: ADR > SPEC > PRD > DOC. Le ADR marcate LOCKED non possono essere sovrascritte automaticamente.

Legenda stato:
- LOCKED — ADR con stato Accepted; vincolante.
- DECISA — voce "Decisa" in DEPENDENCIES.md (SPEC); approvata ma non ADR.
- PROPOSTA — default da validare con spike; NON vincolante.
- APERTA — domanda aperta; richiede decisione prima della fase che la usa.
- USER-RATIFIED — decisione presa dall'utente il 2026-10-18, dopo la stesura dei documenti.

---

## 1. Decisioni LOCKED (ADR)

### ADR-0001 — Confini dei servizi e stack iniziale
- source: docs/adr/0001-system-boundaries-and-stack.md
- status: LOCKED (Accepted, 2026-10-18, decisore: progetto portfolio)
- scope: confini dei servizi, stack frontend/backend, persistenza, contratti, estensibilità, MFA
- decisione:
  - D-0001-a: Tre servizi di dominio: Identity & Access, Document & Indexing, RAG & Answer.
  - D-0001-b: Frontend React con TypeScript; backend Rust.
  - D-0001-c: PostgreSQL con ownership distinta per servizio.
  - D-0001-d: Contratti HTTP/JSON documentati con OpenAPI; job lunghi asincroni.
  - D-0001-e: Trait (porte) per parser, storage, embedding, vector index, JEV, LLM, audit e cache.
  - D-0001-f: Nessun MFA nell'MVP.
- conseguenze dichiarate:
  - Rust aumenta il costo iniziale e richiede spike su async, streaming e tooling.
  - Tre servizi impongono contract test, telemetria distribuita e consistenza eventuale.
  - Il worker di ingestion resta parte del Document Service (non un quarto servizio).
  - Cache, object storage e container strategy restano decisioni separate.

### ADR-0002 — Corpus incrementale e ruolo di JEV
- source: docs/adr/0002-incremental-corpus-and-jev.md
- status: LOCKED (Accepted, 2026-10-18, decisore: progetto portfolio)
- scope: knowledge/evaluation corpus, quarantena, ruolo di JEV, integrazione JEV
- decisione:
  - D-0002-a: Separare knowledge corpus dinamico ed evaluation corpus versionato.
  - D-0002-b: Tutti i documenti scoperti online entrano in quarantena.
  - D-0002-c: Usare JEV per query validation, reranking, evidence sufficiency, contradiction detection, citation validation e triage opzionale immagini.
  - D-0002-d: NON usare JEV per autorizzazione, provenance, licenze, malware scan, deduplicazione esatta o mapping citation.
  - D-0002-e: Integrare JEV dal backend Rust tramite il contratto HTTP ufficiale, dietro la porta `DecisionEngine`.
- conseguenze dichiarate:
  - Serve un workflow di approvazione e pubblicazione atomica.
  - Le soglie JEV devono essere calibrate su dati etichettati.
  - JEV non è prerequisito per autenticazione, viewer o RBAC.
  - Le immagini restano sperimentali e non bloccanti per l'happy path.

Nota: nessuna contraddizione tra ADR-0001 e ADR-0002 (vedi INGEST-CONFLICTS.md).

---

## 2. Decisioni DECISA dal registro dipendenze (SPEC, non ADR)

source: DEPENDENCIES.md

- Frontend: TypeScript + React — DECISA (coerente con D-0001-b)
- Backend: Rust stable — DECISA (coerente con D-0001-b)
- Password hashing: `argon2` (Argon2id) — DECISA come algoritmo; password mai cifrate in modo reversibile (coerente con NFR-002 in docs/product/requirements.md)
- Dati relazionali: PostgreSQL — DECISA; un cluster nell'MVP, ownership distinta per servizio (coerente con D-0001-c)
- Migrazioni: migrazioni SQL per servizio — DECISA; nessun servizio modifica lo schema altrui
- JEV: integrazione HTTPS diretta a `POST /v1/systemone` tramite adapter Rust; nessun SDK Rust community nella prima iterazione; nessun sidecar (sarebbe un quarto servizio). Nessuna etichetta di stato esplicita, ma formulata come scelta ferma e coerente con D-0002-e. Trattata come DECISA.
  - Modello testuale: alias configurabile, inizialmente `jev-latest`
  - Immagini: sperimentali, non richieste dall'happy path
  - Timeout stretto; retry solo transitori entro deadline globale
  - Fallback: rifiuto, baseline locale o risposta degradata secondo policy
  - Test: fake locale del `DecisionEngine`; nessuna API reale nei test unitari

---

## 3. Decisioni USER-RATIFIED (2026-10-18, successive ai documenti)

- U-01: Ambiente locale = Docker Desktop + Docker Compose.
  - source: decisione utente comunicata dall'orchestratore /gsd:ingest-docs (2026-10-18)
  - effetto: chiude la decisione aperta "Container e ambiente locale" (DEPENDENCIES.md) e "Docker/Compose oppure manifest compatibili anche con Podman" (docs/architecture/architecture.md). La direzione proposta in DEPENDENCIES.md (Dockerfile multi-stage separati per frontend e servizi Rust, Compose per PostgreSQL/servizi/frontend/eventuale object storage, immagini non-root, health check, filesystem read-only dove possibile, stesso artefatto test/deploy, nessun segreto in immagini o file Compose versionati) diventa il riferimento.
- U-02: Metodologia di sviluppo = loop a 4 verbi GSD+ECC (vedi CLAUDE.md). Irrilevante per i requisiti di prodotto; non genera requisiti.

---

## 4. Decisioni PROPOSTA (default da validare con spike — NON locked)

source: DEPENDENCIES.md salvo diversa indicazione

Runtime / backend:
- Async runtime: Tokio
- HTTP framework: `axum`, `tower`, `tower-http` (timeout, request ID, tracing, limiti body, CORS)
- Serializzazione: `serde`, `serde_json`
- Accesso PostgreSQL: `sqlx` (evitare ORM condivisi tra servizi)
- Vector search: PostgreSQL + `pgvector` (indice e metrica dopo benchmark). Anche source: README.md ("candidato predefinito"), docs/architecture/architecture.md ("candidato")
- Token: JWT firmati asimmetricamente, verifica locale in ogni servizio. Nota: docs/architecture/interfaces-and-concurrency.md presuppone già `/.well-known/jwks.json` e refresh token.
- HTTP client: `reqwest` con Rustls (JEV, LLM, embedding, fonti online)
- Error handling: `thiserror`; `anyhow` solo ai bordi
- Telemetria: `tracing` + OpenTelemetry

Frontend:
- Build: Vite
- Routing: React Router
- Server state: TanStack Query
- PDF viewer: PDF.js con wrapper React sottile (pagina e highlight citazioni)
- API client: generato da OpenAPI
- Component test: Vitest + Testing Library
- E2E: Playwright (login, upload, query, citation, RBAC)

Persistenza:
- Audit/outbox: tabelle append-only PostgreSQL, eventi idempotenti con `event_id` univoco

Cache:
- Porta `Cache` con primo adapter `NoOpCache`; Redis non introdotto prima di una misura che lo giustifichi (vedi constraints.md C-28). source: DEPENDENCIES.md, docs/architecture/architecture.md

Parametri sperimentali (source: docs/architecture/interfaces-and-concurrency.md):
- Baseline query: top-20 dal retrieval, massimo 8 decisioni JEV contemporanee, deadline complessiva configurabile. Da calibrare.
- Firme Rust delle porte: concettuali, da validare con spike.

---

## 5. Decisioni APERTE

- A-01: Tooling OpenAPI — `utoipa` (generata da Rust) oppure specifica contract-first.
  source: DEPENDENCIES.md, docs/architecture/architecture.md
- A-02: Storage originali PDF/DOCX — object storage S3-compatible (MinIO) oppure PostgreSQL `bytea`.
  source: DEPENDENCIES.md, docs/architecture/architecture.md, docs/adr/0001-system-boundaries-and-stack.md (conseguenze)
- A-03: Embedding model — criteri: qualità, lingua, dimensione vettore, latenza, esecuzione locale. source: DEPENDENCIES.md
- A-04: Generative LLM — criteri: grounding, output strutturato, latenza, privacy, hardware. source: DEPENDENCIES.md
- A-05: OCR/parser — criteri: testo, tabelle, coordinate, PDF nativi e scannerizzati. source: DEPENDENCIES.md
- A-06: Reranker baseline di confronto — JEV vs no-rerank vs cross-encoder. Nota: JEV per il reranking è LOCKED (D-0002-c); l'apertura riguarda solo la baseline di confronto/fallback. source: DEPENDENCIES.md
- A-07: Caching — `NoOpCache` iniziale vs Redis dopo benchmark; criteri Redis: deployment multi-replica, hit rate misurato, invalidazione, costo operativo. source: DEPENDENCIES.md, docs/architecture/architecture.md
- A-08: Container strategy — CHIUSA da U-01 (Docker Desktop + Compose). Restano da verificare in pratica: tempi di build Rust, caching dei layer, dimensione immagini, esperienza Windows. La compatibilità Podman non è più un criterio di decisione. source: DEPENDENCIES.md, docs/architecture/architecture.md
- A-09: Strategia di batching JEV — più domande in una richiesta oppure chiamate concorrenti per candidato. source: docs/architecture/interfaces-and-concurrency.md (spike)
- A-10: Parser/OCR in-process oppure worker/processo isolato (sempre all'interno del Document Service, vincolo D-0001 conseguenze). source: docs/architecture/interfaces-and-concurrency.md (spike)

### Spike richiesti (source: docs/architecture/interfaces-and-concurrency.md; ADR-0001 conseguenze)
- S-01: Streaming multipart e memoria in Axum.
- S-02: Parser/OCR in-process oppure worker isolato.
- S-03: `pgvector` sul corpus incrementale.
- S-04: JEV — batching vs chiamate concorrenti per candidato.
- S-05: Cancellazione verso provider HTTP.
- S-06: Invalidazione cache, se introdotta.
- S-07: Firme Rust definitive delle porte (validazione dello spike generale).

---

## 6. Esclusioni esplicite dall'MVP

- MFA / secondo fattore. source: docs/adr/0001-system-boundaries-and-stack.md, README.md, docs/product/requirements.md
- Kubernetes e service mesh. source: DEPENDENCIES.md
- Message broker dedicato senza requisito misurato. source: DEPENDENCIES.md
- Database vettoriale separato prima del benchmark di `pgvector`. source: DEPENDENCIES.md
- Framework agentico per una pipeline deterministica. source: DEPENDENCIES.md
- Cache distribuita introdotta senza misura. source: DEPENDENCIES.md
- Sidecar JEV (quarto servizio). source: DEPENDENCIES.md
- Plugin dinamici (Open-Closed tramite trait stabili, non plugin runtime). source: docs/architecture/architecture.md
- Auto-approvazione dei documenti scoperti online (disabilitata inizialmente). source: docs/data/corpus-strategy.md
