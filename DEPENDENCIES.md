# Dependencies

Registro delle dipendenze architetturali. Le versioni esatte verranno bloccate nei lockfile quando inizierà l'implementazione.

Legenda: **Decisa** è parte dell'architettura approvata; **Proposta** è il default da validare con uno spike; **Aperta** richiede ancora confronto.

## Runtime e linguaggi

| Area | Scelta | Stato | Motivazione |
|---|---|---|---|
| Frontend | TypeScript + React | Decisa | Tipizzazione dei contratti UI e componenti riusabili |
| Backend | Rust stable | Decisa | Sicurezza di memoria, concorrenza esplicita e apprendimento |
| Async runtime | Tokio | Proposta | Ecosistema de facto per I/O asincrono in Rust |
| HTTP framework | Axum | Proposta | Integrazione naturale con Tokio/Tower e middleware componibili |

## Backend Rust

| Capacità | Candidato | Stato | Note |
|---|---|---|---|
| HTTP server | `axum`, `tower`, `tower-http` | Proposta | Timeout, request ID, tracing, limiti body e CORS |
| Serializzazione | `serde`, `serde_json` | Proposta | Contratti JSON tipizzati |
| PostgreSQL | `sqlx` | Proposta | Query async, transazioni e migrazioni; evitare ORM condivisi tra servizi |
| Vector search | PostgreSQL + `pgvector` | Proposta | Riduce le dipendenze operative nell'MVP |
| Password hashing | `argon2` | Decisa come algoritmo | Password mai cifrate in modo reversibile |
| Token | JWT firmati asimmetricamente | Proposta | Verifica locale in ogni servizio |
| HTTP client | `reqwest` con Rustls | Proposta | JEV, LLM, embedding e fonti online |
| Error handling | `thiserror`; `anyhow` solo ai bordi | Proposta | Errori di dominio espliciti |
| Telemetria | `tracing` + OpenTelemetry | Proposta | Correlazione tra servizi e job |
| OpenAPI | `utoipa` o specifica contract-first | Aperta | Valutare ergonomia e stabilità del contratto |

## Frontend

| Capacità | Candidato | Stato | Note |
|---|---|---|---|
| Build | Vite | Proposta | Setup TypeScript rapido |
| Routing | React Router | Proposta | Routing applicativo |
| Server state | TanStack Query | Proposta | Retry controllati e invalidazione |
| PDF viewer | PDF.js / wrapper React sottile | Proposta | Pagina e highlight delle citazioni |
| API client | Generato da OpenAPI | Proposta | Riduce divergenze frontend/backend |
| Component test | Vitest + Testing Library | Proposta | Test veloci dei componenti |
| E2E | Playwright | Proposta | Login, upload, query, citation e RBAC |

## Persistenza

| Capacità | Scelta | Stato | Note |
|---|---|---|---|
| Dati relazionali | PostgreSQL | Decisa | Un cluster nell'MVP, ownership distinta per servizio |
| Embedding | `pgvector` | Proposta | Indice e metrica dopo benchmark |
| Migrazioni | Migrazioni SQL per servizio | Decisa | Nessun servizio modifica lo schema altrui |
| Originali PDF/DOCX | Object storage S3-compatible oppure PostgreSQL `bytea` | Aperta | MinIO è più realistico; `bytea` riduce l'infrastruttura |
| Audit/outbox | Tabelle append-only PostgreSQL | Proposta | Eventi idempotenti con `event_id` univoco |

Un singolo cluster non significa database condiviso: ogni servizio usa credenziali e schema/database propri e comunica con gli altri attraverso contratti.

## JEV

| Aspetto | Scelta |
|---|---|
| Integrazione | HTTPS diretto a `POST /v1/systemone` tramite adapter Rust |
| SDK | Nessun SDK Rust community nella prima iterazione; usare il contratto HTTP ufficiale |
| Modello testuale | Alias configurabile, inizialmente `jev-latest` |
| Immagini | Sperimentali e non richieste dall'happy path iniziale |
| Timeout/retry | Timeout stretto; retry solo transitori ed entro deadline globale |
| Fallback | Rifiuto, baseline locale o risposta degradata secondo policy |
| Test | Fake locale del `DecisionEngine`; nessuna API reale nei test unitari |

Gli SDK ufficiali disponibili sono TypeScript e Python. Un sidecar dedicato aggiungerebbe un quarto servizio senza un nuovo bounded context; l'API JEV espone invece un contratto OpenAPI pubblico e tipizzato.

Riferimenti ufficiali:

- API: <https://docs.typesafe.ai/api>
- OpenAPI: <https://api.typesafe.ai/openapi.json>
- SDK Python: <https://github.com/typesafe-ai/typesafe-sdk-python>
- SDK TypeScript: <https://github.com/typesafe-ai/typesafe-sdk-js>

## Modelli AI

| Capacità | Stato | Criteri di scelta |
|---|---|---|
| Embedding model | Aperta | Qualità, lingua, dimensione vettore, latenza, esecuzione locale |
| Generative LLM | Aperta | Grounding, output strutturato, latenza, privacy e hardware |
| OCR/parser | Aperta | Testo, tabelle, coordinate, PDF nativi e scannerizzati |
| Reranker baseline | Aperta | Confronto JEV, no-rerank e cross-encoder |

Ogni provider implementa una porta interna. I tipi del provider non attraversano il dominio.

## Caching — decisione aperta

Il codice dipenderà da una porta `Cache`, ma il primo adapter sarà `NoOpCache`. Redis non verrà introdotto prima di avere una misura che lo giustifichi.

Possibili usi futuri:

- JWKS e configurazioni non sensibili;
- embedding di query normalizzate;
- risultati di retrieval con identità, policy e document versions nella chiave;
- rate limiting distribuito;
- stato effimero di job e lock distribuiti.

Non cacheare nell'MVP risposte complete o contenuti autorizzati senza includere identità, versione della policy e versioni dei documenti. I criteri per scegliere Redis saranno deployment multi-replica, hit rate misurato, invalidazione e costo operativo.

## Container e ambiente locale — decisione aperta

Direzione proposta:

- Dockerfile multi-stage separato per frontend e servizi Rust;
- Docker Compose per PostgreSQL, servizi, frontend ed eventuale object storage;
- immagini non-root, health check e filesystem read-only dove possibile;
- stesso artefatto tra test e deployment;
- nessun segreto nelle immagini o nei file Compose versionati.

Prima di chiudere la decisione vanno confrontati tempi di build Rust, caching dei layer, dimensione immagini, esperienza Windows e compatibilità Podman.

## Dipendenze da evitare nell'MVP

- Kubernetes e service mesh.
- Message broker dedicato senza requisito misurato.
- Database vettoriale separato prima del benchmark di `pgvector`.
- Framework agentico per una pipeline deterministica.
- Cache distribuita introdotta senza una misura.
