---
phase: 01-spike-tecnici
plan: 05
subsystem: spike-cancellazione-deadline-http
tags: [reqwest, rustls, axum, hyper, http2, rst-stream, tokio, timeout_at, buffer_unordered, joinset, semaphore, wiremock, jev, deadline, cancellazione, adr]

# Dependency graph
requires:
  - phase: 01-01
    provides: "workspace degli spike, crate spikes/common (env, manifest, csv, hist, timer) e Cargo.toml di s05-cancel con le dipendenze fissate"
  - phase: 01-02
    provides: "harness Compose chiuso, session.sh (with/oneshot/ps/down/release), hw-manifest.sh, runner Linux, file segreti esterno CKA_SPIKES_ENV"
  - phase: 01-04
    provides: "ordine nel runner (misure CPU non sovrapposte), normalizzazione di hw.json e build offline con crate vendorizzate"
provides:
  - "Spike S-05: server axum lento con DropGuard (impl Drop) su runtime tokio separato, h1 e h2c prior knowledge sulla stessa porta, contatori atomici started/completed/dropped e istante del drop per id"
  - "Fan-out con deadline globale in due varianti: A timeout_at + buffer_unordered, B JoinSet + Semaphore con drop del JoinSet alla scadenza"
  - "Sonde di leak: num_alive_tasks del runtime del client, /proc/self/fd, ss established per porta"
  - "Client JEV locale allo spike (POST /v1/systemone, Bearer, https_only, Debug redatto, stato sintetico costante, nessun reinvio)"
  - "Evidenze con soglie scritte prima: 9 scenari × 10.000 cicli senza leak, deadline p99 2,091 ms (A) e 1,793 ms (B), serie JEV reale 20 cancellate + 5 di controllo"
  - "ADR-0010 Proposed: deadline esplicita su ogni porta verso provider, variante A di default, HTTP/2 preferito, reqwest 0.13 con Rustls, retry al più uno dentro la deadline"
  - "Impatto sulle porte (S-07) e sul threat model (T-24, T-01-05-01) in RISULTATI §Esito per 01-16 e 01-17"
affects: [01-11, 01-12, 01-16, 01-17, gap-closure 01-01 hist.rs]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Strumento di misura provato prima della misura: TEST-4/5/6/14 dimostrano che la DropGuard conta davvero e che h2 è davvero h2, poi si affidano 10.000 cicli"
    - "Mock su thread e runtime tokio propri, così num_alive_tasks misura solo il runtime del chiamante"
    - "Cancellazione come drop del future (timeout_at), mai flag cooperativi: in h2 il client manda RST_STREAM e il server droppa l'handler"
    - "Percentili esatti nearest-rank sui campioni invece di common::hist::Recorder (satura a 2047 µs)"
    - "Serie JEV con riscaldamento GET /openapi.json prima di ogni POST da cancellare, finestre temporali separate da 120 s per il riscontro in console"

key-files:
  created:
    - spikes/s05-cancel/RISULTATI.md
    - spikes/s05-cancel/results/hw.json
    - spikes/s05-cancel/results/cancel.csv
    - spikes/s05-cancel/results/deadline.csv
    - spikes/s05-cancel/results/jev_cancel.csv
    - docs/adr/0010-deadline-cancellazione-http.md
  modified:
    - spikes/s05-cancel/src/main.rs
    - spikes/s05-cancel/src/server.rs
    - spikes/s05-cancel/src/client.rs
    - spikes/s05-cancel/src/jev.rs
    - spikes/s05-cancel/src/probe.rs

key-decisions:
  - "Deadline globale per query passata esplicitamente a DecisionEngine, AnswerGenerator, Embedder e Reranker::rank; la cancellazione è il drop del future (ADR-0010 §Decisione 1)"
  - "Variante A (timeout_at + buffer_unordered) come forma di default del fan-out; B solo per task indipendenti e sempre in un JoinSet droppato alla deadline (numeri indistinguibili: p99 2,1 contro 1,8 ms)"
  - "HTTP/2 preferito verso i provider: in h1 ogni cancellazione chiude la connessione e la richiesta successiva paga l'handshake"
  - "reqwest 0.13 con Rustls validato come client dei provider: la riga di DEPENDENCIES.md passa a Decisa nel consolidamento 01-17"
  - "Addebito JEV dopo il drop non determinabile (console «vuota» il 2026-10-21, anche per i 5 controlli): nel budget le cancellate contano come addebitate"

patterns-established:
  - "Deadline come tokio::time::Instant in firma: l'adapter non si costruisce una deadline più lunga"
  - "Una richiesta cancellata dalla deadline non si ritenta mai; retry al più uno per 429/5xx se il tempo residuo supera la latenza attesa"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "La cancellazione verso provider HTTP è verificata su server mock HTTP/1 e HTTP/2 (axum lento con guardia di drop, wiremock) per 10.000 cicli senza leak di task, file descriptor o connessioni (criterio 4; D-22)"
    verification:
      - {test: TEST-4, risks: [RISK-3], ref: "spikes/s05-cancel/src/server.rs:243#drop_guard_counts_dropped_when_not_completed ; spikes/s05-cancel/src/server.rs:254#drop_guard_counts_dropped_when_future_times_out", status: pass, evidence: "rieseguito dal bridge sull'host: cargo test -p s05-cancel --locked --offline 9 passed 0 failed"}
      - {test: TEST-5, risks: [RISK-3], ref: "spikes/s05-cancel/src/server.rs:270#drop_guard_completed_is_not_dropped", status: pass, evidence: "rieseguito dal bridge sull'host: verde"}
      - {test: TEST-6, risks: [RISK-3, RISK-9], ref: "spikes/s05-cancel/src/client.rs:224#server_sees_client_cancel_h2 ; spikes/s05-cancel/src/client.rs:233#server_sees_client_cancel_h1_recorded", status: pass, evidence: "rieseguito dal bridge sull'host: verdi. Il caso h1 non ha asserzioni per design; «visto» è dal rapporto"}
      - {test: TEST-14, risks: [RISK-9], ref: "spikes/s05-cancel/src/server.rs:282#h2_prior_knowledge_negotiates_http2", status: pass, evidence: "rieseguito dal bridge sull'host: resp.version() == HTTP_2"}
      - {test: TEST-9, risks: [RISK-5], ref: "spikes/s05-cancel/results/cancel.csv ; spikes/s05-cancel/results/hw.json", status: pass, evidence: "ricontato dal bridge: header esatto, 9 coppie (mock h1, mock h2, wiremock h1 × 50/200/500), 101 righe ciascuna con cycle 0 e 10000; hw.json JSON valido con git_rev 029390c, run_id 01-05-20261021t124025z-2846 e campi di common::manifest (normalizzato, vedi deviazione 2)"}
      - {test: TEST-10 (leak), risks: [RISK-3, RISK-5], ref: "spikes/s05-cancel/results/cancel.csv righe cycle=0 e cycle=10000", status: pass, evidence: "ricalcolato dal bridge: task, fd e connessioni a fine scenario uguali alla base in tutti e 9 gli scenari (h1 0/15-25/0, h2 3/17-27/1, wiremock 0/15/0); massimo di connessioni in corsa 7 (h1) e 1 (h2), ≤ 8; server_dropped 10000 e server_completed 0 nei 6 scenari axum. Coincide con RISULTATI"}
      - {test: TEST-10 (h2 ≥ 95 % entro 100 ms), risks: [RISK-3], ref: "spikes/s05-cancel/results/cancel.csv colonna server_saw_cancel_ms", status: human_judgment, human_judgment: true, rationale: "i 300 campioni versionati per protocollo (uno ogni 100 cicli) sono tutti ≤ 2,4 ms, quindi entro soglia; ma «100 %, max 7,061 ms h2 / 23,517 ms h1» e i p99 di exit_after_cancel_us vengono dai grezzi per ciclo in results/raw/, non versionati e spariti con il worktree. Il bridge non può ricalcolarli"}
  - must_have: "La deadline globale di un fan-out (timeout_at + buffer_unordered, e JoinSet + Semaphore) fa uscire il chiamante entro 10 ms dalla scadenza, con misura di entrambe le varianti (D-22)"
    verification:
      - {test: TEST-7, risks: [RISK-4], ref: "spikes/s05-cancel/src/client.rs:245#deadline_variant_a_exits_and_drops_inflight", status: pass, evidence: "rieseguito dal bridge sull'host: verde (ritorno entro budget + 50 ms, 8 avviate e 8 droppate)"}
      - {test: TEST-8, risks: [RISK-4], ref: "spikes/s05-cancel/src/client.rs:272#deadline_variant_b_aborts_joinset", status: pass, evidence: "rieseguito dal bridge sull'host: verde (task vivi alla base ± 2, avviate ≤ 8, droppate = avviate)"}
      - {test: TEST-10 (deadline), risks: [RISK-4, RISK-5], ref: "spikes/s05-cancel/results/deadline.csv", status: pass, evidence: "ricalcolato dal bridge, nearest-rank: A n 18 p50 1031 p95 2091 p99 2091 µs; B n 18 p50 1154 p95 1793 p99 1793 µs; solo run 2 e 3; alive_tasks_after 3 in tutte le righe; nessuna uscita anticipata. Soglia p99 < 10 ms rispettata; coincide con RISULTATI. Con n = 18 il p99 coincide con il massimo"}
      - {test: TEST-9 (deadline), risks: [RISK-5], ref: "spikes/s05-cancel/results/deadline.csv", status: pass, evidence: "header esatto, 36 righe = 2 varianti × 3 fanout × 3 budget × run 2 e 3"}
  - must_have: "Con JEV reale, 20 richieste cancellate a 50 ms sono registrate e l'utente verifica nella console se sono state addebitate (D-22)"
    verification:
      - {test: TEST-11, risks: [RISK-6], ref: "spikes/s05-cancel/results/jev_cancel.csv ; spikes/s05-cancel/src/jev.rs ; spikes/s05-cancel/src/main.rs:717#run_jev", status: pass, evidence: "ricontato dal bridge: 25 righe, 20 cancelled (latenza 50,3-52,0 ms) e 5 control (200, jev-1.13.0, 332/21 token, 228-287 ms), sent_at_utc con Z, campi vuoti per le cancellate, 10 colonne. grep retry|loop {|for _ in su jev.rs: nessuna occorrenza"}
      - {test: TEST-12, risks: [RISK-7], ref: "spikes/s05-cancel/src/jev.rs:16#SYNTHETIC_STATE", status: pass, evidence: "costante const di due frasi su una società inventata; grep data/corpus su src vuoto"}
      - {test: TEST-15, risks: [RISK-10], ref: "spikes/s05-cancel/RISULTATI.md §Esito ; docs/adr/0010-deadline-cancellazione-http.md §Conseguenze", status: pass, evidence: "voce «Addebito JEV dopo il drop: non determinabile» con data 2026-10-21 in entrambi; «in lettura dall'utente» assente da RISULTATI (resta solo nella firma storica dell'ADR); i 5 percorsi spikes/ citati dall'ADR esistono"}
      - {test: Task-3 (esito dell'addebito), risks: [RISK-10], ref: "spikes/s05-cancel/RISULTATI.md §Esito «Addebito JEV dopo il drop»", status: human_judgment, human_judgment: true, rationale: "la domanda di D-22 non è chiusa: la console typesafe è risultata «vuota» il 2026-10-21 anche per le 5 richieste di controllo completate con 200, quindi l'esito è «non determinabile» e la lettura va ripetuta sulle stesse finestre (13:32:17.806Z-13:33:00.800Z e 13:35:02.846Z-13:35:11.841Z UTC). La serie prova solo che il client chiude la richiesta, non che cosa fa JEV lato server"}
  - must_have: "reqwest con Rustls è validato come client HTTP dei provider, e la decisione su deadline e cancellazione è registrata in ADR-0010"
    verification:
      - {test: TEST-1, risks: [RISK-1], ref: "grep TYPESAFE_API_KEY spikes/s05-cancel/src ; grep -rqF del valore ; python skills/security-scanner/security_scan.py spikes/s05-cancel", status: pass, evidence: "rieseguito dal bridge: TYPESAFE_API_KEY solo in common::env::required (main.rs:720); il valore caricato dal file segreti non compare in spikes/s05-cancel, ADR-0010 né nei due rapporti tmp (grep -q, chiave mai stampata); scanner «OK su 8 file», exit 0"}
      - {test: TEST-2, risks: [RISK-1], ref: "spikes/s05-cancel/src/jev.rs:186#jev_client_debug_and_errors_redact_key", status: pass, evidence: "rieseguito dal bridge sull'host: verde"}
      - {test: TEST-3, risks: [RISK-2], ref: "git merge-base --is-ancestor a80ffea a5e53e8 ; sezione Soglie fra a80ffea e 58b5004", status: pass, evidence: "rieseguito dal bridge: a80ffea (14:26) antenato del primo commit di results/ a5e53e8; le righe della tabella Soglie sono identiche; la sezione differisce solo per due righe **Modifica:** aggiunte in coda, senza toccare la tabella"}
      - {test: TEST-13, risks: [RISK-8], ref: "grep danger_accept_invalid|tls_built_in_root_certs(false) ; grep http:// fuori da 127.0.0.1 ; spikes/s05-cancel/src/jev.rs:87", status: pass, evidence: "rieseguito dal bridge: entrambi i grep vuoti; https_only(!loopback) verso JEV, verifica dei certificati di default"}
      - {test: ADR-0010 (ratifica), ref: "docs/adr/0010-deadline-cancellazione-http.md Stato", status: human_judgment, human_judgment: true, rationale: "l'ADR è Proposed: la ratifica dell'utente e il passaggio di reqwest+Rustls a Decisa in DEPENDENCIES.md spettano al consolidamento 01-17. Le misure sono su server di prova locali; JEV è reale ma con contenuto sintetico costante"}

# Metrics
duration: "circa 1 h 28 min di calendario (test rossi 14:22 → chiusura del Task 3 15:50, 2026-10-21), compresa l'attesa della lettura in console"
completed: 2026-10-21
---

# Fase 1 Piano 05: S-05, cancellazione e deadline delle richieste HTTP verso i provider · Summary

**Cancellare una richiesta reqwest verso un provider non lascia task, file descriptor o connessioni appesi in 9 scenari da 10.000 cicli (h1, h2, wiremock), e una deadline globale sul fan-out restituisce il controllo entro 2,1 ms (p99, n = 18) in entrambe le varianti; ADR-0010 propone la deadline esplicita su ogni porta, la variante `timeout_at` + `buffer_unordered` di default e HTTP/2 verso i provider, mentre l'addebito JEV delle richieste cancellate resta non determinabile e nel budget si conta come addebitato**

## Performance

- **Duration:** circa 1 h 28 min di calendario. I test rossi `88c9323` (14:22:40, agente test-write separato) precedono le soglie `a80ffea` (14:26:37). Il Task 3 si è chiuso con `58b5004` (15:50:09), dopo l'attesa della lettura in console dell'utente. Il merge su main è `4dccede` (15:51:23).
- **Started:** 2026-10-21T14:22:40+02:00
- **Completed:** 2026-10-21T15:50:09+02:00
- **Tasks:** 3 di 3 (il Task 3 è un checkpoint umano, chiuso con esito «non determinabile»)
- **Files modified:** 11, esattamente i `files_modified` del piano (`git diff --stat f7e4fad 58b5004`). I cinque sorgenti esistevano già come stub di 01-01 e del test-write.

## Accomplishments

- Server di prova axum con handler lento (`spikes/s05-cancel/src/server.rs:165`) e `DropGuard` che conta come cancellata ogni richiesta il cui handler viene distrutto prima di `complete()` (`server.rs:117-123`). Il server gira su un thread con runtime tokio proprio (`server.rs:193-208`) e serve h1 e h2c sulla stessa porta.
- Fan-out con deadline globale in due varianti (`client.rs:90` e `client.rs:118`), client reqwest con pool di 8 per host e senza proxy (`client.rs:37`).
- Sonde di leak (`probe.rs`): task vivi del runtime, `/proc/self/fd`, `ss` sulle connessioni stabilite.
- Client JEV locale (`jev.rs:61`): `Debug` redatto, `https_only`, `bearer_auth`, stato sintetico costante, nessun reinvio, riscaldamento con GET di openapi.json.
- CLI `mock`, `wiremock`, `deadline`, `jev`, `collect` (`main.rs:123`), con `ensure_present` del manifest prima di qualsiasi misura (`main.rs:195`).
- Evidenze: 9 × 10.000 cicli, 36 run di deadline, 25 richieste JEV reali. Soglie committate prima delle misure, RISULTATI completo, ADR-0010 `Proposed`.

## Esito delle soglie, in breve

| Soglia | Esito |
|--------|-------|
| Deadline del fan-out, p99 < 10 ms per entrambe le varianti | rispettata: A 2,091 ms, B 1,793 ms (n = 18, il p99 coincide con il massimo) |
| Leak su 10.000 cicli: task ± 2, fd + 2, connessioni ≤ 8 | rispettata in 9 scenari su 9: fine uguale alla base |
| h2: server vede il drop in ≥ 95 % entro 100 ms | rispettata secondo i grezzi (100 %, max 7,1 ms); i campioni versionati confermano, i grezzi non sono versionati |
| h1: senza soglia | misurato: drop visto nel 100 % dei cicli, max 23,5 ms; ogni cancellazione chiude la connessione |
| JEV: addebito delle cancellate | **non determinabile**: console vuota il 2026-10-21, anche per i controlli |

## Task Commits

1. **Test rossi (agente test-write, distinto dall'esecutore):** `88c9323` (test). RED_EXPECTED: 9 falliti su `todo!` degli stub, fmt, clippy e scanner verdi.
2. **Task 1: soglie, poi server, client, sonde e client JEV:** `a80ffea` (docs: domanda e soglie prima di qualsiasi misura), `029390c` (feat).
3. **Task 2: misure, risultati e ADR:** `a5e53e8` (test: cancel.csv, deadline.csv, hw.json, percentili esatti in `main.rs`), `7739a85` (docs: RISULTATI, serie JEV reale, ADR-0010).
4. **Task 3: riscontro in console:** `58b5004` (docs: addebito non determinabile, lettura del 2026-10-21).

**Plan metadata:** questo SUMMARY e `01-05-RACCONTO.md`, scritti dal bridge. Merge su main `4dccede`.

## Files Created/Modified

- `spikes/s05-cancel/src/server.rs`: `Metrics`, `DropGuard` con `impl Drop`, `DelayPlan` fisso o distribuito, server su runtime separato; test TEST-4, 5, 14.
- `spikes/s05-cancel/src/client.rs`: `build_client`, `call` con lettura a pezzi e tetto, varianti A e B, `cancel_cycle`; test TEST-6, 7, 8.
- `spikes/s05-cancel/src/probe.rs`: le tre sonde.
- `spikes/s05-cancel/src/jev.rs`: client JEV, `SYNTHETIC_STATE`, `Debug` redatto; test TEST-2.
- `spikes/s05-cancel/src/main.rs`: CLI, scenari di cancellazione, deadline, serie JEV, `collect`, percentili esatti `Exact`.
- `spikes/s05-cancel/results/*`: tre CSV a header fisso e `hw.json` normalizzato.
- `spikes/s05-cancel/RISULTATI.md`, `docs/adr/0010-deadline-cancellazione-http.md`: esito e decisione proposta.

## Decisions Made

- **Variante A di default.** Le due varianti hanno numeri indistinguibili; A cancella per struttura (lo stream si distrugge con i future dentro) e non richiede task `'static`, `Arc` né `spawn`. B resta per i task indipendenti, sempre dentro un `JoinSet`.
- **HTTP/2 verso i provider.** In h2 il drop arriva al server come `RST_STREAM` e la connessione resta; in h1 ogni cancellazione chiude la connessione (fino a 7 in corsa nella misura).
- **Mock su runtime separato**, così `num_alive_tasks` conta solo il client sotto misura.
- **Cicli a concorrenza 8** pari al pool: 48 minuti invece di circa 6 ore, e la soglia sulle connessioni diventa una prova concreta.
- **Riscaldamento JEV** con GET di openapi.json prima di ogni POST cancellata, perché il drop a 50 ms colpisca una richiesta già inviata e non l'handshake TLS; 20 cancellate, pausa di 120 s, 5 di controllo.
- **Budget di costo:** le richieste JEV cancellate si contano come addebitate finché la lettura in console non si ripete con esito chiaro.

## Rischi del test design e trattamento

- **RISK-1 (chiave), HIGH:** TEST-1 e TEST-2 verdi, rieseguiti dal bridge; la chiave non è mai stata stampata nemmeno nel controllo.
- **RISK-2 (soglie dopo i numeri), HIGH:** TEST-3 verde; la tabella è identica, sotto ci sono solo due firme aggiunte.
- **RISK-3 (DropGuard), HIGH:** TEST-4, 5, 6 verdi; i contatori `server_dropped = 10.000`, `server_completed = 0` dei sei scenari axum lo confermano su larga scala.
- **RISK-4 (deadline che non interrompe), HIGH:** TEST-7 e 8 verdi; in `deadline.csv` i task tornano a 3 dopo ogni run.
- **RISK-5..9:** coperti da TEST-9..14, verdi. **RISK-10:** l'esito è registrato con data, ma è «non determinabile».

## Deviations from Plan

### Auto-fixed Issues (livello a)

**1. [Rule 3 - Blocking] Build del runner offline con crate vendorizzate.** Il DNS del runner non risolve `index.crates.io`; le crate stanno in `spikes/target/vendor-s05` (ignorata), `Cargo.lock` invariato. Stesso rimedio di 01-04.

**2. [Rule 3 - Blocking] `hw.json` normalizzato fuori repo.** `tools/hw-manifest.sh` (01-02) scrive `cpu`, `ram_total`, `docker_desktop_resources` e `images` in forme che `common::manifest` (01-01) rifiuta. L'esecutore ha applicato la stessa normalizzazione di `s01-upload/bench.sh` con un driver nello scratchpad; l'originale era in `results/raw/`, non versionata. Nessun file chiuso toccato. Il difetto fra 01-01 e 01-02 era già noto.

**3. [Deviazione a] Cicli di cancellazione a concorrenza 8** invece che in serie, documentato in RISULTATI «Anomalie e deviazioni».

**4. [Rule 2 - Security] Segnalazioni dello scanner** (`029390c`): l'handler del mock senza autorizzazione porta un commento `public-endpoint` motivato (`server.rs:164`); `resp.bytes()` è sostituito dalla lettura a pezzi con tetto (`client.rs:55-75`). Nel test-write lo scanner aveva già imposto URL loopback letterali.

**5. [Deviazione a] Percentili esatti in `main.rs`** (`main.rs:278-309`) al posto di `common::hist::Recorder`, per il difetto qui sotto. Il cambio (`a5e53e8`) tocca solo le righe di riepilogo stampate: i CSV contengono valori grezzi per ciclo e sono stati prodotti con il codice di `029390c` (`git_rev` di `hw.json`).

### Difetto in un file chiuso di 01-01 (segnalazione, non corretto qui)

**6. `spikes/common/src/hist.rs:51-53`: `Recorder::record_us` usa `saturating_record`.** Il commento a `:48-50` dice che l'istogramma «si ridimensiona da solo», ma `Histogram::new(3)` senza auto-resize satura: ogni valore oltre 2.047 µs viene registrato come 2.047. I primi riepiloghi della sessione di cancellazione risultavano infatti saturati. Aggirato in `main.rs`; il file non è stato toccato. Serve una **fase decimale di gap-closure** con `riapre_perimetro: [01-01]`.
**Impatto possibile su 01-04:** `spikes/s01-upload/src/upload.rs:77-81` usa cinque `Recorder` per le fasi dell'upload (esposti da `/bench/phases`, `upload.rs:594`). Il SUMMARY di 01-04 dice che quegli istogrammi erano saturati e non sono stati usati, e che le latenze vengono da `curl`: l'impatto sulle evidenze di 01-04 sembra quindi nullo, ma va confermato nella gap-closure. Ogni piano futuro che usa `Recorder` con valori sopra 2 ms è esposto.

---

**Total deviations:** 6 (2 blocchi d'ambiente, 3 di processo o sicurezza, 1 segnalazione su file chiuso); nessuna di livello b né c.
**Impact on plan:** nessuna estensione di perimetro (11 file su 11).

## Issues Encountered

- **Addebito JEV non determinabile.** La console typesafe è risultata «vuota» il 2026-10-21 nelle finestre indicate, **nemmeno le 5 richieste di controllo** completate con 200 compaiono. Le cause possibili sono un ritardo di aggregazione o una console di un altro account o vista. La lettura va ripetuta.
- **Grezzi per ciclo non versionati.** I p99 e i massimi di `exit_after_cancel_us` e `server_saw_cancel_ms` in RISULTATI vengono da `results/raw/`, sparita con il worktree. Il bridge ha ricalcolato solo ciò che sta nei CSV versionati.
- **Natura delle misure.** I numeri di cancellazione e deadline sono su server di prova locali (server axum lento e wiremock su loopback). JEV è reale, ma con un contenuto sintetico costante, e la serie prova solo la chiusura lato client: che cosa fa JEV con una richiesta interrotta resta ignoto.
- **Wiremock:** il drop lato server non è osservabile (nessuna guardia nell'handler); per wiremock si misurano solo leak e uscita del chiamante.

## Discrepanze fra rapporto e prove

- Il rapporto dichiara la sezione Soglie «invariata»: è vera per la tabella; la sezione contiene in più due righe di firma aggiunte dopo le misure. Non è una modifica delle soglie.
- ADR-0010 cita `Reranker::rank` (ADR-0012): ADR-0012 non esiste ancora in `docs/adr/`. Il riferimento viene dal piano ed è coerente con la fase, ma oggi non è navigabile.
- Il resto del rapporto (conteggi, percentili della deadline, righe JEV, assenza della chiave, gate) coincide con le prove ricalcolate dal bridge.

## Threat Flags

- **T-24 (consumo illimitato):** mitigazione misurata in `deadline.csv` (uscita entro 2,1 ms, concorrenza 8, lavoro pendente cancellato).
- **Costo JEV:** le cancellate si contano come addebitate; finché l'addebito non è chiarito, una deadline stretta non riduce il costo delle chiamate interrotte.

## Known Stubs

Nessuno nel codice dello spike.

## User Setup Required

Nessuno di nuovo. Resta da ripetere la lettura in console typesafe sulle finestre 2026-10-21 13:32:17.806Z-13:33:00.800Z (20 cancellate) e 13:35:02.846Z-13:35:11.841Z UTC (5 di controllo, 332 + 21 token ciascuna), controllando account e vista.

## Punti aperti

- Rilettura della console JEV; se resta vuota anche per i controlli, il riscontro va cercato in un'altra fonte (fattura, supporto del provider).
- Gap-closure su `spikes/common/src/hist.rs` (01-01), con verifica dell'impatto su `s01-upload`.
- Coda estrema della deadline non stimata (n = 18 per variante).
- `hw.json`: il difetto di forma fra `hw-manifest.sh` (01-02) e `common::manifest` (01-01) resta aperto.

## Next Phase Readiness

- 01-16 (S-07) può trascrivere da RISULTATI §Esito «Impatto sulle porte»: deadline `tokio::time::Instant` in firma per `DecisionEngine`, `AnswerGenerator`, `Embedder`, `Reranker::rank`; `reqwest::Client` condiviso con pool.
- 01-17 ratifica ADR-0010 e porta «HTTP client `reqwest` con Rustls» a Decisa in `DEPENDENCIES.md`.
- 01-11 e 01-12 (chiamate JEV reali) devono contare nel budget le eventuali richieste cancellate come addebitate.

## Self-Check: PASSED

- 11 file su 11 dichiarati nel diff `f7e4fad..58b5004`, nessuno fuori elenco.
- Rieseguiti dal bridge sull'host: `cargo fmt --all -- --check` verde, `cargo clippy -p s05-cancel --all-targets --locked --offline -- -D warnings` verde, `cargo test -p s05-cancel --locked --offline` 9/9, scanner exit 0, controllo della chiave negativo, link dell'ADR esistenti. Leak e percentili della deadline ricalcolati sui CSV.

**Modifica:** Claude (Anthropic) · 2026-10-21 · bridge del piano 01-05

**Nota (Claude, Anthropic · 2026-10-21 · dopo la fase 1.2):** il valore saturato reale del Recorder di `hist.rs` era 2 µs, non 2047 µs come scritto sopra: con `Histogram::new(3)` il massimo tracciabile iniziale è 2. Il difetto è corretto nella gap-closure 1.2 → `../01.2-gap-closure-hist-rs-il-recorder-delle-latenze-non-satura-sop/01.2-01-SUMMARY.md`. Le misure di 01-05 restano valide (percentili esatti calcolati in proprio).
