# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project status

Corporate Knowledge Assistant: an enterprise RAG assistant for financial/corporate documents, with auth, RBAC, verifiable citations, an in-app document viewer, audit, and incremental knowledge-base updates.

The repo is in the **architecture definition** phase. There is no source code, build system, lockfile, or test runner yet. Every code directory holds only a `.gitkeep`. There are no build, lint, or test commands to run. When implementation starts, add the real commands here.

Project documentation is written in **Italian**. Keep new docs, ADRs, and doc edits in Italian to match.

## Where the decisions live

Read these before proposing or writing code. They are the source of truth:

- `README.md` — MVP scope, the 3 services, performance targets (Time To Queryable), principles.
- `DEPENDENCIES.md` — every stack choice tagged **Decisa** (decided), **Proposta** (default, needs a spike), or **Aperta** (open). Do not treat a Proposta/Aperta item as settled; do not introduce a dependency from the "avoid in MVP" list.
- `docs/architecture/interfaces-and-concurrency.md` — HTTP endpoints per service, the internal Rust port traits, the concurrency model, and the race conditions that must be tested.
- `docs/architecture/architecture.md` — container view, ingestion and query flows, data ownership.
- `docs/adr/` — accepted decisions. Record new irreversible or costly decisions as a new ADR from `0000-template.md`.
- `deliverables/checklist.md` — milestones M0–M4 (M1 = vertical slice upload → retrieval → cited answer).

Several docs (`api-contract.md`, `data-model.md`, `test-plan.md`, etc.) are stubs that point to templates in `../../00-shared-templates/` (outside this repo).

## Architecture

Exactly **three domain microservices**, in Rust; do not add a fourth bounded context:

1. **Identity & Access** (`services/identity-service/`) — users, Argon2id password hashes, roles (`admin`, `manager`, `employee`, `guest`), asymmetrically signed JWTs, JWKS. No MFA in the MVP.
2. **Document & Indexing** (`services/document-service/`) — upload, online sources with quarantine, parsing/OCR, chunking, embedding, versioning, ACLs, `pgvector` index, authorized retrieval, viewer content. The ingestion worker is a run mode of this service, not a separate service.
3. **RAG & Answer** (`services/rag-service/`) — query flow, JEV `DecisionEngine`, reranking, evidence gate, structured generation, citation validation, refusal, audit, evaluation.

Frontend (`apps/web/`, React + TypeScript), reverse proxy, workers, and DB are deployment components, not bounded contexts. `crates/` holds internal Rust libraries (`contracts`, `telemetry`); domain models are **not** shared between services.

Key cross-cutting rules:

- **Data ownership**: one PostgreSQL cluster, but separate schema/DB and credentials per service. No cross-service joins. Each service owns its own SQL migrations. Cross-service data flows through HTTP contracts or a transactional outbox with idempotent consumers (`event_id` unique).
- **Ports and adapters**: use cases depend on internal traits (`DocumentSource`, `DocumentStore`, `DocumentParser`, `Chunker`, `Embedder`, `VectorIndex`, `DecisionEngine`, `AnswerGenerator`, `AccessPolicy`, `AuditSink`, `Cache`). Provider/SDK/DB types never cross a domain port. The first `Cache` adapter is `NoOpCache`; no Redis without a measurement.
- **JEV** (typesafe.ai decision engine) is called over HTTPS (`POST /v1/systemone`) through a Rust adapter behind `DecisionEngine`, no SDK. Use it only for semantic decisions (query validation, rerank, evidence sufficiency/contradiction, claim–citation validation). RBAC, provenance, citation mapping, file validation, and dedup stay deterministic. Unit tests use a deterministic fake, never the real API. A JEV failure must never bypass a security check.
- **Security**: deny by default. No unauthorized chunk may reach the reranker, JEV, LLM, cache, or logs. Re-check authorization before returning citations/links, and again in the viewer.
- **API conventions**: public routes under `/api/v1`, internal under `/internal/v1`; opaque UUIDs; UTC RFC 3339; idempotency key on mutating operations; propagate `request_id` and trace context; errors carry `code`, `message`, `request_id`.
- **Concurrency**: async for I/O, bounded pools for CPU work (`spawn_blocking` or isolated processes for parsing/OCR). Every fan-out has a limit, a global deadline, cancellation, and a retry budget. Streaming upload (never buffer the whole file). Job workers use leases + `FOR UPDATE SKIP LOCKED`. No external calls inside a DB transaction. Document versions publish atomically: queries see the old or the new version, never a partial index.
- **Corpus**: the knowledge corpus (dynamic, online discovery goes through quarantine and approval) is separate from the evaluation corpus (versioned, immutable).

## Configuration

`.env.example` lists the expected variables (`DATABASE_URL`, `JWT_*`, `JEV_API_URL`, `TYPESAFE_API_KEY`, `LLM_PROVIDER`, `LLM_MODEL`, `EMBEDDING_MODEL`, ...). Real `.env*` files are git-ignored.
