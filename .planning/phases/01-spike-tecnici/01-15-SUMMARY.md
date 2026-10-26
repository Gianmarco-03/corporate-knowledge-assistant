---
phase: 01-spike-tecnici
plan: 15
subsystem: spike-reranker-baseline-budget-vram-adr-0009
tags: [s04-jev, reranking, cross-encoder, bge-reranker-v2-m3, tei, tei-rerank, no-rerank, jev, forma-a, ndcg, recall, p95, assert-visible, no-leak, sha256, accept-sha, vram, nvidia-smi, max-batch-tokens, docling-gpu, tei-embed, rtx-3050, adr-0009, session-sh]

# Dependency graph
requires:
  - phase: 01-11
    provides: "libreria s04_jev (candidates::{Acl, Passage, Query, assert_visible, is_green}, jev::Client::{new_from_env, rank_form_a, unauthorized_sent, spent_usd}, metrics::{ndcg_at_k, recall_at_k}), forma A a concorrenza 16 e pin jev-1.13.0 di ADR-0008, usata senza modificarla"
  - phase: 01-10
    provides: "export-candidates di s03-pgvector (top-20 autorizzati per query, bge-m3, ricerca esatta), candidates_manifest.json con lo sha256 atteso, revisione di bge-m3 e sottocomando throughput per il carico di embedding"
  - phase: 01-08
    provides: "docling-client e Docling GPU (docling-serve-cu128 v1.37.0) per il carico di conversione di una pagina-tabella"
  - phase: 01-13
    provides: "GPU libera da OCR e Docling durante le misure (ordine di 01-ONDE.md)"
  - phase: 01-14
    provides: "chunk_bgem3 senza indice HNSW: l'export dei candidati resta esatto e deterministico (sha uguale al manifest)"
  - phase: 01-02
    provides: "harness Compose (tei-embed, tei-rerank, docling, runner), session.sh (with, up, oneshot, ps --assert-only, cid, stop, down per etichetta), hw-manifest.sh, file dei segreti CKA_SPIKES_ENV"
provides:
  - "Bin rerank (spikes/s04-jev/src/bin/rerank/): metodi none, tei e jev con guardia assert_visible prima di ogni invio per tutti i metodi, verifica sha256 dei candidati con --accept-sha solo uguale allo sha calcolato, CSV rerank.csv e tabella riassuntiva; 24 test (13 unitari, 11 ignorati)"
  - "Client TEI /rerank (tei_rerank.rs) con mapping index → candidato che rifiuta risposte incomplete, indici ripetuti o fuori intervallo e punteggi non finiti; GET /info per il pin dei pesi"
  - "vram.sh: fasi desktop, warm per servizio, load concorrente di 180 s e after, servizi solo via session.sh, quota per servizio attribuita per avvio successivo, oom dai log e dallo stato dei container"
  - "Soglie R-1..R-4 e Metodo committati prima di ogni misura (4407e74), invariati fino a HEAD"
  - "Misure: rerank.csv (887 righe), vram.csv (593 righe, configurazione b16384-8192), hw.json della sessione di rerank"
  - "RISULTATI-rerank.md, RISULTATI.md (indice ed Esito di S-04), ADR-0009 Proposed"
affects: [01-16, 01-17, ADR-0009, ADR-0002, ADR-0008, DEPENDENCIES.md riga «Reranker baseline», porta Reranker (S-07), DecisionEngine, deployment GPU locale (tei-embed + docling), threat model T-14, T-24]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Guardia di visibilità sull'intero insieme per ogni metodo, anche quello senza invio, più riconteggio indipendente sul punto di invio (bin per TEI, client di 01-11 per JEV)"
    - "Risposta di un riordinatore ricollegata per indice ai candidati inviati, con rifiuto di ogni risposta che non sia una permutazione completa; pareggi sciolti dall'ordine denso"
    - "File di input derivato verificato contro lo sha256 del manifest del piano che lo produce; l'unica deroga è un --accept-sha uguale allo sha calcolato, da registrare in §Anomalie"
    - "VRAM per servizio su WSL2 attribuita per avvio successivo (picco − minimo della fase warm), totale letto da nvidia-smi sull'host desktop compreso"

key-files:
  created:
    - spikes/s04-jev/vram.sh
    - spikes/s04-jev/RISULTATI-rerank.md
    - spikes/s04-jev/RISULTATI.md
    - docs/adr/0009-reranker-baseline.md
    - spikes/s04-jev/results/rerank/hw.json
    - spikes/s04-jev/results/rerank/rerank.csv
    - spikes/s04-jev/results/rerank/vram.csv
  modified:
    - spikes/s04-jev/src/bin/rerank/main.rs
    - spikes/s04-jev/src/bin/rerank/tei_rerank.rs

key-decisions:
  - "Proposta in ADR-0009 (Proposed, ratifica nel consolidamento 01-17, Task 2): baseline di reranking = no-rerank (adapter identità della porta Reranker), per la regola R-1 scritta prima della misura"
  - "bge-reranker-v2-m3 su TEI GPU: nDCG@5 0,7338 (+0,1561 sul no-rerank, margine richiesto +0,02) ma p95 360 ms contro un tetto di 300 ms: le due condizioni erano congiunte, quindi non adottato"
  - "JEV forma A resta il rank di prodotto (ADR-0002): +0,1872 di nDCG@5 e +0,0541 di recall@8 sulla baseline, p95 483 ms, 0,130760 USD per 4.437 richieste"
  - "Budget VRAM 5,5 GiB totali con il desktop: tre servizi sotto carico a 5.562 MiB senza OOM con i default dei batch (16384 / 8192); nell'MVP sulla GPU restano tei-embed e docling, quota stimata circa 2,8 GiB"
  - "Esecutore di ripresa: vram.csv lasciato dal primo esecutore giudicato non tracciabile e spostato in una cartella raw ignorata; rimisurata la sola configurazione b16384-8192 nella sessione 01-15-20261026t100823z-f6a0 (git_rev 86353bc)"

patterns-established:
  - "Una misura si lega al codice con run_id e git_rev registrati prima della sessione; una serie senza questi metadati non è evidenza, anche se i numeri sono plausibili"
  - "Una soglia congiunta (qualità e latenza) si applica come scritta: un guadagno di qualità grande non compensa un tetto di latenza mancato"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "No-rerank e bge-reranker-v2-m3 su GPU in container TEI sono misurati contro JEV rank sugli stessi candidati top-20 autorizzati, con nDCG@5, recall@8 e latenza p95 (D-21; criterio 4: baseline di reranking scelta)"
    verification:
      - {test: TEST-6, risks: [RISK-4], ref: "spikes/s04-jev/src/bin/rerank/tei_rerank.rs:270#tei_rerank_maps_index_to_candidate ; :290#tei_rerank_rejects_out_of_range_index ; :300#tei_rerank_rejects_incomplete_response ; :315#tei_rerank_request_body_shape ; :329#tei_rerank_posts_rerank_and_maps_reply", status: pass, evidence: "rieseguiti dal bridge sull'host sul main fuso (cargo test -p s04-jev --locked --offline --bin rerank: 13 passati, 11 ignorati); ranked_ids a tei_rerank.rs:135-163 ordina per punteggio con pareggi per indice e rifiuta risposte incomplete, indici ripetuti o fuori intervallo e punteggi non finiti (scelta P-5: errore, mai completamento)"}
      - {test: TEST-7, risks: [RISK-4, RISK-9], ref: "spikes/s04-jev/src/bin/rerank/main.rs:1413#test7_metrics_recomputed_match_results ; spikes/s04-jev/results/rerank/rerank.csv ; spikes/s04-jev/RISULTATI-rerank.md:71-78", status: pass, evidence: "test ignorato rieseguito dal bridge (verde); ricalcolo indipendente in Python sulle ripetizioni 2-3, p95 a rango più vicino: none 0,5777 / 0,7432 / 0 ms (148 righe); tei gpu 0,7338 / 0,7838, p50 287, p95 360, p99 432, massimo 435 ms (148); tei cpu 0,7319 / 0,7823, p95 32.033 ms (147); jev 0,7649 / 0,7973, p95 483, massimo 630 ms (148). Tutti coincidono con la tabella; R-1: +0,1561 ≥ 0,02 ma 360 > 300, baseline no-rerank coerente con la regola"}
      - {test: TEST-13, risks: [RISK-9], ref: "spikes/s04-jev/src/bin/rerank/main.rs:1588#test13_evidence_and_links", status: pass, evidence: "rieseguito dal bridge (verde); ricontato: 887 righe = 74 query × 3 ripetizioni × 4 metodi/device − 1 (Q-042 run 3 su CPU); metodi none, tei, jev presenti; model_resolved = jev-1.13.0 su tutte le 222 righe jev; candidates = 20 su 73 query e 19 su Q-075 (P-4 accettato dal coordinatore: test con candidates in 1..=20, costante per query); somma candidates delle righe jev = 4.437 = 1.479 × 3"}
      - {test: TEST-11, risks: [RISK-7], ref: "spikes/s04-jev/src/bin/rerank/main.rs:1528#test11_reranker_revision_and_hw_manifest ; spikes/s04-jev/results/rerank/hw.json", status: pass, evidence: "rieseguito dal bridge (verde); RISULTATI-rerank.md:50 riporta la revisione 953dc6f6f85a1b2dbfca4c34a2796e7dde08d41e e lo stesso model_sha di /info; hw.json ha tools.reranker BAAI/bge-reranker-v2-m3, reranker_revision uguale, jev_model jev-1.13.0, digest TEI 86-1.9.4, cpu-1.9 e docling-serve-cu128"}
      - {test: "legame misura-codice della sessione di rerank", risks: [RISK-9], ref: "spikes/s04-jev/results/rerank/hw.json (git_rev, run_id, timestamp_utc)", status: pass, evidence: "hw.json porta run_id 01-15-20261025t182756z-d090 e git_rev d530189, timestamp 18:28:21Z, dopo il commit d530189 (18:27:10Z); git diff d530189 exec/01-15 su src e vram.sh è vuoto, quindi il codice misurato è quello di HEAD (a differenza di 01-14). «Compiling s04-jev» e la spesa 0,130760 USD stanno nel log m1 della sessione, in results/raw del worktree, ignorato da git: letto dal bridge, non versionato"}
  - must_have: "La convivenza in 6 GB di VRAM di TEI embedding, TEI reranker e Docling GPU è misurata sotto carico concorrente, e il budget VRAM è deciso (Claude's Discretion; D-13, D-21)"
    verification:
      - {test: TEST-10, risks: [RISK-6], ref: "spikes/s04-jev/src/bin/rerank/main.rs:1462#test10_vram_three_services_under_load ; spikes/s04-jev/results/rerank/vram.csv", status: pass, evidence: "rieseguito dal bridge (verde); ricalcolato: 593 righe, oom false su tutte, una sola configurazione 16384/8192; desktop 1.607-1.626 MiB; quote 1.619 / 1.479 / 1.194 MiB (picco − minimo delle fasi warm: 3.225 − 1.606, 4.254 − 2.775, 5.219 − 4.025); load 181,6 s, 5.183-5.562 MiB, media 5.280; picco 5.562 MiB su 5.632 (margine 70); 383 righe con le tre quote > 0 su 247 s"}
      - {test: "legame misura-codice della sessione VRAM", risks: [RISK-6, RISK-9], ref: "spikes/s04-jev/RISULTATI-rerank.md:90 ; results/raw/rerank/vram-b16384-8192 nel worktree (ignorato)", status: human_judgment, human_judgment: true, rationale: "vram.csv non ha colonne run_id né git_rev e il manifest della sessione VRAM è solo in raw ignorato. Il bridge ha letto i file raw ancora presenti nel worktree .claude/worktrees/01-15: il log della sessione 01-15-20261026t100823z-f6a0 riporta git_rev 86353bc, 0 file tracciati modificati, blob di vram.sh 874ab45 = blob di HEAD, «Compiling s04-jev», DOWN 3 rimossi residui 0; i log dei servizi riportano revisioni di bge-m3 e bge-reranker uguali al Setup, max_batch_tokens 16384 e 8192, 0 «out of memory». I file raw sono stati copiati dal coordinatore in spikes/s04-jev/results/raw/rerank del checkout principale (ignorato da git, 3,6 MB) prima di rimuovere il worktree"}
      - {test: "budget VRAM deciso", risks: [RISK-6], ref: "spikes/s04-jev/RISULTATI-rerank.md:129-133 ; docs/adr/0009-reranker-baseline.md:60-62", status: pass, evidence: "R-3 rispettata con i default, nessuna riduzione dei batch necessaria né misurata; budget 5,5 GiB desktop compreso. La quota di tei-embed + docling per l'MVP (2.813 MiB) è una stima per attribuzione, non una misura a due servizi: dichiarato in RISULTATI e ADR"}
      - {test: TEST-9, risks: [RISK-6], ref: "spikes/s04-jev/src/bin/rerank/main.rs:1126#test9_vram_sh_only_session_sh_for_services", status: pass, evidence: "statico, verde sull'host (bridge): vram.sh usa session.sh up/ps/cid/oneshot, nessun --profile, COMPOSE_PROFILES, compose up/stop/down/run, docker stop/kill/rm; rifiuta container di altri run_id (vram.sh:106-107)"}
      - {test: TEST-9, risks: [RISK-6], ref: "spikes/s04-jev/src/bin/rerank/main.rs:1166#test9_vram_sh_exits_2_without_run_id", status: human_judgment, human_judgment: true, rationale: "test ignorato non eseguibile su Windows: Command::new(\"bash\") risolve il bash di WSL. Equivalente manuale ripetuto dal bridge con Git Bash sul main fuso: env -u CKA_RUN_ID bash spikes/s04-jev/vram.sh stampa il messaggio ed esce 2 (vram.sh:38-41). Il test Rust in sé non ha mai girato"}
      - {test: TEST-9, risks: [RISK-6], ref: "spikes/s04-jev/src/bin/rerank/main.rs:1181#test9_vram_sh_respects_foreign_services_manual", status: human_judgment, human_judgment: true, rationale: "prova manuale con Docker e GPU (P-6): l'unica traccia è il messaggio del commit d7cbb20 («rifiuto con container estranei (exit 1, nessun servizio toccato), interruzione con SIGINT (trap, nessuna riga scritta, down della sessione)»), non rieseguita in ripresa né dal bridge. A favore: il log della sessione tracciata mostra nessun container prima della misura e DOWN rimossi 3 residui 0"}
  - must_have: "Nessun candidato non visibile al ruolo della query raggiunge il reranker TEI o JEV: la visibilità è ricontrollata prima di ogni invio (D-16)"
    verification:
      - {test: TEST-1, risks: [RISK-1], ref: "spikes/s04-jev/src/bin/rerank/main.rs:850#rerank_blocks_invisible_candidate_before_tei", status: pass, evidence: "verde sull'host (bridge): server finto su loopback, 0 richieste ricevute con un passaggio restricted per employee; assert_visible a main.rs:103-104 prima del match sul metodo, riconteggio sul punto di invio a main.rs:113-119"}
      - {test: TEST-1, risks: [RISK-1], ref: "spikes/s04-jev/src/bin/rerank/main.rs:916#rerank_none_also_refuses_invisible_set", status: pass, evidence: "verde sull'host (bridge): anche none rifiuta un insieme con un passaggio non visibile"}
      - {test: TEST-2, risks: [RISK-1], ref: "spikes/s04-jev/src/bin/rerank/main.rs:882#rerank_blocks_invisible_candidate_before_jev", status: pass, evidence: "verde sull'host (bridge): client JEV di 01-11 verso un server finto locale, 0 richieste ricevute"}
      - {test: TEST-3, risks: [RISK-1], ref: "spikes/s04-jev/src/bin/rerank/main.rs:1340#test3_unauthorized_sent_zero_everywhere ; spikes/s04-jev/results/rerank/rerank.csv", status: pass, evidence: "rieseguito dal bridge (verde); ricontato: 0 righe con unauthorized_sent ≠ 0 su 887, per tutti i metodi. I 387 riordini TEI dei carichi VRAM (100 warm + 287 load) stanno in CSV di /work/tmp non versionati; il log raw del carico load riporta 287 righe, unauthorized_sent 0"}
      - {test: "guardia al caricamento", risks: [RISK-1], ref: "spikes/s04-jev/src/bin/rerank/main.rs:474-475", status: pass, evidence: "letto dal bridge: load_sets applica assert_visible a ogni insieme già al caricamento, oltre alla guardia di rank_query; il ruolo dell'export deve coincidere con il role_profile dell'oracolo (main.rs:434-440)"}
      - {test: TEST-8, risks: [RISK-5], ref: "spikes/s04-jev/src/bin/rerank/main.rs:970#candidates_sha_mismatch_refused_without_explicit_override ; :1019#candidates_sha_match_needs_no_override", status: pass, evidence: "verdi sull'host (bridge); verify_candidates_sha a main.rs:203-253: senza override errore con i due sha, override solo esadecimale di 64 caratteri uguale allo sha calcolato. Nella misura lo sha dei candidati rigenerati (8f507d07…) coincide col manifest: nessun --accept-sha (RISULTATI-rerank.md:53)"}
      - {test: TEST-4, risks: [RISK-2], ref: "spikes/s04-jev/src/bin/rerank/main.rs:1090#test4_key_not_read_by_bin_nor_echoed_by_vram_sh ; :1227#test4_key_value_absent_from_artifacts", status: pass, evidence: "il primo verde sull'host (bridge); il secondo (ignorato) rieseguito dal bridge con il file dei segreti caricato in una subshell: verde. vram.sh legge dal file solo i due batch, in subshell (vram.sh:55-65); P-3: il grep della chiave guarda solo il codice prima di #[cfg(test)]"}
      - {test: TEST-14, risks: [RISK-10], ref: "spikes/s04-jev/src/bin/rerank/main.rs:1189#test14_no_literal_urls ; :1214#test14_security_scanner_clean", status: pass, evidence: "il primo verde sull'host, il secondo (ignorato) rieseguito dal bridge: verde. P-2: gli URL di loopback stanno solo nei moduli di test; scanner dell'orchestratore con soglia HIGH su spikes/s04-jev e ADR-0009: 0 rilevamenti su 18 file"}
  - must_have: "JEV per il reranking resta la scelta di prodotto (ADR-0002); la baseline è registrata in ADR-0009 come confronto e ripiego"
    verification:
      - {test: TEST-5, risks: [RISK-3], ref: "spikes/s04-jev/src/bin/rerank/main.rs:1357#test5_thresholds_committed_before_results", status: pass, evidence: "rieseguito dal bridge (verde); a mano: Soglie e Metodo in 4407e74 (19:11), primo file di results/rerank in 86353bc (21:13); le sezioni Soglie e Metodo sono identiche fra 4407e74 e a7341dc, e il diff del file non rimuove alcuna riga"}
      - {test: TEST-12, risks: [RISK-8], ref: "spikes/s04-jev/src/bin/rerank/main.rs:1561#test12_plan_0111_files_untouched", status: pass, evidence: "rieseguito dal bridge (verde); git diff --name-only 829edaa exec/01-15 = esattamente i 9 files_modified; nessun file di 01-11 (lib, jev, candidates, metrics, main, questions, RISULTATI-jev), di spikes/s03-pgvector, Cargo.toml o DEPENDENCIES.md; gate-proprieta-file conforme"}
      - {test: "ADR-0009 e RISULTATI.md", risks: [RISK-9], ref: "docs/adr/0009-reranker-baseline.md:54-62 ; spikes/s04-jev/RISULTATI.md:13-51", status: pass, evidence: "letto dal bridge: Stato Proposed, Decisione no-rerank con JEV rank di prodotto e ripiego all'adapter identità, budget VRAM, Evidenze con percorsi esistenti; RISULTATI.md collega RISULTATI-jev, RISULTATI-rerank e i due hw.json e ha Esito, Impatto su porte, Impatto threat model, Destinazione"}
  - must_have: "Gate d'onda e sicurezza"
    verification:
      - {test: "gate sul main fuso a7341dc", risks: [], ref: "nota dell'orchestratore", status: pass, evidence: "cargo fmt --check ok; clippy host --workspace --all-targets --locked --offline -D warnings ok; cargo test -p s04-jev --locked --offline sull'host: lib 16 pass, bin rerank 13 pass 11 ign, «Compiling s04-jev» visto; scanner con soglia HIGH su spikes/s04-jev e ADR-0009: 0 rilevamenti su 18 file; gate-proprieta-file conforme"}
      - {test: "ignorati", risks: [RISK-1, RISK-2, RISK-3, RISK-6, RISK-7, RISK-8, RISK-9, RISK-10], ref: ".planning/tmp/rapporto-01-15.yaml (gate.ignorati)", status: pass, evidence: "l'orchestratore non li ha rieseguiti; li ha rieseguiti il bridge sull'host sul main fuso: test3, test4_key_value, test5, test7, test10, test11, test12, test13, test14_security_scanner_clean, 9 verdi su 9 (stessi 9 dichiarati dall'esecutore). I due di TEST-9 restano al giudizio umano (sopra)"}

# Metrics
duration: "due sessioni: 2026-10-25 circa 2 h 23 min fino alle misure di rerank, test rossi 11201ac 18:50 → misure 86353bc 21:13 (prove VRAM non tracciate fino a circa 01:01 del 26); ripresa 2026-10-26 sessione VRAM 11:08-11:15 → 7299a24 11:18 → 8e68a99 11:21; merge a7341dc 11:22"
completed: 2026-10-26
---

# Fase 1 Piano 15: S-04/A-06, reranker baseline, convivenza in VRAM e ADR-0009 · Summary

**Sui top-20 autorizzati di S-03 (74 query, ripetizioni 2-3) il reranker bge-reranker-v2-m3 su TEI GPU porta nDCG@5 da 0,5777 a 0,7338 (+0,1561, margine richiesto +0,02), ma il suo p95 per 20 candidati è 360 ms contro un tetto di 300: la soglia R-1 era congiunta, quindi la baseline è no-rerank. JEV forma A resta il rank di prodotto con nDCG@5 0,7649 e recall@8 0,7973 (+0,1872 e +0,0541 sulla baseline), p95 483 ms, 0,130760 USD per 4.437 richieste. Con tei-embed, tei-rerank e Docling GPU caricati insieme per 3 minuti la VRAM totale arriva a 5.562 MiB su 5.632, senza OOM e con i default dei batch, ma il margine è di 70 MiB e dipende dalla quota del desktop. 0 candidati non visibili inviati su 887 righe. ADR-0009 (Proposed): no-rerank come adapter identità, JEV rank, budget VRAM 5,5 GiB con tei-embed e Docling nell'MVP**

## Performance

- **Duration:** due sessioni, circa 2 h 23 min fino alle misure di rerank più le prove VRAM notturne, e circa 15 min di ripresa. 2026-10-25: test rossi `11201ac` (18:50:17), soglie e bin `4407e74` (19:11:03), fix `d7cbb20` (19:26:21), Setup `d530189` (19:27:10), misure di rerank `86353bc` (21:13:05), poi prove VRAM non committate fino a circa 01:01 del 26 e interruzione. 2026-10-26: sessione VRAM 11:08-11:15, `7299a24` (11:18:30), `8e68a99` (11:21:00), merge `a7341dc` (11:22:53).
- **Started:** 2026-10-25T18:50:17+01:00
- **Completed:** 2026-10-26T11:22:53+01:00
- **Tasks:** 2 di 2
- **Files modified:** 9 su 9 dei `files_modified`, `git diff --name-only 829edaa exec/01-15`; `a7341dc^2` = `8e68a99` = punta di `exec/01-15`. Nessun file fuori elenco; nessun file del piano 01-11, di `spikes/s03-pgvector/`, `Cargo.toml` o `DEPENDENCIES.md` toccato.

## Accomplishments

- **Soglie e Metodo prima della misura** (`spikes/s04-jev/RISULTATI-rerank.md:12-46`, commit `4407e74`): R-1 baseline (+0,02 di nDCG@5 e p95 ≤ 300 ms, congiunte), R-2 JEV come differenza senza soglia di esclusione, R-3 VRAM ≤ 5,5 GiB senza OOM con riduzione dei batch prima di dichiarare l'incompatibilità, R-4 `unauthorized_sent` = 0.
- **Bin `rerank`** (`spikes/s04-jev/src/bin/rerank/main.rs`): `rank_query` con guardia su tutti i metodi (`:95-170`), verifica sha256 dei candidati (`:203-253`), regola di rilevanza di S-03 rifatta nel bin (`:298-399`), caricamento con guardia e controllo del ruolo (`:404-483`), CLI, CSV e tabella (`:487-744`).
- **Client TEI** (`spikes/s04-jev/src/bin/rerank/tei_rerank.rs`): costruttore che valida l'URL da `TEI_RERANK_URL` (`:42-71`), `/info` (`:74-86`), `/rerank` (`:90-118`), mapping per indice (`:135-163`).
- **`vram.sh`** (`spikes/s04-jev/vram.sh`): uscita 2 senza `CKA_RUN_ID` (`:38-41`), batch letti in subshell (`:55-65`), rifiuto di container di altri (`:106-107`), campionamento ogni 500 ms (`:113-121`), fasi (`:164-200`), OOM (`:202-216`), quote e CSV (`:228-250`).
- **Misure:** `rerank.csv` (887 righe), `vram.csv` (593 righe), `hw.json`.
- **Sintesi di S-04:** `spikes/s04-jev/RISULTATI.md` (indice, Esito, porta `Reranker`, threat model, destinazione) e `docs/adr/0009-reranker-baseline.md` (Proposed).

## Task Commits

0. **Test rossi (agente test-write, distinto dall'esecutore):** `11201ac` (test). RED_EXPECTED, 13 rossi, 11 ignorati (`.planning/tmp/test-write-01-15.yaml`). L'esecutore non ha tolto né indebolito alcun test: le sole righe rimosse dai due file del test-write sono i commenti STUB, `#![allow(dead_code)]`, gli `use` ridondanti, i `let _ = …` e i `todo!()` (`git diff 11201ac 8e68a99 -- spikes/s04-jev/src/bin`).
1. **Task 1, soglie, bin e script:** `4407e74` (feat), `d7cbb20` (fix), `d530189` (docs, Setup prima delle misure).
2. **Task 2, misure, RISULTATI e ADR:** `86353bc` (test, rerank), `7299a24` (test, VRAM, esecutore di ripresa), `8e68a99` (docs, esecutore di ripresa).

Base dell'esecuzione: `829edaa`. **Merge:** `a7341dc` (`--no-ff` di `exec/01-15`). **Plan metadata:** questo SUMMARY e `01-15-RACCONTO.md`, scritti dal bridge senza commit.

## Files Created/Modified

- `spikes/s04-jev/src/bin/rerank/main.rs`: CLI, `rank_query`, verifica sha, rilevanza, CSV; 19 test (5 unitari, 3 statici, 11 ignorati).
- `spikes/s04-jev/src/bin/rerank/tei_rerank.rs`: client `/rerank` e `/info`, server finto per i test, 5 test.
- `spikes/s04-jev/vram.sh`: misura di convivenza in VRAM.
- `spikes/s04-jev/results/rerank/`: `hw.json`, `rerank.csv`, `vram.csv`.
- `spikes/s04-jev/RISULTATI-rerank.md`, `spikes/s04-jev/RISULTATI.md`, `docs/adr/0009-reranker-baseline.md`.

## Decisions Made

### Decisioni dell'utente già prese (`01-CONTEXT.md` e ADR)

- **D-21** (candidati della baseline: no-rerank e bge-reranker-v2-m3 su TEI GPU; jina-reranker escluso per licenza), **D-16** (no-leak verso qualunque reranker), **D-13** (gate GPU), **ADR-0002** (JEV rank LOCKED), **ADR-0008** (forma A, concorrenza 16, pin `jev-1.13.0`). ADR-0009 resta *Proposed*; la ratifica è dell'utente nel consolidamento (01-17, Task 2).

### Scelte dell'esecutore

- **Guardia anche per `none`** (`main.rs:103-104`): l'insieme intero si controlla prima del `match` sul metodo, quindi nessun ramo può dimenticarla.
- **Riconteggio sul punto di invio a TEI** (`main.rs:113-119`), simmetrico a quello del client JEV di 01-11.
- **Pareggi sciolti dall'ordine denso** in TEI e JEV (`tei_rerank.rs:158`, `main.rs:148`): TEI 1.9.4 non conserva l'ordine degli indici a parità di punteggio (RISULTATI-rerank §Setup).
- **Rilevanza rifatta nel bin** (`main.rs:293-399`): i crate degli spike non si importano fra loro; controllata contro un'implementazione Python indipendente e contro `query-bench` di S-03 (RISULTATI-rerank.md:54-57). Il passo NFC manca, ma testi ed excerpt sono già NFC.
- **VRAM per servizio per avvio successivo** (Metodo, `vram.sh:228-250`): su WSL2 `--query-compute-apps` non vede i processi dei container; il piano chiedeva di campionarlo ogni 500 ms, lo script ne salva solo un'istantanea per fase in raw.
- **Esecutore di ripresa:** serie VRAM non tracciate escluse dalle evidenze e rimisura della sola configurazione prevista; budget MVP senza TEI reranker.

## Esiti delle soglie

| soglia | esito | numeri (ricalcolati dal bridge) |
|---|---|---|
| R-1 baseline | **no-rerank** | TEI GPU nDCG@5 0,7338 vs 0,5777 (+0,1561 ≥ 0,02), p95 360 ms > 300 |
| R-2 JEV come differenza | riportato | +0,1872 nDCG@5, +0,0541 recall@8 sulla baseline; p95 483 ms; +0,0311 nDCG@5 su TEI GPU |
| R-3 VRAM ≤ 5.632 MiB, nessun OOM | pass, margine 70 MiB | picco 5.562 MiB, oom false su 593 righe, default 16384 / 8192 |
| R-4 `unauthorized_sent` = 0 | pass | 0 su 887 righe |

## Deviations from Plan

### Livello c

1. **Interruzione del primo esecutore** dopo `86353bc` per chiusura della sessione: `vram.csv`, `RISULTATI.md` e ADR-0009 non committati (questi ultimi con segnaposto VRAM), nessun rapporto. L'esecutore di ripresa ha giudicato non tracciabili le due serie del `vram.csv` lasciato (una marcata non valida con desktop a 2,8 GiB e picco 5.851 MiB; una del 26 alle 00:55 senza `run_id` né `git_rev`), le ha spostate in `results/raw/rerank/invalid-sessione-interrotta-20261026/` (ignorata da git), ha rimisurato `b16384-8192` nella sessione `01-15-20261026t100823z-f6a0` e ha compilato i segnaposto (`7299a24`, `8e68a99`). Rapporto ricostruito da git.

### Livello a

1. **Manifest della sessione VRAM solo in raw** (`7299a24`, RISULTATI-rerank.md:121): `hw-manifest.sh` lanciato a sessione chiusa, senza container; `hw.json` versionato resta quello della sessione di rerank. La regola W9 vuole il manifest prima della sessione.
2. **Ripiego CPU con 147 righe invece di 148**: Q-042 ripetizione 3 in timeout su `cpu-1.9` (dato facoltativo, fuori soglie). Il bin esce con errore quando un riordino fallisce (`main.rs:740-742`), quindi quella invocazione non è uscita con 0.
3. **P-4, Q-075 con 19 candidati**: mai riempita con candidati non visibili; test accettato dal coordinatore con candidati in 1..=20.
4. **`hw.json` normalizzato** allo schema di `common::manifest`, originale in raw (difetto noto di `hw-manifest.sh`, come nei piani precedenti).
5. **Binario al posto di `cargo run`** (P-1): due bin nel pacchetto, `Cargo.toml` fuori perimetro; comandi con `/target/release/rerank` o `--bin rerank`.

### Scostamenti dall'acceptance scritta

- Il grep di `http://` del Task 1 su `src/bin/rerank/*.rs` trova gli URL di loopback dei test (P-2): il coordinatore ha deciso di leggerlo solo sul codice di produzione, come fa `test14_no_literal_urls`.
- Il grep di `TYPESAFE_API_KEY` di TEST-4 trova i test stessi (P-3): stesso criterio.
- Il piano chiedeva a `vram.sh` di passare `EMBED_MODEL`, `EMBED_REVISION` e `RERANK_REVISION` a `session.sh up`: lo script li prende dal file di `CKA_SPIKES_ENV` della sessione. I log raw dei servizi confermano le revisioni del Setup.

**Total deviations:** 1 di livello c, 5 di livello a; nessuna di livello b.
**Impact on plan:** nessuna estensione di perimetro (9 file su 9).

## Discrepanze fra rapporto, RISULTATI e prove

- **Legame della misura VRAM al codice.** A differenza della misura di rerank, legata da `hw.json` a `d530189` (codice identico a HEAD), `vram.csv` non porta né `run_id` né `git_rev`, e il manifest della sessione è solo in raw. Il bridge ha letto i file raw nel worktree dell'esecutore: il log della sessione riporta `git_rev` `86353bc`, 0 file modificati e il blob di `vram.sh` uguale a quello di HEAD (`874ab45`). Le affermazioni del rapporto sono quindi confermate, ma solo da file che non sono in git e che spariranno con il worktree.
- **Spesa JEV e «Compiling s04-jev».** 0,130760 USD e la compilazione nel runner stanno nel log m1 della sessione di rerank (raw, ignorato), letto dal bridge. Le 4.437 richieste si ricavano invece dal CSV (somma della colonna `candidates` delle righe `jev`).
- **Riordini TEI dei carichi VRAM.** I 387 riordini (100 warm + 287 load) con `unauthorized_sent` = 0 sono in CSV di `/work/tmp`, non versionati; il log raw del carico load conferma 287 righe e 0, quello della fase warm non è stato controllato.
- **Quota del desktop in GiB.** RISULTATI.md:25 e ADR-0009:80 dicono «da 1,5 a 2,9 GiB»; i valori osservati sono 1.479-2.869 MiB, cioè 1,44-2,80 GiB: «2,9» è la cifra delle migliaia di MiB, non i GiB. RISULTATI-rerank.md:122 usa invece correttamente «2,8 GiB». Corretto dal coordinatore nel commit di registrazione (anche «1,65» → «1,64 GiB»).
- **Soglia del desktop.** RISULTATI-rerank.md:131 e ADR-0009:86 dicono «circa 1,65 GiB (5.632 − 3.955 = 1.677 MiB)»: 1.677 MiB sono 1,64 GiB. Arrotondamento, nessun esito cambia.
- **Quote nelle fasi warm del CSV.** Dentro ogni fase warm le colonne di quota del servizio in avvio sono la lettura istantanea meno il minimo della fase, non la quota finale (per esempio 1.169 MiB a fine warm-tei-embed contro 1.619 attribuiti dopo): è il comportamento di `vram.sh:245-247`, coerente col Metodo, ma la tabella di RISULTATI riporta solo la quota finale.
- **ADR-0012.** RISULTATI.md:30 cita ADR-0012 per la separazione di `Reranker` da `DecisionEngine`, ma l'ADR non esiste ancora in `docs/adr/` (riferimento in avanti, già noto da 01-05).
- **Data dell'ADR.** ADR-0009 dice «Data: 2026-10-25», ma il file entra in git il 2026-10-26 (`8e68a99`): la bozza del primo esecutore non era committata.
- **Il resto coincide** con il ricalcolo: le quattro righe della tabella di rerank (medie, p50, p95, p99, massimi), il conteggio delle righe, Q-075 a 19 candidati, `model_resolved`, il ricalcolo VRAM per fase (minimi, massimi, durate, quote, picco, margine), `oom`, l'ordine Soglie → misure, il perimetro.

## Threat Flags

- **T-01-15-01 (passaggi non visibili verso TEI o JEV):** guardia sull'intero insieme per tutti i metodi più riconteggio sul punto di invio; 0 su 887 righe; TEST-1 e TEST-2 verdi con server finti.
- **T-01-15-02 (file dei candidati):** sha256 uguale al manifest, nessun `--accept-sha` usato; TEST-8 verde.
- **T-01-15-03 (VRAM):** nessun OOM con i default, margine 70 MiB; riduzione dei batch non misurata.
- **T-01-15-04 (`TYPESAFE_API_KEY`):** letta solo dalla libreria di 01-11; `vram.sh` legge dal file solo i batch in subshell; TEST-4 verde, scanner pulito.

## Known Stubs

Nessuno.

## User Setup Required

Nessuno.

## Punti aperti

- **Margine VRAM di 70 MiB** con i tre servizi, dipendente dalla quota del desktop di Windows (1,44-2,80 GiB osservati sulla stessa macchina): con il desktop a 2,8 GiB una serie scartata ha superato il tetto.
- **Quota tei-embed + docling di circa 2,8 GiB solo stimata** per attribuzione (1.619 + 1.194 MiB), non misurata in uno scenario a due servizi: va misurata prima di fissare il deployment MVP, soprattutto con il desktop pesante.
- **p95 del reranker TEI sotto contesa 1.045 ms** (contro 360 ms a GPU libera): rilevante se un giorno si riattiva il reranker locale.
- **Configurazioni di batch ridotte** (8192/4096, 4096/2048) non misurate.
- **Ratifica di ADR-0009** nel consolidamento (01-17, Task 2), con la riga `DEPENDENCIES.md` «Reranker baseline» da Aperta a Decisa.
- **TEST-9 comportamentale** (servizi altrui intatti, Ctrl-C) mai rieseguito dopo `d7cbb20`, e `test9_vram_sh_exits_2_without_run_id` non eseguibile su Windows finché il test chiama `bash` senza percorso.
- **Prove raw nel worktree** `.claude/worktrees/01-15` (log della sessione VRAM, manifest, log m1): da decidere se conservarle prima della rimozione del worktree.
- **ADR-0012** (porta `Reranker` separata da `DecisionEngine`) da scrivere, come già notato in 01-05.

## Next Phase Readiness

- **01-16 (porte S-07):** trait `Reranker` stretto del solo `rank` con adapter JEV e adapter identità, errori tipizzati (`Unauthorized`, `Timeout`, `RateLimited`, `Http`, `InvalidResponse`), guardia di visibilità nella porta (RISULTATI.md:28-34).
- **01-17 (consolidamento):** ratifica di ADR-0009 e aggiornamento di `DEPENDENCIES.md`; misura del budget a due servizi da pianificare.

## Self-Check: PASSED

- 9 file su 9 dei `files_modified` nel diff `829edaa..exec/01-15`, nessun file di altre fasi; `a7341dc^2` = `8e68a99`; `gate-proprieta-file` conforme.
- Rieseguiti dal bridge sul main fuso, sull'host: `cargo test -p s04-jev --locked --offline --bin rerank` (13 passati, 11 ignorati) e 9 dei test ignorati (test3, test4_key_value con i segreti in subshell, test5, test7, test10, test11, test12, test13, test14_security_scanner_clean): 9 verdi. Equivalente manuale di `test9_vram_sh_exits_2_without_run_id` con Git Bash: uscita 2.
- Ricalcolati dai CSV: R-1, R-2, R-3, R-4; righe, candidati e richieste JEV; legame di `hw.json` al commit misurato; ordine dei commit e invarianza di Soglie e Metodo.
- Gate sul main fuso `a7341dc` (nota dell'orchestratore): fmt ok; clippy `-D warnings` ok; `cargo test -p s04-jev` lib 16, bin 13 + 11 ignorati; scanner con soglia HIGH, 0 rilevamenti su 18 file. I due test di TEST-9 restano al giudizio umano.

**Modifica:** Claude (Anthropic) · 2026-10-26 · creazione (bridge di 01-15, passi 1-3)
