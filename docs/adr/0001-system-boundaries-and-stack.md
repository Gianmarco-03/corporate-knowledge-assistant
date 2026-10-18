# ADR-0001 — Confini dei servizi e stack iniziale

- Stato: Accepted
- Data: 2026-10-18
- Decisore: progetto portfolio

## Contesto

Il progetto deve dimostrare microservizi significativi senza frammentare prematuramente il dominio e deve consentire nuovi provider mantenendo contratti stabili.

## Decisione

- Tre servizi: Identity & Access, Document & Indexing, RAG & Answer.
- Frontend React con TypeScript; backend Rust.
- PostgreSQL con ownership distinta per servizio.
- Contratti HTTP/JSON OpenAPI; job lunghi asincroni.
- Trait per parser, storage, embedding, vector index, JEV, LLM, audit e cache.
- Nessun MFA nell'MVP.

## Conseguenze

- Rust aumenta il costo iniziale e richiede spike su async, streaming e tooling.
- Tre servizi impongono contract test, telemetria distribuita e consistenza eventuale.
- Il worker di ingestion resta parte del Document Service.
- Cache, object storage e container strategy rimangono decisioni separate.
