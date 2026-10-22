# Phase 2: Fondazione - Context

**Gathered:** 2026-10-21
**Status:** Ready for planning
**Criticità:** normale — proposta da ROADMAP, ratificata dall'utente con questo CONTEXT. Il goal non tocca autenticazione, RBAC con retrieval autorizzato né ingestion: la fase crea solo innesti vuoti per quelle fasi. In `plan`, il test design lo fa un agente Claude separato, non GPT. L'isolamento del DB resta comunque nel perimetro del gate `security-scanner`.

<domain>
## Phase Boundary

La fase costruisce lo scheletro eseguibile e completo delle parti comuni:

- il workspace Cargo (tre servizi, `crates/contracts`, `crates/telemetry`, `spikes/` escluso);
- in ogni servizio: composition root, config tipizzata, pool DB limitato, modello d'errore, propagazione di `request_id` e trace context, telemetria, layer di backpressure configurabile, health check e rotta di prova;
- l'isolamento dei dati per servizio, con il test che lo dimostra;
- il framework delle migrazioni a fascia;
- la shell di `apps/web` con registro delle route e stile di base;
- Compose da clone pulito (PostgreSQL + pgvector, Garage, tre servizi, frontend, reverse proxy);
- la CI GitHub Actions a ogni push con i contract test;
- gli **innesti vuoti** di ogni fase 3-15, registrati in `.planning/WIRING-MAP.md`.

Nessuna logica di dominio: niente autenticazione, upload, retrieval, query o primitive di idempotenza/outbox (D-10).

Requisiti: PLAT-02, PLAT-03, PLAT-06. Criteri di successo 1-5 del ROADMAP, fase 2.

Ricerca a supporto: `02-RICERCA-DISCUSSIONE.md`, con le Aree A-E e le fonti (URL e data di accesso 2026-10-21). Ogni decisione qui sotto cita l'area che la fonda.

**Rischio di pipeline (spec §8):** la fase 1 non è verificata. Le onde 3-11 sono aperte, ADR 0004-0013 non sono emessi, ADR-0003 è `Proposed`. Le decisioni che dipendono da ADR-0012 (D-22) e da ADR-0003 (D-18) restano provvisorie fino a 01-16/01-17. Secondo il ROADMAP, il discuss si riapre solo se le misure ne cambiano le premesse.

</domain>

<decisions>
## Implementation Decisions

### Contratti API (A-01, PLAT-03)
- **D-01: Contract-first.** Ogni servizio ha uno YAML OpenAPI 3.1, fonte di verità del contratto, scritto prima del codice. Una rotta nuova nasce nello YAML durante il `plan` della fase che la introduce.
  - Fonte: Area A. Il contract-first Rust con generatore server è debole (openapi-generator `rust-axum` BETA senza Uuid; progenitor solo client).
  - Decisione utente: utoipa code-first scartato per avere il contratto come artefatto di progetto, coerente con la pianificazione delle fasi consumatrici contro i contratti (spec loop §8).
- **D-02: Trait `Contract` in `crates/contracts`** con `NAME`, `VERSION`, `validate()` e `from_json()` di default (deserializzazione serde + `validate`). Le struct concrete, le loro `impl Contract`, il router axum e gli handler si **scrivono a mano**.
  - Un oggetto vivo a runtime nasce solo da `from_json` sul body HTTP ricevuto.
  - Nessun generatore di codice nostro, né Python né Rust, e nessun servizio generatore a runtime. Motivi: i tipi Rust esistono solo a compile time; la regola dei tre servizi.
  - Percorso della discussione: un generatore Python di fixture è stato proposto e poi scartato dall'utente, perché Schemathesis copre lo stesso ruolo (D-03).
  - Fonte: Area A; decisione utente.
- **D-03: Contract test per servizio, tre difese in CI.**
  1. oasdiff `breaking --fail-on ERR` sullo YAML contro il branch base.
  2. Schemathesis `--mode all` contro il servizio avviato, con i check `response_schema_conformance` e `negative_data_rejection`. Copre risposte fuori spec, rifiuto dei dati non validi e rotte dichiarate ma assenti.
  3. Test unitari Rust mirati su `validate()`.

  Schemathesis si usa come strumento esterno (GitHub Action o immagine Docker): nessun codice Python di prodotto. Pact scartato (broker, overhead); Dredd archiviato.
  - Fonte: Area A; https://schemathesis.readthedocs.io/en/stable/explanations/data-generation/ e https://schemathesis.readthedocs.io/en/latest/reference/checks/ (consultati 2026-10-21).
- **D-04: Client TypeScript generato dagli YAML** con openapi-typescript 7.13 + openapi-fetch 0.17 + openapi-react-query 0.5, in `apps/web/src/api/generated/` (glob già in whitelist come rigenerato). Fonte: Area A.
- **D-05: Errori in `application/problem+json` (RFC 9457)** con le estensioni `code`, `message` e `request_id`. L'header `x-request-id` è presente anche nella risposta. Fonte: Area B (RFC 9457 §3.2, estensioni ammesse).
- **D-06: Backpressure.** Saturazione e load shedding rispondono **503 + Retry-After**; **429** resta per il rate limit per client, che è della fase 14.
  - Lo stack di layer configurabile è: HandleError → LoadShed → ConcurrencyLimit → Timeout → RequestBodyLimit.
  - `docs/architecture/interfaces-and-concurrency.md` §Backpressure e la formulazione di PLAT-04 vanno allineati, con un paragrafo *Storia*.
  - Fonte: Area B (RFC 6585 §4, RFC 9110 Retry-After, axum error_handling).

### Dati e migrazioni (PLAT-02, criteri 3 e 5)
- **D-07: Schema per servizio** in un unico database del cluster: schemi `identity`, `document` e `rag`, ciascuno con un ruolo proprio.
  - Il `search_path` del ruolo è fissato sul proprio schema, così `_sqlx_migrations` resta per schema.
  - `REVOKE` su `public` da PUBLIC.
  - `CREATE EXTENSION vector` lo esegue il superuser dello script di init, nello schema `document`: pgvector non è trusted.
  - Password dei ruoli da file secret.
  - Fonte: Area C (doc PostgreSQL ddl-schemas, ddl-priv, sql-createextension; `vector.control`).
  - Decisione utente, contro la raccomandazione del brief (database per servizio).
- **D-08: Rischi residui di D-07, accettati dall'utente.**
  - I nomi degli oggetti degli altri schemi restano visibili nei cataloghi; il contenuto no.
  - L'advisory lock delle migrazioni di sqlx è unico per database, quindi le migrazioni dei servizi si serializzano.
  - Il test di isolamento (criterio 3): una `SELECT` del ruolo di un servizio su una tabella di un altro schema fallisce con SQLSTATE `42501`.
  - Il rischio sui cataloghi va nel threat model.
  - Fonte: Area C.
- **D-09: Fascia delle migrazioni = intero unico `PPNNNN_nome.sql`** in una cartella `migrations/` per servizio. Esempio: `050001_users.sql` = fase 05, progressivo 0001.
  - Il motivo è nel sorgente di sqlx: la versione è il prefisso prima del primo `_`, e sqlx applica in silenzio le migrazioni fuori ordine.
  - Gate CI: fasce monotone per fase, nessun duplicato, file già committati immutabili, mai `ignore_missing`.
  - La CLI si usa con `--sequential`, oppure i file si creano a mano.
  - Fonte: Area C (sqlx `source.rs`, `migrator.rs`, `opt.rs`).
- **D-10: Primitive trasversali alle fasi proprietarie.**
  - Idempotency key, outbox con consumer idempotente e worker con lease e `SKIP LOCKED` le costruisce ogni servizio nella prima fase che le usa.
  - La duplicazione fra servizi diversi è accettata.
  - Nello stesso servizio, una fase successiva le riusa senza modificarle; una modifica passa da una gap-closure.
  - La Fondazione non crea niente per queste primitive, neanche seam vuoti.
  - La raccomandazione «raccomandata in Fondazione» di ROADMAP/STATE va annotata come superata, con un paragrafo *Storia* (via `gsd-tools`).
  - Fonte: `01-CONTRATTO.md` §6 («Restano alle fasi proprietarie…»); Area C (Newman: niente riuso di dominio fra servizi; microservices.io chassis); decisione utente, contro la raccomandazione del brief.

### Ambiente, build e CI (PLAT-06, criteri 1 e 2)
- **D-11: Le Proposte di runtime diventano Decise** in `DEPENDENCIES.md`, alle versioni del 2026-10-21:
  - toolchain Rust 1.99.1 in `rust-toolchain.toml`, edition 2024 (sqlx 0.9 richiede MSRV 1.94);
  - tokio 1.54, axum 0.8.9 (rotte `/{id}`: `:id` va in panic a runtime), tower 0.5, tower-http 0.7 (`TimeoutLayer::with_status_code`);
  - sqlx 0.9 (`AssertSqlSafe` per l'SQL dinamico), serde, thiserror 2 (anyhow solo ai bordi);
  - tracing + opentelemetry 0.33.x con tracing-opentelemetry 0.34, versioni allineate;
  - reqwest 0.13 con rustls (default); **reqwest-tracing escluso**, perché fermo a otel 0.32: il `traceparent` si inietta a mano con `opentelemetry-http`;
  - config tipizzata con il crate `config` 0.15, validata all'avvio (fail-fast).

  Fonte: Area B.
- **D-12: Immagini.** cargo-chef + `gcr.io/distroless/cc-debian13:nonroot`, con il sottocomando `healthcheck` nel binario di ogni servizio (distroless non ha curl).
  - Compose: `read_only`, `tmpfs /tmp`, `cap_drop: [ALL]`, `no-new-privileges`, `secrets:` montati in `/run/secrets`, `depends_on: condition: service_healthy`.
  - Frontend su `nginxinc/nginx-unprivileged` con fallback SPA.
  - Fonte: Area D.
- **D-13: Reverse proxy Caddy 2.11** in Compose come unico origin. Usa `handle` senza strip del prefisso: `/api/v1/auth/*` e `/api/v1/users/*` → identity, `/api/v1/documents/*` e `/api/v1/ingestion-jobs/*` → document, `/api/v1/queries/*` e `/api/v1/audit-events/*` → rag, `/` → frontend.
  - Il proxy di Vite con gli stessi path serve solo per l'HMR in sviluppo.
  - Fonte: Area D; `docs/architecture/architecture.md` (vista dei container).
- **D-14: Rotte `/internal/v1`.**
  - Caddy non le instrada (404 dall'esterno), e i servizi le espongono solo sulla rete Compose interna.
  - Slot vuoto per il verificatore del token di servizio, assegnato in WIRING-MAP alla fase che introduce la prima rotta interna autenticata (5 o 7, da fissare in `wiring-design`).
  - Rischio residuo nel threat model fino al riempimento dello slot.
  - Fonte: CLAUDE.md (deny by default); decisione utente.
- **D-15: TLS verso Garage.** HTTP solo su una rete Compose `internal: true`, con Garage non esposto all'host. Rischio residuo nel threat model. TLS obbligatorio in fase 15 per qualunque deployment non locale. Chiude il rinvio di ADR-0003 §Conseguenze. Fonte: Area E5; decisione utente.
- **D-16: Niente bind mount del codice.** Il repo resta su `C:\`: Compose costruisce le immagini, Vite e cargo girano sull'host in sviluppo. `.gitattributes` con `eol=lf` per script ed entrypoint. Fonte: Area D (Docker Desktop WSL best practices).
- **D-17: CI a ogni push**, con tre famiglie di job:
  - **rust**: fmt, clippy `-D warnings`, nextest + doctest, PostgreSQL come service container;
  - **web**: npm ci, lint, `tsc --noEmit`, vitest;
  - **contract**: stack Compose avviato, Schemathesis, oasdiff.

  Fonte: Area D; ROADMAP criterio 2; decisione utente («a ogni push»).
- **D-18: Garage nel Compose della Fondazione**, perché il criterio 1 dice «storage degli originali se A-02 lo richiede» e A-02 = Garage (D-01 di fase 1). La configurazione (parti, bucket) è dell'adapter della fase 3. Provvisoria fino alla ratifica di ADR-0003 in 01-17. Fonte: Area E7.

### Telemetria, log e health (criterio 4)
- **D-19: Telemetria.**
  - Log JSON strutturati su stdout.
  - Export OTLP attivo solo se `OTEL_EXPORTER_OTLP_ENDPOINT` è impostata.
  - La Fondazione aggiunge `docker-compose.observability.yml` con Jaeger v2 (OTLP nativo; v1 in EOL dal 2025-12-31), così la trace cross-servizio si vede subito. Il file passa alla fase 14 come innesto (`fase-proprietaria` 14 nella WIRING-MAP).
  - Fonte: https://www.cncf.io/blog/2024/11/12/jaeger-v2-released-opentelemetry-in-the-core/ (consultato 2026-10-21); ROADMAP fase 14 (perimetro: override Compose di osservabilità).
- **D-20: Redazione di default nello scheletro.**
  - Un tipo `Secret<T>`/redatto in `crates/telemetry`, con `Debug` = `[REDACTED]`.
  - I segreti della config sono redatti nel `Debug`.
  - I body di richiesta e risposta non si loggano mai di default.
  - Un test dimostra la redazione.
  - Fonte: OWASP Logging Cheat Sheet (https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html, consultato 2026-10-21); regola no-leak di CLAUDE.md.
- **D-21: Health.** `/livez` controlla solo il processo; `/readyz` include il ping al DB. Fonte: Area B (Kubernetes health checks; la liveness con DB provoca riavvii a cascata).

### Tipi condivisi (Area E2)
- **D-22: In `crates/contracts`, i tipi trasversali subito e quelli di dominio come file vuoti.**
  - La Fondazione scrive subito i tipi che usa lei: `RequestId`, `TraceContext` (stringa W3C validata), gli id opachi UUID, il modello d'errore e il trait `Contract`.
  - Per `Locator`, `ContentHash`, `EmbeddingSpace`, `RetrievalSearchRequest` e `RetrievalSearchResponse` crea file-modulo vuoti, assegnati nella WIRING-MAP alle fasi proprietarie (3 e 7), che li trascrivono da ADR-0012.
  - Fonte: `01-CONTRATTO.md` §3 e §6; decisione utente.

### Shell frontend (criterio 1 e 5)
- **D-23: React Router 8 in data mode** (`createBrowserRouter`, route a oggetti con `lazy`). Registro esplicito `route-registry.ts` con una riga pre-registrata per fase, che importa il file di route della feature; niente `import.meta.glob`. Stack: React 19.3, Vite 8, TanStack Query 5, Vitest 5, Testing Library, Playwright 1.64, Node 24 LTS. Fonte: Area D; regola wiring-design (mai auto-discovery).
- **D-24: TypeScript 7 se il lint regge.** In `plan` si verifica typescript-eslint con TS 7 (TS 7 non ha API; esiste il pacchetto `@typescript/typescript6`). Se non regge: TS 6.x, registrato in DEPENDENCIES. Fonte: Area D.
- **D-25: Stile.** Tailwind 4 (`@tailwindcss/vite`) + shadcn/Radix per i componenti accessibili. Le fasi aggiungono componenti con la CLI nei propri file. Fonte: Area D; decisione utente.

### Vincoli da WIRING-MAP e fase 1
- La WIRING-MAP non contiene righe: nessun seam da **riempire**. La fase 2 **predispone** i seam di tutte le fasi 3-15: righe da `WM-001`, scritte da `wiring-design` in `plan` (spec loop §1-bis, pre-registrazione di progetto). Già decisi qui:
  - `docker-compose.observability.yml` → fase 14 (D-19);
  - slot del verificatore del token di servizio → 5 o 7 (D-14);
  - file di dominio vuoti in `crates/contracts` → 3 e 7 (D-22);
  - una riga del route registry per ogni fase con UI;
  - una fascia di migrazioni per fase e per servizio (D-09).
- `Cargo.toml` alla radice con `[workspace] exclude = ["spikes"]`, `resolver = "3"`. Nessuna CI su `spikes/**` e nessuna path dependency verso `spikes/` (`01-CONTRATTO.md` §6).
- Le firme si trascrivono, non si importano. Collocazione dei tipi: §6 del CONTRATTO di fase 1 (`RetrievedEvidence` interno a document, `AuthorizedEvidence` interno a rag).

### Claude's Discretion
- Struttura interna dei job CI, cache (rust-cache con `save-if` su main) e modalità offline di sqlx (`.sqlx` versionato + `prepare --check`).
- Porte dei servizi, nomi dei ruoli DB, valori di default di pool (somma < 97 connessioni), timeout e limiti di concorrenza del layer di backpressure.
- Tag esatti di Jaeger v2, Garage e cargo-chef, da fissare in `plan` (versioni stabili non verificate in discuss).
- Formato e posizione dello YAML OpenAPI per servizio (es. `services/<svc>/openapi.yaml`), purché coerenti con i glob della whitelist.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Ricerca di fase
- `.planning/phases/02-fondazione/02-RICERCA-DISCUSSIONE.md` — brief a 5 aree (A OpenAPI, B runtime Rust, C dati, D frontend/Compose/CI, E punti ereditati) con fonti e data di accesso.

### Decisioni di progetto
- `.planning/ROADMAP.md` §Phase 2 e §Strategia di consegna — criteri di successo, perimetro file, whitelist, insiemi chiusi.
- `.planning/PROJECT.md` — U-01 (Docker Compose), U-04 (provider-first, insiemi chiusi).
- `DEPENDENCIES.md` — da aggiornare: Proposte → Decise (D-11), OpenAPI → contract-first (D-01), stack frontend (D-23..D-25), Caddy, Jaeger, Schemathesis/oasdiff; regole superate con paragrafo *Storia*.
- `docs/adr/0001-system-boundaries-and-stack.md` — tre servizi, contratti HTTP/JSON OpenAPI.
- `docs/adr/0003-storage-originali-garage.md` — Garage (Proposed), TLS rinviato alla Fondazione (chiuso da D-15).
- `scripts/proprieta-file.whitelist.json` — glob dei file rigenerati/append-only (client TS generato, `Cargo.toml` deps, migrazioni).

### Contratti ereditati
- `.planning/phases/01-spike-tecnici/01-CONTRATTO.md` §1, §3, §6 — DTO candidati, tipi destinati a `crates/contracts`, vincoli verso la Fondazione, primitive alle fasi proprietarie.
- `docs/adr/0012-firme-porte-candidate.md` — **non ancora emesso** (piano 01-16); fonte delle firme da trascrivere (D-22).

### Architettura
- `docs/architecture/interfaces-and-concurrency.md` — rotte per servizio, regole dei contratti, concorrenza, backpressure. Da riallineare in questa fase: split `Reranker`/`DecisionEngine`, `EvidenceRetriever`, `CallContext`, `MutationContext` (Area E4), e 503/429 (D-06).
- `docs/architecture/architecture.md` — vista dei container (reverse proxy), ownership dei dati.
- `.planning/WIRING-MAP.md` — registro dei seam, regole (una `fase-proprietaria` per file d'innesto).
- `docs/superpowers/specs/loop-gsd-ecc-design.md` §1-bis, §8 — pre-registrazione degli innesti, contratto congelato in exec.

### Requisiti
- `.planning/REQUIREMENTS.md` — PLAT-02, PLAT-03, PLAT-06 (e PLAT-04 per la formulazione 429/503).

### Sicurezza
- `docs/security/threat-model.md` — da integrare con i rischi residui di D-08, D-14 e D-15 (compilazione in 01-17).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Nessun codice di prodotto: `services/*`, `crates/*`, `apps/web`, `infra/*` e `.github/workflows` contengono solo `.gitkeep`.
- `spikes/` (fuori dal workspace) contiene materiale di riferimento non importabile: compose e `session.sh` della fase 1 (gestione segreti, togli-env-host di 01.1), adapter Garage dello spike S-01 (parti da 2 MiB, 1 in volo), firme S-07 (quando emesse). Si consultano e si trascrivono, non si importano.

### Established Patterns
- Hook git `.githooks/pre-commit` e `pre-push` attivi. È aperta la questione dell'hook che rifiuta i commit di sole evidenze `.txt`/`.csv` (TODO in ORCHESTRATOR-STATUS).
- Gate di piano: `scripts/gate-perimetro.cjs`, `scripts/gate-proprieta-file.cjs` (legge la colonna `fase-proprietaria` della WIRING-MAP).
- Lezione di fase 1.1: le variabili d'ambiente dell'host (es. `DATABASE_URL` di sistema) possono prevalere su `--env-file` nell'interpolazione di compose. Il Compose di prodotto deve leggere i segreti da `secrets:`/file, non dall'ambiente dell'host.

### Integration Points
- Ogni fase 3-15 si aggancia agli innesti di questa fase: rotte montate per servizio, slot dei provider nel composition root, fascia di migrazioni, voce nel route registry, file vuoti in `crates/contracts`.

</code_context>

<specifics>
## Specific Ideas

- L'utente vuole un'interfaccia `Contract` che garantisca lo sviluppo di ogni nuovo contratto, con un costruttore che riceve una stringa JSON, la controlla e crea l'oggetto (D-02): «parse, don't validate».
- L'utente preferisce scrivere a mano le classi Rust e tenere la toolchain semplice: senza generatori nostri.

</specifics>

<deferred>
## Deferred Ideas

- Rate limit per client (429, `tower_governor`): fase 14 (PLAT-04).
- Autenticazione servizio-servizio sulle rotte `/internal/v1`: fase 5 o 7 (slot vuoto, D-14).
- TLS su tutti i collegamenti per deployment non locali: fase 15 (D-15).
- Dashboard e benchmark di osservabilità: fase 14 (estende D-19).
- Passaggio a Node 26 LTS dopo il 2026-10-28: valutazione in `plan` o in una fase successiva.

</deferred>

## TODO aperti dal discuss

- Aggiornare `DEPENDENCIES.md` (D-01, D-11, D-23..D-25, Caddy, Jaeger, Schemathesis, oasdiff) e `interfaces-and-concurrency.md` (E4, D-06), con paragrafi *Storia*: in `plan` o come primo piano della fase.
- ROADMAP/STATE: annotare che la raccomandazione sulle primitive in Fondazione è superata da D-10 (solo via `gsd-tools`).
- Il tipo `IdempotencyKey`, che il CONTRATTO di fase 1 destina a `crates/contracts`: con D-10 non lo crea la Fondazione. In `wiring-design` va deciso se è un file vuoto di `crates/contracts` assegnato alla prima fase che lo usa (5 o 6) o un tipo interno a ogni servizio. Assunzione provvisoria: file vuoto assegnato alla fase 6.

---

*Phase: 02-fondazione*
*Context gathered: 2026-10-21*

**Modifica:** Claude (Anthropic) · 2026-10-21 · creazione (discuss fase 02)
