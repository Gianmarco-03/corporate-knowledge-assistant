# S-05 — Cancellazione e deadline delle richieste HTTP verso i provider · RISULTATI

**Modifica:** Claude (gsd-executor 01-05) · 2026-10-21 · domanda e soglie scritte prima di qualsiasi misura

## Domanda e decisione da chiudere

- **S-05 (criterio 4 della fase, parte «cancellazione verso provider HTTP»):** quando il chiamante rinuncia a una richiesta HTTP verso un provider (drop del future, deadline scaduta), la richiesta viene davvero cancellata, senza lasciare task, file descriptor o connessioni appesi? E una deadline globale su un fan-out restituisce il controllo al chiamante in tempo?
- **D-22 (01-CONTEXT):** deadline globale per il fan-out verso i provider, misurata in due varianti (`timeout_at` + `buffer_unordered`, `JoinSet` + `Semaphore`), su server mock HTTP/1 e HTTP/2 e su JEV reale, con la domanda aperta se JEV addebita il lavoro di una richiesta cancellata.
- Riga `DEPENDENCIES.md` coinvolta: «HTTP client `reqwest` con Rustls», oggi **Proposta**.

## Soglie (scritte PRIMA della misura)

Tabella immutabile dopo il commit che la introduce: correzioni solo in «Anomalie e deviazioni».

| Metrica | Soglia | Fonte soglia |
|---------|--------|--------------|
| Uscita del chiamante dopo la deadline globale del fan-out (`exit_after_deadline_us`) | p99 < 10 ms, per **entrambe** le varianti (A: `timeout_at` + `buffer_unordered`; B: `JoinSet` + `Semaphore`) | RICERCA §7 S5 |
| Leak su 10.000 cicli per scenario (h1, h2, wiremock × cancellazione a 50, 200, 500 ms) | `num_alive_tasks` torna al valore di base ± 2 dopo 1 s di assestamento; file descriptor aperti (`/proc/self/fd`) entro base + 2; connessioni stabilite verso il mock (`ss -tn state established '( dport = :<porta> )'`) ≤ dimensione del pool (8 connessioni inattive per host, `pool_max_idle_per_host(8)`) | RICERCA §7 S4 |
| HTTP/2: il server vede la cancellazione dopo il drop lato client | `server_dropped` in ≥ 95 % dei cicli, con `server_saw_cancel_ms` ≤ 100 ms (inferenza dichiarata) | inferenza dichiarata (piano 01-05) |
| HTTP/1: il server vede la cancellazione | misurato e documentato **senza soglia**: dipende da quando hyper rileva la chiusura della connessione | [NV] RESEARCH §11.12 |
| JEV reale: addebito delle richieste cancellate a 50 ms | **nessuna soglia numerica**; esito = addebitato / non addebitato / non determinabile, letto dall'utente nella console typesafe | D-22, [NV] RESEARCH §11.13 |

**Modifica:** Claude (gsd-executor 01-05) · 2026-10-21 · setup, risultati, anomalie ed esito dopo le misure (le Soglie sopra sono invariate)

**Modifica:** Claude (gsd-executor 01-05) · 2026-10-21 · esito sull'addebito JEV dopo la lettura dell'utente: non determinabile (Task 3)

## Setup

- Hardware: `results/hw.json` (sha256 `4a3ab747ccc485032f037fc00a143da56f486d2ebc32acbd6ccb98372049d4d9`, sessione `01-05-20261021t124025z-2846`): i7-13700K con 16 core e 24 thread, 32 GiB, Docker Desktop 29.8.2 su WSL 2 (16 CPU e 19,5 GiB alla VM), piano energetico «Bilanciato». Il manifest è normalizzato allo schema di `common::manifest` (vedi «Anomalie e deviazioni»); l'originale sta in `results/raw/`.
- Immagine: `cka-spikes-runner:latest` (`sha256:86a924ad5599d1f010ca454e8c910704c662df46b1557b5343950e14ab91d298`), rustc 1.95.0. La build è `--release --locked --offline` con le crate vendorizzate dall'host in `spikes/target/vendor-s05` (cartella ignorata), perché il DNS del runner non risolve `index.crates.io`: è lo stesso rimedio di 01-04.
- Crate: reqwest 0.13.5 (Rustls, `rustls-platform-verifier`), axum 0.8.9, tokio 1.53.3, wiremock 0.6.5, futures 0.3.34.
- Client verso il mock: `connect_timeout(2 s)`, `pool_idle_timeout(90 s)`, `pool_max_idle_per_host(8)`, nessun proxy. Per h1 `http1_only()`, per h2 `http2_prior_knowledge()` (h2c in chiaro su 127.0.0.1).
- Client verso JEV: Rustls con la verifica dei certificati di default, `https_only(true)`, `connect_timeout(5 s)`, `timeout(60 s)`.
- Mock axum: thread e runtime tokio propri (2 worker), così i suoi task non entrano nel `num_alive_tasks` del runtime del client. Socket e file descriptor restano dello stesso processo, e le sonde li contano.
- Corpus: nessuno. A JEV va solo la costante `SYNTHETIC_STATE` di `src/jev.rs` (due frasi su una società inventata), con una domanda `noul`.
- Sessioni: tutte aperte con `session.sh with 01-05`, solo con il servizio `runner`, con `ps --assert-only runner` verde e senza container di altri piani.
  - `01-05-20261021t124025z-2846`: matrice di cancellazione;
  - `01-05-20261021t132844z-ffa4`: deadline e `collect`;
  - `01-05-20261021t133206z-9fe9`: serie JEV reale.
- Comandi, lanciati da `spikes/` nel runner via `session.sh oneshot` (`B=/target/release/s05-cancel`):
  - `$B mock --proto h1`, `$B mock --proto h2`, `$B wiremock`. Default: cancellazione a 50, 200 e 500 ms; 10.000 cicli; concorrenza 8; ritardo dell'handler 2.000 ms.
  - `$B deadline --variant a`, `$B deadline --variant b`. Default: fanout 8, 16 e 32; budget 200, 500 e 1.000 ms; 3 run; concorrenza 8; h2; ritardi del mock distribuiti in [50, 2.000) ms.
  - `$B collect`: unisce i grezzi di `results/raw/` in `cancel.csv` e `deadline.csv`; in `deadline.csv` esclude la run 1 di warm-up.
  - `$B jev --cancel-at-ms 50`: 20 richieste (da `S05_JEV_CANCEL_REQUESTS`), 120 s di pausa, poi 5 richieste di controllo.
- Protocollo di un ciclo:
  - **la richiesta:** un GET `/slow?id=<n>`, cancellato con `timeout_at` al tempo previsto;
  - **`exit_after_cancel_us`:** l'istante in cui il chiamante riprende il controllo, meno l'istante della cancellazione;
  - **`server_saw_cancel_ms`:** l'istante in cui la `DropGuard` dell'handler `id` viene droppata, meno l'istante della cancellazione;
  - **prima della base:** una richiesta di riscaldamento cancellata, poi la quiete del mock, poi 300 ms;
  - **le righe del CSV:** la riga `cycle = 0` è la base, poi c'è una riga ogni 100 cicli. La riga `cycle = 10000` si campiona dopo 1 s di assestamento e dopo la quiete del mock (nessun handler in corso), con i contatori del server al netto del riscaldamento;
  - **i grezzi per ciclo:** stanno in `results/raw/cancel-*.cycles.csv`.

## Risultati

Percentili esatti (nearest-rank), ricalcolati dai grezzi per ciclo. I tempi sono in µs, salvo dove è indicato altrimenti.

### Cancellazione, 10.000 cicli per scenario (`results/cancel.csv`)

| Scenario | `exit_after_cancel_us` p50 / p95 / p99 / max | drop visto dal server | `server_saw_cancel_ms` p50 / p99 / max | task base → fine | fd base → fine | conn. base → fine (max in corsa) | Esito vs soglia |
|---|---|---|---|---|---|---|---|
| mock h1, 50 ms | 1.084 / 1.937 / 3.744 / 5.103 | 10.000 / 10.000 | 1,211 / 3,861 / 5,278 | 0 → 0 | 15 → 15 | 0 → 0 (7) | leak: rispettata; h1 senza soglia |
| mock h1, 200 ms | 1.165 / 1.902 / 2.283 / 7.239 | 10.000 / 10.000 | 1,340 / 2,798 / 7,513 | 0 → 0 | 20 → 20 | 0 → 0 (7) | leak: rispettata; h1 senza soglia |
| mock h1, 500 ms | 1.123 / 1.920 / 2.171 / 23.321 | 10.000 / 10.000 | 1,261 / 2,524 / 23,517 | 0 → 0 | 25 → 25 | 0 → 0 (7) | leak: rispettata; h1 senza soglia |
| mock h2, 50 ms | 1.317 / 1.749 / 3.641 / 6.743 | 10.000 / 10.000 | 1,497 / 3,791 / 7,061 | 3 → 3 | 17 → 17 | 1 → 1 (1) | rispettate (100 % entro 100 ms) |
| mock h2, 200 ms | 1.172 / 1.861 / 2.512 / 4.581 | 10.000 / 10.000 | 1,361 / 2,604 / 4,868 | 3 → 3 | 22 → 22 | 1 → 1 (1) | rispettate (100 % entro 100 ms) |
| mock h2, 500 ms | 1.125 / 2.025 / 3.155 / 4.270 | 10.000 / 10.000 | 1,300 / 3,306 / 4,471 | 3 → 3 | 27 → 27 | 1 → 1 (1) | rispettate (100 % entro 100 ms) |
| wiremock h1, 50 ms | 1.091 / 1.940 / 3.503 / 6.849 | non osservabile | — | 0 → 0 | 15 → 15 | 0 → 0 (7) | leak: rispettata |
| wiremock h1, 200 ms | 1.128 / 1.898 / 2.091 / 5.026 | non osservabile | — | 0 → 0 | 15 → 15 | 0 → 0 (7) | leak: rispettata |
| wiremock h1, 500 ms | 1.167 / 1.917 / 2.150 / 7.557 | non osservabile | — | 0 → 0 | 15 → 15 | 0 → 0 (7) | leak: rispettata |

- In nessun ciclo la richiesta è terminata prima della cancellazione.
- Nel mock axum, in ogni scenario, `server_dropped` = 10.000 e `server_completed` = 0.
- Wiremock ha ricevuto 10.001 richieste per scenario, riscaldamento compreso.
- In h2 resta una sola connessione stabilita (quella del pool), con 3 task vivi stabili.
- In h1 ogni cancellazione chiude la connessione: alla fine non ne resta nessuna.

### Deadline globale del fan-out (`results/deadline.csv`, run 2 e 3, h2, concorrenza 8)

| Variante | n | `exit_after_deadline_us` p50 | p95 | p99 | max | task dopo 1 s (base 3) | Esito vs soglia (p99 < 10 ms) |
|---|---|---|---|---|---|---|---|
| A — `timeout_at` + `buffer_unordered` | 18 | 1.031 | 2.091 | 2.091 | 2.091 | 3 in tutte le run | rispettata |
| B — `JoinSet` + `Semaphore` | 18 | 1.154 | 1.793 | 1.793 | 1.793 | 3 in tutte le run | rispettata |

- La deadline è scaduta in tutte le 36 run, senza uscite anticipate: 98 richieste sono state completate e le altre cancellate.
- Lato server, su tutte e tre le run di ciascuna variante:
  - A ha avviato 272 richieste: 76 completate e 196 cancellate;
  - B ne ha avviate 272: 77 completate e 195 cancellate.
- Le richieste non ancora avviate alla scadenza non partono mai. Quelle in volo vengono cancellate, non lasciate finire.

### JEV reale (`results/jev_cancel.csv`)

- **20 richieste `cancelled`**, droppate fra 50,3 e 52,0 ms dall'invio, su una connessione HTTP/2 già aperta (prima di ognuna c'è un riscaldamento `GET /openapi.json`). Non c'è risposta, quindi `status`, `model_resolved` e i token sono vuoti.
- **5 richieste `control` complete:** status 200, `model_resolved` = `jev-1.13.0`, 332 token di input e 21 di output ciascuna, latenza fra 228 e 287 ms. Una richiesta di controllo dura circa 230 ms: la cancellazione a 50 ms colpisce quindi una richiesta già inviata e ancora in elaborazione.
- **Orari UTC di invio:**
  - cancellate: dalle 2026-10-21T13:32:17.806Z alle 13:33:00.800Z;
  - di controllo: dalle 13:35:02.846Z alle 13:35:11.841Z.
- **Nessun reinvio:** 25 righe, cioè 20 + 5 POST `systemone`.

## Anomalie e deviazioni

- **Cicli a concorrenza 8, non sequenziali.** Con i cicli in serie la matrice avrebbe richiesto circa 6 ore (10.000 × (50 + 200 + 500) ms × 3 server). Con 8 cicli in volo, pari al pool, ci sono voluti circa 48 minuti. La soglia sulle connessioni (≤ 8) diventa così una prova concreta: in h1 si arriva a 7 connessioni contemporanee, tutte chiuse alla fine.
- **La base dei fd cambia fra gli scenari dello stesso processo** (15, 20, 25 per h1): restano aperti il listener e il thread dei mock degli scenari precedenti. Il confronto si fa sempre con la base del proprio scenario (riga `cycle = 0`).
- **Wiremock:** il drop lato server non è osservabile, perché nell'handler di wiremock non c'è una guardia. Quindi `server_saw_cancel_ms`, `server_dropped` e `server_completed` restano vuoti. Wiremock usa un pool di server: i tre scenari hanno riusato la stessa porta, azzerata fra un uso e l'altro.
- **Campione della deadline piccolo:** 18 run per variante (9 configurazioni × 2 run dopo il warm-up), come da piano. Per questo il p99 coincide con il massimo. Il margine sulla soglia (2,1 ms contro 10 ms) è ampio, ma non è una stima della coda estrema.
- **Difetto in `spikes/common/src/hist.rs`** (file chiuso, piano 01-01):
  - **il difetto:** `Recorder::record_us` usa `saturating_record`, che non ridimensiona l'istogramma creato con `Histogram::new(3)`. Ogni valore oltre 2.047 µs viene saturato;
  - **l'effetto:** i primi percentili stampati durante la sessione di cancellazione erano saturati (max 2.046-2.047);
  - **cosa ho fatto:** `main.rs` ora calcola i percentili esatti, e tutti i numeri di questa pagina sono ricalcolati dai grezzi per ciclo. Il file non è stato toccato: la correzione spetta a una fase di gap-closure.
- **`hw.json` normalizzato.** `tools/hw-manifest.sh` (01-02) scrive `cpu`, `ram_total`, `docker_desktop_resources` e `images` in forme che `common::manifest` (01-01) rifiuta. È il difetto già noto fra 01-01 e 01-02. Ho applicato la stessa normalizzazione di `s01-upload/bench.sh` (01-04), con un driver fuori dal repo; l'originale è in `results/raw/`.
- **Un solo manifest per tre sessioni:** `hw.json` viene dalla sessione di cancellazione. Le sessioni di deadline e JEV sono girate subito dopo, sulla stessa macchina e con la stessa immagine.
- **Riscaldamento JEV:** prima di ogni richiesta cancellata c'è un `GET /openapi.json`, che non è una chiamata `systemone`. Serve perché il drop a 50 ms colpisca una richiesta già trasmessa e non l'handshake TLS. Il numero di POST resta 20 + 5.
- **[NV] di RESEARCH §11.12, chiusi:**
  - `axum::serve` serve h2c con prior knowledge (TEST-14 restituisce `HTTP_2`);
  - quando il client chiude una connessione h1, hyper droppa l'handler in pochi ms (100 % dei cicli, p99 ≤ 3,9 ms);
  - in wiremock 0.6.5 `set_delay` e `received_requests` funzionano come descritto;
  - `num_alive_tasks` è stabile in tokio 1.53, senza `tokio_unstable`.
- **[NV] di §11.13 (addebito JEV):** resta aperto. La lettura in console del 2026-10-21 è risultata vuota, anche per le richieste di controllo, quindi l'esito è «non determinabile» (vedi «Esito»). La lettura si può ripetere più avanti.

## Esito

- **Decisione proposta sul modello di deadline.** Ogni query ha una deadline globale, passata esplicitamente a ogni porta che chiama un provider.
  - **Variante A, il default per il fan-out** (`timeout_at` + `buffer_unordered(N)`): la cancellazione segue la struttura del codice, perché lo stream si distrugge e con lui ogni future in volo. Non servono task `'static` né `Arc`, e il limite di concorrenza è un parametro.
  - **Variante B, per i casi che richiedono task indipendenti** (`JoinSet` + `Semaphore`): parallelismo su più worker, oppure lavoro che deve continuare dopo il ritorno dell'handler.
  - **Il criterio di scelta:** le due varianti hanno numeri indistinguibili (p99 2,1 e 1,8 ms, task tornati alla base), quindi si sceglie in base alla struttura del codice e non alle prestazioni.
- **HTTP/1 o HTTP/2 verso i provider:** si preferisce HTTP/2, negoziato con ALPN (JEV lo negozia).
  - Il server vede la cancellazione in entrambi i casi.
  - In h1 però ogni cancellazione chiude la connessione, e la richiesta successiva paga un nuovo handshake TCP+TLS: con una deadline stretta, la richiesta dopo una cancellazione può consumare il budget nell'handshake.
  - In h2 la connessione resta aperta (`RST_STREAM`), e il server vede il drop entro 7,1 ms nel caso peggiore.
- **Budget di retry dentro la deadline globale.** Un retry è ammesso solo se valgono tutte le condizioni:
  - il tempo che resta alla deadline supera la latenza attesa della chiamata (per JEV circa 300 ms; il p95 va misurato su più campioni in S-04);
  - la risposta è un 429 o un 5xx (con `Retry-After` rispettato, se cade entro la deadline);
  - è il primo retry di quella chiamata.

  Una richiesta cancellata dalla deadline non si ritenta mai.
- **Alternative scartate:**
  - un timeout per ogni richiesta senza deadline globale: non limita la durata del fan-out;
  - `tokio::spawn` senza `JoinSet`: alla scadenza i task restano vivi, ed è proprio il caso che il test dei task (base ± 2) esclude.
- **Addebito JEV dopo il drop: non determinabile** (lettura dell'utente del 2026-10-21).
  - **Le finestre controllate nella console typesafe:**
    - le 20 richieste cancellate, dalle 13:32:17.806Z alle 13:33:00.800Z UTC;
    - le 5 di controllo, dalle 13:35:02.846Z alle 13:35:11.841Z UTC, con 332 + 21 token ciascuna.
  - **La risposta dell'utente, testuale:** «vuota». Nella console non compare nessuna richiesta nelle finestre indicate, nemmeno le 5 di controllo.
  - **Perché l'esito è «non determinabile»:** la console non mostra neppure le 5 richieste di controllo, che sono state completate con status 200. Quindi non distingue né conferma l'addebito. Le cause possibili sono due: un ritardo di aggregazione della console, oppure una console che riguarda un altro account o un'altra vista.
  - **Conseguenza per il budget di costo:** le richieste cancellate si contano come addebitate.
  - **Punto aperto:** la lettura si può ripetere più avanti, sulle stesse finestre orarie.
- **Impatto sulle porte (S-07):**
  - `DecisionEngine`, `AnswerGenerator`, `Embedder` e il trait stretto `Reranker::rank` (ADR-0012) ricevono una deadline esplicita (`tokio::time::Instant`) o un contesto di query che la contiene;
  - l'adapter non si costruisce una deadline sua più lunga, e la cancellazione è il drop del future;
  - l'adapter HTTP condivide un `reqwest::Client` con pool (connessioni h2 riusate) e non apre un client per ogni richiesta.
- **Impatto sul threat model:**
  - **T-24 (consumo illimitato):** la mitigazione è misurata in `results/deadline.csv`. Il chiamante esce entro 2,1 ms dalla deadline, la concorrenza è limitata a 8 e il lavoro pendente si cancella: i task tornano alla base e le richieste non avviate non vengono mai inviate.
  - **T-01-05-01 (fuga della chiave):** la chiave non compare nel repo (grep `-F` del valore su `spikes/s05-cancel` e `docs/adr`: nessuna occorrenza).
- **Destinazione:**
  - ADR-0010 (`docs/adr/0010-deadline-cancellazione-http.md`, Proposed);
  - la riga di `DEPENDENCIES.md` «HTTP client `reqwest` con Rustls» passa da Proposta a **Decisa**, perché le soglie sono rispettate. La riga la scrive il consolidamento (01-17).
