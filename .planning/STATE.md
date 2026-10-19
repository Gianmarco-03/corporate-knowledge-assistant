---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Onda 1 completata; prossima onda 2 (01-03, 01-04)
last_updated: "2026-10-19T19:51:00.541Z"
last_activity: 2026-10-19 -- onda 1 completata (01-01, 01-02)
progress:
  total_phases: 15
  completed_phases: 0
  total_plans: 18
  completed_plans: 2
  percent: 0
---

# Project State

**Modifica:** Claude (gsd-roadmapper) · 2026-10-18 · inizializzazione dopo la creazione della roadmap
**Modifica:** Claude (gsd-roadmapper) · 2026-10-18 · riallineamento alla roadmap rivista per U-04 (15 fasi, posizione: discuss della Fase 1)

## Project Reference

See: .planning/PROJECT.md (updated 2026-10-18)

**Core value:** Un utente autenticato ottiene risposte supportate da citazioni verificabili, solo dai documenti che è autorizzato a vedere, oppure un rifiuto motivato.
**Current focus:** Phase 1 — Spike tecnici (discuss); poi discuss delle fasi 2-15 secondo la "Strategia di consegna"

## Current Position

Phase: 1 of 15 (Spike tecnici)
Plan: 2 of 18 in current phase
Status: Executing — onda 1 completata
Last activity: 2026-10-19 -- onda 1 completata (01-01, 01-02)

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**

- Total plans completed: 0
- Average duration: -
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| - | - | - | - |

**Recent Trend:**

- Last 5 plans: -
- Trend: -

*Updated after each plan completion*

## Accumulated Context

### Decisions

Le decisioni complete sono in PROJECT.md (Key Decisions e blocchi `<decisions>`).
Recenti, rilevanti per il lavoro corrente:

- Init: ADR-0001 e ADR-0002 LOCKED (tre servizi Rust + React/TS, PostgreSQL per servizio, JEV solo segnali dietro `DecisionEngine`).
- Init: U-01 Docker Desktop + Compose; U-02 loop GSD+ECC.
- Init: U-03 (FR-007) viewer — versione e pagina obbligatorie, passaggio evidenziato solo con coordinate del parser (VIEW-01).
- Init: 7 requisiti derivati dal roadmapper (PLAT-06, PLAT-07, SEC-05, EVAL-02, OBS-01, DLV-01, DLV-02) da confermare dall'utente.
- 2026-10-18: U-04 strategia provider-first e insiemi di file chiusi per fase. Ordine: discuss di tutte le fasi → Spike (1) → plan Fondazione e Provider (2-4) → plan consumer (5-15) → riconciliazione dei piani provider → execute in ordine numerico.
- [Phase 1]: Spesa Anthropic di 01-12 dimezzata (Sonnet solo su complex/no_answer, fallback 15 query, tetto 2 USD); ripasso di plan prima dell onda 7 → `01-02-SUMMARY.md#punti-aperti`
- [Phase 1]: Pesi Docling tenuti con --all (28,6 GB); chiavi provider inserite; gate GPU verde → `01-02-SUMMARY.md#task-4-checkpoint-umano`

### Pending Todos

- Discuss delle fasi 1-15 prima di ogni plan (vedi ROADMAP "Strategia di consegna").

### Blockers/Concerns

- Phase 1: gli spike S-01..S-05, S-07 e le decisioni A-02..A-06, A-09, A-10 bloccano la pianificazione delle Fasi 2-4.
- Decisioni di fasi consumer che vincolano i provider, da chiudere nei rispettivi discuss prima del plan delle Fasi 3-4: Q-02 (Fase 6 → VectorIndex), Q-04 (Fase 11 → AuditSink), Q-05 e policy di fallback (Fase 8 → AnswerGenerator/DecisionEngine), Q-07 (Fase 13 → adapter JEV), prima fonte online (Fase 12 → DocumentSource).
- Phase 2: collocazione delle primitive trasversali (idempotency key, outbox, worker con lease) da ratificare; raccomandata in Fondazione per non far toccare file comuni alle fasi consumer.
- Stub su template ancora da compilare: data-model, api-contract, test-plan, llm-evaluation, project-charter (si completano per sezione nelle fasi che li richiedono, in append).

## Deferred Items

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| v2 | IMG-01 triage immagini, CACHE-01 Redis, CONV-01 conversazioni | Deferred | Init |

## Session Continuity

Last session: 2026-10-19T19:39:45.302Z
Stopped at: Onda 1 completata; prossima onda 2 (01-03, 01-04)
Resume file: None
