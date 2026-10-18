# Test design 01-05 — S-05 cancellazione e deadline

```yaml
reviewed_plan:
  path: .planning/phases/01-spike-tecnici/01-05-PLAN.md
  sha256: 6344843d7e2769c4c20ba2e04a6c270601238603aca4f9fc40d014c6b8f8a004
risks:
  - id: RISK-1
    severity: HIGH
    title: TYPESAFE_API_KEY stampata in log, errori, CSV o RISULTATI
    source: threat_model T-01-05-01
  - id: RISK-2
    severity: HIGH
    title: Soglie scritte o ritoccate dopo aver visto i numeri (misura non valida)
    source: must_have "artifacts RISULTATI.md contains Soglie (scritte PRIMA della misura)"
  - id: RISK-3
    severity: HIGH
    title: DropGuard conta male (drop non registrato o registrato anche a richiesta completata), quindi «server vede la cancellazione» e leak sono falsi
    source: must_have "10.000 cicli senza leak di task, file descriptor o connessioni"
  - id: RISK-4
    severity: HIGH
    title: La deadline del fan-out non interrompe davvero i task (variante B senza abort, variante A che attende i futuri in volo)
    source: must_have "deadline globale ... fa uscire il chiamante entro 10 ms dalla scadenza"; threat_model T-01-05-04
  - id: RISK-5
    severity: MEDIUM
    title: Evidenze incomplete (scenari mancanti, ultima riga non a 10000 cicli, header diversi, hw.json assente)
    source: must_have "10.000 cicli" e "misura di entrambe le varianti"
  - id: RISK-6
    severity: MEDIUM
    title: Serie JEV con retry o numero di richieste diverso da 20+5 (costo non controllato, riscontro in console impossibile)
    source: threat_model T-01-05-03
  - id: RISK-7
    severity: MEDIUM
    title: Contenuto reale inviato a JEV
    source: threat_model T-01-05-02
  - id: RISK-8
    severity: MEDIUM
    title: Verifica TLS disattivata o client senza Rustls verso JEV
    source: threat_model T-01-05-05
  - id: RISK-9
    severity: MEDIUM
    title: HTTP/2 non effettivamente negoziato nel caso h2 (misura h2 che in realtà è h1)
    source: must_have "server mock HTTP/1 e HTTP/2"
  - id: RISK-10
    severity: LOW
    title: Esito sull'addebito JEV non riportato o senza data; ADR-0010 senza link a evidenze esistenti
    source: must_have "l'utente verifica nella console se sono state addebitate"
tests:
  - {id: TEST-1, title: La chiave non compare in output e artefatti, type: check-statico+runtime, priority: P0, catches: [RISK-1]}
  - {id: TEST-2, title: jev_client_debug_and_errors_redact_key, type: cargo-unit, priority: P0, catches: [RISK-1]}
  - {id: TEST-3, title: Commit delle Soglie precede ogni file in results/ e la sezione non cambia dopo, type: check-git, priority: P0, catches: [RISK-2]}
  - {id: TEST-4, title: drop_guard_counts_dropped_when_not_completed, type: cargo-unit, priority: P0, catches: [RISK-3]}
  - {id: TEST-5, title: drop_guard_completed_is_not_dropped, type: cargo-unit, priority: P0, catches: [RISK-3]}
  - {id: TEST-6, title: server_sees_client_cancel_h2, type: cargo-integration-locale, priority: P1, catches: [RISK-3, RISK-9]}
  - {id: TEST-7, title: deadline_variant_a_exits_and_drops_inflight, type: cargo-integration-locale, priority: P0, catches: [RISK-4]}
  - {id: TEST-8, title: deadline_variant_b_aborts_joinset, type: cargo-integration-locale, priority: P0, catches: [RISK-4]}
  - {id: TEST-9, title: Validità delle evidenze (header, scenari, cicli, hw.json), type: check-evidenze, priority: P1, catches: [RISK-5]}
  - {id: TEST-10, title: Soglie di leak ricalcolate dai CSV, type: check-evidenze, priority: P1, catches: [RISK-3, RISK-4, RISK-5]}
  - {id: TEST-11, title: jev_cancel.csv con 25 righe, 20 cancelled e 5 control, nessun retry nel codice, type: check-evidenze, priority: P1, catches: [RISK-6]}
  - {id: TEST-12, title: Stato JEV costante sintetico, type: check-statico, priority: P2, catches: [RISK-7]}
  - {id: TEST-13, title: Nessun bypass TLS e nessun http:// fuori da loopback, type: check-statico, priority: P1, catches: [RISK-8]}
  - {id: TEST-14, title: h2_prior_knowledge_negotiates_http2, type: cargo-integration-locale, priority: P2, catches: [RISK-9]}
  - {id: TEST-15, title: Esito addebito registrato con data, ADR-0010 con link esistenti, type: check-documento, priority: P2, catches: [RISK-10]}
uncovered: []
```

## Come leggere questo design

S-05 misura due cose che diventeranno regole del prodotto: che cancellare una richiesta verso un provider non lasci task, file descriptor o connessioni appesi, e che una deadline globale su un fan-out restituisca il controllo in tempo. Un test di misura sbagliato qui produce una regola di concorrenza sbagliata in ADR-0010. Per questo i rischi HIGH sono, oltre alla chiave API, gli strumenti stessi di misura: il contatore di drop (RISK-3) e il meccanismo di deadline (RISK-4). Si testano con server locali su `127.0.0.1:0`, senza rete esterna.

I test Rust stanno nei moduli `#[cfg(test)]` dei file `spikes/s05-cancel/src/*.rs` elencati in `files_modified` (il crate è un binario: `cargo test -p s05-cancel` esegue i test unitari del binario). I test che leggono `/proc/self/fd` o `ss` girano solo nel runner Linux; quelli indicati come «locale» girano anche sull'host perché usano solo tokio, axum e reqwest su loopback.

## Test

**TEST-1 — La chiave non trapela.** Statico: `grep -n "TYPESAFE_API_KEY" spikes/s05-cancel/src/*.rs` compare solo nell'argomento di `common::env::required`; nessun `println!`/`eprintln!`/`tracing` che formatti header o la variabile che contiene la chiave. A runtime, dopo il Task 2: con la chiave caricata in una variabile della shell, `grep -rF "$TYPESAFE_API_KEY" spikes/s05-cancel docs/adr/0010-*.md` non trova nulla (il comando non stampa la chiave: usa `-q` e stampa solo il nome del file che fallisce). Lo scanner di sicurezza su `s05-cancel` esce 0.

**TEST-2 — `jev_client_debug_and_errors_redact_key`** (in `jev.rs`). Costruisce il client JEV con una chiave fittizia riconoscibile (`"sk-test-REDACT-ME"`) passata come parametro (il costruttore deve accettarla per poterla testare senza variabile d'ambiente) e un URL verso una porta loopback chiusa; esegue una richiesta che fallisce in connessione; verifica che né `format!("{:?}", client)` né `format!("{:#}", err)` contengano `REDACT-ME`. Se il client non implementa `Debug`, la prima asserzione decade e resta la seconda.

**TEST-3 — Soglie prima delle misure.** Il commit che introduce per la prima volta un file in `spikes/s05-cancel/results/` è successivo al commit che introduce la sezione «## Soglie (scritte PRIMA della misura)» (`git log --diff-filter=A --format=%H -- spikes/s05-cancel/results/ | tail -1` è discendente di `git log --diff-filter=A -S"## Soglie (scritte PRIMA della misura)" --format=%H -- spikes/s05-cancel/RISULTATI.md | tail -1`, verificato con `git merge-base --is-ancestor`). La sezione Soglie estratta (dal titolo al titolo successivo) è identica fra quel commit e HEAD. Contiene le quattro voci: p99 < 10 ms, leak (task ± 2, fd base + 2, connessioni ≤ pool), h2 ≥ 95% entro 100 ms, JEV senza soglia numerica.

**TEST-4 — `drop_guard_counts_dropped_when_not_completed`** (in `server.rs`). `Metrics` nuova; si crea un `DropGuard` e lo si lascia uscire di scope senza `complete()`; `dropped == 1`, `completed == 0`. Variante asincrona: un future che tiene il guard e dorme 1 s, avvolto in `tokio::time::timeout(10 ms)`; dopo il timeout `dropped == 1`.

**TEST-5 — `drop_guard_completed_is_not_dropped`** (in `server.rs`). Guard su cui si chiama `complete()` prima del drop: `completed == 1`, `dropped == 0`. Insieme a TEST-4 garantisce che `server_dropped + server_completed = started` per costruzione.

**TEST-6 — `server_sees_client_cancel_h2`** (in `main.rs` o `client.rs`, `#[tokio::test(flavor = "multi_thread")]`). Server axum lento (`DELAY_MS = 2000`) su `127.0.0.1:0`; client reqwest `http2_prior_knowledge()`; si cancella la richiesta dopo 50 ms (drop del future). Entro 500 ms `dropped` diventa 1 e `completed` resta 0. È la prova che lo strumento rileva davvero il drop prima di affidargli 10.000 cicli. Per HTTP/1 il test equivalente registra il valore senza asserzione (il piano non fissa soglia per h1).

**TEST-7 — `deadline_variant_a_exits_and_drops_inflight`** (in `client.rs`). Server lento con `DELAY_MS = 5000`, fan-out di 8 richieste, budget 200 ms con variante A. Il chiamante ritorna entro budget + 50 ms (margine largo per un test, la soglia vera è nelle misure) e, dopo 500 ms di assestamento, il server ha `dropped == 8` (le richieste in volo sono state cancellate, non abbandonate). Il secondo controllo distingue una deadline che ritorna in tempo ma lascia i futuri vivi da una che li cancella.

**TEST-8 — `deadline_variant_b_aborts_joinset`** (in `client.rs`). Come TEST-7 con variante B (`JoinSet` + `Semaphore(8)`), fanout 16: ritorno entro budget + 50 ms; dopo assestamento `num_alive_tasks()` torna al valore di base ± 2 e il server ha `dropped` pari alle richieste avviate. Verifica che il drop del `JoinSet` faccia abort dei task.

**TEST-9 — Validità delle evidenze.** Il verify del Task 2 (header esatti, `hw.json` JSON valido) più: `hw.json` passa `common::manifest::ensure_present` (in pratica: ha `timestamp_utc`, `git_rev`, `cpu`, `rustc` e il digest dell'immagine runner in `images`); in `cancel.csv` le coppie `(scenario, proto, cancel_at_ms)` sono esattamente 9 (h1, h2, wiremock × 50/200/500) e per ciascuna esiste una riga con `cycle = 10000`; `deadline.csv` contiene varianti a e b × fanout 8/16/32 × budget 200/500/1000 con `run` 2 e 3 (la 1 è warm-up scartata).

**TEST-10 — Soglie ricalcolate dai CSV.** Script `node -e` (inline, nessun file nuovo) che, per ogni scenario di `cancel.csv`, confronta la riga a `cycle = 10000` con la prima: `alive_tasks` ≤ base + 2, `open_fds` ≤ base + 2, `established_conns` ≤ dimensione del pool dichiarata in RISULTATI §Setup; per h2, quota di cicli con `server_dropped` e `server_saw_cancel_ms ≤ 100` ≥ 95%; per `deadline.csv`, p99 di `exit_after_deadline_us` < 10000 per variante. Il risultato deve coincidere con la colonna «esito vs soglia» di RISULTATI: un «rispettata» in RISULTATI con il controllo rosso è un errore di trascrizione. Il controllo non giudica la decisione, verifica che i numeri dichiarati siano quelli dei file.

**TEST-11 — Serie JEV controllata.** `jev_cancel.csv` ha 25 righe dati, 20 con `kind = cancelled` e 5 con `kind = control`, `sent_at_utc` in RFC 3339 UTC (`Z` finale) e nessuna colonna oltre l'header; per le cancellate `status`, `model_resolved` e i token sono vuoti. Statico: `jev.rs` non contiene un ciclo di retry (`grep -nE "retry|loop \{|for _ in" spikes/s05-cancel/src/jev.rs` letto dal verificatore: nessun reinvio).

**TEST-12 — Stato sintetico.** In `jev.rs` lo `state` è una costante (`const` o `static`) di testo; nessuna lettura da `spikes/data/` nel crate (`grep -rn "data/corpus" spikes/s05-cancel/src` vuoto).

**TEST-13 — TLS.** `grep -rn "danger_accept_invalid\|tls_built_in_root_certs(false)" spikes/s05-cancel/src` vuoto; `grep -rn "http://" spikes/s05-cancel/src | grep -v "127.0.0.1"` vuoto; scanner con regola `tls-bypass` verde.

**TEST-14 — `h2_prior_knowledge_negotiates_http2`** (in `server.rs`). Server avviato come nel sottocomando `mock --proto h2`; risposta a un client `http2_prior_knowledge()` con `response.version() == http::Version::HTTP_2`. Chiude il [NV] di RESEARCH §11.12 con una prova e impedisce che la riga h2 di `cancel.csv` sia in realtà HTTP/1.

**TEST-15 — Esito addebito e ADR.** Il verify del Task 3 (`Addebito JEV dopo il drop` presente, «in lettura dall'utente» assente) più: la voce contiene una data `2026-` e uno dei tre esiti (addebitato / non addebitato / non determinabile); ADR-0010 contiene la stessa voce in «Conseguenze»; ogni percorso `spikes/...` citato in ADR-0010 esiste (`grep -oE 'spikes/[A-Za-z0-9_./-]+' | xargs -n1 test -f`).

## Esecuzione singola

`cd spikes && cargo test -p s05-cancel <nome_test>` per TEST-2, 4, 5, 6, 7, 8, 14 (sull'host bastano tokio e loopback; la parte di TEST-8 su `num_alive_tasks` richiede la feature `tokio_unstable` solo se l'API lo esige: in quel caso il test gira nel runner con `RUSTFLAGS="--cfg tokio_unstable"` e lo si annota). I controlli TEST-3, 9, 10, 11, 15 sono comandi `git`/`node -e` lanciati dalla radice dopo il Task 2 o il Task 3.

**Modifica:** Claude (test designer, Anthropic) · 2026-10-18 · test design 01-05

**Modifica:** Claude (Anthropic) · 2026-10-18 · reviewed_plan.sha256 aggiornato da 1450420091b9… a 6344843d7e27… per l'allineamento a 01-02 v2 (comando del runner via session.sh with/oneshot; finestra di misura con prenotazione session.sh). I test del design non cambiano: cambiano solo i comandi di avvio dei servizi e la prenotazione della finestra di misura.
