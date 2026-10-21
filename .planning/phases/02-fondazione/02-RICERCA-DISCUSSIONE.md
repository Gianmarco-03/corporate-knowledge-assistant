# 02 — Ricerca decisionale (discuss fase 2: Fondazione)

Brief caveman per area. Ricerca: Claude (Anthropic), canale di default (WebSearch/WebFetch/context7, 4 ricercatori in parallelo), accesso fonti **2026-10-21** (tutte). [NV] = non verificato. Fonti interne del repo prevalgono sull'esterno finché utente non le riapre.

- Area A — Contratti OpenAPI, contract test, client TS (A-01, PLAT-03)
- Area B — Runtime Rust: stack, errori, request_id/trace, backpressure, config, pool, health, shutdown (criterio 4)
- Area C — Dati: isolamento per servizio, migrazioni a fascia, primitive trasversali (PLAT-02, criteri 3 e 5)
- Area D — Frontend shell, proxy, immagini, Compose, CI, toolchain (PLAT-06, criteri 1, 2, 5)
- Area E — Punti ereditati dalla fase 1 e rischio pipeline
- §F — Domande armate per l'utente

---

## Area A — OpenAPI, contract test, client TS

Repo: DEPENDENCIES «OpenAPI: `utoipa` o contract-first — Aperta». ADR-0001 «Contratti HTTP/JSON OpenAPI». Whitelist proprieta-file ha già glob `**/openapi/generated/**` e `apps/web/src/api/generated/**` («da confermare con la decisione»).

Finding:
- utoipa 6.0.0 (2026-09-22), ~55,8M download; OpenAPI 3.1 da 5.0.0, 3.2 opzionale in 6.0; MSRV 1.88. https://crates.io/api/v1/crates/utoipa · https://raw.githubusercontent.com/juhaku/utoipa/master/utoipa/CHANGELOG.md
- utoipa-axum 0.3.0 (2026-09-22) su axum ^0.8.4: `OpenApiRouter` + `routes!`, `split_for_parts` → (Router, OpenApi) da stessa fonte → niente drift rotta/spec. https://docs.rs/utoipa-axum/latest/utoipa_axum/
- aide 0.15.1 (2025-08); 0.16 ferma in alpha da ~11 mesi. https://crates.io/api/v1/crates/aide
- Contract-first Rust debole: openapi-generator `rust-axum` BETA, niente Uuid/Null/union, 3.1 non dichiarato (https://openapi-generator.tech/docs/generators/rust-axum/); progenitor solo client 3.0.x (https://github.com/oxidecomputer/progenitor); dropshot = altro framework, sostituisce axum (https://github.com/oxidecomputer/dropshot).
- Drift: test che serializza `openapi.json` versionato, CI rossa su diff (utoipa-axum stesso usa `insta`). Prassi diffusa [NV].
- Breaking: oasdiff, 3.1 GA da v1.15.0 (limiti: `$dynamicRef`, `components.pathItems`); oasdiff-action `breaking --fail-on ERR` contro ref base. https://github.com/oasdiff/oasdiff/blob/main/docs/OPENAPI-31.md · https://github.com/oasdiff/oasdiff-action
- Contract test: Schemathesis supporta 2.0/3.0/3.1/3.2, property-based, action `schemathesis/action@v3` (https://schemathesis.readthedocs.io/en/stable/); Dredd archiviato 2024-11-08 (https://github.com/apiaryio/dredd); Pact Rust `pact_consumer` 1.4.10 vivo, ma broker/overhead consumer-driven sproporzionato per 3 servizi stesso team [valutazione].
- Client TS: openapi-typescript 7.13.0 («OpenAPI 3.0 and 3.1»), openapi-fetch 0.17.0 (~6 kB), openapi-react-query 0.5.4 (peer TanStack Query ^5.80). https://openapi-ts.dev/introduction · https://registry.npmjs.org/openapi-react-query/latest. @hey-api/openapi-ts 0.99.0 pre-1.0 («pin exact version»), plugin TanStack (https://heyapi.dev/openapi-ts/plugins/tanstack-query). orval 8.43.0, 3.1/nullable non confermato (https://orval.dev/).
- OpenAPI 3.1: Schema Object superset JSON Schema 2020-12. https://spec.openapis.org/oas/v3.1.0.html

**Default raccomandato A-01:** code-first utoipa 6 + utoipa-axum 0.3. Per servizio: snapshot `openapi.json` versionato (CI rossa su diff) + oasdiff breaking contro base + 1 job Schemathesis su servizio avviato = contract test PLAT-03. DTO inter-servizio in `crates/contracts`. Frontend: openapi-typescript + openapi-fetch + openapi-react-query.
Scartati: aide (alpha ferma), contract-first rust-axum (BETA, no Uuid), progenitor (solo client), dropshot (sostituisce axum), Pact (overhead), Dredd (archiviato), orval (3.1 incerto), hey-api (pre-1.0; seconda scelta).

---

## Area B — Runtime Rust

Repo: DEPENDENCIES «Proposta» per Tokio, axum/tower/tower-http, sqlx, serde, thiserror/anyhow, tracing+OTel, reqwest+Rustls. Errori `code`/`message`/`request_id` (CLAUDE.md; interfaces-and-concurrency §Regole). Fase 1 S-05/ADR-0010 (deadline/cancellazione) non ancora eseguito.

Versioni (crates.io API, 2026-10-21):

| crate | versione | nota |
|---|---|---|
| Rust stable | 1.99.1 | edition 2024 stabile da 1.85 (https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/) |
| tokio | 1.54.0 | LTS 1.51.x (→2027-03), 1.53.x (→2027-09) (https://github.com/tokio-rs/tokio/blob/master/README.md) |
| axum | 0.8.9 | path `/{id}`, `:id` **panica a runtime**; handler `Sync` (https://raw.githubusercontent.com/tokio-rs/axum/main/axum/CHANGELOG.md) |
| tower / tower-http | 0.5.3 / 0.7.1 | `TimeoutLayer::new` deprecato → `with_status_code` (https://github.com/tower-rs/tower-http/blob/main/tower-http/CHANGELOG.md) |
| sqlx | 0.9.0 | MSRV 1.94; `query*()` vuole `&'static str` o `AssertSqlSafe`; `cargo install --locked sqlx-cli` rotto (https://raw.githubusercontent.com/transact-rs/sqlx/main/CHANGELOG.md) |
| serde / thiserror / anyhow | 1.0.229 / 2.0.21 / 1.0.104 | |
| tracing / tracing-subscriber | 0.1.44 / 0.3.23 | |
| opentelemetry* | 0.33.1 | lockstep con tracing-opentelemetry 0.34.0 (https://crates.io/api/v1/crates/tracing-opentelemetry/0.34.0/dependencies) |
| reqwest | 0.13.5 | rustls **default** (aws-lc), feature `rustls-tls`→`rustls` (https://github.com/seanmonstar/reqwest/blob/master/CHANGELOG.md) |
| reqwest-tracing | 0.7.1 | si ferma a otel 0.32 → incompatibile |

Errore:
- RFC 9457 (obsoleta 7807): `application/problem+json`, type/status/title/detail/instance, estensioni ammesse (nome ≥3 car., lettere/cifre/`_`), client ignorano le ignote. https://www.rfc-editor.org/rfc/rfc9457.html
- `code`, `request_id` validi come estensioni → busta del progetto e RFC 9457 compatibili [inferenza dall'RFC].
- Comparabili: Google AIP-193 `{code,message,details}`+ErrorInfo (https://google.aip.dev/193); Stripe `{error:{type,code,message,...}}` (https://docs.stripe.com/api/errors); Azure `{error:{code,message,...}}` + header `x-ms-request-id` obbligatorio (https://github.com/microsoft/api-guidelines/blob/vNext/azure/Guidelines.md).

Request id / trace:
- tower-http `set_x_request_id(MakeRequestUuid)` prima di `TraceLayer`, `propagate_x_request_id()` dopo; id upstream non sovrascritto. https://docs.rs/tower-http/latest/tower_http/request_id/index.html
- W3C Trace Context L1 REC (2021-11-23): `traceparent: 00-<32hex>-<16hex>-<2hex>`, tutto-zero invalido. https://www.w3.org/TR/trace-context/
- Propagazione con `opentelemetry-http` (HeaderExtractor/Injector) + propagator globale; iniezione manuale su reqwest (reqwest-tracing in ritardo). API esatte [NV].

Backpressure:
- axum vuole servizi `Infallible` → `HandleErrorLayer` attorno a Timeout/LoadShed/ConcurrencyLimit. https://docs.rs/axum/latest/axum/error_handling/index.html
- 429 = troppe richieste per cliente (RFC 6585 §4, https://www.rfc-editor.org/rfc/rfc6585.html); Retry-After con 429 e 503 (RFC 9110). → 503+Retry-After per load shedding, 429 per rate limit [inferenza]. NB: interfaces-and-concurrency §Backpressure dice «`429` e `Retry-After` quando le code superano le soglie»; PLAT-04 dice «`429` + `Retry-After`».
- tower_governor 0.8.0 (axum 0.8); SmartIpKeyExtractor spoofabile via XFF senza proxy fidato. https://docs.rs/tower_governor/latest/tower_governor/
- `DefaultBodyLimit` 2 MB solo su estrattori; `RequestBodyLimit` globale. https://docs.rs/axum/latest/axum/extract/struct.DefaultBodyLimit.html

Config: `config` 0.15.27 (2026-09-30, mantenuto, ~20,7M dl); figment 0.10.19 (fermo da 2024-05); envy 0.4.2 (fermo 2021). crates.io API. Pratica: validare tutto all'avvio, fail-fast.

Pool: default sqlx max 10, acquire_timeout 30 s (https://raw.githubusercontent.com/transact-rs/sqlx/main/sqlx-core/src/pool/options.rs); PG max_connections «typically 100», 3 riservate (https://www.postgresql.org/docs/current/runtime-config-connection.html); somma pool < max (https://wiki.postgresql.org/wiki/Number_Of_Database_Connections). Offline: `cargo sqlx prepare --workspace` → `.sqlx` versionato, `SQLX_OFFLINE=true`, `prepare --check` in CI (sqlx-cli README).

Health: k8s `/livez` + `/readyz` (`/healthz` deprecato); liveness con DB → restart a cascata. https://kubernetes.io/docs/reference/using-api/health-checks/ · https://kubernetes.io/docs/concepts/configuration/liveness-readiness-startup-probes/. draft-inadarei health-check scaduto 2021 (https://datatracker.ietf.org/doc/draft-inadarei-api-health-check/).

Shutdown: `axum::serve(...).with_graceful_shutdown(signal)` + timeout; flush provider OTel dopo serve. https://raw.githubusercontent.com/tokio-rs/axum/main/examples/graceful-shutdown/src/main.rs

**Default raccomandato B:** Proposte confermate alle versioni sopra, edition 2024, toolchain pin 1.99.1 (≥1.94 per sqlx). Errore = problem+json con estensioni `code`, `request_id`. Request id tower-http + traceparent manuale con opentelemetry-http. Stack HandleError→LoadShed→ConcurrencyLimit→Timeout→RequestBodyLimit configurabile. Config: crate `config` o serde+env, fail-fast. Health `/livez` (solo processo) e `/readyz` (ping DB). Pool esplicito da config, somma < 97.

---

## Area C — Dati e primitive trasversali

Repo: CLAUDE.md «un cluster PostgreSQL, ma schema/database e credenziali separati per servizio»; DEPENDENCIES «Migrazioni SQL per servizio — Decisa», «Audit/outbox append-only — Proposta»; ROADMAP criteri 3 e 5 («fascia di migrazioni»). **Conflitto interno:** ROADMAP fase 2 «collocazione primitive trasversali (idempotency key, outbox append-only, worker con lease): raccomandata in Fondazione» vs `01-CONTRATTO.md` §6 «Restano alle fasi proprietarie: la persistenza dell'idempotenza, l'outbox, …».

Isolamento:
- Connessione = 1 database; schemi non separati rigidamente; USAGE revocato «non completamente sicuro», nomi visibili nei cataloghi. https://www.postgresql.org/docs/current/ddl-schemas.html · https://www.postgresql.org/docs/current/ddl-priv.html
- PUBLIC ha CONNECT+TEMPORARY di default su ogni db → `REVOKE CONNECT, TEMPORARY ON DATABASE x FROM PUBLIC` + GRANT al ruolo. (ddl-priv)
- microservices.io: private-tables / schema / database-server per service; barriere vere = utente per servizio. https://microservices.io/patterns/data/database-per-service.html
- pgvector `vector.control` 0.8.7 **non trusted** → `CREATE EXTENSION` da superuser, una volta per db. https://github.com/pgvector/pgvector/blob/master/vector.control · https://www.postgresql.org/docs/current/sql-createextension.html
- Init Compose: `/docker-entrypoint-initdb.d` solo su volume vuoto; `_FILE` solo per POSTGRES_PASSWORD/USER/DB; PG18 volume su `/var/lib/postgresql` (non `/data`). https://hub.docker.com/_/postgres. Tag `pgvector/pgvector:0.8.7-pg18-trixie` (allineato a pgvector 0.8.7 di onda 1 fase 1).

Migrazioni sqlx 0.9 (sorgente):
- versione = `splitn(2,'_')[0].parse::<i64>()` → `0200_0001_x.sql` = versione **200**: fascia con underscore **rotta**; nessun controllo duplicati visto. https://raw.githubusercontent.com/launchbadge/sqlx/main/sqlx-core/src/migrate/source.rs
- migrazione non applicata con versione **più bassa** dell'ultima: **applicata in silenzio**; checksum diverso → `VersionMismatch`; applicata e assente → `VersionMissing` salvo `ignore_missing`. https://raw.githubusercontent.com/launchbadge/sqlx/main/sqlx-core/src/migrate/migrator.rs
- advisory lock = CRC32(current_database()) → con schema-per-service lock condiviso. https://raw.githubusercontent.com/launchbadge/sqlx/main/sqlx-postgres/src/migrate.rs
- CLI: inferisce timestamp se gap fra versioni → usare `--sequential` o file a mano. https://raw.githubusercontent.com/launchbadge/sqlx/main/sqlx-cli/src/opt.rs
→ fascia = intero unico senza underscore interni (es. `PPNNNN_nome.sql`, `020001_init.sql` = fase 02 progressivo 0001) + gate CI: fasce monotone, no duplicati, file applicati immutabili, mai `ignore_missing`.

Primitive:
- Outbox nella stessa transazione, relay polling, consumer idempotente con `processed_events` PK event_id. https://microservices.io/patterns/data/transactional-outbox.html · https://microservices.io/patterns/communication-style/idempotent-consumer.html
- SKIP LOCKED adatto a code multi-consumer, vista inconsistente per uso generale. https://www.postgresql.org/docs/current/sql-select.html. Lease: `locked_until` + `lease_token` fencing + heartbeat + sweep (pratica, fonti secondarie: https://www.vincentschmalbach.com/building-a-queue-with-postgresql-skip-locked/).
- Idempotency-Key: draft-ietf-httpapi-idempotency-key-header-07 **Expired** (https://datatracker.ietf.org/doc/draft-ietf-httpapi-idempotency-key-header/); 400 mancante, 422 riuso con payload diverso, 409 in corso (https://www.ietf.org/archive/id/draft-ietf-httpapi-idempotency-key-header-07.html). Stripe: risposta memorizzata anche se 500, pruning ≥24 h, niente salvataggio su validazione fallita (https://docs.stripe.com/api/idempotent_requests).
- Dove vivono: microservice chassis = meccanismo condiviso accettato (https://microservices.io/patterns/microservice-chassis.html); Newman: no riuso di modelli di dominio fra servizi (https://www.goodreads.com/author_blog_posts/8594685-answering-questions-from-devoxx-on-microservices). → meccanismo generico in crate interno, tabelle e migrazioni nel servizio [inferenza].

**Default raccomandato C:** database per servizio sullo stesso cluster (non schema), ruolo owner per servizio, REVOKE da PUBLIC; init superuser crea ruoli/db/`vector` (solo document) con password da secret. Test isolamento: connessione o SELECT cross-db fallisce (42501 / connessione rifiutata). Fascia `PPNNNN` + gate. Primitive: meccanismo generico in Fondazione, uso e tabelle di dominio nelle fasi proprietarie — da decidere col conflitto ROADMAP/CONTRATTO.

---

## Area D — Frontend, proxy, immagini, Compose, CI

Repo: DEPENDENCIES «Proposta» Vite, React Router, TanStack Query, client generato, Vitest, Playwright; «Container e ambiente locale — Aperta» (multi-stage, non-root, health, read-only, niente segreti); architecture.md «Reverse proxy / routing» componente di deployment. Decisione utente 2026-10-18: Docker Desktop + Compose.

Versioni npm (registry `/latest`, 2026-10-21): vite 8.3.5; react 19.3.1; **react-router 8.5.0** (v7 su dist-tag); @tanstack/react-query 5.104.5; vitest 5.0.5; @testing-library/react 16.3.3; @playwright/test 1.64.1; **typescript 7.0.2**; tailwindcss 4.3.3; msw 3.0.3. Node LTS attiva 24.22.1, 26 LTS dal 2026-10-28 (https://raw.githubusercontent.com/nodejs/Release/main/schedule.json).
- React Router 8: Node 22.22+, React 19.2.7+, `react-router-dom` rimosso. https://reactrouter.com/upgrading/v7. Modi declarative/data/framework; SPA dietro proxy → **data mode** (`createBrowserRouter`, route a oggetti, `lazy`). https://reactrouter.com/start/modes · https://reactrouter.com/start/data/route-object
- Vite 8 Rolldown, solo ESM (https://vite.dev/blog/announcing-vite8); Vite non fa typecheck → `tsc --noEmit` (https://vite.dev/guide/features).
- TS 7.0 port Go **senza API**: typescript-eslint passa da `@typescript/typescript6`. https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/ → rischio toolchain lint.
- Route registry: registro esplicito `route-registry.ts` (array per feature, riga pre-registrata) vs `import.meta.glob` (magico, ordine da nomi file, contro regola wiring-design «mai auto-discovery»).
- Stile: CSS Modules (zero dipendenze, a11y a mano); Tailwind 4 (`@tailwindcss/vite`); shadcn 4.22 + Radix 1.8 (sorgenti nel repo, ARIA pronta). https://tailwindcss.com/docs/installation/using-vite · https://ui.shadcn.com/docs/installation/vite
- Proxy: Vite `server.proxy` solo in dev; Caddy 2.11.7 in Compose = stesso origin in dev/E2E (cookie HttpOnly senza CORS) [beneficio NV]; `handle` (senza strip) per path `/api/v1/...`. https://caddyserver.com/docs/caddyfile/directives/handle_path
- Immagini Rust: cargo-chef 0.1.78 (workspace ok, stessa versione Rust negli stadi) https://github.com/LukeMathWalker/cargo-chef; runtime `gcr.io/distroless/cc-debian13:nonroot` (debian12 non più aggiornata) https://github.com/GoogleContainerTools/distroless; healthcheck = sottocomando del binario (distroless senza curl), `depends_on: condition: service_healthy`; `read_only`, `tmpfs /tmp`, `cap_drop ALL`, `no-new-privileges`; segreti `secrets:` → `/run/secrets`. https://docs.docker.com/reference/compose-file/services/ · https://docs.docker.com/compose/how-tos/use-secrets/
- Frontend: `nginxinc/nginx-unprivileged` 8080, tmpfs /tmp, fallback SPA. https://hub.docker.com/r/nginxinc/nginx-unprivileged
- CI: dtolnay/rust-toolchain, Swatinem/rust-cache@v2 (save-if main), fmt/clippy `-D warnings`/nextest + `cargo test --doc` (https://nexte.st/docs/integrations/test-coverage/), service container `pgvector/pgvector:pg18-trixie` (https://docs.github.com/en/actions/tutorials/use-containerized-services/create-postgresql-service-containers), Playwright `install --with-deps` (https://playwright.dev/docs/ci-intro). Workspace: `exclude = ["spikes"]`, `resolver = "3"` (https://doc.rust-lang.org/cargo/reference/workspaces.html). `rust-toolchain.toml` con versione esatta (https://rust-lang.github.io/rustup/overrides.html).

Rischi D: repo su `C:\` → bind mount WSL2 lenti, niente inotify (HMR rotto) https://docs.docker.com/desktop/features/wsl/best-practices/; CRLF negli `.sh` rompe container → `.gitattributes eol=lf` [NV oggi]; TS 7 vs typescript-eslint; Garage tag semver da scegliere [NV].

**Default raccomandato D:** Vite 8 + React 19.3 + React Router 8 data mode + TanStack Query 5 + Vitest 5 + Playwright 1.64, Node 24 LTS; TS 7 se lint compatibile, altrimenti 6.x [da verificare in plan]. Registro route esplicito. Caddy come unico origin in Compose + proxy Vite con stessi path per HMR. cargo-chef + distroless debian13 nonroot + healthcheck nel binario. CI: job rust + job web + job contract.

---

## Area E — Punti ereditati e rischio pipeline

Da `01-CONTRATTO.md` (§1, §3, §6) e ADR-0003:
- E1. `Cargo.toml` radice con `exclude = ["spikes"]`, nessuna CI su `spikes/**`, nessuna path dep verso `spikes/` (§6 Vincoli verso la Fondazione). Vincolo, non domanda.
- E2. Tipi destinati a `crates/contracts` «da ratificare in Fondazione»: `Locator`, `ContentHash`, id opachi, `RequestId`, `IdempotencyKey`, `TraceContext`, `EmbeddingSpace`, `RetrievalSearchRequest/Response` (§3). Firme da ADR-0012 (piano 01-16, **non ancora eseguito**).
- E3. Forma definitiva di `POST /internal/v1/retrieval/search`, errori e versione OpenAPI «li decide la Fondazione» (§1).
- E4. Riallineare `docs/architecture/interfaces-and-concurrency.md` a split `Reranker`/`DecisionEngine`, `EvidenceRetriever`, `CallContext`, `MutationContext` (§6; ORCHESTRATOR TODO).
- E5. TLS verso lo storage «resta alla Fondazione» (ADR-0003 §Conseguenze; `01-04-SUMMARY` T-01-04-07 `with_allow_http(true)` solo rete compose).
- E6. Test di contratto serde sul confine DTO ↔ `AuthorizedEvidence` (MEDIUM residuo 01-16, «si porta anche alla Fondazione»).
- E7. Garage dietro DocumentStore (ADR-0003 **Proposed**, ratifica in 01-17): criterio 1 «storage originali se A-02 lo richiede» → Garage in Compose.

Rischio pipeline: fase 1 non verificata (onde 3-11 aperte; ADR 0004-0013 non emessi). Spec §8 pipeline sfalsata: non blocca il discuss; ROADMAP «il discuss delle Fasi 2-4 si riapre solo se le misure dello spike ne cambiano le premesse». Decisioni dipendenti da E2/E7 = provvisorie fino a 01-16/01-17.

Criticità: ROADMAP «normale». Goal non tocca autenticazione, RBAC con retrieval autorizzato né ingestion (solo hook vuoti + isolamento DB). → proposta **normale**: test design da agente Claude separato in plan.

---

## §F — Domande armate (prosa nel discuss)

1. A-01: code-first utoipa 6 + snapshot + oasdiff + Schemathesis; client openapi-typescript/fetch/react-query (Area A).
2. Stack runtime: conferma Proposte alle versioni correnti; reqwest 0.13 rustls default; reqwest-tracing escluso (Area B).
3. Errore: problem+json RFC 9457 con estensioni `code`/`request_id` vs busta JSON propria (Area B).
4. Backpressure: 503+Retry-After per load shedding, 429 per rate limit, vs testo attuale «429 quando le code superano le soglie» (Area B).
5. Isolamento: database per servizio vs schema per servizio (Area C).
6. Fascia migrazioni `PPNNNN` + gate (Area C).
7. Primitive trasversali: conflitto ROADMAP vs CONTRATTO fase 1 (Area C).
8. Frontend: React Router 8 (non 7), TS 7 o 6, stile base (Area D).
9. Proxy: Caddy unico origin in Compose (Area D).
10. TLS storage locale (E5); tipi contracts da trascrivere ora o vuoti (E2).
11. Criticità normale (Area E).

**Modifica:** Claude (Anthropic) · 2026-10-21 · creazione (discuss fase 02, ricerca a tre lenti)
