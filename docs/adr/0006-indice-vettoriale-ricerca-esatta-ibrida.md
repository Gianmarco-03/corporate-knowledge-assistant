# ADR-0006 — Ricerca vettoriale esatta con il predicato ACL nella stessa query, metrica coseno, ibrida non adottata, soglia HNSW a 50.000 chunk

- Stato: Proposed
- Data: 2026-10-25
- Decisori: utente (D-12, D-15 di `01-CONTEXT.md`; ratifica nel consolidamento, piano 01-17); proposta di Claude (gsd-executor 01-14) dallo spike S-03/A-03, parte indice
- Sostituisce: nessuno

**Modifica:** Claude (gsd-executor 01-14) · 2026-10-25 · ADR proposto con le misure dello spike S-03/A-03, parte indice (piano 01-14)

## Contesto

Il Document & Indexing service deve restituire i chunk più vicini alla query **solo fra quelli che il soggetto può vedere** (deny by default): nessun chunk negato deve arrivare a reranker, JEV, LLM, cache o log (T-13). Il modello di embedding è `BAAI/bge-m3` a 1.024 dimensioni (ADR-0007); i chunk stanno in PostgreSQL con `pgvector`.

Decisioni dell'utente che vincolano la scelta:

- **D-12:** ricerca esatta nell'MVP, con il predicato di autorizzazione nella stessa query SQL del ranking; lo spike misura la soglia oltre cui passare a HNSW con `hnsw.iterative_scan`. Motivo: un indice ANN con una clausola `WHERE` si comporta da post-filtro, perché l'indice restituisce `ef_search` candidati e il filtro li sfoltisce dopo, con una perdita di recall silenziosa quando il filtro è selettivo. Pin `pgvector` ≥ 0.8.4 (correzione della corruzione HNSW durante il vacuum); l'immagine usata è `pgvector/pgvector:0.8.7-pg18-trixie`, estensione 0.8.7.
- **D-15:** ricerca ibrida `tsvector` + fusione RRF (k = 60) come baseline da misurare, con il filtro ACL in **entrambe** le parti; `pg_textsearch` (BM25) solo con un guadagno misurato.

## Opzioni considerate

1. **Ricerca esatta autorizzata** (seq scan o indice sul documento, `ORDER BY` distanza, `LIMIT` dopo il filtro, predicato `EXISTS` su `doc_acl` nella stessa query). Recall esatta per definizione e nessuna perdita silenziosa; la latenza cresce con il numero di chunk visibili: p95 SQL sul profilo meno selettivo con 4 worker 23,7 ms a 10k, 100,9 ms a 50k, 111,2 ms a 100k, 522,2 ms a 300k.
2. **HNSW con `hnsw.iterative_scan`** (`m = 16`, `ef_construction = 64`, `ef_search` 40/100/200, `off`/`relaxed_order`/`strict_order`, su `vector` e `halfvec`). p95 ≤ 16 ms a ogni scala e selettività con `relaxed_order`; recall filtrata contro l'esatto ≥ 0,95 a ogni selettività solo a 10k (0,973), e 0,91 / 0,84 / 0,73 al meglio a 50k / 100k / 300k su un corpus sintetico pessimistico per l'ANN. Con `iterative_scan = off` la perdita di D-12 è misurata: a selettività 1% meno di k risultati in tutte le query. Il planner usa l'indice solo con il predicato tenuto come filtro della scansione (`EXISTS (… OFFSET 0)`) e `enable_seqscan = off` a piccola scala. `halfvec`: un terzo dello spazio (781 MiB contro 2.344 MiB a 300k), build più rapida, recall entro 0,03 nella combinazione migliore.
3. **IVFFlat**: escluso senza misura (RICERCA F3 di `01-RICERCA-DISCUSSIONE.md`): va costruito dopo i dati, i centroidi invecchiano con la crescita del corpus e la qualità degrada senza `REINDEX`; con 10-30 documenti i centroidi sono inutili, e l'iterative scan esiste solo in `relaxed_order`.
4. **Densa contro ibrida.** Sul corpus reale, 60 query valutabili: densa recall@10 0,9000 e nDCG@10 0,7226; ibrida RRF con la CTE `allowed` in `sem` e `lex` recall@10 0,9000 e nDCG@10 0,7371 (+0,0145), p95 +2,7 ms; la parte lessicale da sola 0,1500.
5. **Metrica**: coseno contro prodotto interno. Con vettori di norma 1 i top-10 coincidono in 74 query su 74.

## Decisione

- **Ricerca esatta con il predicato ACL nella stessa query SQL** per l'MVP, su `vector(1024)` con distanza coseno. Nessun indice ANN, nessun post-filtro applicativo. Il predicato lo costruisce il chiamante (`AccessPolicy`) e l'adapter di `VectorIndex` lo applica nella query del ranking, con i ruoli in bind.
- **Soglia di rivalutazione: 50.000 chunk**, il primo punto misurato in cui la ricerca esatta supera p95 50 ms di SQL sul profilo meno selettivo con `max_parallel_workers_per_gather = 4`. Superata la soglia, il passaggio a HNSW richiede una nuova misura della recall filtrata su dati reali e con parametri più alti di `m = 16` / `ef_construction = 64`: con i parametri dello spike HNSW non è un sostituto validato. Se si adotta: `halfvec`, `hnsw.iterative_scan = relaxed_order` (mai `off` con un filtro), forma della query fissata dall'adapter.
- **Ricerca ibrida non adottata**: il guadagno di recall@10 misurato (0,0000) sta sotto la soglia di +0,03 di D-15. La firma della porta conserva `query_text`, così una rivalutazione della parte lessicale non cambia l'interfaccia.

## Conseguenze

- Positive:
  - nessun chunk negato in top-k su 2.067 righe di leak test (4 scale, esatta e 18 combinazioni HNSW, 9 ruoli, parti semantica, lessicale e fusa, più il corpus reale), contati contro una matrice attesa indipendente da `doc_acl`;
  - recall esatta, senza la perdita silenziosa dei filtri ANN;
  - latenza end-to-end (embedding della query + SQL) p95 46,0 ms a 10k chunk, contro 200 ms;
  - inserimento concorrente senza errori: 20 chunk/s per 5 minuti su 100k chunk, p95 delle query in carico 1,29 volte quello a riposo;
  - nessun indice da costruire o mantenere nell'MVP (a 300k un indice HNSW `vector` costa 2,3 GiB e 97 s di build).
- Negative:
  - la latenza cresce in modo lineare con i chunk visibili: oltre 50.000 chunk il profilo meno selettivo supera 50 ms, a 300k sta a 522 ms;
  - la soglia vale per questo hardware (`hw.json` dello spike) e per `shared_buffers = 2GB`;
  - la recall filtrata di HNSW oltre 10k è misurata su un corpus sintetico (copie perturbate dei vettori reali), probabilmente pessimistico: il dato reale manca.
- Follow-up:
  - una metrica di produzione sul numero di chunk visibili e sul p95 della ricerca fa scattare la rivalutazione alla soglia (T-24);
  - rivalutazione di HNSW alla soglia con dati reali, `m`/`ef_construction` più alti e la stessa griglia di leak test;
  - righe `DEPENDENCIES.md` «Vector search» ed «Embedding» da Proposta a Decisa nel consolidamento (piano 01-17).

## Evidenze

- Spike: [`spikes/s03-pgvector/RISULTATI.md`](../../spikes/s03-pgvector/RISULTATI.md) (indice di S-03) e [`spikes/s03-pgvector/RISULTATI-indice.md`](../../spikes/s03-pgvector/RISULTATI-indice.md) (soglie scritte prima della misura, metodo, setup, risultati, anomalie, esito).
- Hardware: [`spikes/s03-pgvector/results/index/hw.json`](../../spikes/s03-pgvector/results/index/hw.json) (sessione `01-14-20261025t154840z-0866`).
- CSV e piani: [`spikes/s03-pgvector/results/index/threshold.csv`](../../spikes/s03-pgvector/results/index/threshold.csv), [`recall.csv`](../../spikes/s03-pgvector/results/index/recall.csv), [`hybrid.csv`](../../spikes/s03-pgvector/results/index/hybrid.csv), [`leak.csv`](../../spikes/s03-pgvector/results/index/leak.csv), [`ingest_under_load.csv`](../../spikes/s03-pgvector/results/index/ingest_under_load.csv), [`latency.csv`](../../spikes/s03-pgvector/results/index/latency.csv); 96 piani `EXPLAIN` in [`spikes/s03-pgvector/results/index/plans/`](../../spikes/s03-pgvector/results/index/plans/).

| Misura | Esito | Soglia |
|---|---|---|
| Soglia esatto → HNSW | 50.000 chunk (23,7 ms a 10k, 100,9 ms a 50k) | p95 SQL > 50 ms, `sel100`, 4 worker |
| Recall filtrata HNSW | 0,973 a 10k; 0,91 / 0,84 / 0,73 a 50k / 100k / 300k | ≥ 0,95 a ogni selettività, k risultati = 100% |
| Ibrida contro densa | +0,0000 recall@10 (+0,0145 nDCG@10) | ≥ +0,03 recall@10 |
| Leak | 0 su 2.067 righe | = 0 |
| Inserimento concorrente | 0 errori, 142,5 / 110,0 ms = 1,29 × | 0 errori, < 2 × |
| End-to-end a 10k | 46,0 ms | < 200 ms |
| Metrica | top-10 coseno = prodotto interno in 74/74 query | ranking invariato |
| `restricted-only` sul corpus reale | 0,48% dei chunk | ≤ 5% |
