# Dependencies

**Modifica:** Claude (Anthropic) · 2026-10-26 · righe aggiornate con le decisioni degli spike ratificate dall'utente nel piano 01-17 (ADR 0003-0011), ciascuna con il link al proprio ADR e un paragrafo *Storia* sotto la tabella

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
| Vector search | PostgreSQL + `pgvector`, ricerca esatta | Decisa | Ricerca esatta con il predicato ACL nella stessa query SQL, coseno su `vector(1024)`, nessun indice ANN né post-filtro; ricerca ibrida non adottata (+0,0000 di recall@10 contro +0,03); rivalutazione verso HNSW oltre 50.000 chunk con una nuova misura della recall filtrata ([ADR-0006](docs/adr/0006-indice-vettoriale-ricerca-esatta-ibrida.md); D-12, D-15) |
| Password hashing | `argon2` | Decisa come algoritmo | Password mai cifrate in modo reversibile |
| Token | JWT firmati asimmetricamente | Proposta | Verifica locale in ogni servizio |
| HTTP client | `reqwest` 0.13 con Rustls | Decisa | JEV, LLM, embedding e fonti online; un `reqwest::Client` condiviso con pool, HTTP/2 preferito, verifica dei certificati di default, `https_only` verso gli host esterni, un solo retry dentro la deadline globale ([ADR-0010](docs/adr/0010-deadline-cancellazione-http.md); esito di S-05) |
| Error handling | `thiserror`; `anyhow` solo ai bordi | Proposta | Errori di dominio espliciti |
| Telemetria | `tracing` + OpenTelemetry | Proposta | Correlazione tra servizi e job |
| OpenAPI | `utoipa` o specifica contract-first | Aperta | Valutare ergonomia e stabilità del contratto |

*Storia (2026-10-26, piano 01-17):* fino alla ratifica, «Vector search» era «PostgreSQL + `pgvector`», **Proposta**, con la nota «Riduce le dipendenze operative nell'MVP»; lo spike S-03 (ADR-0006) l'ha resa Decisa nella forma della ricerca esatta. «HTTP client» era «`reqwest` con Rustls», **Proposta**; lo spike S-05 (ADR-0010) ha rispettato tutte le soglie su deadline e cancellazione e l'ha resa Decisa, con la versione 0.13.

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
| Embedding | `pgvector`, `vector(1024)` | Decisa | Metrica coseno, ricerca esatta senza indice ANN fino a 50.000 chunk (end-to-end 46,0 ms a 10.000 chunk, 0 chunk negati restituiti) ([ADR-0006](docs/adr/0006-indice-vettoriale-ricerca-esatta-ibrida.md); D-12, D-15) |
| Migrazioni | Migrazioni SQL per servizio | Decisa | Nessun servizio modifica lo schema altrui |
| Originali PDF/DOCX | Garage (object storage S3-compatible) dietro la porta `DocumentStore` | Decisa | Chiave di contenuto `sha256/<hex>`, client `object_store` con parti multipart da 2 MiB e una parte in volo, PUT idempotente verificato con HEAD (da provare nell'adapter di prodotto), GC degli orfani a 24 h; PostgreSQL `bytea` scartato per il canale laterale della deduplica (T-11); misurato con Garage v2.4.1 ([ADR-0003](docs/adr/0003-storage-originali-garage.md); D-01, D-02) |
| Audit/outbox | Tabelle append-only PostgreSQL | Proposta | Eventi idempotenti con `event_id` univoco |

*Storia (2026-10-26, piano 01-17):* fino alla ratifica, «Embedding» era «`pgvector`», **Proposta**, con «Indice e metrica dopo benchmark»; il benchmark di S-03 (ADR-0006) ha fissato metrica e assenza di indice ANN. «Originali PDF/DOCX» era **Aperta** fra «Object storage S3-compatible oppure PostgreSQL `bytea`», con la nota «MinIO è più realistico; `bytea` riduce l'infrastruttura»; lo spike S-01 (ADR-0003) ha scelto Garage, scartato `bytea` e non ha usato MinIO.

Un singolo cluster non significa database condiviso: ogni servizio usa credenziali e schema/database propri e comunica con gli altri attraverso contratti.

## JEV

| Aspetto | Scelta |
|---|---|
| Integrazione | HTTPS diretto a `POST /v1/systemone` tramite adapter Rust |
| SDK | Nessun SDK Rust community nella prima iterazione; usare il contratto HTTP ufficiale |
| Modello testuale | `jev-1.13.0` pinnato da configurazione (`JEV_MODEL`), senza default implicito; la versione risolta (`response.model`) e l'`usage` si registrano in ogni chiamata e in ogni run ([ADR-0008](docs/adr/0008-jev-forma-rank-concorrenza-pin.md); D-17) |
| Immagini | Sperimentali e non richieste dall'happy path iniziale |
| Timeout/retry | Timeout stretto; retry solo transitori ed entro deadline globale |
| Fallback | Rifiuto, baseline locale o risposta degradata secondo policy |
| Test | Fake locale del `DecisionEngine`; nessuna API reale nei test unitari |

*Storia (2026-10-26, piano 01-17):* fino alla ratifica, «Modello testuale» era «Alias configurabile, inizialmente `jev-latest`». Lo spike S-04 (ADR-0008) ha misurato che `jev-latest` si risolve in `jev-1.13.0` e `jev-preview` in `jev-1.14.0`: un alias mobile cambierebbe i segnali Noul su cui si calibrano le soglie senza che il codice cambi, quindi la versione si pinna.

Gli SDK ufficiali disponibili sono TypeScript e Python. Un sidecar dedicato aggiungerebbe un quarto servizio senza un nuovo bounded context; l'API JEV espone invece un contratto OpenAPI pubblico e tipizzato.

Riferimenti ufficiali:

- API: <https://docs.typesafe.ai/api>
- OpenAPI: <https://api.typesafe.ai/openapi.json>
- SDK Python: <https://github.com/typesafe-ai/typesafe-sdk-python>
- SDK TypeScript: <https://github.com/typesafe-ai/typesafe-sdk-js>

## Modelli AI

| Capacità | Stato | Criteri di scelta | Scelta |
|---|---|---|---|
| Embedding model | Decisa | Qualità, lingua, dimensione vettore, latenza, esecuzione locale | `BAAI/bge-m3` a revisione fissata, servito da TEI 1.9.4 CUDA (`float16`) sulla GPU locale, revisione e digest verificati all'avvio; nDCG@10 0,7226 con distacco 0,063 dal secondo, query p95 12 ms, 25 pagine in 1,02 s, VRAM 1,42 GiB ([ADR-0007](docs/adr/0007-embedding-tei-gpu.md); D-13, D-14, D-38) |
| Generative LLM | Decisa | Grounding, output strutturato, latenza, privacy e hardware | Sonnet 5.5 per tutte le query, `simple` e `complex`: la complessità restituita da JEV non cambia il modello; structured output `{answer, claims, refusal}` con id brevi mappati in codice; senza JEV la risposta è sempre `degraded`, mai `validated` ([ADR-0011](docs/adr/0011-llm-generativo-routing-structured-output.md); D-18, D-19, D-20, D-33) |
| OCR/parser | Decisa | Testo, tabelle, coordinate, PDF nativi e scannerizzati | PDFium per testo, pagine e rettangoli; Docling (`docling-serve` 1.37.0) solo sulle pagine-tabella; OCR Tesseract 5.5.0 da CLI (lo sfidante PP-OCRv6 non ha vinto nettamente); parsing e OCR in processo figlio isolato con tetto di decompressione nel prescan; locator a due varianti, PDF a pagina e rettangoli, DOCX e Markdown a percorso di sezione e intervallo di caratteri ([ADR-0004](docs/adr/0004-parser-ocr-isolamento.md), [ADR-0005](docs/adr/0005-locator-citazione.md); D-07..D-10) |
| Reranker baseline | Decisa | Confronto JEV, no-rerank e cross-encoder | Baseline no-rerank (adapter identità della porta `Reranker`); JEV forma A resta il rank di prodotto; `bge-reranker-v2-m3` su TEI misurato (+0,1561 di nDCG@5) ma non installato nell'MVP per la latenza (p95 360 ms contro 300) ([ADR-0009](docs/adr/0009-reranker-baseline.md); D-21) |

*Storia (2026-10-26, piano 01-17):* fino alla ratifica le quattro righe erano **Aperte** e la tabella non aveva la colonna «Scelta»: avevano solo i criteri, che restano invariati. Gli spike S-02, S-03, S-04 e S-06 le hanno chiuse con gli ADR collegati. Per «Generative LLM» il piano 01-17 prevedeva «Haiku 5.5 per le semplici, Sonnet 5.5 per le complesse»: la regola di routing D-31 è fallita sulle misure e l'utente ha scelto Sonnet anche per le `simple` (D-33, ADR-0011), che è la riga scritta qui. Per «Embedding model» il default iniziale Qwen3-Embedding-0.6B (D-14) è stato sostituito da `bge-m3` con D-38, sulla misura di 01-10. Per «OCR/parser», Tesseract è rimasto dopo la prova dello sfidante PP-OCRv6 (ADR-0004).

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
