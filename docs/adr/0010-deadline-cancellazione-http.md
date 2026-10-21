# ADR-0010 — Deadline globale e cancellazione delle richieste HTTP verso i provider

- Stato: Proposed
- Data: 2026-10-21
- Decisori: l'utente (ratifica nel consolidamento, piano 01-17); la proposta è di Claude (gsd-executor 01-05), dallo spike S-05
- Sostituisce: nessuno

**Modifica:** Claude (gsd-executor 01-05) · 2026-10-21 · ADR proposto con le misure dello spike S-05; esito sull'addebito JEV in lettura dall'utente
**Modifica:** Claude (gsd-executor 01-05) · 2026-10-21 · esito sull'addebito JEV dopo la lettura dell'utente: non determinabile (console vuota)

## Contesto

Il RAG & Answer Service fa fan-out verso provider HTTP esterni: JEV (`POST /v1/systemone`) per rerank e validazioni, l'LLM per la generazione e il servizio di embedding. `docs/architecture/interfaces-and-concurrency.md` (§«Modello di concorrenza») chiede quattro cose per ogni fan-out: una deadline globale, un limite, la cancellazione dei task residui dopo il timeout e un budget di retry. Il baseline proposto è di massimo 8 decisioni JEV contemporanee, con una deadline complessiva configurabile.

La decisione D-22 di 01-CONTEXT chiede di verificare quattro punti:

- che la cancellazione verso un provider HTTP non lasci task, file descriptor o connessioni appesi;
- che il server veda davvero il drop, con HTTP/1 e con HTTP/2;
- che la deadline globale restituisca il controllo in tempo, con due varianti: `timeout_at` + `buffer_unordered` e `JoinSet` + `Semaphore`;
- se JEV addebita il lavoro di una richiesta cancellata.

È coinvolta anche la riga di `DEPENDENCIES.md` «HTTP client `reqwest` con Rustls», oggi Proposta. La minaccia coinvolta è T-24 (consumo illimitato di risorse e di costo).

## Opzioni considerate

Tutti i numeri vengono dallo spike S-05, sullo stesso hardware (i7-13700K, Docker Desktop su WSL 2, runner Linux) e con soglie scritte prima della misura. Le evidenze sono in `spikes/s05-cancel/RISULTATI.md`.

1. **Variante A: `tokio::time::timeout_at(deadline, stream::iter(..).map(call).buffer_unordered(8).collect())`.**
   - Vantaggi:
     - il chiamante esce dopo la deadline con p50 1,0 ms e p99 2,1 ms (18 run, h2);
     - la cancellazione è strutturale: alla scadenza lo stream si distrugge, i future in volo vengono droppati e quelli non avviati non partono mai;
     - non servono task `'static`, `Arc` né `spawn`.
   - Svantaggi: tutto il fan-out avanza su un solo task, quindi non c'è parallelismo CPU fra le chiamate (irrilevante per l'I/O).
2. **Variante B: `JoinSet` + `Arc<Semaphore>(8)`, con il drop del `JoinSet` alla scadenza.**
   - Vantaggi:
     - uscita con p50 1,2 ms e p99 1,8 ms;
     - il drop del `JoinSet` fa `abort` di ogni task, e i task vivi tornano alla base dopo 1 s in tutte le run;
     - i task sono indipendenti e girano su più worker.
   - Svantaggi:
     - servono task `'static` (clone del client e degli input);
     - la correttezza dipende dal non staccare mai un task dal `JoinSet`, perché un `tokio::spawn` libero sopravvive alla deadline.
3. **HTTP/1.1 verso il provider.**
   - Il server vede il drop in 10.000 cicli su 10.000, con p99 ≤ 3,9 ms.
   - Ogni cancellazione però chiude la connessione: durante la misura si arriva a 7 connessioni contemporanee, e ogni richiesta successiva paga un nuovo handshake.
4. **HTTP/2 verso il provider** (prior knowledge sul mock, ALPN verso JEV).
   - Il server vede il drop (`RST_STREAM`) in 10.000 cicli su 10.000 per ogni tempo di cancellazione, con il massimo a 7,1 ms (soglia: ≥ 95 % entro 100 ms).
   - Resta una sola connessione, con 3 task stabili.
   - JEV negozia HTTP/2.

In tutti e nove gli scenari di cancellazione (mock h1, mock h2, wiremock × 50, 200 e 500 ms; 10.000 cicli ciascuno) task, file descriptor e connessioni tornano alla base.

## Decisione

1. **Deadline globale per ogni query**, sotto forma di `tokio::time::Instant` (o di un contesto di query che la contiene), passata esplicitamente a ogni porta che chiama un provider: `DecisionEngine`, `AnswerGenerator`, `Embedder` e `Reranker::rank` (ADR-0012). Un adapter non si costruisce una deadline sua più lunga, e la cancellazione è il drop del future.
2. **Variante A come forma di default del fan-out**, con un limite di concorrenza esplicito (baseline 8). La variante B è ammessa solo quando servono task indipendenti, e sempre dentro un `JoinSet` che viene droppato alla deadline; mai `tokio::spawn` libero.
3. **HTTP/2 preferito verso i provider**, con un `reqwest::Client` condiviso e il pool (connessioni riusate). Il client non si crea per ogni richiesta.
4. **`reqwest` 0.13 con Rustls** come client HTTP dei provider. La verifica dei certificati resta quella di default, senza `danger_*`, e si usa `https_only` verso gli host esterni.
5. **Budget di retry dentro la deadline.** Il retry è ammesso al massimo una volta per chiamata, solo per 429 o 5xx, e solo se il tempo che resta alla deadline supera la latenza attesa (con `Retry-After` rispettato se cade entro la deadline). Una richiesta cancellata dalla deadline non si ritenta mai.

## Conseguenze

- Positive:
  - **T-24:** la durata di una query verso i provider è limitata dalla deadline più circa 2 ms, il lavoro in volo si cancella e le richieste non ancora avviate non partono (`spikes/s05-cancel/results/deadline.csv`);
  - **nessun leak** di task, file descriptor o connessioni dopo 10.000 cancellazioni per scenario (`spikes/s05-cancel/results/cancel.csv`);
  - la cancellazione in HTTP/2 libera subito il lavoro del server (drop dell'handler entro pochi ms), senza costi di riconnessione.
- Negative:
  - ogni porta verso un provider porta una deadline in firma: è un vincolo per S-07 e per tutti gli adapter, fake compresi;
  - con HTTP/1 (provider senza h2) ogni cancellazione costa una riconnessione: con deadline strette va messo in conto l'handshake;
  - le misure della deadline sono 18 run per variante: il p99 coincide con il massimo, con ampio margine sulla soglia ma senza una stima della coda estrema.
- **Addebito JEV dopo il drop: non determinabile** (lettura dell'utente del 2026-10-21).
  - **Le finestre controllate:** le 20 richieste cancellate a 50 ms (2026-10-21, dalle 13:32:17.806Z alle 13:33:00.800Z UTC) e le 5 di controllo (dalle 13:35:02.846Z alle 13:35:11.841Z UTC, 332 + 21 token ciascuna).
  - **La risposta dell'utente, testuale:** «vuota». Nella console di typesafe.ai non compare nessuna richiesta nelle finestre indicate, nemmeno le 5 di controllo.
  - **Perché non è determinabile:** la console non mostra neppure le richieste di controllo, completate con 200. Non distingue quindi né conferma l'addebito: può esserci un ritardo di aggregazione, oppure la console riguarda un altro account o un'altra vista.
  - **Conseguenza:** nel budget di costo le richieste cancellate si contano come addebitate.
  - **Punto aperto:** la lettura si può ripetere più avanti.

## Evidenze

- `spikes/s05-cancel/RISULTATI.md`: soglie, setup, risultati, anomalie, esito.
- `spikes/s05-cancel/results/hw.json`: manifest hardware.
- `spikes/s05-cancel/results/cancel.csv`, `spikes/s05-cancel/results/deadline.csv`, `spikes/s05-cancel/results/jev_cancel.csv`.

| Misura | Valore | Soglia |
|---|---|---|
| Uscita dopo la deadline, variante A (p50 / p99) | 1,031 / 2,091 ms | p99 < 10 ms |
| Uscita dopo la deadline, variante B (p50 / p99) | 1,154 / 1,793 ms | p99 < 10 ms |
| Drop visto dal server, h2 (3 × 10.000 cicli) | 100 %, max 7,061 ms | ≥ 95 % entro 100 ms |
| Drop visto dal server, h1 (3 × 10.000 cicli) | 100 %, max 23,517 ms | senza soglia |
| Task, fd e connessioni a fine scenario (9 scenari) | uguali alla base | task ± 2, fd + 2, conn. ≤ 8 |
| JEV: richieste cancellate / di controllo | 20 droppate a 50-52 ms / 5 × 200, `jev-1.13.0` | nessuna soglia; addebito non determinabile (console vuota, 2026-10-21) |
