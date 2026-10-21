---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Fase 1.2 chiusa; prossima onda 4 di fase 1 (01-07, 01-11)
last_updated: "2026-10-21T14:49:30.792Z"
last_activity: 2026-10-21 -- fase 1.2 verificata (passed) e chiusa
progress:
  total_phases: 17
  completed_phases: 2
  total_plans: 20
  completed_plans: 8
  percent: 12
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
Plan: 6 of 18 in current phase
Status: Executing — fasi 1.1 e 1.2 chiuse; prossima onda 4 di fase 1
Last activity: 2026-10-21 -- fase 1.2 verificata (passed) e chiusa

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**

- Total plans completed: 2
- Average duration: -
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01.1 | 1 | - | - |
| 01.2 | 1 | - | - |

**Recent Trend:**

- Last 5 plans: -
- Trend: -

*Updated after each plan completion*

## Accumulated Context

### Roadmap Evolution

- Phase 01.1 inserted after Phase 1: Gap-closure session.sh (file chiuso 01-02): env host vince su --env-file; riapre_perimetro [1]; prima dell'onda 3 di fase 1 (URGENT)
- Phase 01.2 inserted after Phase 1: Gap-closure spikes/common/src/hist.rs (file chiuso 01-01): saturating_record satura a 2047 µs; riapre_perimetro [1]; prima dell'onda 4 di fase 1 (URGENT)

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
- [Phase 01.1]: dc() toglie con env -u, dal solo processo compose, le variabili dell'host omonime (in maiuscolo) delle chiavi di spikes.env; la shell non cambia → `phases/01.1-gap-closure-session-sh-segreti-di-spikes-env-prevalgono-su-e/01.1-01-SUMMARY.md#decisions-made`
- [Phase 01.1]: Id di piano 01-1-01 nelle sessioni di session.sh: le regex rifiutano il punto; fixture TEST-11/12 allineate, non indebolite → `phases/01.1-gap-closure-session-sh-segreti-di-spikes-env-prevalgono-su-e/01.1-01-SUMMARY.md#deviations-from-plan`
- [Phase 01.1]: Commit unico RED+GREEN+regressione: l'hook rifiuta i commit di soli .txt (security-scope-empty); regola dell'hook da decidere → `phases/01.1-gap-closure-session-sh-segreti-di-spikes-env-prevalgono-su-e/01.1-01-SUMMARY.md#punti-aperti`
- [Phase 1]: Utente 2026-10-21: ratifica-tutti sugli 85 oracoli CONFIRMED (74 query, 11 parser nelle misure) → `phases/01-spike-tecnici/01-06-SUMMARY.md#decisions-made`
- [Phase 1]: Derivatore degli oracoli = esecutore del piano: deviazione accettata dall'utente, TEST-10 non verde → `../spikes/data/corpus/oracles/ORACOLI.md#da-dove-vengono-i-valori`
- [Phase 1]: TODO-01 P-006: «teneteli», restano riquadro WEB e didascalia nell'oracolo → `../spikes/data/corpus/oracles/ORACOLI.md#to-do-di-dominio`
- [Phase 1]: TODO-02: resta la regola di complessità; Q-008/010/012/040/048/056 fuori dalle misure (GPT non CONFIRMED), reinviabili → `../spikes/data/corpus/oracles/ORACOLI.md#to-do-di-dominio`
- [Phase 1]: Deadline globale tokio::time::Instant esplicita su ogni porta verso provider; cancellazione = drop del future → `../docs/adr/0010-deadline-cancellazione-http.md#decisione`
- [Phase 1]: Fan-out: variante A timeout_at+buffer_unordered di default, B JoinSet+Semaphore solo per task indipendenti; HTTP/2 preferito → `../docs/adr/0010-deadline-cancellazione-http.md#decisione`
- [Phase 1]: Addebito JEV dopo il drop non determinabile (console vuota 2026-10-21): cancellate contate come addebitate, lettura da ripetere → `../spikes/s05-cancel/RISULTATI.md#esito`
- [Phase 1]: Difetto hist.rs (01-01): Recorder satura a 2047 µs; gap-closure riapre_perimetro [1], verificare s01-upload → `phases/01-spike-tecnici/01-05-SUMMARY.md#deviations-from-plan`
- [Phase 01.2]: Il Recorder registra con record (auto-resize); il difetto reale saturava a 2 µs, non a 2047 → `phases/01.2-gap-closure-hist-rs-il-recorder-delle-latenze-non-satura-sop/01.2-01-SUMMARY.md#il-difetto-reale-più-grave-di-quello-del-context`
- [Phase 01.2]: Rifiuto di record contato da rejected() (metodo additivo), ripiego saturato e avviso unico su stderr, firme invariate → `phases/01.2-gap-closure-hist-rs-il-recorder-delle-latenze-non-satura-sop/01.2-01-SUMMARY.md#decisions-made`
- [Phase 01.2]: TEST-15/16 adottati su proposta del test design: ramo Err provato con new_with_max(2047, 3) dal mod tests → `phases/01.2-gap-closure-hist-rs-il-recorder-delle-latenze-non-satura-sop/01.2-01-SUMMARY.md#decisions-made`

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
Stopped at: Fase 1.2 chiusa; prossima onda 4 di fase 1 (01-07, 01-11)
Resume file: None
