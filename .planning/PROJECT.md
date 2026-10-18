# Corporate Knowledge Assistant (CKA)

**Modifica:** Claude (gsd-roadmapper) · 2026-10-18 · creazione da ingest dei documenti (`.planning/intel/`)

## What This Is

Assistente RAG aziendale per documenti finanziari e societari: un dipendente autenticato pone domande sui documenti che il suo ruolo gli consente di vedere e riceve risposte con citazioni verificabili, apribili in un viewer in-app alla versione e alla pagina citate. Il sistema include RBAC deny-by-default, audit, corpus aggiornabile in modo incrementale (upload e fonti online in quarantena) e il decision layer JEV per le decisioni semantiche. È un progetto portfolio sviluppato da una sola persona.

## Core Value

Un utente autenticato ottiene risposte supportate da citazioni verificabili, solo dai documenti che è autorizzato a vedere, oppure un rifiuto motivato.

**Successo lato sviluppatore:** demo MVP end-to-end (login → upload → queryable → domanda → risposta citata → viewer alla pagina citata → audit) con suite di evaluation e test RBAC anti-leakage verdi.

## Requirements

### Validated

(Nessuno: da validare con le consegne)

### Active

Elenco completo con ID e tracciabilità in `.planning/REQUIREMENTS.md` (37 requisiti v1). In sintesi:

- [ ] Login email/password con hash Argon2id e gestione ruoli `admin`, `manager`, `employee`, `guest` (AUTH)
- [ ] Upload PDF/DOCX/Markdown, ingestion asincrona osservabile, pubblicazione atomica di versioni (DOC)
- [ ] Retrieval autorizzato deny-by-default: nessun chunk negato esce dal Document Service (SEC)
- [ ] Risposta con citazioni a versione immutabile, rifiuto per evidenza insufficiente o contraddittoria (QRY)
- [ ] Viewer in-app alla versione e pagina citate, con ricontrollo di accesso (VIEW)
- [ ] Audit di login, upload, ACL, ingestion, query, rifiuti e deny, consultabile secondo ruolo (AUD)
- [ ] Scoperta online in quarantena con provenance e approvazione esplicita (ONL)
- [ ] Evaluation corpus separato e suite di metriche RAG (EVAL)
- [ ] Osservabilità distribuita, concorrenza bounded, Time To Queryable misurato (OBS, PLAT)
- [ ] Demo riproducibile e case study (DLV)

### Out of Scope

- MFA / secondo fattore — escluso dall'MVP (ADR-0001, D-0001-f)
- Kubernetes e service mesh — sproporzionati per l'MVP locale (DEPENDENCIES.md)
- Message broker dedicato — transactional outbox su PostgreSQL basta finché non c'è un requisito misurato
- Database vettoriale separato — prima va misurato `pgvector`
- Framework agentico — la pipeline è deterministica
- Cache distribuita (Redis) senza misura — primo adapter `NoOpCache` (A-07 da chiudere in Fase 11)
- Sidecar JEV / quarto servizio — l'integrazione JEV è HTTPS diretto dal backend Rust
- Plugin dinamici a runtime — Open-Closed via trait stabili, non plugin
- Auto-approvazione dei documenti scoperti online — disabilitata nell'MVP
- Compatibilità Podman — non più criterio dopo la decisione U-01 (Docker Desktop + Compose)

## Context

- **Stato:** definizione dell'architettura completata a livello di ADR, SPEC e PRD; nessun codice. Le directory `apps/`, `services/`, `crates/`, `tests/`, `infra/`, `evaluation/` contengono solo `.gitkeep`.
- **Fonti di verità:** `README.md`, `DEPENDENCIES.md`, `docs/architecture/architecture.md`, `docs/architecture/interfaces-and-concurrency.md`, `docs/data/corpus-strategy.md`, `docs/adr/0001-*`, `docs/adr/0002-*`, `docs/product/requirements.md`. Sintesi in `.planning/intel/`.
- **Lacune M0:** sono stub su template esterni `docs/product/project-charter.md`, `docs/architecture/api-contract.md`, `docs/architecture/data-model.md`, `docs/security/threat-model.md`, `docs/quality/test-plan.md`, `docs/quality/llm-evaluation.md`, `deliverables/checklist.md`. Il threat model si chiude in Fase 1; data model e contratto API si completano nella fase che introduce ciascuna entità o endpoint.
- **Dominio iniziale:** documenti finanziari/societari ricchi di testo, tabelle e grafici; corpus iniziale 10-30 documenti; TAT-DQA candidato per l'evaluation, DocLayNet per test di parsing, FinanceBench dopo verifica delle condizioni d'uso.
- **Metodologia:** loop GSD+ECC a 4 verbi (`discuss | plan | execute | verify`, vedi `CLAUDE.md`). Le fasi marcate "Criticità: critica" in ROADMAP ricevono `implementation-risk-audit` + `test-design` da GPT via AgentBridge; le altre ricevono test design da un agente Claude separato.
- **Lingua:** documenti e commit in italiano; codice e identificatori in inglese.

## Constraints

- **Tech stack**: frontend React + TypeScript (Vite); backend Rust stable; PostgreSQL con `pgvector` (candidato) — ADR-0001 e DEPENDENCIES.md
- **Architettura**: esattamente tre servizi di dominio (Identity & Access, Document & Indexing, RAG & Answer); worker di ingestion come modalità del Document Service — ADR-0001
- **Dati**: un cluster PostgreSQL, schema/database e credenziali separati per servizio, migrazioni per servizio, nessuna join cross-service; dati tra servizi solo via HTTP o transactional outbox con `event_id` univoco — ADR-0001, BR-007
- **Contratti**: HTTP/JSON con OpenAPI, `/api/v1` pubblico e `/internal/v1` interno, UUID opachi, UTC RFC 3339, idempotency key sulle mutazioni, propagazione di `request_id` e trace context, errori `code`/`message`/`request_id` — interfaces-and-concurrency.md (C-01)
- **Sicurezza**: deny-by-default; nessun chunk non autorizzato raggiunge reranker, JEV, LLM, cache o log; ricontrollo autorizzativo prima di citazioni/link e nel viewer — NFR-001, BR-004
- **JEV**: solo segnali semantici dietro la porta `DecisionEngine`; mai autorizzazione, provenance, licenze, malware, deduplicazione esatta o mapping citazioni; fake deterministico nei test unitari — ADR-0002
- **Concorrenza**: ogni fan-out con limite, deadline globale, cancellazione e budget di retry; upload in streaming; worker con lease e `FOR UPDATE SKIP LOCKED`; nessuna chiamata esterna in transazione. Vale per tutte le fasi, la verifica end-to-end è in Fase 11 — NFR-003, C-06, C-07
- **Ambiente**: Docker Desktop + Docker Compose su Windows; immagini non-root, health check, nessun segreto in immagini o Compose versionati — U-01
- **Prestazioni (iniziali, p95, da riesaminare dopo baseline)**: upload ack ≤ 500 ms; PDF digitale 25 pagine/10 MB ≤ 15 s; 100 pagine ≤ 60 s; scannerizzato 25 pagine ≤ 45 s; job fallito visibile ≤ 5 s — README.md (C-31)

## Key Decisions

<decisions status="locked" source="docs/adr/0001-system-boundaries-and-stack.md">
- D-0001-a: Tre servizi di dominio: Identity & Access, Document & Indexing, RAG & Answer.
- D-0001-b: Frontend React con TypeScript; backend Rust.
- D-0001-c: PostgreSQL con ownership distinta per servizio.
- D-0001-d: Contratti HTTP/JSON documentati con OpenAPI; job lunghi asincroni.
- D-0001-e: Trait (porte) per parser, storage, embedding, vector index, JEV, LLM, audit e cache.
- D-0001-f: Nessun MFA nell'MVP.
- Conseguenza vincolante: il worker di ingestion (anche se in processo isolato) resta parte del Document Service.
</decisions>

<decisions status="locked" source="docs/adr/0002-incremental-corpus-and-jev.md">
- D-0002-a: Knowledge corpus dinamico separato dall'evaluation corpus versionato.
- D-0002-b: Tutti i documenti scoperti online entrano in quarantena.
- D-0002-c: JEV per query validation, reranking, evidence sufficiency, contradiction detection, citation validation e triage opzionale delle immagini (il triage semantico dei documenti online è ammesso solo come segnale consultivo; emendamento dell'ADR consigliato).
- D-0002-d: JEV NON usato per autorizzazione, provenance, licenze, malware scan, deduplicazione esatta o mapping citazioni.
- D-0002-e: JEV integrato dal backend Rust via contratto HTTP ufficiale (`POST /v1/systemone`), dietro la porta `DecisionEngine`.
- Conseguenze: workflow di approvazione e pubblicazione atomica; soglie JEV calibrate su dati etichettati; JEV non prerequisito per autenticazione, viewer o RBAC.
</decisions>

<decisions status="user-ratified" date="2026-10-18">
- U-01: Ambiente locale Docker Desktop + Docker Compose (chiude A-08).
- U-02: Metodologia loop GSD+ECC (solo processo).
- U-03 (FR-007): un unico requisito di viewer: la citazione apre versione e pagina corrette (obbligatorio); il passaggio è evidenziato quando il parser fornisce coordinate, altrimenti fallback alla pagina (PDF scannerizzati, DOCX). Diventa VIEW-01.
</decisions>

| Decisione | Motivazione | Esito |
|-----------|-------------|-------|
| Tre servizi Rust + React/TS (ADR-0001) | Confini di dominio chiari, portfolio su Rust | — Pending |
| Corpus incrementale con quarantena, JEV solo segnali (ADR-0002) | Sicurezza deterministica, decisioni semantiche delegabili | — Pending |
| Docker Desktop + Compose (U-01) | Ambiente locale unico e riproducibile | — Pending |
| Viewer: pagina obbligatoria, passaggio se ci sono coordinate (U-03) | Risolve il WARNING FR-007 senza vincolare parser/OCR | — Pending |
| Argon2id, PostgreSQL un cluster, migrazioni per servizio, JEV via HTTPS diretto (DEPENDENCIES "Decisa") | Allineate agli ADR | — Pending |

**Proposte da validare (non decise):** Tokio, axum/tower, serde, sqlx, `pgvector`, JWT asimmetrici, reqwest+Rustls, thiserror/anyhow, tracing+OpenTelemetry, Vite, React Router, TanStack Query, PDF.js, client OpenAPI generato, Vitest, Playwright, audit/outbox append-only, `NoOpCache`. Ognuna si conferma nella fase indicata in ROADMAP.md.

**Decisioni aperte** (A-01..A-10, Q-01..Q-07) e spike (S-01..S-07): restano aperte; la fase il cui `discuss` deve chiuderle è indicata in ROADMAP.md, sezione "Decisioni aperte per fase".

## Evolution

Dopo ogni transizione di fase: requisiti validati → Validated con riferimento di fase; requisiti invalidati → Out of Scope con motivo; nuove decisioni → Key Decisions; "What This Is" aggiornato se diverge. Dopo ogni milestone: revisione completa e verifica del Core Value.

---
*Last updated: 2026-10-18 after ingest dei documenti e creazione della roadmap*
