---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Fase 1.3 chiusa; 01-07 in ripasso (audit GPT), poi ripresa execute
last_updated: "2026-10-23T13:46:44.939Z"
last_activity: 2026-10-23 — 01.4-01 completato; onda 6 fusa con gate verde
progress:
  total_phases: 19
  completed_phases: 3
  total_plans: 23
  completed_plans: 15
  percent: 16
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
Plan: 9 of 18 in current phase
Status: Executing — 01-07 e 01-12 chiusi; prossima onda 5 (01-18, 01-08)
Last activity: 2026-10-23 — 01.4-01 completato; onda 6 fusa con gate verde

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**

- Total plans completed: 3
- Average duration: -
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01.1 | 1 | - | - |
| 01.2 | 1 | - | - |
| 01.3 | 1 | - | - |

**Recent Trend:**

- Last 5 plans: -
- Trend: -

*Updated after each plan completion*

## Accumulated Context

### Roadmap Evolution

- Phase 01.1 inserted after Phase 1: Gap-closure session.sh (file chiuso 01-02): env host vince su --env-file; riapre_perimetro [1]; prima dell'onda 3 di fase 1 (URGENT)
- Phase 01.2 inserted after Phase 1: Gap-closure spikes/common/src/hist.rs (file chiuso 01-01): saturating_record satura a 2047 µs; riapre_perimetro [1]; prima dell'onda 4 di fase 1 (URGENT)
- Phase 01.3 inserted after Phase 1: Gap-closure spikes/compose.yaml (file chiuso 01-02): tmpfs /work del worker root 755, uid 10001 non crea cartelle dei job; + commento s05-cancel/src/main.rs:279-281 (file chiuso 01-05); riapre_perimetro [1]; blocca 01-07 (URGENT)

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
- [Phase 01.1]: Hook pre-commit invariato: con evidenze solo .txt/.csv il RED entra nel commit della correzione, in sezione distinta (decisione utente) → `../.planning/ORCHESTRATOR-STATUS.md#decisioni-ratificate-in-corsa`
- [Phase 2]: Contract-first: YAML OpenAPI per servizio, trait Contract (from_json + validate) in crates/contracts, classi Rust a mano, nessun generatore; contract test = oasdiff + Schemathesis --mode all + unit test (D-01..D-03) → `phases/02-fondazione/02-CONTEXT.md#contratti-api-a-01-plat-03`
- [Phase 2]: Schema per servizio in un database, search_path per ruolo, test 42501; cataloghi visibili e lock migrazioni unico accettati (D-07, D-08) → `phases/02-fondazione/02-CONTEXT.md#dati-e-migrazioni-plat-02-criteri-3-e-5`
- [Phase 2]: Migrazioni a fascia PPNNNN_nome.sql con gate CI: sqlx legge la versione prima del primo _ e applica fuori ordine in silenzio (D-09) → `phases/02-fondazione/02-CONTEXT.md#dati-e-migrazioni-plat-02-criteri-3-e-5`
- [Phase 2]: Primitive idempotency/outbox/lease alle fasi proprietarie, nessun seam in Fondazione; supera la raccomandazione del ROADMAP (D-10) → `phases/02-fondazione/02-CONTEXT.md#dati-e-migrazioni-plat-02-criteri-3-e-5`
- [Phase 2]: Errori problem+json RFC 9457 con code/message/request_id; 503+Retry-After per saturazione, 429 solo rate limit (D-05, D-06) → `phases/02-fondazione/02-CONTEXT.md#contratti-api-a-01-plat-03`
- [Phase 2]: Proposte runtime Decise (Rust 1.99.1, axum 0.8.9, sqlx 0.9, otel 0.33, reqwest 0.13 senza reqwest-tracing, config); Caddy unico origin; Garage su rete interna senza TLS fino a fase 15 (D-11..D-18) → `phases/02-fondazione/02-CONTEXT.md#ambiente-build-e-ci-plat-06-criteri-1-e-2`
- [Phase 2]: Frontend React Router 8 data mode, TS 7 se il lint regge, Tailwind 4 + shadcn; registro route esplicito (D-23..D-25) → `phases/02-fondazione/02-CONTEXT.md#shell-frontend-criterio-1-e-5`
- [Phase 3]: Firme S-07 con variazioni: search con testo+vettore, DocumentStore::delete per GC fase 6, parse su file locale; nel brief execute 01-16 (D-01..D-05) → `phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md#firme-delle-porte-conferma-s-07-area-a`
- [Phase 3]: Predicato = insieme di etichette (indice agnostico da Q-02); ACL in proiezione document_acl della fase 3 con set_acl e versione per documento (D-06..D-08) → `phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md#autorizzazione-nellindice-area-b`
- [Phase 3]: Locator multi-span con al massimo un confine (PDF e testo), contro raccomandazione; emenda D-10 di fase 1; brief di 01-09 e 01-16 (D-10) → `phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md#parser-e-chunker-aree-d-e`
- [Phase 3]: Chunk in caratteri con TEI che rifiuta invece di troncare; Docling in fase 3 condizionato ad ADR-0004/D-27; SSRF nel processo + rete Compose dedicata (D-11..D-13) → `phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md#parser-e-chunker-aree-d-e`
- [Phase 3]: Contract suite unica su reale e fake; reali in CI (PG, Garage, PDFium+Tesseract, TEI CPU) + gate locale GPU; fake deterministici con guasti dietro feature testkit (D-14..D-18) → `phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md#contract-test-e-fake-area-i`
- [Phase 1]: Hook pre-commit blocca solo CRITICAL/HIGH (--fail-on HIGH); i MEDIUM sono avvisi da motivare nel SUMMARY; gate di plan/verify invariati → `../skills/security-scanner/SKILL.md`
- [Phase 1]: Gap-closure 1.3: tmpfs /work del worker scrivibile da uid 10001 in compose.yaml (file chiuso 01-02) + commento s05-cancel → `../.planning/ORCHESTRATOR-STATUS.md#limiti-trovati`
- [Phase 1]: Ripasso di plan 01-07: object stream su somma reale decompressa ≤512 MiB, PDF con password utente vuota ammessi, allegati ignorati e registrati, memoria su VmHWM → `../.planning/ORCHESTRATOR-STATUS.md#limiti-trovati`
- [Phase 1]: Forma A per il rank JEV: nDCG@5 1,0 contro 0,729 (C) e 0,679 (B), p95 606 ms ≤ 2.000 → `../spikes/s04-jev/RISULTATI-jev.md#esito`
- [Phase 1]: Concorrenza JEV 16 (massimo misurato con 0 risposte 429, non limite trovato); retry unico su 429/529 entro la deadline → `../docs/adr/0008-jev-forma-rank-concorrenza-pin.md#decisione`
- [Phase 1]: Pin jev-1.13.0 da JEV_MODEL senza default; jev-latest mobile, jev-preview già 1.14.0 → `../docs/adr/0008-jev-forma-rank-concorrenza-pin.md#decisione`
- [Phase 1]: Varianza Noul oltre soglia per 1 candidato su 600 (std 0,0954): margine di calibrazione ≥ ±0,1 attorno al taglio → `../spikes/s04-jev/RISULTATI-jev.md#esito`
- [Phase 1]: 01-11 fuso prima di 01-07 contro 01-ONDE (decisione utente: 01-07 fermo su tre blocchi esterni) → `phases/01-spike-tecnici/01-11-SUMMARY.md#integrazione-fuori-ordine`
- [Phase 1]: instructs_model misurato solo sui falsi positivi (0/1.800): il rilevamento sul testo ostile va a 01-12 o a un oracolo nuovo → `phases/01-spike-tecnici/01-11-SUMMARY.md#issues-encountered`
- [Phase 4]: Split S-07 Reranker/DecisionEngine; baseline solo sul Reranker, guasto JEV = errore tipizzato; criterio 1 ROADMAP da riformulare (D-01, D-02) → `phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md#porte-e-baseline-aree-a-b-d-e`
- [Phase 4]: Adapter solo segnali (soglie nel caso d'uso fase 8, calibrate 13/14); generate con Complexity di dominio, single-turn fino a Q-05; structured output validato lato client (D-03..D-06) → `phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md#porte-e-baseline-aree-a-b-d-e`
- [Phase 4]: Circuit breaker scritto a mano, uno per provider, solo guasti del provider; semaforo globale + limite per query (D-07..D-10) → `phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md#resilienza-area-c`
- [Phase 4]: Snapshot OpenAPI JEV con hash; minimizzazione verso JEV e Anthropic; inference_geo global, nessun dato utente (D-11..D-13) → `phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md#adapter-jev-area-b`
- [Phase 4]: AuditSink PG append-only per servizio, evento tipizzato; hash chain (contro raccomandazione), verify_chain → fase 11 (D-14..D-19) → `phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md#auditsink-area-f-forma-provvisoria-fino-a-q-04`
- [Phase 4]: NoOpCache + fake in memoria con scope; replay wiremock in CI + gate locale a 0,50 USD; prova no-leak a sentinelle (D-20..D-23) → `phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md#contract-test-prova-no-leak-e-adapter-reali-aree-h-i`
- [Phase 01.3]: tmpfs /work del worker con uid/gid 10001 e mode=0700 in sintassi corta; scartati 1777, /work nell'immagine e sintassi lunga → `phases/01.3-gap-closure-compose-yaml-work-del-worker-scrivibile-dall-ute/01.3-01-SUMMARY.md#decisions-made`
- [Phase 01.3]: Docker monta il tmpfs con noexec/nosuid/nodev: per 01-07 nessun binario si esegue da /work → `phases/01.3-gap-closure-compose-yaml-work-del-worker-scrivibile-dall-ute/01.3-01-SUMMARY.md#punti-aperti`
- [Phase 01.3]: TEST-5 e TEST-13 del design 01.3-01 difettosi (regex non ancorata, conflitto su «nessun down -v»): da correggere se riusati → `phases/01.3-gap-closure-compose-yaml-work-del-worker-scrivibile-dall-ute/01.3-01-SUMMARY.md#deviations-from-plan`
- [Phase ?]: Fase 01: Sonnet 5.5 anche sulle query simple (D-33), dopo regola D-31 fallita e misura D-32 (1,00 vs 0,72) → `phases/01-spike-tecnici/01-12-SUMMARY.md#decisions-made`
- [Phase ?]: Fase 01: prescan v6 e prenotazione su 3×VmHWM, calibrazione 10/10 PDF benigni, 2 job parse nel budget → `phases/01-spike-tecnici/01-07-SUMMARY.md#decisions-made`
- [Phase ?]: M8 di 01-18 dichiarata non raggiunta 24/26 (D-34, JBIG2 a 01-13); driver budget/overhead di 01-07 alla gap-closure 1.4 (D-35) → `phases/01-spike-tecnici/01-18-SUMMARY.md#decisions-made`
- [Phase ?]: D-27 chiusa con l'opzione 4 (D-36), riavvio Docling e precisione pagine-tabella aperti per l'ADR di ingestion; falso positivo del prescan alla 1.4 (D-37) → `phases/01-spike-tecnici/01-CONTEXT.md#implementation-decisions`
- [Phase ?]: Modello di embedding bge-m3 su TEI CUDA (D-38, emenda D-14): nDCG@10 0,723 contro Qwen3 0,660 → `phases/01-spike-tecnici/01-CONTEXT.md#implementation-decisions`

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

Last session: 2026-10-21T16:59:55.544Z
Stopped at: Fase 1.3 chiusa; 01-07 in ripasso (audit GPT), poi ripresa execute
Resume file: .planning/phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md
