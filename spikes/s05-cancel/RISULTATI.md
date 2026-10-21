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
