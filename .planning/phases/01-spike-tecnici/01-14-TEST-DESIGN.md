# Test design 01-14 — S-03 indice vettoriale, ricerca ibrida e leak test

```yaml
reviewed_plan:
  path: .planning/phases/01-spike-tecnici/01-14-PLAN.md
  sha256: 0fcd1e798d5f27357de0cd3b925968c392d6a8533e42e2bb5a7af594698bc63b
risks:
  - id: RISK-1
    severity: CRITICAL
    title: La ricerca ibrida lascia passare chunk non autorizzati dalla parte lessicale (CTE allowed applicata solo a sem, o fusione RRF dopo il filtro)
    source: threat_model T-01-14-01
  - id: RISK-2
    severity: CRITICAL
    title: HNSW con filtro restituisce chunk negati o il leak test non copre ogni scala, configurazione e parte
    source: must_have "leak test per ogni profilo ... 0 chunk negati ... a ogni scala e configurazione"
  - id: RISK-3
    severity: HIGH
    title: Il leak test conta i negati con la stessa doc_acl usata dal filtro (misura circolare)
    source: threat_model T-01-14-01 (leak.csv denied_in_topk = 0)
  - id: RISK-4
    severity: HIGH
    title: SQL composto con valori in ingresso o nome tabella da input
    source: threat_model T-01-14-03
  - id: RISK-5
    severity: HIGH
    title: Soglie scritte o ritoccate dopo la misura
    source: Task 1 acceptance "Soglie committate prima di ogni file in results/index/"
  - id: RISK-6
    severity: HIGH
    title: Selettività dei ruoli sintetici diversa da 100/25/5/1% (e restricted-only > 5%), quindi la soglia HNSW e la recall filtrata sono misurate sul caso sbagliato
    source: must_have "soglia oltre cui passare a HNSW ... è un numero misurato"
  - id: RISK-7
    severity: MEDIUM
    title: Recall filtrata di HNSW calcolata male rispetto all'esatto, o returns_k_rate non misurato
    source: threat_model T-01-14-02
  - id: RISK-8
    severity: MEDIUM
    title: Il piano modifica file del piano 01-10 (libreria, schema)
    source: key_link "riuso della libreria del piano 01-10 senza modificarla"
  - id: RISK-9
    severity: MEDIUM
    title: Ingestion sotto carico non misurata per la durata dichiarata o errori nascosti
    source: threat_model T-01-14-04
  - id: RISK-10
    severity: MEDIUM
    title: Dati sintetici non riproducibili (seme non fisso) o vettori non rinormalizzati
    source: Task 1 action synth
  - id: RISK-11
    severity: MEDIUM
    title: Evidenze incomplete (scale, EXPLAIN, header, hw.json) o RISULTATI/ADR con link rotti
    source: must_have "deciso in ADR-0006 con misure, piani EXPLAIN e hardware"
tests:
  - {id: TEST-1, title: hybrid_lexical_match_on_denied_doc_not_returned, type: cargo-integration-db, priority: P0, catches: [RISK-1]}
  - {id: TEST-2, title: hybrid_semantic_match_on_denied_doc_not_returned, type: cargo-integration-db, priority: P0, catches: [RISK-1]}
  - {id: TEST-3, title: Statico CTE allowed in sem e lex, type: check-statico, priority: P0, catches: [RISK-1]}
  - {id: TEST-4, title: hnsw_filtered_search_never_returns_denied, type: cargo-integration-db, priority: P0, catches: [RISK-2]}
  - {id: TEST-5, title: leak.csv completo per scala × config × ruolo × parte e denied_in_topk = 0, type: check-evidenze, priority: P0, catches: [RISK-2]}
  - {id: TEST-6, title: leak_counter_independent_of_doc_acl, type: cargo-unit, priority: P0, catches: [RISK-3]}
  - {id: TEST-7, title: Ruoli sintetici in doc_acl coerenti con la selettività dichiarata, type: check-db, priority: P0, catches: [RISK-3, RISK-6]}
  - {id: TEST-8, title: hostile_role_and_query_text_are_bound, type: cargo-integration-db, priority: P1, catches: [RISK-4]}
  - {id: TEST-9, title: Nessun format! con SQL, tabella da enum chiuso, scanner verde, type: check-statico, priority: P1, catches: [RISK-4]}
  - {id: TEST-10, title: Soglie committate prima dei risultati e invariate, type: check-git, priority: P0, catches: [RISK-5]}
  - {id: TEST-11, title: synth_role_selectivity_matches_target, type: cargo-integration-db, priority: P0, catches: [RISK-6]}
  - {id: TEST-12, title: restricted-only ≤ 5% sul corpus reale, type: check-evidenze, priority: P1, catches: [RISK-6]}
  - {id: TEST-13, title: recall_vs_exact_known_values, type: cargo-unit, priority: P1, catches: [RISK-7]}
  - {id: TEST-14, title: git diff non tocca file del piano 01-10, type: check-git, priority: P1, catches: [RISK-8]}
  - {id: TEST-15, title: ingest_under_load ≥ 300 righe, errori dichiarati, soglia ricalcolata, type: check-evidenze, priority: P1, catches: [RISK-9]}
  - {id: TEST-16, title: synth_is_deterministic_and_normalized, type: cargo-unit, priority: P2, catches: [RISK-10]}
  - {id: TEST-17, title: Validità delle evidenze e link, type: check-evidenze, priority: P1, catches: [RISK-11]}
  - {id: TEST-18, title: rrf_fusion_known_values, type: cargo-unit, priority: P2, catches: [RISK-1, RISK-7]}
uncovered: []
```

## Come leggere questo design

Questo piano decide come il prodotto cercherà i chunk e porta il leak test alla sua forma completa: parte semantica e lessicale, quattro scale, configurazioni esatta e HNSW. I due CRITICAL sono le due strade per cui un chunk negato può entrare nei top-k: la parte lessicale della ricerca ibrida (RISK-1) e l'indice approssimato con filtro (RISK-2). Come nel piano 01-10, il leak test ha senso solo se il conteggio dei negati non usa la stessa tabella che filtra (RISK-3). E la soglia HNSW misurata vale solo se la selettività dei ruoli sintetici è quella dichiarata (RISK-6).

Dove vivono i test: i moduli `#[cfg(test)]` dei file in `spikes/s03-pgvector/src/bin/index-bench/` (`hybrid.rs`, `leak.rs`, `synth.rs`, `main.rs`). I test «db» sono `#[ignore]`, richiedono `DATABASE_URL` verso il Postgres del compose, lavorano in uno schema temporaneo `test_<pid>` creato con le stesse tabelle di `sql/schema.sql` e `sql/index.sql` e lo eliminano alla fine. Comando: `cd spikes && cargo test -p s03-pgvector --bin index-bench -- --ignored <nome>` dal runner; i test non «db» senza `--ignored`. La libreria del piano 01-10 si usa e non si modifica.

## Test

**TEST-1 — `hybrid_lexical_match_on_denied_doc_not_returned`** (in `hybrid.rs`, db). Documento `D_res` (restricted) con un chunk che contiene la parola rara «zettabilancio» e vettore lontano dalla query; documento `D_pub` (public) con chunk senza quella parola. Query testuale «zettabilancio», ruolo employee, k = 5: il risultato non contiene chunk di `D_res` né nella lista `lex` intermedia (se la funzione la espone) né nella fusione RRF finale. Con ruolo admin il chunk di `D_res` è primo.

**TEST-2 — `hybrid_semantic_match_on_denied_doc_not_returned`** (in `hybrid.rs`, db). Speculare: chunk di `D_res` con vettore identico alla query e nessuna parola in comune; ruolo employee, nessun chunk di `D_res` in output. TEST-1 e TEST-2 insieme provano che il filtro sta in entrambe le sottoquery e non solo dopo la fusione.

**TEST-3 — Statico.** `grep -c "IN (SELECT doc_id FROM allowed)" spikes/s03-pgvector/src/bin/index-bench/hybrid.rs` ≥ 2 (acceptance del Task 1) e la CTE `allowed` è costruita con `role = ANY($n)` legato a parametro.

**TEST-4 — `hnsw_filtered_search_never_returns_denied`** (in `leak.rs` o `main.rs`, db). Tabella con indice HNSW (m = 16, ef_construction = 64) su 2.000 chunk di cui il 95% di documenti restricted vicinissimi alla query e il 5% public; ruolo employee; per ogni combinazione di `ef_search` ∈ {40, 100, 200} e `iterative_scan` ∈ {off, relaxed_order, strict_order}: 0 chunk restricted nei risultati. Con `iterative_scan = off` il numero di risultati può essere < k (perdita di recall attesa, non leak): il test registra il conteggio senza fallire su quello, fallisce solo su un negato.

**TEST-5 — Completezza ed esito di `leak.csv`.** `awk -F, 'NR>1 && $6!=0' spikes/s03-pgvector/results/index/leak.csv` non stampa nulla (o RISULTATI dichiara il fallimento con i casi). Completezza: per ogni `chunks` ∈ {10000, 50000, 100000, 300000}, per ogni `config` usata in `threshold.csv` e `recall.csv` (esatta e ogni combinazione HNSW misurata), per ogni ruolo di `acl.json`, per `restricted-only` e per `sel100/sel25/sel5/sel1`, esistono righe con `part = sem` e `part = lex` (e `hybrid` se la configurazione ibrida è misurata), con `queries > 0`. Un buco nella griglia è un fallimento: «0 negati» su una combinazione non misurata non vale.

**TEST-6 — `leak_counter_independent_of_doc_acl`** (in `leak.rs`). La funzione che conta `denied_in_topk` riceve i risultati (con `doc_key`) e una mappa di visibilità costruita da `acl.json` + classificazioni del manifest (per i ruoli reali) o dalla regola di assegnazione dei ruoli sintetici (per `selNN`), senza interrogare `doc_acl`. Test: risultato con un chunk di un documento restricted per employee → conteggio 1.

**TEST-7 — `doc_acl` coerente con la verità.** Dopo `synth` e prima delle misure, confronto (`psql -At` nel container + `node -e` inline) fra le righe di `doc_acl` e la relazione attesa: per i ruoli reali quella di `acl.json` + manifest, per i ruoli sintetici quella prodotta dalla regola deterministica di `synth`. Le due relazioni coincidono esattamente; una riga in più è un leak latente.

**TEST-8 — `hostile_role_and_query_text_are_bound`** (in `hybrid.rs`, db). Ruolo `"employee' OR '1'='1"` → 0 risultati, nessun errore di sintassi; testo di query `"'); DROP TABLE doc; --"` passato a `websearch_to_tsquery` → nessun errore e `doc` ancora presente.

**TEST-9 — Statico SQL.** Nessun `format!` che componga SQL con valori in `src/bin/index-bench/*.rs`; le tabelle `chunk_synth_<n>` e `chunk_<modello>` vengono da `match` su enum chiusi (`Scale::{N10k, N50k, N100k, N300k}`); lo scanner `python skills/security-scanner/security_scan.py spikes/s03-pgvector/src/bin spikes/s03-pgvector/sql` esce 0. Nota per l'esecutore: `CREATE INDEX CONCURRENTLY` non può stare in un blocco transazionale; se `index.sql` è eseguito in una transazione, va diviso o eseguito istruzione per istruzione.

**TEST-10 — Soglie prima delle misure.** Il commit che introduce «## Soglie (scritte PRIMA della misura)» in `RISULTATI-indice.md` è antenato del primo commit che aggiunge file in `results/index/`; la sezione è identica a HEAD e contiene: soglia esatto → HNSW (p95 50 ms con 4 worker sul profilo meno selettivo), recall filtrata ≥ 0,95 e returns_k = 100%, ibrida se recall@10 + 0,03, leak = 0, ingest 0 errori e p95 < 2× riposo, end-to-end < 200 ms a 10k, regola sulla metrica.

**TEST-11 — `synth_role_selectivity_matches_target`** (in `synth.rs`, db). Dopo `synth --n 10000` nello schema di test: per `sel100`, `sel25`, `sel5`, `sel1` la quota di chunk visibili (`count(*) where exists doc_acl …` / totale) è entro ±0,5 punti percentuali dal target. Il test è la condizione per credere a `recall.csv` per selettività.

**TEST-12 — `restricted-only` ≤ 5%.** RISULTATI-indice riporta la selettività misurata di `restricted-only` sul corpus reale, e un ricalcolo (`psql` nel container) dà lo stesso numero, ≤ 5%. È la verifica rinviata dal piano 01-03. Se supera il 5%, RISULTATI lo dichiara e il profilo `sel5` sintetico diventa il riferimento dello stress.

**TEST-13 — `recall_vs_exact_known_values`** (in `main.rs` o `leak.rs`). Funzione pura: esatto = [a, b, c, d], HNSW = [a, c, x, y] → recall 0,5; HNSW = [] → 0; esatto vuoto → valore documentato, mai NaN. `returns_k_rate` per 4 query con risultati di lunghezza 10, 10, 7, 10 e k = 10 → 0,75.

**TEST-14 — Perimetro.** `git diff --name-only <base>..HEAD` del piano non contiene `spikes/s03-pgvector/src/{lib,chunk,tei,db,metrics,main}.rs` né `sql/schema.sql` né `RISULTATI-embedding.md`.

**TEST-15 — Ingestion sotto carico.** `ingest_under_load.csv` ha ≥ 300 righe; la somma di `errors` è riportata in RISULTATI; `ingest_rows` cresce in modo monotono non decrescente; il p95 in carico confrontato con il p95 a riposo di `latency.csv` alla stessa scala dà lo stesso esito dichiarato in RISULTATI (< 2×).

**TEST-16 — `synth_is_deterministic_and_normalized`** (in `synth.rs`). Due generazioni con lo stesso seme da 5 vettori di partenza producono vettori identici bit a bit; ogni vettore generato ha norma L2 = 1 ± 1e-5; la distribuzione delle classificazioni dei documenti sintetici coincide con quella del corpus reale (stesse proporzioni, tolleranza ±1 documento).

**TEST-17 — Evidenze e link.** Il verify del Task 2 più: `threshold.csv` ha le quattro scale × `parallel_workers` {0, 4} × run 2..3; per ogni configurazione di `threshold` esiste un `plans/<config>.json` JSON valido che contiene `"Plan"`; `latency.csv` ha gli stage `embed_query`, `sql`, `end_to_end`; `hw.json` ha i digest di pgvector e TEI e `tools.pgvector`; `RISULTATI.md` di S-03 collega `RISULTATI-embedding.md`, `RISULTATI-indice.md` e i due `hw.json`, tutti esistenti; ogni percorso `spikes/...` citato in ADR-0006 esiste. I numeri di RISULTATI (soglia in chunk, recall per selettività, delta ibrida) coincidono con un ricalcolo da CSV.

**TEST-18 — `rrf_fusion_known_values`** (in `hybrid.rs`). Se la fusione RRF è calcolata anche in Rust (per il confronto o la verifica), con k = 60: un elemento primo in entrambe le liste ha punteggio 2/61; primo in una e assente nell'altra 1/61; l'ordinamento a parità è deterministico. Se la fusione è solo in SQL, il test si sposta in `db` con due liste costruite a mano e verifica lo stesso ordine.

## Copertura

I due CRITICAL hanno test di comportamento su un database reale (TEST-1, 2, 4) più il controllo di completezza della griglia (TEST-5). I HIGH sono tutti coperti; RISK-3 e RISK-6 hanno ciascuno due test, perché da loro dipende se i numeri del leak e della soglia significano qualcosa.

**Modifica:** Claude (test designer, Anthropic) · 2026-10-18 · test design 01-14

**Modifica:** Claude (Anthropic) · 2026-10-18 · reviewed_plan.sha256 aggiornato da 07a08d7f63e3… a 3b1591dd547b… per la revisione del piano: denied_in_topk contato contro la matrice attesa (acl.json + manifest, seme per i ruoli sintetici), doc_acl confrontata prima di misurare. Il design non è stato rigenerato: va riletto contro la revisione.

**Modifica:** Claude (Anthropic) · 2026-10-18 · reviewed_plan.sha256 aggiornato da 3b1591dd547b… a 0fcd1e798d5f… per l'allineamento a 01-02 v2 (postgres e tei-embed avviati con session.sh up, sottocomandi con session.sh oneshot; finestra di misura con prenotazione session.sh). I test del design non cambiano: cambiano solo i comandi di avvio dei servizi e la prenotazione della finestra di misura.
