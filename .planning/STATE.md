---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Onda 2 completata; prima dell'onda 3 gap-closure fase 1.1 (session.sh)
last_updated: "2026-10-21T10:14:05.661Z"
last_activity: 2026-10-21 -- onda 2 completata (01-03, 01-04)
progress:
  total_phases: 16
  completed_phases: 0
  total_plans: 18
  completed_plans: 4
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
Plan: 4 of 18 in current phase
Status: Executing — onda 2 completata
Last activity: 2026-10-21 -- onda 2 completata (01-03, 01-04)

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

### Roadmap Evolution

- Phase 01.1 inserted after Phase 1: Gap-closure session.sh (file chiuso 01-02): env host vince su --env-file; riapre_perimetro [1]; prima dell'onda 3 di fase 1 (URGENT)

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
- [Phase 1]: Fonte IT unica Banca d'Italia; niente SEC EDGAR (UA con recapito personale), 10-K dal PDF Berkshire → `phases/01-spike-tecnici/01-03-SUMMARY.md#decisions-made`
- [Phase 1]: Manifest del corpus immutabile: fetch.sh e build.sh non riscrivono gli hash; sostituzione = voce nuova con motivo → `phases/01-spike-tecnici/01-03-SUMMARY.md#decisions-made`
- [Phase 1]: Trappola «Progetto Cormorano» al posto di «Aurora», che collide con un Project Aurora reale nel BIS AER 2025 → `phases/01-spike-tecnici/01-03-SUMMARY.md#decisions-made`
- [Phase 1]: Garage dietro DocumentStore solo con parti da 2 MiB e 1 in volo: il default di object_store supera la soglia VmHWM → `../spikes/s01-upload/RISULTATI.md#decisione-proposta`
- [Phase 1]: PG bytea scartato per gli originali: canale laterale della deduplica 0/3 anche con ON CONFLICT DO UPDATE → `../docs/adr/0003-storage-originali-garage.md#decisione`
- [Phase 1]: PUT idempotente con HEAD proposto per i 500 di Garage sulla dedup concorrente, non misurato: da provare nell'adapter → `../spikes/s01-upload/RISULTATI.md#decisione-proposta`
- [Phase 1]: GC orfani con N = 24 h e protocollo live|deleting + pending, provato sulle sole interleaving enumerate → `../docs/adr/0003-storage-originali-garage.md#conseguenze`
- [Phase 1]: Mitigazione del canale laterale su Garage spostata da parti 5 a 2 MiB dopo la misura di memoria, prima dei tempi → `../spikes/s01-upload/RISULTATI.md#anomalie-e-deviazioni`
- [Phase 1]: Parti da 2 MiB sotto il minimo S3 di 5 MiB: portabilità non verificata, da valutare in 01-16/01-17 → `phases/01-spike-tecnici/01-04-SUMMARY.md#punti-aperti`
- [Phase 1]: DATABASE_URL di sistema (altro progetto) eliminata dall'utente; difetto di session.sh (env host vince) → gap-closure fase 1.1 prima dell'onda 3 → `phases/01-spike-tecnici/01-04-SUMMARY.md#punti-aperti`

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
Stopped at: Onda 2 completata; prima dell'onda 3 gap-closure fase 1.1 (session.sh)
Resume file: None
