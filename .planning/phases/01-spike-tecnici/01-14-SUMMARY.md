---
phase: 01-spike-tecnici
plan: 14
subsystem: spike-indice-vettoriale-ricerca-esatta-autorizzata-ibrida-leak-adr-0006
tags: [s03-pgvector, pgvector, hnsw, iterative-scan, relaxed-order, strict-order, ef-search, halfvec, ricerca-esatta, acl, doc-acl, leak, rrf, tsvector, websearch-to-tsquery, ibrida, explain, soglia, recall-filtrata, returns-k, ingestion-concorrente, corpus-sintetico, splitmix64, coseno, prodotto-interno, adr-0006, sqlx, bind]

# Dependency graph
requires:
  - phase: 01-10
    provides: "libreria s03_pgvector (db::connect, check_doc_acl, fetch_chunks, apply_schema; metrics::expected_visibility, leaked_chunks, relevant_chunks, recall_at_k, ndcg_at_k, load_ratified_queries; tei::Client::embed_query), schema doc/doc_acl/chunk_bgem3, 3.950 chunk bge-m3 nel volume pgdata, esito bge-m3 in RISULTATI-embedding.md"
  - phase: 01-13
    provides: "GPU libera da Docling e OCR: la latenza di TEI e l'ingestion concorrente misurate su RTX 3050 non condivisa (ordine di 01-ONDE.md, onda 8)"
  - phase: 01-02
    provides: "harness Compose (postgres pgvector 0.8.7-pg18-trixie con shm_size 1 GB, tei-embed 86-1.9.4), session.sh (with, up, oneshot, ps --assert-only), hw-manifest.sh"
  - phase: 01-03
    provides: "corpus comune con manifest.json, acl.json (4 ruoli e profilo restricted-only), oracoli queries.json ratificati da 01-06"
provides:
  - "Bin index-bench (prepare, synth, threshold, hnsw, drop-hnsw, hybrid, leak, ingest-under-load) in spikes/s03-pgvector/src/bin/index-bench/, con 21 test (8 unitari, 13 ignorati: 7 db e 6 di evidenza o git)"
  - "sql/index.sql: colonna generata emb_h halfvec(1024), tabelle chunk_synth_<n> per quattro scale con GIN su tsv e indice su doc_id, indici HNSW m = 16 / ef_construction = 64 su vector e halfvec"
  - "Soglie I-1..I-9 e Metodo scritti e committati prima di ogni misura (328ef35), invariati fino a HEAD"
  - "Misure: threshold.csv (288 righe), latency.csv (36), recall.csv (288), hybrid.csv (12), leak.csv (2.067), ingest_under_load.csv (300), hw.json, 96 piani EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)"
  - "RISULTATI-indice.md, indice ed Esito complessivo di S-03 (RISULTATI.md), ADR-0006 Proposed"
  - "Forma della porta VectorIndex::search con predicato costruito dal chiamante e applicato nella query (RISULTATI.md «Impatto su porte»)"
affects: [01-15, 01-16, 01-17, ADR-0006, DEPENDENCIES.md righe «Vector search» ed «Embedding», porta VectorIndex (S-07), retrieval autorizzato del Document Service, threat model T-13, T-20, T-24]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Predicato ACL dentro ogni sottoquery: CTE allowed (role = ANY($n) in bind) usata sia in sem sia in lex, fusione RRF dopo, nessun post-filtro"
    - "Forma ANN del predicato: EXISTS (… OFFSET 0) come subplan filtro della scansione HNSW, con enable_seqscan = off nella sessione HNSW; la forma esatta resta il semi-join EXISTS di 01-10"
    - "Impostazioni di sessione (enable_indexscan, hnsw.ef_search, hnsw.iterative_scan, enable_seqscan) via set_config con parametri legati, mai SET composto"
    - "Un letterale SQL completo per (tabella, strategia) scelto con match su enum chiusi (Table, Scale, Dense, Strategy, Workers): nessun nome o valore nel testo SQL a runtime"
    - "Corpus sintetico riproducibile con SplitMix64 derivato da seme, etichetta del flusso e indice dell'elemento (synth.rs:107-156): un elemento si ricalcola da solo, la matrice attesa si rifà in memoria"
    - "Verifica d'uso dell'indice per configurazione: delta di pg_stat_user_indexes.idx_scan ≥ query per l'indice della configurazione e = 0 per gli altri, altrimenti il comando si ferma"

key-files:
  created:
    - spikes/s03-pgvector/src/bin/index-bench/ingest.rs
    - spikes/s03-pgvector/sql/index.sql
    - spikes/s03-pgvector/RISULTATI-indice.md
    - spikes/s03-pgvector/RISULTATI.md
    - docs/adr/0006-indice-vettoriale-ricerca-esatta-ibrida.md
    - spikes/s03-pgvector/results/index/hw.json
    - spikes/s03-pgvector/results/index/threshold.csv
    - spikes/s03-pgvector/results/index/latency.csv
    - spikes/s03-pgvector/results/index/recall.csv
    - spikes/s03-pgvector/results/index/hybrid.csv
    - spikes/s03-pgvector/results/index/leak.csv
    - spikes/s03-pgvector/results/index/ingest_under_load.csv
    - spikes/s03-pgvector/results/index/plans/*.json (96)
  modified:
    - spikes/s03-pgvector/src/bin/index-bench/main.rs
    - spikes/s03-pgvector/src/bin/index-bench/synth.rs
    - spikes/s03-pgvector/src/bin/index-bench/hybrid.rs
    - spikes/s03-pgvector/src/bin/index-bench/leak.rs

key-decisions:
  - "Proposta in ADR-0006 (Proposed, ratifica nel consolidamento 01-17, Task 2): ricerca esatta con il predicato ACL nella stessa query per l'MVP, vector(1024), coseno, nessun indice ANN"
  - "Soglia di rivalutazione 50.000 chunk (I-1): p95 SQL sel100 con 4 worker 23,7 ms a 10k, 100,9 ms a 50k"
  - "HNSW non validato oltre 10k: recall filtrata minima 0,9149 / 0,8432 / 0,7324 a 50k / 100k / 300k (vector, ef_search 200, relaxed_order) contro 0,95; soglie invariate, caveat del corpus sintetico dichiarato"
  - "Ibrida tsvector + RRF non adottata (I-3): recall@10 0,9000 = densa, nDCG@10 +0,0145, p95 +2,7 ms; query_text resta nella firma della porta"
  - "Esecutore: nessun HNSW su chunk_bgem3, così l'esatta di 01-10 e l'export dei candidati di 01-15 restano deterministici"
  - "Esecutore: maintenance_work_mem 768MB per lo shm_size di 1 GB del container postgres (compose.yaml di 01-02, non toccato)"

patterns-established:
  - "Un indice approssimato si misura solo dopo aver verificato nel piano EXPLAIN e nelle statistiche dell'indice che sia davvero usato; altrimenti la misura è di un'altra configurazione"
  - "La ricerca esatta di riferimento gira con enable_indexscan = off dove esiste un indice HNSW, così nessun piano dell'esatta può usare l'indice approssimato"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "La ricerca esatta con il predicato di autorizzazione nella stessa query SQL è misurata a 10k/50k/100k/300k chunk, e la soglia oltre cui passare a HNSW con iterative_scan è un numero misurato (D-12; criterio 3)"
    verification:
      - {test: TEST-11, risks: [RISK-6], ref: "spikes/s03-pgvector/src/bin/index-bench/synth.rs:686#test11_synth_role_selectivity_matches_target", status: human_judgment, human_judgment: true, rationale: "test db ignorato: verde solo nel rapporto dell'esecutore (runner, --include-ignored, 40 verdi); né l'orchestratore sul main fuso né il bridge l'hanno rieseguito. A runtime synth_cmd si ferma se una selettività esce da ±0,005 (main.rs:517-523), ma l'output con le quote misurate sta solo nei log di sessione, non versionati"}
      - {test: TEST-16, risks: [RISK-10], ref: "spikes/s03-pgvector/src/bin/index-bench/synth.rs:609#test16_synth_is_deterministic_and_normalized", status: pass, evidence: "rieseguito dal bridge sull'host sul main fuso: cargo test -p s03-pgvector --locked --offline --bin index-bench, 8 passati, 13 ignorati"}
      - {test: TEST-13, risks: [RISK-7], ref: "spikes/s03-pgvector/src/bin/index-bench/main.rs:1363#test13_recall_vs_exact_known_values", status: pass, evidence: "verde sull'host (bridge); recall_vs_exact e returns_k_rate a main.rs:175-189, esatto vuoto = 1,0 documentato"}
      - {test: "I-1 soglia esatto → HNSW", risks: [RISK-6], ref: "spikes/s03-pgvector/results/index/threshold.csv ; spikes/s03-pgvector/RISULTATI-indice.md:60-75", status: pass, evidence: "ricalcolato dal bridge (peggiore dei p95 delle run 2 e 3): exact w4 sel100 23,711 / 100,927 / 111,167 / 522,239 ms a 10k / 50k / 100k / 300k, tutti i 32 valori della tabella di RISULTATI coincidono; soglia = 50.000 chunk. 288 righe = 4 scale × 3 metodi × 2 worker × 4 selettività × 3 run"}
      - {test: "I-2 recall filtrata HNSW (soglia 0,95 e returns_k 100%)", risks: [RISK-7], ref: "spikes/s03-pgvector/results/index/recall.csv ; spikes/s03-pgvector/RISULTATI-indice.md:79-98", status: non_raggiunta, evidence: "ricalcolato dal bridge: migliore combinazione ef_search 200 relaxed_order a ogni scala; minimo sulle 4 selettività vector 0,9730 / 0,9149 / 0,8432 / 0,7324, halfvec 0,9716 / 0,9108 / 0,8365 / 0,7041; raggiunta solo a 10k. sel100 al meglio 0,8500 a 100k e 0,7446 a 300k; off a sel1 returns_k 0 ovunque; returns_k 1,0000 in tutte le righe relaxed/strict; relaxed mai sotto strict; scarto halfvec-vector massimo 0,0608"}
      - {test: "piani EXPLAIN", risks: [RISK-11], ref: "spikes/s03-pgvector/results/index/plans/ (96 file)", status: pass, evidence: "contati e letti dal bridge: 96 = 4 scale × {exact, hnsw_vector, hnsw_halfvec} × w{0,4} × 4 ruoli; tutti i 64 piani hnsw_* contengono l'indice _emb_hnsw o _embh_hnsw, nessuno dei 32 piani exact cita hnsw; filtro EXISTS(SubPlan 1) sulla scansione HNSW. Il threshold si ferma se un piano esatto contiene hnsw (main.rs:571-573) e hnsw se il piano ANN non usa l'indice (main.rs:765-767, :801-803)"}
  - must_have: "La ricerca ibrida tsvector + RRF (k=60) con filtro ACL in entrambe le parti è confrontata con la sola parte densa sulle query ratificate (D-15)"
    verification:
      - {test: TEST-3, risks: [RISK-1], ref: "spikes/s03-pgvector/src/bin/index-bench/hybrid.rs:1286#test3_allowed_cte_in_both_parts", status: pass, evidence: "verde sull'host (bridge); grep -c 'IN (SELECT doc_id FROM allowed)' hybrid.rs = 21 (acceptance ≥ 2); CTE allowed con role = ANY($2) in ogni letterale ibrido (es. hybrid.rs:758, :764, :774)"}
      - {test: TEST-18, risks: [RISK-1, RISK-7], ref: "spikes/s03-pgvector/src/bin/index-bench/hybrid.rs:1248#test18_rrf_fusion_known_values", status: pass, evidence: "verde sull'host (bridge); rrf_fuse a hybrid.rs:389-399, confronto con la fusione SQL in fusion_matches hybrid.rs:403-412"}
      - {test: "I-3 ibrida contro densa", risks: [RISK-1], ref: "spikes/s03-pgvector/results/index/hybrid.csv ; spikes/s03-pgvector/RISULTATI-indice.md:100-110", status: non_raggiunta, evidence: "letto dal bridge: recall@10 all densa 0,9000, ibrida 0,9000 (delta 0,0000 contro +0,03), nDCG@10 0,7226 → 0,7371 (+0,0145), p95 10,999 → 13,743 ms (+2,7); lessicale 0,1500. Ibrida non adottata come da soglia; la densa riproduce 0,9000 e 0,7226 di 01-10"}
      - {test: "fusione SQL = rrf_fuse in ogni query", risks: [RISK-1], ref: "spikes/s03-pgvector/RISULTATI-indice.md:114 ; leak.rs:228-230, :276-280", status: human_judgment, human_judgment: true, rationale: "il contatore fusion_mismatch è solo stampato a fine leak, non scritto in un CSV; lo «0 differenze» viene dai log di sessione, non versionati"}
  - must_have: "Il leak test per ogni profilo di ruolo dà 0 chunk negati nei top-k, sia nella parte semantica sia in quella lessicale, a ogni scala e configurazione (D-12, D-15, D-16)"
    verification:
      - {test: TEST-1, risks: [RISK-1], ref: "spikes/s03-pgvector/src/bin/index-bench/hybrid.rs:1306#test1_hybrid_lexical_match_on_denied_doc_not_returned", status: human_judgment, human_judgment: true, rationale: "test db ignorato, verde solo nel rapporto dell'esecutore (runner); non rieseguito dall'orchestratore né dal bridge. Il bridge ha letto la SQL: il filtro allowed sta nella lex (hybrid.rs:774) prima della fusione"}
      - {test: TEST-2, risks: [RISK-1], ref: "spikes/s03-pgvector/src/bin/index-bench/hybrid.rs:1378#test2_hybrid_semantic_match_on_denied_doc_not_returned", status: human_judgment, human_judgment: true, rationale: "come TEST-1: verde solo per l'esecutore nel runner; filtro nella sem letto a hybrid.rs:764 e, per l'ANN, :830"}
      - {test: TEST-4, risks: [RISK-2], ref: "spikes/s03-pgvector/src/bin/index-bench/leak.rs:332#test4_hnsw_filtered_search_never_returns_denied", status: human_judgment, human_judgment: true, rationale: "test db ignorato, verde solo nel rapporto dell'esecutore; non verifica che l'indice sia usato (lo fa il comando leak a runtime, leak.rs:246-258)"}
      - {test: TEST-5, risks: [RISK-2], ref: "spikes/s03-pgvector/src/bin/index-bench/main.rs:1477#test5_leak_csv_complete_and_zero ; spikes/s03-pgvector/results/index/leak.csv", status: pass, evidence: "test di evidenza rieseguito dal bridge sull'host sul main fuso (verde); ricontato: 2.067 righe = 4 scale × 19 configurazioni × 9 ruoli × 3 parti (2.052) + 15 righe a 3.950 chunk (5 ruoli reali × 3 parti, exact); queries 74 ovunque; awk -F, 'NR>1 && $6!=0' non stampa nulla"}
      - {test: TEST-6, risks: [RISK-3], ref: "spikes/s03-pgvector/src/bin/index-bench/leak.rs:294#test6_leak_counter_independent_of_doc_acl", status: pass, evidence: "verde sull'host (bridge); denied_in_topk (leak.rs:22-24) delega a metrics::leaked_chunks di 01-10 e non riceve connessioni"}
      - {test: TEST-7, risks: [RISK-3, RISK-6], ref: "spikes/s03-pgvector/src/bin/index-bench/synth.rs:721#test7_doc_acl_matches_expected_after_synth", status: human_judgment, human_judgment: true, rationale: "test db ignorato, verde solo per l'esecutore nel runner"}
      - {test: "I-8 doc_acl = matrice attesa prima di ogni leak", risks: [RISK-3], ref: "spikes/s03-pgvector/src/bin/index-bench/leak.rs:158-161 ; main.rs:513-514 ; ingest.rs:135-136, :278-280", status: pass, evidence: "il confronto è bloccante nel codice (check_doc_acl con bail prima della misura); leak.csv ha righe per tutte e 4 le scale e per il corpus reale, quindi ogni leak è arrivato in fondo dopo il confronto. La matrice per i sintetici viene da synth::expected_visibility (synth.rs:273-295), non da doc_acl"}
      - {test: TEST-8, risks: [RISK-4], ref: "spikes/s03-pgvector/src/bin/index-bench/hybrid.rs:1426#test8_hostile_role_and_query_text_are_bound", status: human_judgment, human_judgment: true, rationale: "test db ignorato, verde solo per l'esecutore. Statico: ruoli e testo passano da .bind (hybrid.rs:200-202, :244-248, :283-287); coperto anche da TEST-9"}
      - {test: TEST-9, risks: [RISK-4], ref: "spikes/s03-pgvector/src/bin/index-bench/main.rs:1393#test9_sql_only_bound_no_composed_strings ; hybrid.rs:1276#test9_tables_and_settings_from_closed_enums ; synth.rs:594#test9_scale_tables_come_from_closed_enum ; main.rs:1428#test9_security_scanner_clean", status: pass, evidence: "i tre unitari verdi sull'host e test9_security_scanner_clean (ignorato) rieseguito dal bridge: «OK - nessun problema di sicurezza rilevato su 7 file»; scanner --fail-on HIGH dell'orchestratore su spikes/s03-pgvector e ADR-0006: 0 rilevamenti su 118 file"}
      - {test: TEST-12, risks: [RISK-6], ref: "spikes/s03-pgvector/src/bin/index-bench/leak.rs:382#test12_restricted_only_selectivity_on_real_corpus ; RISULTATI-indice.md:131", status: human_judgment, human_judgment: true, rationale: "test db ignorato, verde solo per l'esecutore; inoltre debole: oltre il 5% pretende solo che RISULTATI contenga «sel5», che c'è comunque, quindi non fallirebbe mai sul numero. Il valore 0,0048 (19/3.950) sta nei log di sessione, non in un CSV"}
  - must_have: "L'inserimento durante le query è misurato: errori e degradazione della latenza p95 con ingestion concorrente (criterio 3)"
    verification:
      - {test: TEST-15, risks: [RISK-9], ref: "spikes/s03-pgvector/src/bin/index-bench/main.rs:1533#test15_ingest_under_load_evidence ; spikes/s03-pgvector/results/index/ingest_under_load.csv ; latency.csv", status: pass, evidence: "test di evidenza rieseguito dal bridge (verde); ricalcolato: 300 righe, errori 0, ingest_rows monotona fino a 6.000; p95 nearest-rank dei p95 al secondo 142,454 ms; a riposo stage sql 100k peggiore delle run 2-3 110,015 ms; rapporto 1,295 < 2. Mediana dei p50 109,4 ms. Il picco di 936,176 ms è la riga t = 1 (secondo successivo 169,399)"}
      - {test: "numero di query in carico", risks: [RISK-9], ref: "spikes/s03-pgvector/RISULTATI-indice.md:118", status: human_judgment, human_judgment: true, rationale: "2.679 query è il totale stampato da ingest.rs:282-286, non ricavabile dal CSV per secondo; coerente con 300 s a circa 110 ms, ma non verificabile da git"}
  - must_have: "Indice e metrica sono decisi in ADR-0006 con misure, piani EXPLAIN e hardware"
    verification:
      - {test: TEST-10, risks: [RISK-5], ref: "spikes/s03-pgvector/src/bin/index-bench/main.rs:1724#test10_thresholds_committed_before_results", status: pass, evidence: "rieseguito dal bridge sull'host (verde); controllo a mano: Soglie e Metodo introdotti in 328ef35 (16:36), primi file di results/index/ in 4e946cf (18:21); git diff 328ef35 1f52ddd -- RISULTATI-indice.md aggiunge solo righe dopo il Metodo"}
      - {test: TEST-14, risks: [RISK-8], ref: "spikes/s03-pgvector/src/bin/index-bench/main.rs:1770#test14_plan_0110_files_untouched", status: pass, evidence: "rieseguito dal bridge (verde); git diff --name-only 4586ff3 exec/01-14 non contiene lib/chunk/tei/db/metrics/main.rs, schema.sql, RISULTATI-embedding.md, Cargo.toml, compose.yaml, DEPENDENCIES.md; gate-proprieta-file «esito: conforme»"}
      - {test: TEST-17, risks: [RISK-11], ref: "spikes/s03-pgvector/src/bin/index-bench/main.rs:1563#test17_evidence_and_links ; docs/adr/0006-indice-vettoriale-ricerca-esatta-ibrida.md ; spikes/s03-pgvector/RISULTATI.md", status: pass, evidence: "rieseguito dal bridge (verde); sei header esatti; hw.json valido con i digest di pgvector e TEI e tools.pgvector 0.8.7, sha256 b20185dc… = RISULTATI-indice.md:50; RISULTATI.md collega le due parti e i due hw.json; link dell'ADR verso file esistenti; i numeri chiave di RISULTATI e ADR coincidono col ricalcolo del bridge salvo le discordanze elencate sotto"}
      - {test: "I-6 end-to-end a 10k", risks: [], ref: "spikes/s03-pgvector/results/index/latency.csv", status: pass, evidence: "ricalcolato: end_to_end 10k peggiore delle run 2-3 = 46,047 ms < 200 (embed 22,975, sql 27,711); 126,399 / 150,911 / 616,959 ms a 50k / 100k / 300k"}
      - {test: "I-7 metrica coseno", risks: [], ref: "spikes/s03-pgvector/src/bin/index-bench/main.rs:1011-1057 ; RISULTATI-indice.md:124-126", status: human_judgment, human_judgment: true, rationale: "il confronto coseno/prodotto interno (74/74) e le norme sono solo stampati da hybrid_cmd, senza CSV né arresto su differenza; dato dei log di sessione, non versionati"}
      - {test: "ADR-0006 e RISULTATI.md", risks: [RISK-11], ref: "docs/adr/0006-indice-vettoriale-ricerca-esatta-ibrida.md:27-31 ; spikes/s03-pgvector/RISULTATI.md:12-39", status: pass, evidence: "letto dal bridge: Stato Proposed, Decisione, Conseguenze con soglia come trigger, IVFFlat escluso con F3, Evidenze con link a RISULTATI.md e hw.json; DEPENDENCIES.md non toccato"}
  - must_have: "Gate d'onda e sicurezza"
    verification:
      - {test: "gate sul main fuso 050eb8f", risks: [], ref: "nota dell'orchestratore", status: pass, evidence: "cargo fmt --check ok; clippy host --workspace --all-targets --locked --offline -D warnings ok; cargo test -p s03-pgvector --locked --offline sull'host: lib 20 pass 5 ign, bin index-bench 8 pass 13 ign, «Compiling s03-pgvector» visto; scanner --fail-on HIGH su spikes/s03-pgvector e ADR-0006: 0 rilevamenti su 118 file. I 13 ignorati non sono stati rieseguiti dall'orchestratore"}
      - {test: "ignorati d'integrazione nel runner", risks: [RISK-1, RISK-2, RISK-3, RISK-4, RISK-6], ref: ".planning/tmp/rapporto-01-14.yaml (note)", status: human_judgment, human_judgment: true, rationale: "i 7 test db (TEST-1, 2, 4, 7, 8, 11, 12) li ha eseguiti solo l'esecutore nel runner (40 verdi con --include-ignored); servono il Postgres della sessione e la finestra di misura di session.sh, che il bridge non ha aperto. I 6 di evidenza o git li ha rieseguiti il bridge sull'host: 6 verdi"}

# Metrics
duration: "circa 2 h 05 min il 2026-10-25: test rossi b69482e 16:22 → Soglie 328ef35 16:36 → bin 22d6d5f 16:58 → misure e RISULTATI 4e946cf / 1f52ddd 18:21-18:23; merge su main 050eb8f 18:26"
completed: 2026-10-25
---

# Fase 1 Piano 14: S-03/A-03, indice vettoriale, ricerca ibrida, leak test e ADR-0006 · Summary

**La ricerca esatta con il predicato ACL nella stessa query resta nel budget fino a 10k chunk (p95 SQL 23,7 ms con 4 worker, end-to-end 46,0 ms contro 200) e lo supera a 50k (100,9 ms): la soglia misurata è 50.000 chunk. HNSW con `iterative_scan = relaxed_order` risponde in p95 ≤ 16 ms a ogni scala, ma con `m = 16` / `ef_construction = 64` la recall filtrata arriva a 0,95 solo a 10k (0,91, 0,84 e 0,73 a 50k, 100k e 300k, su un corpus sintetico pessimistico), quindi non è un sostituto validato. L'ibrida tsvector + RRF non guadagna recall@10 (0,9000 contro 0,9000) e non è adottata. Il leak test dà 0 chunk negati su 2.067 righe (4 scale × 19 configurazioni × 9 ruoli × sem/lex/fusa, più il corpus reale), l'ingestion a 20 chunk/s per 5 minuti non dà errori e porta il p95 a 1,29 volte quello a riposo. ADR-0006 (Proposed): ricerca esatta autorizzata, coseno, `vector` nell'MVP, HNSW da rimisurare su dati reali oltre i 50k**

## Performance

- **Duration:** circa 2 h 05 min. Test rossi `b69482e` (16:22:00), Soglie `328ef35` (16:36:39), bin `22d6d5f` (16:58:29), fix `4042abe` (18:19:03), misure `4e946cf` (18:21:35), RISULTATI e ADR `1f52ddd` (18:23:38), merge `050eb8f` (18:26:52).
- **Started:** 2026-10-25T16:22:00+01:00
- **Completed:** 2026-10-25T18:26:52+01:00
- **Tasks:** 2 di 2
- **Files modified:** 17 voci su 17 dei `files_modified` (16 file più la cartella dei piani EXPLAIN con 96 file), `git diff --name-only 4586ff3 exec/01-14`; `050eb8f^2` = `1f52ddd` = punta di `exec/01-14`. Nessun file fuori elenco; nessun file di 01-10 (lib, chunk, tei, db, metrics e main del crate, `spikes/s03-pgvector/sql/schema.sql`, `spikes/s03-pgvector/RISULTATI-embedding.md`), `Cargo.toml`, `spikes/compose.yaml` o `DEPENDENCIES.md` toccato.

## Accomplishments

- **Soglie e Metodo prima della misura** (`spikes/s03-pgvector/RISULTATI-indice.md:17-44`, commit `328ef35`): I-1 soglia esatto → HNSW, I-2 recall filtrata ≥ 0,95 con returns_k 100%, I-3 ibrida a +0,03, I-4 leak 0, I-5 ingestion 0 errori e < 2×, I-6 end-to-end < 200 ms a 10k, I-7 metrica, I-8 `doc_acl` = matrice attesa, I-9 `restricted-only` ≤ 5%.
- **Query** (`hybrid.rs`): densa esatta col semi-join `EXISTS` di 01-10 (`hybrid.rs:419-430`), densa ANN con `EXISTS (… OFFSET 0)` (`:431-442`, `:455-466`), lessicale con `websearch_to_tsquery` e `ts_rank_cd` (`:1191-1208`), ibrida con CTE `allowed` in `sem` e `lex` e RRF in SQL (`:755-788`), impostazioni via `set_config` legato (`:157-181`).
- **Corpus sintetico** (`synth.rs`): SplitMix64 con flussi per etichetta (`:107-156`), perturbazione gaussiana di norma 0,5 e rinormalizzazione (`:160-170`), classificazioni col metodo dei resti più grandi (`:192-222`), ruoli `selNN` su prefissi annidati di una permutazione (`:264-295`), insert a lotti con `UNNEST` (`:437-514`).
- **Misure** (`main.rs`): `threshold` con piani EXPLAIN e latenza end-to-end (`:537-635`), `hnsw` con build, recall per griglia e latenza (`:651-839`), `hybrid` con verifica della metrica (`:900-1060`); `leak` con contatore sulla matrice attesa e verifica d'uso dell'indice (`leak.rs:150-282`); `ingest-under-load` con due task concorrenti (`ingest.rs:117-288`).
- **Sintesi di S-03:** `RISULTATI.md` (indice, Esito, impatto su porte e threat model, destinazione) e `docs/adr/0006-indice-vettoriale-ricerca-esatta-ibrida.md` (Proposed).

## Task Commits

0. **Test rossi (agente test-write, distinto dall'esecutore):** `b69482e` (test). RED_EXPECTED, 8 rossi, 13 ignorati, 0 verdi inattesi (`.planning/tmp/test-write-01-14.yaml`). L'esecutore non ha tolto né indebolito alcun test: le sole righe rimosse dai quattro file del test-write sono gli stub `todo!()`, i `let _ = …`, gli `use` inutilizzati e `#![allow(dead_code)]` (`git diff b69482e 1f52ddd`).
1. **Task 1, soglie e bin:** `328ef35` (docs), `22d6d5f` (feat).
2. **Task 2, misure, RISULTATI e ADR:** `4042abe` (fix), `4e946cf` (test), `1f52ddd` (docs).

Base dell'esecuzione: `4586ff3`. **Merge:** `050eb8f` (`--no-ff` di `exec/01-14`). **Plan metadata:** questo SUMMARY e `01-14-RACCONTO.md`, scritti dal bridge senza commit.

## Files Created/Modified

- `spikes/s03-pgvector/src/bin/index-bench/main.rs`: CLI, funzioni pure (recall, returns_k, split di `index.sql`), `prepare`, `synth`, `threshold`, `hnsw`, `drop-hnsw`, `hybrid`, modulo `testdb` di supporto, 8 test (6 ignorati di evidenza o git).
- `spikes/s03-pgvector/src/bin/index-bench/hybrid.rs`: enum chiusi, letterali SQL per tabella e strategia, `rrf_fuse`, 6 test (3 db).
- `spikes/s03-pgvector/src/bin/index-bench/leak.rs`: contatore, matrice attesa completa, griglia HNSW, comando `leak`, 3 test (2 db).
- `spikes/s03-pgvector/src/bin/index-bench/synth.rs`: generatore, matrice attesa sintetica, SQL per scala, 4 test (2 db).
- `spikes/s03-pgvector/src/bin/index-bench/ingest.rs`: ingestion concorrente.
- `spikes/s03-pgvector/sql/index.sql`: colonna `halfvec`, tabelle sintetiche, indici HNSW.
- `spikes/s03-pgvector/results/index/`: `hw.json`, sei CSV, 96 piani.
- `spikes/s03-pgvector/RISULTATI-indice.md`, `spikes/s03-pgvector/RISULTATI.md`, `docs/adr/0006-indice-vettoriale-ricerca-esatta-ibrida.md`.

## Decisions Made

### Decisioni dell'utente già prese (`01-CONTEXT.md`)

- **D-12** (ricerca esatta nell'MVP, soglia HNSW misurata), **D-15** (ibrida RRF k = 60 come baseline, ACL in entrambe le parti), **D-16** (corpus pubblico e sintetico, RBAC reale). ADR-0006 resta *Proposed*; la ratifica è dell'utente nel consolidamento (01-17, Task 2).

### Scelte dell'esecutore

- **Forma ANN `EXISTS (… OFFSET 0)` con `enable_seqscan = off`** (`hybrid.rs:94-102`, `:169-181`): col semi-join il planner trasforma il filtro in hash join + seq scan e non usa mai l'indice; `OFFSET 0` impedisce di appiattire la sottoquery, che resta un filtro valutato riga per riga sulla scansione ordinata dell'indice.
- **Esatta con `enable_indexscan = off`** dove un indice HNSW esiste (`hybrid.rs:157-164`), così il riferimento non può usare l'indice approssimato.
- **HNSW solo sulle tabelle sintetiche** e rimosso dopo il leak di ogni scala (`sql/index.sql:15-19`): spazio disco e misure di threshold e ingestion senza indice.
- **Contatore del leak sulla matrice attesa** (`leak.rs:43-61`), con `doc_acl` confrontata prima (`leak.rs:158-161`): misura non circolare.
- **Binario `/target/release/index-bench`** invece di `cargo run -p s03-pgvector --release -- <sub>`: con due bin il comando del piano è ambiguo e `Cargo.toml` non è fra i file del piano.
- **Interpretazione di I-2**: la stessa combinazione deve valere per le quattro selettività; riportato anche il caso per selettività singola, che fallisce ugualmente oltre 10k.

## Esiti delle soglie

| soglia | esito | numeri (ricalcolati dal bridge) |
|---|---|---|
| I-1 soglia esatto → HNSW | **50.000 chunk** | p95 SQL sel100 w4 23,711 / 100,927 / 111,167 / 522,239 ms |
| I-2 recall filtrata ≥ 0,95, returns_k 100% | **raggiunta solo a 10k** | min sulle selettività 0,9730 / 0,9149 / 0,8432 / 0,7324 (vector, ef 200, relaxed) |
| I-3 ibrida +0,03 recall@10 | **non raggiunta**, ibrida non adottata | +0,0000 recall@10, +0,0145 nDCG@10, +2,7 ms |
| I-4 leak = 0 | pass | 0 su 2.067 righe |
| I-5 ingestion 0 errori, < 2× | pass | 0 errori, 142,454 / 110,015 = 1,29× |
| I-6 end-to-end < 200 ms a 10k | pass | 46,047 ms |
| I-7 metrica coseno | pass dichiarato | 74/74 top-10 uguali (solo log, non versionato) |
| I-8 `doc_acl` = matrice attesa | pass | confronto bloccante nel codice, leak completi |
| I-9 `restricted-only` ≤ 5% | pass dichiarato | 0,0048 = 19/3.950 (solo log) |

## Deviations from Plan

### Livello a

1. **`maintenance_work_mem` 768MB invece di 2GB** (`4042abe`, `main.rs:659-665`): con 2GB la build parallela falliva per lo `shm_size` di 1 GB del container postgres (`compose.yaml`, file di 01-02, non toccato). A 300k l'indice `vector` (2.343,8 MiB) non sta in 768 MB e la build finisce fuori memoria; non è verificato se cambi la qualità del grafo. Il Metodo di RISULTATI-indice (riga 40) dice ancora 2GB; la correzione sta in «Anomalie» 1.
2. **Forma della query HNSW diversa da quella esatta** (`22d6d5f`, Anomalie 2): `EXISTS (… OFFSET 0)` ed `enable_seqscan = off`. Il predicato resta nella stessa query, nessun post-filtro applicativo; piani EXPLAIN e `idx_scan` lo confermano.
3. **Nessun HNSW su `chunk_bgem3`** (Anomalie 3): ma `sql/index.sql:23-24` aggiunge alla tabella reale la colonna generata `emb_h` (modifica dei dati nel volume `pgdata`, nessun file di 01-10).
4. **`hw.json` normalizzato** allo schema di `common::manifest` con un driver fuori repo, originale in `results/raw/` (non versionato): difetto noto di `hw-manifest.sh` già visto in 01-04, 01-07, 01-08, 01-10, 01-11, 01-13.
5. **File di ambiente temporaneo** spikes-01-14.env (nella cartella $HOME/.cka-spikes, fuori dal repo) per passare `EMBED_MODEL`, `EMBED_REVISION` e `TEI_MAX_BATCH_TOKENS` a `session.sh`, cancellato a fine piano (come 01-10). Non verificabile da git.
6. **Binario al posto di `cargo run`** (Anomalie 7): due bin nel pacchetto, `Cargo.toml` fuori perimetro. Il commento di testa di `main.rs:6` suggerisce `--bin index-bench`.
7. **Falso positivo dello scanner** (`plaintext-pwd-compare` su una variabile `pass`): rinominata `round` (`22d6d5f`).

### Scostamenti dall'acceptance scritta

- Il `<verify>` e l'`<action>` del Task 2 usano `cargo run -p s03-pgvector --release -- <sottocomando>`: non univoco con due bin (deviazione 6).
- Il Metodo scritto prima della misura dice `maintenance_work_mem = 2GB`; la misura è a 768MB, dichiarata.

**Total deviations:** 7 di livello a; nessuna di livello b o c.
**Impact on plan:** nessuna estensione di perimetro (17 voci su 17).

## Discrepanze fra rapporto, RISULTATI e prove

- **Ordine del fix di `maintenance_work_mem`.** RISULTATI-indice (Anomalie 1) dice «Correzione nel commit `4042abe`, prima di ogni misura HNSW». Il commit è delle 18:19, due minuti prima di `4e946cf` con i CSV, mentre la sessione `01-14-20261025t154840z-0866` è partita alle 16:48 e `hw.json` porta `git_rev` `22d6d5f`. La correzione era quindi nel worktree, non committata, durante le misure HNSW: il binario misurato non corrisponde a un commit dell'epoca. Il codice finale di `build_index` è quello di `4042abe`; che sia identico a quello misurato non è verificabile.
- **Parallelismo nei piani a sel100.** RISULTATI-indice (riga 75) dice che il piano esatto a `sel100` è un `Parallel Seq Scan` con 4 worker lanciati. Nei piani salvati con `max_parallel_workers_per_gather = 4` i worker lanciati sono 0 a 10k (nessun `Gather`), 2 a 50k, 3 a 100k e 4 solo a 300k: 4 è il tetto, il planner sceglie in base alla dimensione della tabella.
- **Picco nell'ingestion.** RISULTATI-indice (Anomalie 9) ipotizza autovacuum o checkpoint per «un secondo su 300» a 936 ms. Nel CSV quel secondo è il primo (`t = 1`, p50 = p95 = 936,176 ms, quindi una sola query conclusa): il task delle query di `ingest.rs:154-189` non ha riscaldamento, ed è più probabile la prima query a freddo.
- **Recall con `iterative_scan = off` a sel1.** RISULTATI dice 0,03-0,20; il CSV dà 0,0243-0,1959 (0,02-0,20). Nessun esito cambia.
- **Linearità.** ADR-0006 (Conseguenze negative) dice che la latenza «cresce in modo lineare con i chunk visibili»; i numeri (23,7 / 100,9 / 111,2 / 522,2 ms) non sono lineari, anche per il diverso numero di worker lanciati.
- **SQL p95 a 10k.** RISULTATI.md ed «Esito parziale» citano 23,7 ms (threshold.csv); `latency.csv` dà per lo stage `sql` alla stessa configurazione 27,711 ms (misura diversa, intercalata con TEI). Non contraddittorio, ma i due numeri convivono senza spiegazione.
- **Solo nei log di sessione (non versionati):** 2.679 query dell'ingestion, I-7 (74/74 e norme), fusione SQL = `rrf_fuse` (0 differenze), I-9 0,0048, tempi di `synth` e selettività esatte. Il bridge non li vede.
- **Il resto coincide** con il ricalcolo: tutti i p95 della tabella I-1, le recall per scala, tipo e selettività, hybrid.csv, conteggi e zeri di leak.csv, rapporto dell'ingestion, end-to-end, 96 piani, sha256 di `hw.json`, ordine Soglie → misure.

*Esito al bridge (orchestratore, 2026-10-25):* le frasi di RISULTATI-indice (Metodo `maintenance_work_mem`, Anomalie 1 sulla tracciabilità di `4042abe`, worker lanciati per scala, Anomalie 9 sul picco a freddo, recall 0,02-0,20) e di ADR-0006 («lineare») sono state corrette nel commit del bridge. Resta senza spiegazione la differenza fra il p95 SQL a 10k di `threshold.csv` (23,7 ms) e lo stage `sql` di `latency.csv` (27,7 ms): punto aperto.

## Threat Flags

- **T-01-14-01 (leak denso, lessicale, ibrido):** CTE `allowed` in ogni sottoquery; 0 negati su 2.067 righe contati contro la matrice attesa. I test di comportamento su DB (TEST-1, 2, 4) non sono stati rieseguiti fuori dall'esecutore.
- **T-01-14-02 (HNSW con filtro):** perdita di recall misurata e dichiarata in ADR-0006; con `off` returns_k 0 a sel1.
- **T-01-14-03 (SQL):** solo `bind`, letterali da enum chiusi, `set_config` legato; scanner pulito.
- **T-01-14-04 (query costose durante l'ingestion):** 0 errori, 1,29×; la soglia di 50k è il trigger di rivalutazione (T-24).

## Known Stubs

Nessuno.

## User Setup Required

Nessuno.

## Punti aperti

- **Latenza dell'embedding che cresce con la scala** (22,975 → 105,343 ms p95 da 10k a 300k) con TEI indipendente dalla tabella: causa non indagata (contesa di CPU o memoria nella VM WSL col seq scan parallelo, ipotesi). I-6 si giudica a 10k come da Metodo.
- **HNSW non validato** oltre 10k: rimisura su dati reali con `m` / `ef_construction` più alti prima di qualunque adozione; qualità del grafo costruito fuori memoria a 300k non verificata.
- **TEST-12 debole** (test del test-write): non fallisce sul numero se la selettività supera il 5%. Da rafforzare in /verify 1.
- **Ratifica di ADR-0006** nel consolidamento (01-17, Task 2); righe `DEPENDENCIES.md` «Vector search» ed «Embedding» da Proposta a Decisa lì.
- **T-20** (inversione degli embedding): la mitigazione «mai via API» resta da verificare sulle rotte del Document service.
- **`publish_version` atomica** fuori da questa misura (RISULTATI.md:28).

## Next Phase Readiness

- **01-15 (reranker):** `chunk_bgem3` senza indice HNSW, quindi `export-candidates` di 01-10 resta esatto e deterministico; la colonna `emb_h` aggiunta non cambia l'ordine.
- **01-16 (porte S-07):** `VectorIndex::search(query_embedding, query_text, predicate: AuthorizationPredicate, k, deadline)` con predicato costruito da `AccessPolicy` e applicato nella query (RISULTATI.md:25-28).
- **01-17 (consolidamento):** ratifica di ADR-0006 e aggiornamento di `DEPENDENCIES.md`.

## Self-Check: PASSED

- 17 voci su 17 dei `files_modified` nel diff `4586ff3..exec/01-14`, nessun file di altre fasi; `050eb8f^2` = `1f52ddd`; `gate-proprieta-file` conforme.
- Rieseguiti dal bridge sul main fuso, sull'host: `cargo test -p s03-pgvector --locked --offline --bin index-bench` (8 passati, 13 ignorati) e i 6 ignorati di evidenza o git (`test5`, `test9_security_scanner_clean`, `test10`, `test14`, `test15`, `test17`: 6 verdi).
- Ricalcolati dai CSV: I-1, I-2, I-3, I-4, I-5, I-6; contati i piani EXPLAIN e verificato l'uso dell'indice; sha256 di `hw.json`; ordine dei commit e invarianza delle Soglie.
- Gate sul main fuso `050eb8f` (nota dell'orchestratore): fmt ok; clippy `-D warnings` ok; `cargo test -p s03-pgvector` sull'host lib 20 + 5 ignorati, bin 8 + 13 ignorati; scanner con soglia HIGH, 0 rilevamenti su 118 file. I 7 test db sono verdi solo nel runner dell'esecutore.

**Modifica:** Claude (Anthropic) · 2026-10-25 · creazione (bridge di 01-14, passi 1-3)
