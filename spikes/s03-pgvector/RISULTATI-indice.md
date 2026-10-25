# S-03 / A-03 — Indice vettoriale, ricerca ibrida e leak test · RISULTATI

**Modifica:** Claude (gsd-executor 01-14) · 2026-10-25 · domanda, soglie e metodo scritti prima di ogni misura (Task 1 del piano 01-14)

## Domanda e decisione da chiudere

Spike S-03, parte indice (assunzione A-03; decisioni D-12 e D-15 di `01-CONTEXT.md`). Domande:

1. Fino a quanti chunk la ricerca esatta, con il predicato di autorizzazione nella stessa query SQL del ranking, resta nel budget di latenza? Oltre quella soglia l'indice HNSW con `hnsw.iterative_scan` recupera abbastanza dei risultati esatti a ogni selettività del filtro?
2. La ricerca ibrida `tsvector` + fusione RRF (k = 60), con il filtro ACL dentro entrambe le parti, migliora il recupero rispetto alla sola parte densa sulle query ratificate?
3. Nessun chunk negato entra nei top-k, nella parte semantica come in quella lessicale, a ogni scala e configurazione (leak test, hard gate)?
4. L'inserimento di nuovi chunk durante le query produce errori o degrada la latenza?
5. Quale metrica di distanza e quale tipo di vettore (`vector` o `halfvec`)?

Il modello di embedding è quello scelto dal piano 01-10 (`RISULTATI-embedding.md` §Esito: BAAI/bge-m3, 1024 dimensioni, servito da TEI CUDA). Indice e metrica si decidono in ADR-0006.

## Soglie (scritte PRIMA della misura)

Queste soglie sono fissate in questo commit, prima di qualunque file in `results/index/`, e non si ritoccano sui risultati: una correzione va in «Anomalie e deviazioni», con motivo. Fonti: RICERCA pgvector §5 «Metriche» (`01-RICERCA-DISCUSSIONE.md`) e Claude's Discretion di `01-CONTEXT.md`.

| Id | Metrica | Soglia | Fonte |
|----|---------|--------|-------|
| I-1 | Soglia esatto → HNSW | il più piccolo numero di chunk, fra 10k/50k/100k/300k, a cui la ricerca esatta autorizzata supera p95 50 ms di SQL con `max_parallel_workers_per_gather = 4` sul profilo meno selettivo (`sel100`); vale il peggiore dei p95 delle ripetizioni utili; se nessuna scala lo supera, la soglia è «oltre 300k» | D-12, RICERCA pgvector §5 |
| I-2 | Recall filtrata di HNSW rispetto all'esatto (k = 10) | ≥ 0,95 con almeno una combinazione di `ef_search` (40/100/200) e `iterative_scan` (`off`/`relaxed_order`/`strict_order`) a ogni selettività 100/25/5/1%, e percentuale di query che restituiscono k risultati = 100% nella stessa combinazione; si giudica per tipo (`vector`, `halfvec`) e per scala | D-12, RICERCA pgvector §5 |
| I-3 | Ricerca ibrida | adottata se recall@10 (riga `all`, query valutabili) migliora di almeno 0,03 rispetto alla sola parte densa | D-15 |
| I-4 | Leak | 0 chunk negati in top-k per ogni profilo, parte semantica e lessicale, ogni scala e configurazione (hard gate) | D-12, D-15, D-16, T-13 |
| I-5 | Inserimento concorrente | 0 errori e p95 delle query in carico < 2 × p95 a riposo | criterio 3 |
| I-6 | Latenza end-to-end (embedding della query + SQL) | p95 < 200 ms a 10k chunk | Claude's Discretion, quota del TTQ |
| I-7 | Metrica di distanza | coseno su vettori normalizzati, confermata se il prodotto interno non cambia il ranking (verifica sul set reale: top-10 identici per ogni query ratificata) | RICERCA pgvector §5 |
| I-8 | `doc_acl` letta dal DB coincide con la matrice attesa prima di ogni `leak` | nessuna differenza, altrimenti il comando si ferma | T-01-14-01 |
| I-9 | Selettività di `restricted-only` sul corpus reale (quota di chunk visibili) | ≤ 5% (`acl.json`, `max_chunk_selectivity`); se la supera, il profilo sintetico `sel5` diventa il riferimento dello stress | piano 01-03 |

## Metodo (scritto PRIMA della misura)

- **Query.** Le 74 query ratificate di `spikes/data/corpus/oracles/queries.json` (tre assi verdi, `todo_ref` vuoto: stessa regola del piano 01-10), con il vettore di bge-m3 calcolato da TEI a ogni comando. Per la parte lessicale la configurazione testuale è la lingua del testo della query: `it` e `cross-it-en` → `italian`, `en` e `cross-en-it` → `english`.
- **Corpus sintetico** (`index-bench synth --n <n>`). Scale 10k, 50k, 100k, 300k chunk, una tabella `chunk_synth_<n>` per scala. Il chunk `i` copia testo e lingua del chunk reale `i mod 3.950` di `chunk_bgem3` (ordine per `id`) e ne perturba il vettore con rumore gaussiano di norma attesa 0,5 (deviazione standard 0,5/√1024 per componente), seme fisso `0x0114`, poi rinormalizza a norma 1. Documenti sintetici da 50 chunk ciascuno (200, 1.000, 2.000, 6.000 documenti), con classificazioni nelle stesse proporzioni dei 19 documenti reali indicizzati (metodo dei resti più grandi, ordine mescolato dal seme). `doc_acl` riceve le righe dei ruoli e dei profili di `acl.json` secondo la classificazione e quelle dei ruoli sintetici: `sel100` vede tutti i documenti sintetici, `sel25`, `sel5` e `sel1` il primo 25%, 5% e 1% di una permutazione dei documenti fissata dal seme (insiemi annidati). La selettività è quindi esatta a livello di chunk. I ruoli sintetici non hanno righe sui documenti reali.
- **Matrice attesa.** Per i ruoli reali sul corpus reale `metrics::expected_visibility(acl.json, manifest)` del piano 01-10; per i documenti sintetici la regola di `synth` ricalcolata in memoria dal seme e dalle classificazioni reali. Il contatore dei negati legge solo questa matrice; `doc_acl` si confronta con la stessa matrice prima di misurare.
- **Ricerca esatta.** La query del piano 01-10 portata sulle tabelle del piano: `EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2))` nella stessa query del ranking, `ORDER BY` distanza coseno, `LIMIT` dopo il filtro. `threshold` misura solo su tabelle senza indice HNSW (il comando lo verifica); dove un indice HNSW esiste (`leak`, configurazione esatta) la query gira in una transazione con `enable_indexscan = off`, così nessun piano può usare l'indice approssimato.
- **Soglia (`threshold`).** Per ogni scala, selettività (100/25/5/1%) e `max_parallel_workers_per_gather` 0 e 4: 20 query di riscaldamento e 200 query misurate (le 74 query in ciclo), una connessione, in sequenza; 3 ripetizioni, la prima si scarta. p50/p95/p99 da istogramma. Piano `EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)` della prima query per ogni configurazione in `results/index/plans/`.
- **HNSW (`hnsw`).** Indici `CREATE INDEX CONCURRENTLY … USING hnsw` con `m = 16`, `ef_construction = 64` su `emb` (`vector_cosine_ops`) e su `emb_h` (`halfvec_cosine_ops`, colonna generata da `emb`), costruiti con `maintenance_work_mem = 768MB` (2GB nel primo tentativo, vedi Anomalie 1) e `max_parallel_maintenance_workers = 4`; tempo di costruzione e dimensione dell'indice. Recall filtrata = quota dei 10 id esatti presenti nei 10 risultati HNSW, media sulle 74 query, per ruolo sintetico e per ogni combinazione di `ef_search` × `iterative_scan`; `returns_k_rate` = quota di query con 10 risultati. Latenza HNSW nella stessa griglia di `threshold` con `ef_search = 100` e `iterative_scan = relaxed_order` (metodi `hnsw_vector` e `hnsw_halfvec`). Il comando verifica nel piano che l'indice HNSW sia usato.
- **Ibrida (`hybrid`).** Sul corpus reale (`chunk_bgem3`, 3.950 chunk): densa = ricerca esatta top-10; ibrida = query RRF di RESEARCH §5 con la CTE `allowed` dentro `sem` e `lex`, 50 candidati per parte, `ts_rank_cd` con `websearch_to_tsquery`, top-10 per punteggio RRF (a parità, `id` crescente). Recall@10 e nDCG@10 con la regola di rilevanza del piano 01-10 sulle query valutabili, per lingua (`it`, `en`, `cross`, `all`); p95 della SQL su 3 passate delle 74 query, la prima scartata. Nello stesso comando: verifica della metrica (I-7) e delle norme dei vettori.
- **Leak (`leak`).** Per ogni scala e configurazione (esatta e ognuna delle 18 combinazioni HNSW `vector`/`halfvec` × `ef_search` × `iterative_scan`), per ogni ruolo di `acl.json`, per `restricted-only` e per `sel100/sel25/sel5/sel1`, le 74 query con la query ibrida: chunk negati fra i 50 candidati di `sem`, fra i 50 di `lex` e fra i top-10 della fusione (`part` = `sem`, `lex`, `hybrid`). Per le configurazioni HNSW il comando verifica con le statistiche dell'indice che l'indice sia stato usato. In più la configurazione esatta sul corpus reale con i ruoli di `acl.json` (righe `chunks = 3950`).
- **Latenza end-to-end (`latency.csv`, nel comando `threshold`).** Per ogni scala e ripetizione 200 query di `sel100` con 4 worker: embedding della query (TEI), SQL, e la somma misurata dall'inizio dell'embedding alla fine della SQL. I-6 si giudica a 10k sul peggiore dei p95 delle ripetizioni utili.
- **Inserimento concorrente (`ingest-under-load`).** Scala 100k, nessun indice HNSW. Task A: query esatte di `sel100` con 4 worker in ciclo, una connessione, per 300 secondi. Task B: due documenti al secondo da 10 chunk (20 chunk/s), un commit per documento (documento, righe di `doc_acl`, chunk), su un'altra connessione. Una riga al secondo: p50 e p95 delle query concluse in quel secondo, righe inserite cumulative, errori (query e inserimenti falliti). p95 in carico = 95° percentile (nearest-rank) della colonna `query_p95`; p95 a riposo = stage `sql` di `latency.csv` a 100k, peggiore delle ripetizioni utili. A fine misura i documenti inseriti si cancellano e `doc_acl` si riconfronta con la matrice attesa.

**Modifica:** Claude (gsd-executor 01-14) · 2026-10-25 · setup, risultati, anomalie ed esito parziale dopo le misure (Soglie e Metodo sopra invariati; le differenze di metodo stanno in «Anomalie e deviazioni»)

## Setup

- Hardware: [`results/index/hw.json`](results/index/hw.json) (sha256 `b20185dcd46b4b3a3d65e1f92c7d6aa67672484665b11c2f01177f00c4a93028`, sessione `01-14-20261025t154840z-0866`): i7-13700K con 16 core e 24 thread, 32 GiB, RTX 3050 6 GiB, Docker Desktop 29.8.2 su WSL 2 (16 CPU e 19,5 GiB alla VM), piano energetico «Bilanciato». Manifest normalizzato allo schema di `common::manifest`; l'originale sta in `results/raw/index-hw-raw.json` (cartella locale, ignorata da git come negli altri spike).
- Immagini: `pgvector/pgvector:0.8.7-pg18-trixie` (PostgreSQL 18, estensione `vector` 0.8.7, `shared_buffers = 2GB`, `shm_size` 1 GB) e `ghcr.io/huggingface/text-embeddings-inference:86-1.9.4` (CUDA, float16) con `BAAI/bge-m3` alla revisione `5617a9f61b028005a4858fdac845db406aefb181` e `max_batch_tokens` 16.384, i due digest in `hw.json`. Runner `cka-spikes-runner`, rustc 1.95.0, build `--release --locked --offline`.
- Sessione aperta con `session.sh` e i soli servizi `postgres`, `tei-embed` e `runner` (`ps --assert-only` verde, nessun container di altri piani). Ogni sottocomando è girato nel runner con `session.sh oneshot` come `/target/release/index-bench <sottocomando>`.
- Corpus reale: 23 documenti in `doc`, 3.950 chunk in `chunk_bgem3` (piano 01-10). Corpus sintetico: `chunk_synth_10000`, `_50000`, `_100000`, `_300000`, generati in 2,8 / 16,3 / 37,1 / 114,1 s, con selettività dei ruoli sintetici esatte (1,0000 / 0,2500 / 0,0500 / 0,0100) e `doc_acl` uguale alla matrice attesa a ogni scala.
- Sequenza: per ogni scala `synth` → `threshold` (senza HNSW) → `hnsw` → `leak` → `drop-hnsw`; poi `leak --real`, `hybrid` e `ingest-under-load --n 100000` (scala 100k, senza HNSW).

## Risultati

Tutti i p95 sono il peggiore delle ripetizioni utili (run 2 e 3; la run 1 è nel CSV ma non entra nel giudizio). Piani `EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)`: 96 file in [`results/index/plans/`](results/index/plans/), uno per scala × metodo × worker × selettività, nome `<chunks>-<metodo>-w<worker>-<ruolo>.json`.

### I-1 — Soglia esatto → HNSW ([`threshold.csv`](results/index/threshold.csv))

p95 della SQL in ms, ricerca esatta autorizzata, per `max_parallel_workers_per_gather` 0 e 4:

| Chunk | w | sel100 | sel25 | sel5 | sel1 | Piano sel100 con 4 worker |
|---|---|---|---|---|---|---|
| 10.000 | 0 | 34,815 | 8,031 | 1,325 | 0,534 | |
| 10.000 | 4 | **23,711** | 7,951 | 1,347 | 0,548 | [`10000-exact-w4-sel100.json`](results/index/plans/10000-exact-w4-sel100.json) |
| 50.000 | 0 | 155,647 | 62,623 | 17,871 | 2,919 | |
| 50.000 | 4 | **100,927** | 52,415 | 27,759 | 1,933 | [`50000-exact-w4-sel100.json`](results/index/plans/50000-exact-w4-sel100.json) |
| 100.000 | 0 | 275,711 | 81,215 | 31,567 | 4,931 | |
| 100.000 | 4 | **111,167** | 42,783 | 32,575 | 4,383 | [`100000-exact-w4-sel100.json`](results/index/plans/100000-exact-w4-sel100.json) |
| 300.000 | 0 | 1.375,231 | 296,703 | 83,711 | 8,831 | |
| 300.000 | 4 | **522,239** | 92,607 | 39,135 | 9,247 | [`300000-exact-w4-sel100.json`](results/index/plans/300000-exact-w4-sel100.json) |

Esito: **soglia = 50.000 chunk**. A 10k il profilo meno selettivo sta a 23,7 ms; a 50k supera i 50 ms (100,9 ms). Il piano a `sel100` è un `Parallel Seq Scan` con hash join su `doc_acl`; con 4 worker richiesti, i piani salvati ne lanciano 0 a 10k (nessun Gather), 2 a 50k, 3 a 100k e 4 solo a 300k (verifica del bridge sui piani EXPLAIN); a `sel1` il planner passa a `doc_acl_role_doc_idx` + `chunk_synth_<n>_doc_idx`, e per questo i profili selettivi crescono molto meno. Il corpus reale di oggi (3.950 chunk) sta sotto la soglia di un fattore 12.

Per confronto, HNSW con `ef_search = 100` e `relaxed_order` (metodi `hnsw_vector` e `hnsw_halfvec`): p95 fra 1,0 e 2,1 ms a `sel100`/`sel25`, 2,7-8,0 ms a `sel5` e 8,3-15,7 ms a `sel1` a ogni scala; ogni piano HNSW salvato usa l'indice (`Index Scan using chunk_synth_<n>_emb_hnsw` o `_embh_hnsw`, con il predicato ACL come filtro della scansione).

### I-2 — Recall filtrata di HNSW ([`recall.csv`](results/index/recall.csv))

Per ogni scala e tipo, la combinazione migliore e la recall minima fra le quattro selettività nella stessa combinazione (k = 10, 74 query, `returns_k_rate` nella stessa combinazione):

| Chunk | Tipo | Build (s) | Indice (MiB) | Migliore combinazione | Recall minima sulle 4 selettività | `returns_k_rate` | Esito |
|---|---|---|---|---|---|---|---|
| 10.000 | vector | 1,4 | 78,1 | `ef_search` 200, `relaxed_order` | 0,9730 | 1,0000 | ok |
| 10.000 | halfvec | 1,4 | 26,1 | `ef_search` 200, `relaxed_order` | 0,9716 | 1,0000 | ok |
| 50.000 | vector | 8,6 | 390,6 | `ef_search` 200, `relaxed_order` | 0,9149 | 1,0000 | **fallita** |
| 50.000 | halfvec | 5,6 | 130,2 | `ef_search` 200, `relaxed_order` | 0,9108 | 1,0000 | **fallita** |
| 100.000 | vector | 11,4 | 781,3 | `ef_search` 200, `relaxed_order` | 0,8432 | 1,0000 | **fallita** |
| 100.000 | halfvec | 7,9 | 260,4 | `ef_search` 200, `relaxed_order` | 0,8365 | 1,0000 | **fallita** |
| 300.000 | vector | 97,2 | 2.343,8 | `ef_search` 200, `relaxed_order` | 0,7324 | 1,0000 | **fallita** |
| 300.000 | halfvec | 35,1 | 781,3 | `ef_search` 200, `relaxed_order` | 0,7041 | 1,0000 | **fallita** |

Anche guardando una selettività alla volta, oltre 10k nessuna combinazione arriva a 0,95 a `sel5` e `sel1` (50k: 0,9149 e 0,9203 al meglio; 300k: 0,7324 e 0,8459). A 100k e 300k neppure la ricerca senza filtro (`sel100`) ci arriva: 0,8500 e 0,7446 con `ef_search` 200, quindi il limite viene dal grafo (`m = 16`, `ef_construction = 64`) su questi dati prima che dal filtro (vedi «Anomalie», corpus sintetico).

Con `iterative_scan = off` la perdita silenziosa di D-12 è misurata: a `sel1` `returns_k_rate` = 0 a ogni scala e `ef_search` (meno di 10 risultati per tutte le query, recall 0,02-0,20); a `sel25` con `ef_search` 40 restituisce 10 risultati solo nel 58-59% delle query a 10k. Con `relaxed_order` o `strict_order` `returns_k_rate` = 1,0000 in tutte le righe; `relaxed_order` ha sempre una recall uguale o migliore di `strict_order`. `halfvec` costa un terzo dello spazio di `vector` e costruisce più in fretta, con recall entro 0,03 da `vector` nella combinazione migliore (`ef_search` 200; scarti fino a 0,06 in singole celle della griglia).

Esito: **I-2 raggiunta solo a 10k** (entrambi i tipi); **non raggiunta a 50k, 100k e 300k**. Alla soglia di I-1 (50k) HNSW con i parametri del piano non è un sostituto validato della ricerca esatta.

### I-3 — Ibrida contro densa ([`hybrid.csv`](results/index/hybrid.csv))

Corpus reale, 60 query valutabili su 74 (stessa regola del piano 01-10), ricerca esatta per la parte densa:

| Modo | recall@10 all | nDCG@10 all | nDCG@10 it / en / cross | p95 SQL (ms) |
|---|---|---|---|---|
| densa | 0,9000 | 0,7226 | 0,6884 / 0,7198 / 0,7968 | 10,999 |
| lessicale | 0,1500 | 0,1500 | 0,1667 / 0,2083 / 0,0000 | 0,410 |
| ibrida RRF | 0,9000 | 0,7371 | 0,6938 / 0,7506 / 0,7968 | 13,743 |

La densa riproduce i numeri del piano 01-10 (0,9000 e 0,7226). L'ibrida guadagna 0,0000 di recall@10 (soglia +0,03) e +0,0145 di nDCG@10, al costo di +2,7 ms di p95. Esito: **ibrida non adottata** (I-3 non raggiunta). La parte lessicale con `websearch_to_tsquery` sulla domanda in linguaggio naturale trova poco (AND di tutti i termini), e sulle query cross-lingua per costruzione nulla.

### I-4 — Leak ([`leak.csv`](results/index/leak.csv))

2.067 righe: 4 scale × 19 configurazioni (esatta e le 18 combinazioni HNSW) × 9 ruoli (`admin`, `manager`, `employee`, `guest`, `restricted-only`, `sel100`, `sel25`, `sel5`, `sel1`) × 3 parti (`sem`, `lex`, `hybrid`), più 15 righe sul corpus reale (5 ruoli × 3 parti, configurazione esatta), 74 query per riga. **`denied_in_topk` = 0 in tutte le righe** (`awk -F, 'NR>1 && $6!=0' leak.csv` non stampa nulla). In ogni configurazione HNSW il comando ha verificato con `pg_stat_user_indexes.idx_scan` che l'indice sia stato usato; la fusione SQL coincide con `rrf_fuse` in tutte le query (0 differenze). Esito: **hard gate superato**.

### I-5 — Inserimento concorrente ([`ingest_under_load.csv`](results/index/ingest_under_load.csv))

300 righe, una al secondo, scala 100k senza HNSW: 2.679 query `sel100` con 4 worker e 6.000 chunk inseriti (600 documenti, un commit per documento), **0 errori** fra query e inserimenti. p95 in carico (95° percentile dei p95 al secondo) = 142,454 ms; p95 a riposo (stage `sql` di `latency.csv` a 100k, peggiore delle run 2 e 3) = 110,015 ms; rapporto 1,29 contro la soglia di 2. Mediana dei p50 al secondo 109,4 ms. Un solo secondo ha un p95 di 936 ms (gli altri stanno sotto 170 ms). A fine misura i documenti inseriti sono stati cancellati e `doc_acl` è tornata uguale alla matrice attesa. Esito: **I-5 raggiunta**.

### I-6 — Latenza end-to-end ([`latency.csv`](results/index/latency.csv))

A 10k, `sel100` con 4 worker: embedding della query p95 22,975 ms, SQL p95 27,711 ms, end-to-end p95 **46,047 ms** (run 2; run 3 43,839 ms). Esito: **I-6 raggiunta** (soglia 200 ms). Per informazione, l'end-to-end p95 vale 126,4 ms a 50k, 150,9 ms a 100k e 617,0 ms a 300k.

### I-7 — Metrica

Sulle 74 query ratificate i top-10 per distanza coseno e per prodotto interno (`<#>`) coincidono in tutte le query (0 differenze); le norme dei vettori di documento e di query stanno tutte fra 1,000000 e 1,000000. Esito: **coseno confermato** su vettori normalizzati.

### I-8 e I-9

- I-8: `doc_acl` è stata confrontata con la matrice attesa prima di ogni `leak` (4 scale e corpus reale) e dopo `synth` e `ingest-under-load`: nessuna differenza. **Raggiunta.**
- I-9: selettività di `restricted-only` sul corpus reale 0,0048 (19 chunk su 3.950). **Raggiunta** (≤ 5%); lo stress resta sul profilo reale e sui ruoli sintetici.

## Anomalie e deviazioni

1. **`maintenance_work_mem` della build HNSW a 768 MB invece di 2 GB.** Con 2 GB la build parallela falliva subito (`could not resize shared memory segment … No space left on device`): il grafo della build parallela sta in memoria condivisa e `shm_size` del container postgres è 1 GB (`compose.yaml`, file di un altro piano). La correzione è nel commit `4042abe`, ma le misure HNSW sono state eseguite con la modifica ancora non committata: `hw.json` porta `git_rev 22d6d5f` e `4042abe` è stato committato due minuti prima dei CSV (verifica del bridge, 2026-10-25). Il codice misurato è quello di `4042abe`, ma la tracciabilità non è quella prescritta. A 300k l'indice `vector` (2.343,8 MiB) non sta in 768 MB e pgvector completa la build fuori memoria: il tempo di 97,2 s non rappresenta una build tutta in memoria. Non è verificato se questo cambi la qualità del grafo.
2. **Forma della query ANN.** Con la forma della ricerca esatta (semi-join `EXISTS` appiattito in hash join) il planner non sceglie mai l'indice HNSW, e a 10k preferisce il seq scan anche senza filtro. Per misurare HNSW la parte densa usa `EXISTS (… OFFSET 0)` (il predicato resta un subplan valutato come filtro della scansione dell'indice) e la sessione HNSW imposta `enable_seqscan = off`. Il predicato è comunque nella stessa query del ranking, nessun post-filtro applicativo; i piani salvati e le statistiche dell'indice lo confermano. Conseguenza per l'adozione futura di HNSW: la forma della query va fissata, non lasciata al planner.
3. **Nessun indice HNSW su `chunk_bgem3`.** HNSW sta solo sulle tabelle sintetiche, così la ricerca esatta del piano 01-10 e l'export dei candidati del piano 01-15 restano deterministici sul corpus reale; sul corpus reale si misurano `leak` (configurazione esatta) e `hybrid`. `sql/index.sql` aggiunge però a `chunk_bgem3` la colonna generata `emb_h halfvec(1024)` (modifica dei dati nel DB, nessun file del piano 01-10 toccato).
4. **Corpus sintetico difficile per l'ANN.** Ogni chunk reale compare in 2-76 copie con rumore gaussiano isotropo di norma 0,5 in 1.024 dimensioni: le copie sono quasi equidistanti dalla query, e il top-10 esatto è fatto di copie di pochi chunk. È il caso peggiore per un grafo HNSW (la recall senza filtro cala a 0,85 a 100k e a 0,74 a 300k), quindi le recall di I-2 oltre 10k sono probabilmente pessimistiche per un corpus reale della stessa dimensione. Il giudizio di I-2 resta quello misurato: non si corregge la soglia, si dichiara il limite. Una rivalutazione di HNSW va fatta su dati reali e con `m`/`ef_construction` più alti.
5. **`hw.json` normalizzato.** L'output di `tools/hw-manifest.sh` (oggetti per `cpu`, `docker_desktop_resources` e `images`) non è letto da `common::csv::Writer`, che vuole lo schema di `common::manifest`. Stessa normalizzazione dei piani 01-04, 01-07, 01-08, 01-10 e 01-11, con un driver fuori dal repo; l'originale è in `results/raw/index-hw-raw.json`, non versionato.
6. **File di ambiente della sessione.** `session.sh` toglie dall'ambiente le variabili dell'host con nomi da segreto e legge solo il file di `CKA_SPIKES_ENV`: per passare `EMBED_MODEL`, `EMBED_REVISION` e `TEI_MAX_BATCH_TOKENS` la sessione ha usato una copia temporanea del file dei segreti con queste tre righe in più, cancellata a fine piano (stessa deviazione del piano 01-10).
7. **Invocazione dei comandi.** Il pacchetto ha ora due bin (`s03-pgvector` del piano 01-10 e `index-bench`) e `Cargo.toml` non è nei file del piano: `cargo run -p s03-pgvector --release -- <sottocomando>` del piano non è univoco. Il binario è stato costruito con `cargo build --release --locked --offline -p s03-pgvector --bin index-bench` e lanciato come `/target/release/index-bench <sottocomando>`.
8. **Latenza dell'embedding che cresce con la scala.** Lo stage `embed_query` (TEI su GPU, indipendente dalla tabella) passa da p95 22 ms a 10k a 105 ms a 300k, nella stessa sessione. Embedding e SQL girano in sequenza, non in parallelo; la causa non è stata indagata (contesa di CPU o memoria della VM WSL con il seq scan parallelo sono le ipotesi). I-6 si giudica a 10k come da Metodo.
9. **Picco isolato nell'ingestion.** Un secondo su 300 ha p95 936 ms. È la riga t=1, cioè la prima query a freddo, perché il task delle query non ha riscaldamento (verifica del bridge); l'ipotesi iniziale di autovacuum o checkpoint non è confermata. Il p95 dei p95 al secondo lo assorbe per costruzione.
10. **`restricted-only` sulle scale sintetiche.** Sulle tabelle sintetiche `restricted-only` vede il 10,5% dei chunk, perché i documenti sintetici copiano la distribuzione delle classificazioni per documento e non per chunk. I-9 si giudica sul corpus reale come da Soglie.
11. **Formato.** In `hybrid.csv` la riga `lexical,cross` riporta `-0.0000` come nDCG: è uno zero (nessuna query cross con risultati lessicali rilevanti), segno del formato in virgola mobile.

## Esito parziale

- **Ricerca esatta con il predicato ACL nella stessa query per l'MVP**: confermata. Sul corpus attuale (3.950 chunk) e fino a 10k chunk la SQL sta a p95 ≤ 24 ms con 4 worker e l'end-to-end a 46 ms.
- **Soglia misurata: 50.000 chunk** (I-1). Oltre la soglia HNSW con `iterative_scan = relaxed_order` è veloce (p95 ≤ 16 ms a ogni scala e selettività) ma con i parametri del piano non raggiunge la recall filtrata 0,95 (I-2 fallita a 50k, 100k e 300k, su un corpus sintetico pessimistico). Il passaggio a HNSW resta un'idea differita che richiede una nuova misura su dati reali e con parametri più alti; `iterative_scan = off` con filtro è da escludere (`returns_k_rate` 0 a `sel1`).
- **Metrica coseno** su vettori normalizzati (I-7). **`halfvec`** è il tipo indicato se si attiva HNSW (un terzo dello spazio, recall equivalente); con la ricerca esatta resta `vector`.
- **Ibrida non adottata** (I-3: +0,0000 di recall@10, +0,0145 di nDCG@10).
- **Leak 0** in ogni scala, configurazione, ruolo e parte (I-4); `doc_acl` coerente con la matrice attesa (I-8); inserimento concorrente senza errori e con p95 1,29 volte quello a riposo (I-5).
**Modifica:** Claude (Anthropic, orchestratore) · 2026-10-25 · correzioni dal bridge di 01-14: maintenance_work_mem nel Metodo, tracciabilità di 4042abe, worker lanciati per scala, picco a freddo dell'ingestion, recall con iterative_scan off
