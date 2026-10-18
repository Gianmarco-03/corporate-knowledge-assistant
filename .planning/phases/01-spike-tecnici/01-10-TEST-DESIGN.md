# Test design 01-10 — S-03/A-03 embedding e serving TEI

```yaml
reviewed_plan:
  path: .planning/phases/01-spike-tecnici/01-10-PLAN.md
  sha256: dea5b3be9b9591893da74b59045f79404aee4d6412769913bb9c421aed485bca
risks:
  - id: RISK-1
    severity: CRITICAL
    title: exact_search restituisce chunk di documenti non visibili al ruolo (predicato ACL assente, sbagliato o applicato dopo il LIMIT)
    source: threat_model T-01-10-01
  - id: RISK-2
    severity: HIGH
    title: Il contatore leaked_chunks usa la stessa tabella doc_acl della query, quindi un doc_acl popolato male dà leak = 0 per costruzione
    source: threat_model T-01-10-01 (hard gate leaked_chunks = 0)
  - id: RISK-3
    severity: HIGH
    title: SQL composto con valori in ingresso (iniezione) o nome tabella da input
    source: threat_model T-01-10-02
  - id: RISK-4
    severity: HIGH
    title: Metriche di qualità (recall, MRR, nDCG) o regola di rilevanza implementate male, decisione del modello falsata
    source: must_have "misurati sulle stesse query etichettate con recall@5/@10, MRR@10 e nDCG@10"
  - id: RISK-5
    severity: HIGH
    title: Soglie o regola di rilevanza scritte o ritoccate dopo la misura
    source: Task 1 acceptance "Soglie committate prima di ogni file in results/embedding/"
  - id: RISK-6
    severity: MEDIUM
    title: Chunk a cavallo di due pagine o sezioni, offset sbagliati, locator di misura inaffidabile
    source: Task 1 action chunk.rs
  - id: RISK-7
    severity: MEDIUM
    title: Istruzione asimmetrica applicata ai documenti o non applicata alla query (o flag --no-instruction inefficace)
    source: must_have "effetto dell'istruzione asimmetrica sulla query è misurato"
  - id: RISK-8
    severity: MEDIUM
    title: Pesi del modello non quelli registrati (revisione HF diversa da model_sha di /info)
    source: threat_model T-01-10-04
  - id: RISK-9
    severity: MEDIUM
    title: export-candidates non deterministico, il piano 01-15 non può riprodurre lo stesso sha256; testo dei candidati versionato
    source: threat_model T-01-10-03
  - id: RISK-10
    severity: MEDIUM
    title: Misure GPU eseguite con gate GPU non verde, o evidenze incomplete (header, matrice modelli × lingue × k, hw.json)
    source: must_have "dopo il gate GPU verde"
  - id: RISK-11
    severity: MEDIUM
    title: DATABASE_URL o credenziali in log, CSV o RISULTATI
    source: threat_model T-01-10-05
  - id: RISK-12
    severity: LOW
    title: Oracoli pendenti (non ratificati) usati nelle metriche
    source: must_have 01-06 propagato "oracoli pendenti esclusi"
tests:
  - {id: TEST-1, title: exact_search_never_returns_invisible_doc_even_if_nearest, type: cargo-integration-db, priority: P0, catches: [RISK-1]}
  - {id: TEST-2, title: exact_search_returns_k_visible_when_invisible_dominate, type: cargo-integration-db, priority: P0, catches: [RISK-1]}
  - {id: TEST-3, title: leak_counter_uses_acl_json_not_doc_acl, type: cargo-unit, priority: P0, catches: [RISK-2]}
  - {id: TEST-4, title: Ricalcolo indipendente di leaked_chunks e awk sul CSV, type: check-evidenze, priority: P0, catches: [RISK-1, RISK-2]}
  - {id: TEST-5, title: exact_search_binds_hostile_role_string, type: cargo-integration-db, priority: P1, catches: [RISK-3]}
  - {id: TEST-6, title: Nessun format! con SQL e tabella da enum chiuso, scanner verde, type: check-statico, priority: P1, catches: [RISK-3]}
  - {id: TEST-7, title: metrics_known_values, type: cargo-unit, priority: P0, catches: [RISK-4]}
  - {id: TEST-8, title: relevance_rule_exact_partial_and_wrong_page, type: cargo-unit, priority: P0, catches: [RISK-4]}
  - {id: TEST-9, title: Soglie e regola di rilevanza committate prima dei risultati e invariate, type: check-git, priority: P0, catches: [RISK-5]}
  - {id: TEST-10, title: chunk_never_crosses_page_or_section, type: cargo-unit, priority: P1, catches: [RISK-6]}
  - {id: TEST-11, title: query_instruction_per_model_and_flag, type: cargo-unit, priority: P1, catches: [RISK-7]}
  - {id: TEST-12, title: Revisione dei pesi registrata e coincidente con model_sha, type: check-evidenze, priority: P1, catches: [RISK-8]}
  - {id: TEST-13, title: export_candidates_is_deterministic, type: cargo-integration-db, priority: P1, catches: [RISK-9]}
  - {id: TEST-14, title: Candidati con testo solo in results/raw ignorato, type: check-git, priority: P1, catches: [RISK-9]}
  - {id: TEST-15, title: Validità delle evidenze e gate GPU, type: check-evidenze, priority: P1, catches: [RISK-10]}
  - {id: TEST-16, title: Nessuna credenziale negli artefatti, type: check-statico+runtime, priority: P2, catches: [RISK-11]}
  - {id: TEST-17, title: Solo oracoli ratificati nel bench, type: cargo-unit, priority: P2, catches: [RISK-12]}
uncovered: []
```

## Come leggere questo design

Il piano ha due anime: una misura di qualità dei modelli (A-03) e la prima implementazione della ricerca autorizzata, che i piani 01-14 e 01-15 riusano. Il rischio CRITICAL è la seconda: se `exact_search` lascia passare un chunk di un documento non visibile, il leak si propaga a reranker e LLM nelle misure successive. C'è poi un rischio di misura nascosto (RISK-2): il piano dice che `leaked_chunks` è un hard gate, ma non dice con quale verità lo si conta. Se lo si conta interrogando la stessa `doc_acl` che filtra la query, un errore nel popolamento di `doc_acl` produce sia il leak sia il contatore a zero. Il contatore deve usare `acl.json` e la classificazione del manifest, che sono l'oracolo di visibilità del piano 01-03.

Dove vivono i test: unitari nei moduli `#[cfg(test)]` di `spikes/s03-pgvector/src/{chunk,tei,db,metrics}.rs`; quelli marcati «db» sono `#[ignore]` e richiedono `DATABASE_URL` verso il Postgres del compose (profilo `db`), e creano tabelle temporanee proprie con nome univoco (`chunk_test_<uuid>` non è ammesso dall'enum chiuso: i test usano uno schema temporaneo `CREATE SCHEMA test_<pid>` con le stesse tabelle di `sql/schema.sql` e `SET search_path`, poi `DROP SCHEMA … CASCADE`). Si lanciano dal runner: `cargo test -p s03-pgvector -- --ignored <nome>`.

## Test

**TEST-1 — `exact_search_never_returns_invisible_doc_even_if_nearest`** (in `db.rs`, db). Due documenti: `D_pub` (public) e `D_res` (restricted), `doc_acl` popolata come da `acl.json`. Si inseriscono 5 chunk di `D_res` con vettore identico al vettore di query (distanza 0) e 5 chunk di `D_pub` con vettori lontani. `exact_search(query, roles = ["employee"], k = 3)` restituisce 3 chunk tutti di `D_pub`. Con `roles = ["admin"]` i primi 3 sono di `D_res`. È il test che distingue un filtro nella query da un post-filtro: un post-filtro dopo `LIMIT 3` restituirebbe 0 risultati con employee.

**TEST-2 — `exact_search_returns_k_visible_when_invisible_dominate`** (in `db.rs`, db). Come TEST-1 ma con 100 chunk di `D_res` più vicini e 10 di `D_pub`: con employee e k = 10 il risultato ha esattamente 10 chunk, tutti di `D_pub`. Copre il caso in cui il filtro è applicato in una sottoquery limitata.

**TEST-3 — `leak_counter_uses_acl_json_not_doc_acl`** (in `metrics.rs` o nel modulo dove vive il conteggio). La funzione che calcola `leaked_chunks` riceve i risultati (con `doc_key`), il `role_profile` e la mappa di visibilità costruita da `acl.json` + manifest, e non accede al DB. Il test le passa un risultato che contiene un chunk di un documento restricted per il ruolo employee e verifica che il conteggio sia 1. Se nel codice il conteggio è fatto via SQL su `doc_acl`, il test non si può scrivere e questo è il segnale che la misura è circolare.

**TEST-4 — Ricalcolo indipendente.** Dopo il Task 2: `awk -F, 'NR>1 && $8!=0' spikes/s03-pgvector/results/embedding/models.csv` non stampa nulla (colonna `leaked_chunks`). In più, uno script `node -e` inline legge `doc_acl` dal DB (`psql -At -c "select d.doc_key, a.role from doc d join doc_acl a on a.doc_id = d.id"` nel container) e la confronta con la matrice attesa da `acl.json` + classificazioni del manifest: le due relazioni devono coincidere esattamente. Una riga in più in `doc_acl` è un leak latente anche se il CSV dice 0.

**TEST-5 — `exact_search_binds_hostile_role_string`** (in `db.rs`, db). `roles = ["employee' OR '1'='1"]`: la query non fallisce per sintassi e restituisce 0 righe (nessun ruolo con quel nome). Prova che il ruolo passa come parametro.

**TEST-6 — Statico SQL.** `grep -n "format!" spikes/s03-pgvector/src/db.rs` letto dal verificatore: nessuna stringa SQL con interpolazione di valori; il nome della tabella per modello è prodotto da un `match` su un `enum Model { Qwen3, BgeM3, E5 }` che restituisce `&'static str`. Scanner `python skills/security-scanner/security_scan.py spikes/s03-pgvector` exit 0 (regola `sql-format`).

**TEST-7 — `metrics_known_values`** (in `metrics.rs`). Valori calcolati a mano: lista di 10 risultati con rilevanti in posizione 1 e 3, 2 rilevanti totali. recall@5 = 1,0; recall@1 = 0,5; MRR@10 = 1,0; nDCG@10 con guadagno binario = (1 + 1/log2(4)) / (1 + 1/log2(3)) ≈ 0,9197 (tolleranza 1e-4). Caso senza rilevanti nei risultati: tutte le metriche 0, nessuna divisione per zero. Caso query senza rilevanti attesi (no_answer): la funzione la esclude o restituisce un valore documentato, mai NaN.

**TEST-8 — `relevance_rule_exact_partial_and_wrong_page`** (in `metrics.rs`). Regola delle Soglie: (a) chunk che contiene l'excerpt normalizzato, stesso documento e pagina → rilevante; (b) chunk che contiene il 50% dei caratteri dell'excerpt come sottostringa contigua (excerpt a cavallo di due chunk) → rilevante; con il 49% → non rilevante; (c) stesso testo ma pagina diversa → non rilevante; (d) stesso testo in un altro documento → non rilevante; (e) excerpt con spazi multipli e forme NFD → rilevante dopo NFC e collasso.

**TEST-9 — Soglie prima delle misure.** Come per gli altri spike: il commit che aggiunge la sezione «## Soglie (scritte PRIMA della misura)» in `RISULTATI-embedding.md` è antenato (`git merge-base --is-ancestor`) del primo commit che aggiunge un file in `results/embedding/`; la sezione, estratta fino al titolo successivo, è identica fra quel commit e HEAD; contiene la regola di rilevanza, la regola di scelta (nDCG@10, pari sotto 0,02), il limite di calo 0,10, query p95 ≤ 50 ms, 25 pagine p95 ≤ 4 s, VRAM ≤ 2,5 GiB e leak = 0.

**TEST-10 — `chunk_never_crosses_page_or_section`** (in `chunk.rs`). Testo di tre pagine separate da form feed, pagina 2 lunga 4.000 caratteri: ogni chunk ha un solo `page`, `body == testo_pagina[char_start..char_end]`, la sovrapposizione fra chunk consecutivi della stessa pagina è 200 caratteri, nessun chunk supera 1.600 caratteri. Per Markdown: due sezioni `##` adiacenti non finiscono nello stesso chunk e `section_path` riflette l'annidamento (`# A > ## B`). Offset in caratteri, non in byte (test con testo che contiene «à», «€»).

**TEST-11 — `query_instruction_per_model_and_flag`** (in `tei.rs`). Funzione pura che costruisce il corpo della richiesta: per Qwen3 la query contiene l'istruzione (o `prompt_name`), per e5-instruct il prefisso, per bge-m3 nessun prefisso; con il flag disattivato nessun modello ha prefisso; `embed_documents` non aggiunge mai istruzioni; `normalize: true` e `truncate: true` sempre presenti.

**TEST-12 — Revisione dei pesi.** RISULTATI §Setup contiene per ciascuno dei tre modelli uno sha di 40 caratteri esadecimali, e lo stesso valore compare come `model_sha` nella risposta di `/info` riportata in Setup; `hw.json` contiene il digest dell'immagine TEI `86-1.9.4` e del pgvector.

**TEST-13 — `export_candidates_is_deterministic`** (db). Due esecuzioni di `export-candidates --k 20` sullo stesso DB producono file con lo stesso sha256; l'ordinamento a parità di distanza è per `id`. Il manifest versionato registra conteggi e sha256 coincidenti.

**TEST-14 — Candidati non versionati.** `git ls-files spikes/s03-pgvector/results/raw` vuoto; `candidates_manifest.json` contiene solo conteggi, sha256 e metadati (nessuna chiave `body` o `text`: `node -e` che cerca chiavi testuali).

**TEST-15 — Evidenze e gate.** L'ultima riga di `spikes/results/gpu-check.txt` è `VERDETTO: VERDE` (letto, non scritto, da questo piano). Il verify del Task 2 (header esatti, JSON validi) più: `models.csv` ha le combinazioni 3 modelli × {it, en, cross} × {5, 10}; `throughput.csv` ha batch 16/32/64 × max_batch_tokens 4096/8192/16384 per ogni modello con device `gpu`, run 2 e 3, e almeno una riga `cpu` per il vincente; `query_instruction.csv` ha le righe con e senza istruzione per Qwen3 ed e5. I numeri «esito vs soglia» di RISULTATI coincidono con un ricalcolo da CSV (p95 query, VRAM di picco, nDCG per lingua).

**TEST-16 — Credenziali.** `grep -rn "DATABASE_URL" spikes/s03-pgvector/src` solo nella lettura da env; a runtime `grep -rqF "$POSTGRES_PASSWORD" spikes/s03-pgvector docs/adr/0007-*.md` non trova nulla.

**TEST-17 — Solo oracoli ratificati** (in `main.rs` o `metrics.rs`). La funzione che carica `queries.json` scarta le voci senza i tre assi verdi (`claude` valorizzato, `gpt == "CONFIRMED"`, `user == "ratified"`); test con un array di tre voci di cui una pendente: ne restano due. RISULTATI §Anomalie riporta il numero di esclusi.

## Copertura

Il CRITICAL (RISK-1) è coperto da TEST-1, TEST-2 e TEST-4; il contatore indipendente (RISK-2) da TEST-3 e TEST-4. Tutti i HIGH hanno almeno un test. Nota per l'esecutore: il piano non specifica come si calcola `leaked_chunks`; TEST-3 lo vincola a usare `acl.json` + manifest.

**Modifica:** Claude (test designer, Anthropic) · 2026-10-18 · test design 01-10

**Modifica:** Claude (Anthropic) · 2026-10-18 · reviewed_plan.sha256 aggiornato da dc81bf0fa415… a dea5b3be9b95… per la revisione del piano: leaked_chunks contato contro la matrice attesa da acl.json + manifest (expected_visibility), doc_acl confrontata con quella matrice prima di misurare. Il design non è stato rigenerato: va riletto contro la revisione.
