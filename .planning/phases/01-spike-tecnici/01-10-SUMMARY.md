---
phase: 01-spike-tecnici
plan: 10
subsystem: spike-embedding-tei-gpu-ricerca-autorizzata
tags: [s03-pgvector, embedding, bge-m3, qwen3-embedding, multilingual-e5-large-instruct, tei, text-embeddings-inference, cuda, rtx-3050, vram, pgvector, ricerca-esatta, acl, doc-acl, leak, ndcg, recall, mrr, regola-di-rilevanza, istruzione-asimmetrica, chunking, pdftotext, sqlx, unnest, uuid-v8, candidati-top20]

# Dependency graph
requires:
  - phase: 01-01
    provides: "workspace degli spike, crate s03-pgvector con le dipendenze dichiarate (sqlx 0.9, pgvector, reqwest, tokio, clap, serde, uuid), crate common (env, csv, hist, timer, manifest)"
  - phase: 01-02
    provides: "harness Compose con postgres (pgvector 0.8.7-pg18) e tei-embed (TEI 86-1.9.4, EMBED_MODEL/EMBED_REVISION/TEI_MAX_BATCH_TOKENS), runner con pdftotext, session.sh (with, up, down, stop, oneshot, ps --assert-only), hw-manifest.sh, gate GPU"
  - phase: 01-03
    provides: "corpus comune spikes/data/corpus (manifest.json con classificazioni, acl.json con 4 ruoli e il profilo restricted-only, raw/ e synthetic/), documento TTQ der-it-01 da 25 pagine"
  - phase: 01-06
    provides: "oracoli ratificati queries.json (74 su 80 con i tre assi verdi) e regole di normalizzazione di ORACOLI.md"
  - phase: 01-08
    provides: "GPU libera da Docling: le misure di VRAM e latenza GPU valgono con la RTX 3050 non condivisa"
provides:
  - "Libreria s03_pgvector riusata da 01-14 e 01-15: chunk (chunk_pages, chunk_markdown, chunk_corpus, id stabili), tei (documents_body, query_body, Client::embed_documents/embed_query), db (schema, insert UNNEST, exact_search con ACL nella stessa query, check_doc_acl, export_candidates), metrics (load_ratified_queries, expected_visibility, leaked_chunks, relevant_chunks, recall/MRR/nDCG)"
  - "Schema dello spike sql/schema.sql: doc, doc_acl con indice (role, doc_id), chunk_qwen3 / chunk_bgem3 / chunk_e5 con vector(1024) e tsvector generato"
  - "CLI s03-pgvector: load, query-bench [--no-instruction], throughput, export-candidates, relevance-check"
  - "Misura dei tre modelli sulle stesse 60 query valutabili (it 24, en 24, cross 12) con recall@5/@10, MRR@10, nDCG@10 e leak, effetto dell'istruzione, griglia di serving GPU 3 × 3 × 3 e dato CPU di ripiego"
  - "Candidati top-20 autorizzati di bge-m3 rigenerabili in modo deterministico (1.479 candidati, 74 query, leak 0), con manifest versionato senza testo"
  - "Esito in RISULTATI-embedding.md e ADR-0007 Proposed: bge-m3 su TEI 1.9.4 CUDA"
affects: [01-14, 01-15, 01-16, 01-17, ADR-0007, DEPENDENCIES.md «Embedding model», porta Embedder, porta VectorIndex, retrieval autorizzato del Document Service]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Predicato ACL dentro la query del ranking: WHERE EXISTS su doc_acl con role = ANY($2), ORDER BY distanza poi id, LIMIT dopo il filtro"
    - "Verità di visibilità indipendente dal filtro: il leak si conta contro la matrice attesa da acl.json + manifest (expected_visibility), mai contro doc_acl; doc_acl si confronta con quella matrice prima di ogni misura e il comando si ferma su qualunque differenza"
    - "Nome di tabella per modello solo da enum chiuso, con macro che concatena letterali (per_model!): nessun SQL composto a runtime"
    - "Embedding dei documenti e della query separati: l'istruzione è una proprietà del modello che l'adapter applica solo alla query"
    - "Id dei chunk deterministici (UUID v8 da FNV-1a di documento, pagina o sezione, offset) per rigenerare gli stessi candidati in un altro worktree"
    - "Soglie, regola di rilevanza e metodo committati prima di ogni risultato e invariati dopo (b4d3321 antenato di 1cbf84c)"

key-files:
  created:
    - spikes/s03-pgvector/src/lib.rs
    - spikes/s03-pgvector/src/chunk.rs
    - spikes/s03-pgvector/src/tei.rs
    - spikes/s03-pgvector/src/db.rs
    - spikes/s03-pgvector/src/metrics.rs
    - spikes/s03-pgvector/sql/schema.sql
    - spikes/s03-pgvector/RISULTATI-embedding.md
    - spikes/s03-pgvector/results/embedding/hw.json
    - spikes/s03-pgvector/results/embedding/models.csv
    - spikes/s03-pgvector/results/embedding/query_instruction.csv
    - spikes/s03-pgvector/results/embedding/throughput.csv
    - spikes/s03-pgvector/results/embedding/chunks_manifest.json
    - spikes/s03-pgvector/results/embedding/candidates_manifest.json
    - docs/adr/0007-embedding-tei-gpu.md
  modified:
    - spikes/s03-pgvector/src/main.rs

key-decisions:
  - "Decisione utente 2026-10-23, D-38 (01-CONTEXT.md:77, emenda D-14): il modello di embedding è bge-m3 servito da TEI CUDA, non più Qwen3-Embedding-0.6B; ADR-0007 resta Proposed fino a verify"
  - "Configurazione di serving proposta: bge-m3 revisione 5617a9f6…, TEI 86-1.9.4 float16, --max-batch-tokens 16384, lotti client da 64"
  - "Porta Embedder: embed_documents ed embed_query separati, dimensione (1024) e identità del modello (id + revisione) come dati della porta; cambio di modello = reindicizzazione completa"
  - "Istruzione di Qwen3 ed e5 come prefisso esplicito Instruct/Query, non prompt_name di TEI"
  - "section_path dei chunk Markdown senza H1, allineato ai dati di queries.json; oracoli e regola di rilevanza invariati, incoerenza con ORACOLI.md riga 39 registrata per 01-06"
  - "Query irraggiungibili (Q-029, Q-047) escluse dalle medie per tutti e tre i modelli, come scritto nel Metodo prima della misura"

patterns-established:
  - "Un piano che misura un filtro di sicurezza conta le violazioni con una verità calcolata fuori dalla tabella che implementa il filtro"
  - "Le esportazioni destinate ad altri worktree hanno ordine totale nella SQL (distanza, id) e id derivati dal contenuto, così lo sha256 si riproduce"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "Qwen3-Embedding-0.6B, bge-m3 e multilingual-e5-large-instruct sono misurati sulle stesse query etichettate IT/EN/cross-lingual con recall@5/@10, MRR@10 e nDCG@10 per lingua (D-14; criterio 3)"
    verification:
      - {test: TEST-7, risks: [RISK-4], ref: "spikes/s03-pgvector/src/metrics.rs:308#metrics_known_values ; :332#metrics_no_relevant_in_results_are_zero ; :340#metrics_no_answer_query_never_nan", status: pass, evidence: "host: cargo test -p s03-pgvector --locked --offline, rieseguito dal bridge: 20 passed, 5 ignored; i blocchi di test di metrics.rs sono identici fra i test rossi 0df057a e il merge 69af2c9"}
      - {test: TEST-8, risks: [RISK-4], ref: "spikes/s03-pgvector/src/metrics.rs:401#relevance_rule_exact_partial_and_wrong_page ; :448#relevance_rule_normalizes_nfc_and_whitespace ; :472#relevance_rule_markdown_section", status: pass, evidence: "verdi sull'host (bridge)"}
      - {test: TEST-17, risks: [RISK-12], ref: "spikes/s03-pgvector/src/metrics.rs:596#only_ratified_oracles_enter_the_bench ; :615#each_red_axis_excludes_the_oracle ; :650#real_queries_json_keeps_74_of_80", status: pass, evidence: "verdi sull'host; 74 tenute, 6 escluse (Q-008, Q-010, Q-012, Q-040, Q-048, Q-056)"}
      - {test: TEST-10, risks: [RISK-6], ref: "spikes/s03-pgvector/src/chunk.rs:336#chunk_never_crosses_page_or_section ; :394#chunk_markdown_sections_are_separate_and_nested ; :433#stable_id_is_deterministic_and_distinct ; :448#markdown_front_matter_skipped_and_h1_is_title_only", status: pass, evidence: "verdi sull'host; i due test nuovi dell'esecutore sono solo aggiunte (diff dei blocchi di test 0df057a → 69af2c9 senza righe tolte)"}
      - {test: "misure di qualità", ref: "spikes/s03-pgvector/results/embedding/models.csv ; query_instruction.csv", status: pass, evidence: "ricalcolato dal bridge: 24 righe = 3 modelli × {it, en, cross, all} × {5, 10}; la riga all coincide con la media pesata 24/24/12 di it, en, cross per recall, MRR e nDCG su tutti i modelli (scarto ≤ 0,0001 di arrotondamento); nDCG@10 all bge-m3 0,7226, Qwen3 0,6408 (0,6595 senza istruzione), e5 0,6180"}
      - {test: "query irraggiungibili", ref: "spikes/s03-pgvector/RISULTATI-embedding.md:131 ; spikes/s03-pgvector/src/main.rs:473 ; :509-515", status: human_judgment, human_judgment: true, rationale: "l'esclusione di Q-029 e Q-047 è uguale per tutti i modelli per costruzione (evaluable = !no_answer && total > 0, total calcolato sull'intero corpus prima della ricerca), e il conteggio 60 valutabili è nel CSV; ma l'elenco dei due id viene da per_query_*.csv e dall'output di relevance-check, entrambi in results/raw non più disponibile: il bridge non l'ha potuto rileggere"}
  - must_have: "Gli embedding sono serviti da un container TEI CUDA sulla RTX 3050 via passthrough GPU, misurato in throughput, latenza della query e VRAM, dopo il gate GPU verde (D-13)"
    verification:
      - {test: TEST-15, risks: [RISK-10], ref: "spikes/results/gpu-check.txt (ultima riga) ; spikes/s03-pgvector/results/embedding/throughput.csv ; models.csv ; query_instruction.csv", status: pass, evidence: "letto dal bridge: ultima riga «VERDETTO: VERDE»; header esatti dei tre CSV; throughput 27 righe GPU per modello (batch 16/32/64 × max_batch_tokens 4096/8192/16384 × run 1-3) + 3 righe CPU bge-m3; query_instruction con on/off per Qwen3 ed e5"}
      - {test: "M-4, M-5, M-6 ricalcolati", ref: "spikes/s03-pgvector/results/embedding/throughput.csv", status: pass, evidence: "ricalcolato dal bridge con la regola delle Soglie (chunk/s mediano più alto fra le configurazioni con VRAM ≤ 2.560 MiB, run 2 e 3): bge-m3 64/16384, 69,28 chunk/s, 25 pagine p95 1,0174 s, query p95 12,00 ms, VRAM 1.453 MiB; Qwen3 32/8192 30,93 / 2,2484 / 17,10 / 1.662; e5 64/8192 71,31 / 0,9921 / 11,89 / 1.389; griglia GPU VRAM 1.192-2.115, query p95 11,11-27,57 ms, 25 pagine 0,944-2,462 s"}
      - {test: "dato CPU di ripiego", ref: "throughput.csv righe bgem3,cpu", status: pass, evidence: "ricalcolato: run 2 e 3 → 3,03 chunk/s mediani, 25 pagine 24,02 s (massimo) fuori dal TTQ di 15 s; query p95 utile peggiore 49,95 ms (il 50,21 di RISULTATI:107 e i 50 ms dell'ADR includono la run 1 scartata; nessun esito dipende da questo numero)"}
      - {test: "deviazione TEI CPU", ref: "spikes/s03-pgvector/RISULTATI-embedding.md:135", status: human_judgment, human_judgment: true, rationale: "docker run di cka-tei-cpu con etichetta cka.run_id, alias tei-embed e stop con session.sh stop è dichiarato dall'esecutore; i log della sessione non sono versionati e il bridge non lo può osservare. Nessun container residuo è verificabile a posteriori come prova"}
  - must_have: "L'effetto dell'istruzione asimmetrica sulla query è misurato, così la porta Embedder separa embedding dei documenti ed embedding della query (Claude's Discretion S-07)"
    verification:
      - {test: TEST-11, risks: [RISK-7], ref: "spikes/s03-pgvector/src/tei.rs:171#query_instruction_per_model_and_flag ; :222#documents_body_never_has_instruction", status: pass, evidence: "verdi sull'host; blocco di test di tei.rs identico fra 0df057a e 69af2c9"}
      - {test: "M-9", ref: "spikes/s03-pgvector/results/embedding/query_instruction.csv", status: pass, evidence: "ricalcolato: nDCG@10 all e5 0,6180 con / 0,4752 senza = +0,1428; Qwen3 0,6408 / 0,6595 = −0,0187; recall@10 Qwen3 invariato (0,8250), e5 0,8167 / 0,6500"}
  - must_have: "La ricerca usata per le metriche è esatta con il predicato ACL nella stessa query SQL (D-12, D-16)"
    verification:
      - {test: TEST-1, risks: [RISK-1], ref: "spikes/s03-pgvector/src/db.rs:618#exact_search_never_returns_invisible_doc_even_if_nearest", status: pass, evidence: "gate d'onda nel runner su main fuso 69af2c9: cargo test --ignored db::tests 5/5 verdi (dato del gate, non ripetuto dal bridge: richiede il Postgres della sessione)"}
      - {test: TEST-2, risks: [RISK-1], ref: "spikes/s03-pgvector/src/db.rs:659#exact_search_returns_k_visible_when_invisible_dominate", status: pass, evidence: "gate d'onda nel runner, 5/5"}
      - {test: TEST-3, risks: [RISK-2], ref: "spikes/s03-pgvector/src/metrics.rs:513#leak_counter_uses_acl_json_not_doc_acl ; :533#expected_visibility_matches_acl_json_and_manifest ; :563#expected_visibility_deny_by_default_on_unknown_classification ; spikes/s03-pgvector/src/db.rs:435#acl_diff_reports_missing_and_extra_rows", status: pass, evidence: "verdi sull'host; leaked_chunks (metrics.rs:129-135) riceve solo doc_key, ruolo e matrice, nessun accesso al DB"}
      - {test: "TEST-4 (CSV)", risks: [RISK-1, RISK-2], ref: "awk -F, 'NR>1 && $8!=0' spikes/s03-pgvector/results/embedding/models.csv ; candidates_manifest.json leaked_candidates", status: pass, evidence: "rieseguito dal bridge: nessuna riga con leaked_chunks ≠ 0 su 24; leaked_candidates 0 su 1.479"}
      - {test: "TEST-4 (doc_acl reale = matrice attesa)", risks: [RISK-2], ref: "spikes/s03-pgvector/src/db.rs:188-198#check_doc_acl ; spikes/s03-pgvector/src/main.rs:290 ; :441 ; :707", status: pass, evidence: "il confronto è nel codice ed è bloccante: load, query-bench ed export-candidates chiamano check_doc_acl, che fa fallire il comando su qualunque differenza; i CSV e il manifest dei candidati esistono, quindi i comandi sono arrivati in fondo. Il bridge non ha rieseguito la lettura psql (DB della sessione non più attivo)"}
      - {test: TEST-5, risks: [RISK-3], ref: "spikes/s03-pgvector/src/db.rs:690#exact_search_binds_hostile_role_string", status: pass, evidence: "gate d'onda nel runner, 5/5"}
      - {test: TEST-6, risks: [RISK-3], ref: "spikes/s03-pgvector/src/db.rs:609#model_table_comes_from_closed_enum ; spikes/s03-pgvector/src/db.rs:23-31#per_model ; grep -n 'format!' spikes/s03-pgvector/src/db.rs", status: pass, evidence: "verificato dal bridge: format! in db.rs solo in messaggi d'errore (:133, :178, :181, :404) e nei test per lo schema temporaneo test_<pid>_<uuid> (:497-533); nessun SQL con valori in ingresso; scanner --fail-on HIGH su spikes/s03-pgvector: 0 problemi su 12 file, exit 0"}
      - {test: TEST-13, risks: [RISK-9], ref: "spikes/s03-pgvector/src/db.rs:707#exact_search_ties_are_ordered_by_id ; :736#export_candidates_is_deterministic", status: pass, evidence: "gate d'onda nel runner, 5/5; ORDER BY c.emb <=> $1, c.id nella SQL (db.rs:290)"}
      - {test: "TEST-13 (due esportazioni reali, stesso sha256)", risks: [RISK-9], ref: "spikes/s03-pgvector/RISULTATI-embedding.md:75 ; results/raw/candidates_top20.jsonl (ignorato)", status: human_judgment, human_judgment: true, rationale: "lo sha256 8f507d07… è nel manifest, ma la seconda esportazione e il suo hash stanno solo nel log dell'esecutore; results/raw non è più disponibile"}
  - must_have: "Il modello di embedding e il suo serving sono decisi in ADR-0007 con misure e hardware"
    verification:
      - {test: TEST-9, risks: [RISK-5], ref: "git merge-base --is-ancestor b4d3321 1cbf84c ; diff della sezione «## Soglie (scritte PRIMA della misura)» fra b4d3321 e HEAD", status: pass, evidence: "rieseguito dal bridge: il primo commit che aggiunge file in results/embedding/ è 1cbf84c e b4d3321 ne è antenato; righe 1-53 di RISULTATI-embedding.md (Domanda, Soglie, Regola di rilevanza, Metodo) identiche fra b4d3321 e HEAD"}
      - {test: "M-1, M-2, M-3 ricalcolati", ref: "spikes/s03-pgvector/results/embedding/models.csv", status: pass, evidence: "ricalcolato: bge-m3 − Qwen3 con istruzione 0,0818, − Qwen3 senza 0,0631, − e5 0,1046 (≥ 0,02, nessun pari); M-2 bge-m3 en − it 0,0314; M-3 bge-m3 all − cross −0,0742; e5 all − cross 0,1999 (fallirebbe M-3)"}
      - {test: "M-7, M-8", ref: "models.csv colonna leaked_chunks ; candidates_manifest.json", status: pass, evidence: "0 su tutte le righe e su 1.479 candidati; M-8 per costruzione (vedi TEST-4)"}
      - {test: "esito e decisione", ref: "spikes/s03-pgvector/RISULTATI-embedding.md#Esito ; docs/adr/0007-embedding-tei-gpu.md ; .planning/phases/01-spike-tecnici/01-CONTEXT.md:77#D-38", status: pass, evidence: "ADR-0007 Stato Proposed con link a RISULTATI-embedding.md e hw.json (file esistenti); D-38 registrata dall'utente il 2026-10-23; DEPENDENCIES.md non toccato (nessun file fuori dai 15 del piano nel diff del merge)"}
      - {test: TEST-12, risks: [RISK-8], ref: "spikes/s03-pgvector/RISULTATI-embedding.md:64-68 ; results/embedding/chunks_manifest.json loads.*.tei_model_sha ; candidates_manifest.json tei_model_sha ; results/embedding/hw.json images", status: pass, evidence: "letto dal bridge: tre sha di 40 caratteri esadecimali; il tei_model_sha letto da /info al momento del load (main.rs:321-322) coincide con la revisione di RISULTATI per tutti e tre i modelli; hw.json ha i digest di pgvector 0.8.7-pg18-trixie e di TEI 86-1.9.4 e tools.tei 1.9.4; sha256 di hw.json = 066e65a8… come in RISULTATI:60"}
      - {test: "TEST-12 (a ogni avvio di tei-embed)", risks: [RISK-8], ref: "results/raw/info/*.json (ignorati)", status: human_judgment, human_judgment: true, rationale: "le copie di /info dei riavvii per la griglia di throughput non sono rileggibili; resta la prova al load e all'export"}
  - must_have: "Nessun segreto nel repo; i candidati con testo restano fuori da git (T-01-10-03, T-01-10-05)"
    verification:
      - {test: TEST-14, risks: [RISK-9], ref: "git ls-files spikes/s03-pgvector/results/raw ; spikes/.gitignore:4 ; candidates_manifest.json", status: pass, evidence: "rieseguito: nessun file tracciato in results/raw (regola **/results/raw/); il manifest ha solo chiavi di conteggio, hash e metadati (nessuna chiave body o text; «body» compare solo come nome di campo nella descrizione del file grezzo)"}
      - {test: TEST-16, risks: [RISK-11], ref: "grep -n DATABASE_URL spikes/s03-pgvector/src ; valori del file segreti contro git grep HEAD ; $HOME/.cka-spikes/", status: pass, evidence: "rieseguito dal bridge: DATABASE_URL letta solo da env (db.rs:62, test :485); i valori delle 7 chiavi segrete del file $HOME/.cka-spikes/spikes.env non compaiono in alcun file tracciato a HEAD; la copia derivata spikes-01-10.env non esiste più; nel repo ne compare solo il percorso (RISULTATI:134)"}
  - must_have: "Gate d'onda"
    verification:
      - {test: "fmt, clippy, test, perimetro, scanner", ref: "gate d'onda dell'orchestratore ; node scripts/gate-proprieta-file.cjs ; python skills/security-scanner/security_scan.py --fail-on HIGH spikes/s03-pgvector", status: pass, evidence: "gate d'onda: fmt e clippy verdi sull'host, s03-pgvector 20 unit verdi + 5 db ignorati, db nel runner 5/5; rieseguiti dal bridge: unit 20/20 sull'host, gate-proprieta-file «esito: conforme», scanner exit 0"}

# Metrics
duration: "circa 1 h 10 min il 2026-10-22: test rossi 0df057a 21:31 → misure 1cbf84c 22:39; merge su main 69af2c9 22:41"
completed: 2026-10-22
---

# Fase 1 Piano 10: S-03/A-03, modello di embedding e serving TEI su GPU · Summary

**bge-m3 vince sulle 60 query valutabili con nDCG@10 complessivo 0,7226, contro 0,6408 di Qwen3-Embedding-0.6B (0,6595 senza istruzione) e 0,6180 di multilingual-e5-large-instruct, e passa tutte le soglie M-1…M-8 servito da TEI 1.9.4 CUDA sulla RTX 3050: query p95 12 ms, documento da 25 pagine in 1,02 s, 1.453 MiB di VRAM, leak 0 su 74 query e 1.479 candidati con il predicato ACL nella stessa query della ricerca esatta. L'istruzione di query vale +0,143 su e5 e −0,019 su Qwen3, quindi la porta `Embedder` separa documenti e query. Su CPU il documento da 25 pagine costa 24 s, fuori dal TTQ. L'utente ha registrato la scelta come D-38; ADR-0007 resta Proposed fino a verify**

## Performance

- **Duration:** circa 1 h 10 min. Test rossi `0df057a` (21:31:56), Task 1 `b4d3321` (21:45:56), diagnostica `bc3e0ab` (21:48:44), misure ed esito `1cbf84c` (22:39:28), merge `69af2c9` (22:41:31).
- **Started:** 2026-10-22T21:31:56+02:00
- **Completed:** 2026-10-22T22:41:31+02:00
- **Tasks:** 2 di 2
- **Files modified:** 15 su 15 `files_modified` del piano (`git diff --name-only 69af2c9^1 69af2c9`, confrontato con il frontmatter del PLAN: identici). Nessun file di altri piani; `Cargo.toml` del crate, oracoli e `DEPENDENCIES.md` non toccati.

## Accomplishments

- **Chunker di misura** (`spikes/s03-pgvector/src/chunk.rs`): finestre da 1.600 caratteri con 200 di sovrapposizione (`chunk.rs:85-100`), mai a cavallo di pagina (`:124-140`) o sezione (`:178-232`), offset in caratteri, id stabili UUID v8 (`:63-80`), testo PDF da `pdftotext -layout` con lettura tollerante (`:268-283`).
- **Client TEI** (`spikes/s03-pgvector/src/tei.rs`): istruzione solo sulla query e solo per Qwen3 ed e5 (`tei.rs:21-26`), corpi `documents_body` e `query_body` (`:29-40`), `embed_documents` a lotti ed `embed_query` separati (`:114-140`), controllo della dimensione 1024 (`:107-109`).
- **Accesso al DB** (`spikes/s03-pgvector/src/db.rs`): tabella da enum chiuso con `per_model!` (`db.rs:23-31`, `:56-58`), confronto bloccante di `doc_acl` con la matrice attesa (`:166-198`), insert a lotti di 500 con `UNNEST` (`:222-273`), `exact_search` con `EXISTS` su `doc_acl` nella stessa query e ordine `distanza, id` (`:278-309`), esportazione dei candidati (`:389-426`).
- **Metriche** (`spikes/s03-pgvector/src/metrics.rs`): solo oracoli ratificati (`metrics.rs:57-82`), matrice attesa deny-by-default (`:107-125`), leak contro quella matrice (`:129-135`), normalizzazione di ORACOLI.md (`:142-195`), regola di rilevanza (`:217-259`), recall, MRR e nDCG a guadagno binario (`:262-296`).
- **CLI** (`spikes/s03-pgvector/src/main.rs`): `load` (`main.rs:277-329`), `query-bench` (`:431-600`), `throughput` (`:626-693`), `export-candidates` con controllo del leak prima di scrivere (`:697-772`), `relevance-check` (`:169-192`).
- **Evidenze e decisione**: `results/embedding/` (tre CSV, due manifest, `hw.json`), `spikes/s03-pgvector/RISULTATI-embedding.md`, `docs/adr/0007-embedding-tei-gpu.md`.

## Task Commits

1. **Test rossi (agente test-write, distinto dall'esecutore):** `0df057a` (test). RED_EXPECTED: 17 unit rossi su `todo!()`, 5 db ignorati.
2. **Task 1: soglie, libreria s03 e CLI:** `b4d3321` (feat).
3. **Task 1, diagnostica e lettura tollerante di `pdftotext`:** `bc3e0ab` (feat).
4. **Task 2: misure dei tre modelli su GPU, dato CPU, candidati, esito e ADR-0007:** `1cbf84c` (feat).

**Merge:** `69af2c9`. **Plan metadata:** questo SUMMARY e `01-10-RACCONTO.md`, scritti dal bridge senza commit.

## Files Created/Modified

- `spikes/s03-pgvector/src/lib.rs`: moduli `chunk`, `tei`, `db`, `metrics` ed enum `Model` (creato dal test-write, invariato dall'esecutore).
- `spikes/s03-pgvector/src/chunk.rs`: chunker; TEST-10.
- `spikes/s03-pgvector/src/tei.rs`: client TEI; TEST-11.
- `spikes/s03-pgvector/src/db.rs`: schema, insert, ricerca autorizzata, candidati; TEST-1, 2, 5, 6, 13.
- `spikes/s03-pgvector/src/metrics.rs`: oracoli, visibilità, leak, rilevanza, metriche; TEST-3, 7, 8, 17.
- `spikes/s03-pgvector/src/main.rs`: CLI di misura.
- `spikes/s03-pgvector/sql/schema.sql`: `doc`, `doc_acl`, `chunk_qwen3`, `chunk_bgem3`, `chunk_e5`.
- `spikes/s03-pgvector/results/embedding/*` (6 file), `spikes/s03-pgvector/RISULTATI-embedding.md`, `docs/adr/0007-embedding-tei-gpu.md`.

## Esiti delle soglie

Modello scelto bge-m3, configurazione 64 / 16384. Tutti i valori ricalcolati dal bridge dai CSV committati.

| Id | Misura | Valore | Soglia | Esito |
|----|--------|--------|--------|-------|
| M-1 | nDCG@10 complessivo | bge-m3 0,7226; Qwen3 0,6408 (0,6595 senza istruzione); e5 0,6180 | il più alto, pari sotto 0,02 | raggiunta (distacco 0,063 dalla migliore variante di Qwen3) |
| M-2 | en − it, bge-m3 | 0,031 | ≤ 0,10 | raggiunta |
| M-3 | all − cross, bge-m3 | −0,074 (il cross-lingual va meglio della media) | ≤ 0,10 | raggiunta |
| M-4 | query p95, GPU | 12,00 ms | ≤ 50 ms | raggiunta |
| M-5 | documento da 25 pagine (69 chunk) p95, GPU | 1,017 s | ≤ 4 s | raggiunta |
| M-6 | VRAM di picco sopra il riposo | 1.453 MiB | ≤ 2.560 MiB | raggiunta |
| M-7 | leak nei top-k | 0 su 24 righe e su 1.479 candidati | = 0 | raggiunta |
| M-8 | `doc_acl` = matrice attesa | nessuna differenza (il comando si sarebbe fermato) | nessuna differenza | raggiunta |
| M-9 | effetto dell'istruzione | e5 +0,143; Qwen3 −0,019 | riportato | — |
| M-10 | throughput | GPU 69,28 chunk/s; CPU 3,03 chunk/s, 25 pagine in 24,02 s | riportato | — |

Per gli altri modelli M-4…M-7 passano; M-3 fallirebbe per e5 (0,200), che pure è il migliore sull'italiano (0,7157 contro 0,6884). Il campione è piccolo (60 query valutabili, 12 cross-lingual) e le differenze non hanno intervallo di confidenza: lo dichiarano RISULTATI e l'ADR.

## Decisions Made

### Decisione dell'utente (2026-10-23, già in `01-CONTEXT.md`)

- **D-38** (`01-CONTEXT.md:77`, con la *Storia* di D-14 a `:44`): il modello di embedding è bge-m3 (MIT) servito da TEI CUDA, non più Qwen3-Embedding-0.6B. Emenda D-14; ADR-0007 resta `Proposed` fino a verify, e la riga `DEPENDENCIES.md` «Embedding model» la scrive il consolidamento (01-17).

### Scelte dell'esecutore

- **Istruzione come prefisso esplicito** `Instruct: …\nQuery: …` per Qwen3 ed e5 (`tei.rs:17-26`), invece del `prompt_name` di TEI: non dipende dai nomi di prompt del modello e rende identica la richiesta fra i due. Il compito è quello delle model card. Varianti del prompt di Qwen3 non provate (RISULTATI:141).
- **`section_path` senza H1** (`chunk.rs:159-174`): i dati di `queries.json` omettono il titolo del documento, ORACOLI.md riga 39 dice «come compaiono nel file». Il chunker si allinea ai dati; oracoli e regola di rilevanza non sono toccati, l'incoerenza va a 01-06.
- **Normalizzazione senza togliere i bordi** (`metrics.rs:137-142`): ORACOLI.md non prevede il trim, e con il trim il caso (b) di TEST-8 cadeva per lo spazio finale dell'excerpt.
- **Query irraggiungibili escluse e contate**, come scritto nel Metodo prima della misura (RISULTATI:47, `main.rs:473`).
- **Id dei chunk deterministici** FNV-1a → UUID v8 (`chunk.rs:63-80`) invece di UUID v4, perché 01-15 rigeneri i candidati con lo stesso sha256.
- **sha256 con `sha256sum`** in sottoprocesso (`main.rs:201-215`): `Cargo.toml` del crate è fuori dai file del piano e `sha2` non è fra le dipendenze.
- **Binario compilato una volta per sessione** invece di `cargo run` per ogni sottocomando (RISULTATI:136).

## Deviations from Plan

### Livello a (dentro il piano)

1. **Parametri di `tei-embed` per modello con una copia derivata del file segreti.** Dalla gap-closure 1.1 `session.sh` toglie a compose le variabili dell'host omonime di chiavi del file segreti, e `EMBED_MODEL`, `EMBED_REVISION`, `TEI_MAX_BATCH_TOKENS` lo sono: il comando del piano non aveva effetto. L'esecutore ha puntato `CKA_SPIKES_ENV` a `$HOME/.cka-spikes/spikes-01-10.env`, copia fuori dal repo con le sole righe TEI sostituite, cancellata a fine lavoro (RISULTATI:134). **Verificato dal bridge:** il file non esiste più; i valori delle chiavi segrete non compaiono in alcun file tracciato; il `tei_model_sha` registrato al load coincide con la revisione voluta per tutti e tre i modelli.
2. **`hw.json` normalizzato** allo schema di `common::manifest`, come in 01-04, 01-05, 01-07, 01-08, 01-11 e 01-12; originale in `results/raw/embedding-hw-raw.json` (RISULTATI:133). Il difetto è in file chiusi di 01-01 e 01-02: registrato, non corretto.
3. **`pdftotext` dell'host lossy**: xpdf 4.00 sull'host produceva byte non UTF-8; la lettura sostituisce le sequenze invalide con U+FFFD (`chunk.rs:280-282`, `bc3e0ab`). Le misure usano poppler 25.03.0 del runner.
4. **TEI CPU via `docker run`** con etichetta `cka.run_id` della sessione e alias di rete `tei-embed`, dopo aver fermato il `tei-embed` GPU, fermato con `session.sh stop` (RISULTATI:135). Non osservabile a posteriori dal bridge.
5. **Tre sessioni**: la prima fermata sull'`hw.json`, la seconda per i tre modelli, la terza per candidati e CPU; il processo agente della seconda chiuso per pressione di memoria, la sessione chiusa con `down` + `release` regolari (RISULTATI:143).

### Livello b

Nessuna. Una segnalazione per un file posseduto da altri: l'incoerenza fra ORACOLI.md riga 39 e `queries.json` sul `section_path` è registrata per 01-06; gli oracoli sono invariati (nessun diff su `spikes/data/corpus/oracles/` da `69af2c9^1` a HEAD).

---

**Total deviations:** 5 di livello a, nessuna di livello b o c. Nessun test tolto o indebolito: i blocchi di test di `metrics.rs` e `tei.rs` sono identici fra `0df057a` e `69af2c9`, quelli di `chunk.rs` e `db.rs` hanno solo tre test aggiunti (`stable_id_is_deterministic_and_distinct`, `markdown_front_matter_skipped_and_h1_is_title_only`, `acl_diff_reports_missing_and_extra_rows`).
**Impact on plan:** nessuna estensione di perimetro (15 file su 15).

## Discrepanze fra rapporto, RISULTATI e prove

- **Query p95 su CPU**: RISULTATI:107 riporta 50,21 ms e l'ADR 50 ms, valore della run 1, che il Metodo scarta; il peggiore delle run utili è 49,95 ms. Il dato CPU non è giudicato contro M-4: nessun esito cambia.
- **«Distacco 0,063»** (RISULTATI:115, ADR:43) è misurato contro Qwen3 *senza* istruzione (0,6595), la sua variante migliore; contro la configurazione di riferimento di `models.csv` (con istruzione) è 0,082. Entrambi sopra il margine di pari.
- **D-38 parla di «72 query»** (`01-CONTEXT.md:77`); le prove dicono 74 ratificate, di cui 60 valutabili, 12 `no_answer` e 2 irraggiungibili. Il numero di D-38 non trova riscontro nei CSV; da correggere alla prossima revisione del CONTEXT (documento dell'utente, non toccato dal bridge).
- **Evidenze grezze non rileggibili**: `results/raw` è ignorata e il worktree di 01-10 è stato rimosso. Il bridge non ha potuto rileggere le copie di `/info` dei singoli riavvii, i `per_query_*.csv` con gli id delle irraggiungibili, la seconda esportazione dei candidati, l'`hw.json` originale. Le voci corrispondenti sono `human_judgment` in coverage.
- Il resto coincide: commit e ordine, 15 file, invarianza di Domanda, Soglie, Regola e Metodo dopo `b4d3321`, conteggio dei test (17 + 3 unit, 5 db), valori di M-1…M-10 dai CSV, header, sha256 di `hw.json`, digest delle immagini, revisioni dei modelli nei manifest.

## Scanner di sicurezza

`python skills/security-scanner/security_scan.py --fail-on HIGH spikes/s03-pgvector`, rieseguito dal bridge: «nessun problema di sicurezza rilevato su 12 file», exit 0.

## Threat Flags

- **T-01-10-01** (fuga di chunk): predicato ACL nella stessa query (`db.rs:285-292`); `doc_acl` confrontata con la matrice attesa prima di ogni misura (`db.rs:188-198`); leak contato contro la matrice e non contro `doc_acl` (`metrics.rs:129-135`); 0 su 74 query e 1.479 candidati; l'esportazione rifiuta di scrivere se trova un leak (`main.rs:738-740`).
- **T-01-10-02** (SQL): solo `bind`, tabella da enum chiuso, scanner verde.
- **T-01-10-03** (candidati): testo solo in `results/raw/` ignorata; manifest con conteggi e sha256.
- **T-01-10-04** (pesi): revisione fissata e verificata contro `/info`, registrata nei manifest; digest delle immagini in `hw.json`.
- **T-01-10-05** (credenziali): `DATABASE_URL` solo da env e mai stampata; nessun valore segreto nel repo; copia derivata del file segreti cancellata.

## Known Stubs

Nessuno. Il chunker è di misura per scelta del piano: quello di prodotto non è materia di questo spike.

## User Setup Required

Nessuno.

## Punti aperti

- **ADR-0007 da `Proposed` ad `Accepted`** e riga «Embedding model» di `DEPENDENCIES.md` da Aperta a Decisa: consolidamento 01-17, dopo verify.
- **Query irraggiungibili** (Q-029, Q-047): `pdftotext -layout` non conserva come sottostringa contigua gli excerpt di pagine a riquadri e colonne. È un limite del chunker di misura, coerente con l'esito di S-02 sull'ordine di lettura (01-08): il parser di prodotto è materia dell'ADR-0004.
- **ORACOLI.md riga 39 contro `queries.json`** sul `section_path` dei Markdown: da riallineare in 01-06 (documento) senza cambiare gli oracoli.
- **Serving su CPU fuori TTQ** (24 s per 25 pagine): la GPU è requisito per il budget di ingestion.
- **Campione piccolo**: 60 query valutabili, 12 cross-lingual; il vantaggio di bge-m3 regge in inglese e cross-lingual, in italiano e5 è avanti di 0,027.
- **`hw-manifest.sh` / `common::manifest`**: difetto noto, fase decimale.
- **Numero di query in D-38** (72 invece di 74 ratificate / 60 valutabili): da correggere nel CONTEXT.

## Next Phase Readiness

- **01-14 (indice)** consuma senza modifiche `s03_pgvector::chunk`, `tei`, `db::exact_search`, `metrics` e le tabelle di `sql/schema.sql`; la ricerca esatta di questo piano è il riferimento contro cui misurare l'HNSW.
- **01-15 (reranker)** rigenera i candidati top-20 di bge-m3 con `export-candidates --model bgem3 --k 20` e li confronta con lo sha256 di `candidates_manifest.json`.
- **01-16 (S-07)** trascrive `Embedder` con `embed_documents` ed `embed_query` separati e dimensione e identità del modello come dati.

## Self-Check: PASSED

- 15 file su 15 dei `files_modified` nel diff del merge `69af2c9`, nessun file di altri piani.
- Rieseguiti dal bridge: ordine dei commit con `git merge-base`, invarianza delle righe 1-53 di RISULTATI, diff dei blocchi di test fra test rossi e merge, `cargo test -p s03-pgvector` sull'host (20 verdi, 5 ignorati), `gate-proprieta-file` conforme, scanner exit 0, ricalcolo di M-1…M-10 e delle righe `all` dai CSV, header, sha256 di `hw.json`, assenza della copia derivata dei segreti e dei valori segreti nel repo.
- Test db: gate d'onda nel runner su main fuso `69af2c9`, `--ignored db::tests` 5/5, non ripetuto dal bridge.

**Modifica:** Claude (Anthropic) · 2026-10-23 · creazione (bridge di 01-10)
