# Corporate Knowledge Assistant

Assistente RAG aziendale per documenti finanziari e societari, con autenticazione, RBAC, citazioni verificabili, visualizzazione in-app, audit e aggiornamento incrementale della knowledge base.

## Stato

`Architecture definition` — stiamo definendo contratti, concorrenza e dipendenze prima del primo vertical slice.

## Obiettivo dell'MVP

Un utente autenticato può:

1. accedere con email e password;
2. caricare un documento PDF, DOCX o Markdown;
3. seguirne l'indicizzazione asincrona fino allo stato `queryable`;
4. porre una domanda sui soli documenti autorizzati;
5. ricevere una risposta supportata da citazioni;
6. aprire il documento nell'app alla pagina e al passaggio citato;
7. consultare, se autorizzato, gli eventi di audit.

Ruoli iniziali: `admin`, `manager`, `employee`, `guest`. Il secondo fattore di autenticazione non fa parte dell'MVP.

## Dominio e corpus

Il dominio iniziale è finanziario/societario perché consente di lavorare con documenti reali ricchi di testo, tabelle e grafici.

Il sistema parte con un corpus piccolo e comprensibile, indicativamente 10–30 documenti. In seguito può scoprire nuove versioni e nuovi documenti da fonti online tramite connettori. Un documento scoperto non diventa automaticamente interrogabile: entra in quarantena, viene validato, deduplicato, classificato e approvato secondo la policy configurata.

Sono separati due insiemi:

- **knowledge corpus**: documenti disponibili agli utenti; può evolvere online;
- **evaluation corpus**: documenti, domande e risposte attese versionati e immutabili, usati per misurare regressioni.

La strategia completa è descritta in `docs/data/corpus-strategy.md`.

## Architettura

Tre microservizi significativi:

1. **Identity & Access Service** — utenti, credenziali, ruoli e token.
2. **Document & Indexing Service** — upload, fonti online, parsing, versionamento, chunking, embedding, indicizzazione e visualizzazione autorizzata.
3. **RAG & Answer Service** — query, retrieval, integrazione JEV, generazione, validazione delle citazioni, rifiuto e audit delle risposte.

Frontend, reverse proxy, worker e database sono componenti di deployment, non ulteriori bounded context.

Dettagli: `docs/architecture/architecture.md` e `docs/architecture/interfaces-and-concurrency.md`.

## Stack deciso

- Frontend: React con TypeScript.
- Backend: Rust.
- Persistenza: PostgreSQL; `pgvector` è il candidato predefinito per gli embedding.
- Decision layer: JEV tramite API HTTP e adapter interno.
- Autenticazione: password sottoposte a hash Argon2id; nessun MFA nell'MVP.
- Contratti sincroni: HTTP/JSON documentato con OpenAPI.
- Operazioni lunghe: job asincroni, idempotenti e osservabili.

Caching, object storage, modello generativo, embedding model e strategia container sono ancora decisioni aperte. Vedere `DEPENDENCIES.md`.

## JEV nella pipeline

JEV è un decision engine, non un generatore di testo e non un'autorità di sicurezza. Verrà usato per:

- validazione semantica e classificazione della query;
- relevance filtering e reranking;
- rilevazione di evidenze insufficienti o contraddittorie;
- supporto alla decisione di rifiuto;
- validazione semantica claim–citazione;
- triage opzionale di pagine e immagini.

RBAC, provenienza dei chunk, link delle citazioni, validazione dei file e decisioni irreversibili restano deterministici.

## Requisiti prestazionali iniziali

La metrica di ingestion è il **Time To Queryable**.

| Scenario | Target iniziale p95 |
|---|---:|
| Upload acknowledgment | ≤ 500 ms |
| PDF digitale, 25 pagine, massimo 10 MB | ≤ 15 s |
| PDF digitale, 100 pagine | ≤ 60 s |
| PDF scannerizzato, 25 pagine | ≤ 45 s |
| Stato di job fallito visibile | ≤ 5 s |

I target saranno riesaminati dopo una baseline su hardware documentato.

## Struttura prevista

- `apps/web/`: frontend React/TypeScript e visualizzatore documenti.
- `services/identity-service/`: servizio Identity & Access in Rust.
- `services/document-service/`: servizio Document & Indexing in Rust e worker associati.
- `services/rag-service/`: servizio RAG & Answer in Rust.
- `crates/`: librerie Rust interne, senza condivisione dei modelli di dominio tra servizi.
- `tests/`: test unit, integration, E2E, security ed evaluation.
- `infra/`: container, configurazioni locali e deployment.
- `docs/`: requisiti, architettura, ADR, sicurezza e operations.
- `evaluation/`: dataset manifest, configurazioni e risultati versionati.

## Principi

- Deny by default per accesso ai documenti.
- Nessun chunk non autorizzato raggiunge reranker, JEV, LLM, cache o log.
- Ogni operazione ripetibile espone un'idempotency key o usa un identificatore stabile.
- Concorrenza limitata e backpressure; nessun fan-out senza limite.
- I contratti tra servizi sono versionati e testati.
- Gli adapter esterni dipendono da porte interne stabili.
- Un documento online non è attendibile solo perché proviene da un URL valido.

## Avvio

Non ancora disponibile. Il primo vertical slice verrà implementato dopo l'approvazione dei contratti elementari e del modello di concorrenza.
