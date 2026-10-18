# Architettura — Corporate Knowledge Assistant

## Driver

- RBAC deny-by-default e assenza di leakage verso JEV, LLM, cache e log.
- Citazioni riproducibili fino a versione, pagina e posizione.
- Ingestion incrementale da upload e fonti online.
- Estensibilità tramite porte e adapter.
- Job lunghi asincroni, osservabili e idempotenti.
- Massimo tre microservizi di dominio significativi.

## Vista dei container

```text
Browser / React TypeScript
          |
          v
Reverse proxy / routing
    |          |          |
    v          v          v
Identity    Document     RAG & Answer
Service     Service      Service
    |          |          |
    +----------+----------+
               |
        PostgreSQL cluster
      (ownership separata)

Document Service --> object storage [decisione aperta]
Document Service --> embedding provider
RAG Service      --> JEV API
RAG Service      --> generative LLM
```

Reverse proxy, frontend, worker e infrastruttura non sono bounded context aggiuntivi.

## Identity & Access Service

Gestisce utenti, hash Argon2id, ruoli, autenticazione, token firmati, chiavi pubbliche e audit di autenticazione. Non include MFA nell'MVP. Gli altri servizi verificano localmente firma, issuer, audience e scadenza e applicano le proprie policy di risorsa.

## Document & Indexing Service

Gestisce upload, fonti online, quarantena, validazione deterministica, parsing/OCR, chunking, embedding, ACL, versionamento, vector index, retrieval autorizzato, job e contenuto per il viewer. Il worker è una modalità di esecuzione dello stesso servizio.

## RAG & Answer Service

Gestisce query, `DecisionEngine`, candidati autorizzati, reranking, evidence gate, generazione strutturata, citation validation, rifiuto, conversazioni, audit query ed evaluation.

## Flusso ingestion

```text
upload/discovery
  -> quarantine
  -> deterministic validation
  -> provenance and policy check
  -> parse/OCR
  -> chunk + positional metadata
  -> batch embedding
  -> transactional publish of a document version
  -> queryable
```

Una nuova versione viene pubblicata atomicamente: le query vedono la versione precedente o quella nuova, mai un indice parziale.

## Flusso query

```text
authenticate
  -> semantic query validation (JEV)
  -> authorized candidate retrieval
  -> bounded parallel rerank
  -> evidence sufficiency / contradiction gate
  -> structured answer generation
  -> bounded parallel citation validation
  -> authorization re-check
  -> response or refusal
  -> audit
```

## Dati e ownership

- Un cluster PostgreSQL nell'MVP.
- Credenziali e schema/database distinti per servizio.
- Nessuna join cross-service.
- `pgvector` candidato per chunk ed embedding del Document Service.
- Eventi tramite transactional outbox e consumer idempotenti.
- Consistenza forte nel servizio; consistenza eventuale per proiezioni e audit aggregato.

## Estensibilità

Le porte sono definite in `interfaces-and-concurrency.md`. Adapter per parser, embedding, JEV, LLM, storage e cache possono essere sostituiti senza modificare i casi d'uso. Open–Closed non implica plugin dinamici nell'MVP: implica dipendenze orientate verso trait stabili e testabili.

## Decisioni aperte

- Object storage S3-compatible oppure `bytea` PostgreSQL.
- Cache `NoOp` iniziale oppure Redis dopo benchmark.
- Parser/OCR, embedding model e generative LLM.
- Docker/Compose oppure manifest compatibili anche con Podman.
- Specifica OpenAPI contract-first oppure generata da Rust.
