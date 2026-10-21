# Indice delle decisioni

Una riga per decisione, con puntatore all'artefatto che porta il rationale. Le stesse righe stanno in `STATE.md` (scritte con `gsd-tools state add-decision`).

## Fase 1 — Spike tecnici

- 2026-10-19 · Spesa Anthropic di 01-12 dimezzata (Sonnet solo su complex/no_answer, fallback 15 query, tetto 2 USD); ripasso di plan prima dell'onda 7 → `phases/01-spike-tecnici/01-02-SUMMARY.md#punti-aperti`
- 2026-10-19 · Pesi Docling tenuti con --all (28,6 GB); chiavi provider inserite; gate GPU verde → `phases/01-spike-tecnici/01-02-SUMMARY.md#task-4-checkpoint-umano`
- 2026-10-21 · Fonte IT unica Banca d'Italia; niente SEC EDGAR (UA con recapito personale), 10-K dal PDF Berkshire → `phases/01-spike-tecnici/01-03-SUMMARY.md#decisions-made`
- 2026-10-21 · Manifest del corpus immutabile: fetch.sh e build.sh non riscrivono gli hash; sostituzione = voce nuova con motivo → `phases/01-spike-tecnici/01-03-SUMMARY.md#decisions-made`
- 2026-10-21 · Trappola «Progetto Cormorano» al posto di «Aurora», che collide con un Project Aurora reale nel BIS AER 2025 → `phases/01-spike-tecnici/01-03-SUMMARY.md#decisions-made`
- 2026-10-21 · Garage dietro DocumentStore solo con parti da 2 MiB e 1 in volo: il default di object_store supera la soglia VmHWM → `../spikes/s01-upload/RISULTATI.md#decisione-proposta`
- 2026-10-21 · PG bytea scartato per gli originali: canale laterale della deduplica 0/3 anche con ON CONFLICT DO UPDATE → `../docs/adr/0003-storage-originali-garage.md#decisione`
- 2026-10-21 · PUT idempotente con HEAD proposto per i 500 di Garage sulla dedup concorrente, non misurato: da provare nell'adapter → `../spikes/s01-upload/RISULTATI.md#decisione-proposta`
- 2026-10-21 · GC orfani con N = 24 h e protocollo live|deleting + pending, provato sulle sole interleaving enumerate → `../docs/adr/0003-storage-originali-garage.md#conseguenze`
- 2026-10-21 · Mitigazione del canale laterale su Garage spostata da parti 5 a 2 MiB dopo la misura di memoria, prima dei tempi → `../spikes/s01-upload/RISULTATI.md#anomalie-e-deviazioni`
- 2026-10-21 · Parti da 2 MiB sotto il minimo S3 di 5 MiB: portabilità non verificata, da valutare in 01-16/01-17 → `phases/01-spike-tecnici/01-04-SUMMARY.md#punti-aperti`
- 2026-10-21 · DATABASE_URL di sistema (altro progetto) eliminata dall'utente; difetto di session.sh (env host vince) → gap-closure fase 1.1 prima dell'onda 3 → `phases/01-spike-tecnici/01-04-SUMMARY.md#punti-aperti`
- 2026-10-21 · Utente 2026-10-21: ratifica-tutti sugli 85 oracoli CONFIRMED (74 query, 11 parser nelle misure) → `phases/01-spike-tecnici/01-06-SUMMARY.md#decisions-made`
- 2026-10-21 · Derivatore degli oracoli = esecutore del piano: deviazione accettata dall'utente, TEST-10 non verde → `../spikes/data/corpus/oracles/ORACOLI.md#da-dove-vengono-i-valori`
- 2026-10-21 · TODO-01 P-006: «teneteli», restano riquadro WEB e didascalia nell'oracolo → `../spikes/data/corpus/oracles/ORACOLI.md#to-do-di-dominio`
- 2026-10-21 · TODO-02: resta la regola di complessità; Q-008/010/012/040/048/056 fuori dalle misure (GPT non CONFIRMED), reinviabili → `../spikes/data/corpus/oracles/ORACOLI.md#to-do-di-dominio`
- 2026-10-21 · Deadline globale tokio::time::Instant esplicita su ogni porta verso provider; cancellazione = drop del future → `../docs/adr/0010-deadline-cancellazione-http.md#decisione`
- 2026-10-21 · Fan-out: variante A timeout_at+buffer_unordered di default, B JoinSet+Semaphore solo per task indipendenti; HTTP/2 preferito → `../docs/adr/0010-deadline-cancellazione-http.md#decisione`
- 2026-10-21 · Addebito JEV dopo il drop non determinabile (console vuota 2026-10-21): cancellate contate come addebitate, lettura da ripetere → `../spikes/s05-cancel/RISULTATI.md#esito`
- 2026-10-21 · Difetto hist.rs (01-01): Recorder satura a 2047 µs; gap-closure riapre_perimetro [1], verificare s01-upload → `phases/01-spike-tecnici/01-05-SUMMARY.md#deviations-from-plan`

## Fase 1.1 — Gap-closure di session.sh

- 2026-10-21 · dc() toglie con env -u, dal solo processo compose, le variabili dell'host omonime (in maiuscolo) delle chiavi di spikes.env; la shell non cambia → `phases/01.1-gap-closure-session-sh-segreti-di-spikes-env-prevalgono-su-e/01.1-01-SUMMARY.md#decisions-made`
- 2026-10-21 · Id di piano 01-1-01 nelle sessioni di session.sh: le regex rifiutano il punto; fixture TEST-11/12 allineate, non indebolite → `phases/01.1-gap-closure-session-sh-segreti-di-spikes-env-prevalgono-su-e/01.1-01-SUMMARY.md#deviations-from-plan`
- 2026-10-21 · Commit unico RED+GREEN+regressione: l'hook rifiuta i commit di soli .txt (security-scope-empty); regola dell'hook da decidere → `phases/01.1-gap-closure-session-sh-segreti-di-spikes-env-prevalgono-su-e/01.1-01-SUMMARY.md#punti-aperti`

- 2026-10-21 · Hook pre-commit invariato: con evidenze solo .txt/.csv il RED entra nel commit della correzione, in sezione distinta (decisione utente) → `../.planning/ORCHESTRATOR-STATUS.md#decisioni-ratificate-in-corsa`

## Fase 1.2 — Gap-closure di hist.rs

- 2026-10-21 · Il Recorder registra con record (auto-resize); il difetto reale saturava a 2 µs, non a 2047 → `phases/01.2-gap-closure-hist-rs-il-recorder-delle-latenze-non-satura-sop/01.2-01-SUMMARY.md#il-difetto-reale-più-grave-di-quello-del-context`
- 2026-10-21 · Rifiuto di record contato da rejected() (metodo additivo), ripiego saturato e avviso unico su stderr, firme invariate → `phases/01.2-gap-closure-hist-rs-il-recorder-delle-latenze-non-satura-sop/01.2-01-SUMMARY.md#decisions-made`
- 2026-10-21 · TEST-15/16 adottati su proposta del test design: ramo Err provato con new_with_max(2047, 3) dal mod tests → `phases/01.2-gap-closure-hist-rs-il-recorder-delle-latenze-non-satura-sop/01.2-01-SUMMARY.md#decisions-made`

**Modifica:** Claude (Anthropic) · 2026-10-19 · creazione dell'indice con le decisioni dell'onda 1 della fase 1
