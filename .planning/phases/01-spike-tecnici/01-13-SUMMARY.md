---
phase: 01-spike-tecnici
plan: 13
subsystem: spike-parser-ocr-ttq-render-ostile-adr-0004
tags: [s02-parser, ocr, tesseract, tessdata_best, tsv, pp-ocrv6, paddleocr, paddlepaddle, onednn, pdfium, render, phys, bbox, afm, cer, ttq-parse, docling, budget, max_admitted, m10, jbig2, deep-nesting, dct-sof, jpx-siz, prescan, adr-0004, supply-chain, cooldown, uv, apt-pin]

# Dependency graph
requires:
  - phase: 01-07
    provides: "lanciatore isolato e contratto W17 (job JSON chiuso, allowlist ocr_render/ocr_tesseract/extract_pdf, budget con prenotazione v6, calibrate), immagine worker con Tesseract 5.5.0 e tessdata_best"
  - phase: 01-18
    provides: "manifest dei casi ostili e generatore gen_hostile.py (merge 3fb03bc), metodo di conformità M10 e concorrenza parse = 2 da budget-real.csv, D-34 sul residuo JBIG2"
  - phase: 01-08
    provides: "extract_pdf (prescan + PDFium), euristica delle pagine-tabella, docling-client, soglia IoU E-3 non raggiunta (77,0% contro 95%) che rende non valutabile il riferimento PDFium"
  - phase: 01-09
    provides: "RISULTATI-locator.md e ADR-0005, riassunti nell'indice di S-02"
  - phase: 01-06
    provides: "oracolo parser-it.json, 6 pagine IT con i tre assi verdi, atteso del CER"
  - phase: 01.5
    provides: "prescan sulle dimensioni codificate DCT (SOF) e JPX (SIZ), manifest sof/manifest.json e gen_sof.py; obbligo per 01-13 di provare i tre casi su ocr_render (01.5-CONTRATTO.md)"
provides:
  - "Bin ocr (render, tesseract da figlio isolato; map, score, hostile-verdict, throughput-row, paddle-check, limits-from-runs, tess-jobs, ttq-select, ttq-merge su dati derivati), 47 test più 4 ignorati di evidenza"
  - "Soglie OCR, bbox, throughput, prova ostile, regola «vince nettamente» e TTQ committate prima di ogni misura (40e67c3) e mai ritoccate"
  - "Misure: ocr.csv (48 righe), bbox_map.csv (90), ocr_throughput.csv (12), ttq_parse.csv (46), hostile-render.csv (15), limits-ocr_render.json, limits-ocr_tesseract.json, hw.json, paddle-weights.sha256 (42 righe)"
  - "Immagine paddle con lock pip a hash (cooldown al 2026-10-10), tre librerie apt a versione fissata, selezione PP-OCRv6 verificata in build e pesi ancorati"
  - "RISULTATI-ocr.md, indice ed Esito complessivo di S-02 (RISULTATI.md), ADR-0004 Proposed con Tesseract confermato"
  - "Concorrenza OCR pubblicata: 2 job ocr_tesseract nel worker da 2 GiB; dimensionamento oltre i 2 GiB estrapolato (D-28)"
affects: [01-17, ADR-0004, DEPENDENCIES.md riga OCR/parser, porta DocumentParser (S-07), worker di ingestion del Document Service, threat model T-09, gap-closure di 01-07 su calibrate]

# Tech tracking
tech-stack:
  added: [tesseract 5.5.0 CLI (gia nell'immagine worker, ora usato), paddleocr 3.7.0 + paddlex 3.7.2 + paddlepaddle 3.3.1 (solo immagine dello sfidante, non adottati), uv 0.13.5 per il lock]
  patterns:
    - "Picco del discendente nel vm_hwm_kb del figlio: il figlio campiona /proc/<pid>/status del processo che lancia, così prenotazione e rapporto prenotazione/picco vedono la memoria vera del job"
    - "Trasformazione px→pt registrata per ogni immagine (transform.json) e controllo analitico a due stadi con rettangoli d'inchiostro calcolati dalle metriche AFM"
    - "Lock pip con cooldown di supply chain (exclude-newer) e pacchetti apt espliciti a versione fissata, con l'elenco delle dipendenze tirate scritto nell'immagine"
    - "Manifest d'integrità dei pesi committato prima di ogni misura e confrontato con diff a ogni sessione; configurazione risolta di build e di sessione confrontate"
    - "Sfidante con un solo tentativo e argomenti fissati in codice prima della misura (V6_REDUCED_DET_ARGS)"

key-files:
  created:
    - spikes/s02-parser/RISULTATI-ocr.md
    - spikes/s02-parser/RISULTATI.md
    - docs/adr/0004-parser-ocr-isolamento.md
    - spikes/s02-parser/scan-gen/make_scans.sh
    - spikes/s02-parser/scan-gen/gen_geom.py
    - spikes/s02-parser/ocr-run/run_ttq.sh
    - spikes/s02-parser/ocr-run/run_hostile_render.sh
    - spikes/docker/paddle.Dockerfile
    - spikes/docker/paddle-requirements.lock
    - spikes/s02-parser/results/ocr/hw.json
    - spikes/s02-parser/results/ocr/limits-ocr_render.json
    - spikes/s02-parser/results/ocr/limits-ocr_tesseract.json
    - spikes/s02-parser/results/ocr/paddle-weights.sha256
    - spikes/s02-parser/results/ocr/ocr.csv
    - spikes/s02-parser/results/ocr/bbox_map.csv
    - spikes/s02-parser/results/ocr/ocr_throughput.csv
    - spikes/s02-parser/results/ocr/ttq_parse.csv
    - spikes/s02-parser/results/ocr/hostile-render.csv
  modified:
    - spikes/s02-parser/src/bin/ocr/main.rs
    - spikes/s02-parser/src/bin/ocr/score.rs
    - spikes/s02-parser/src/bin/ocr/geom.rs
    - spikes/s02-parser/src/bin/ocr/tsv.rs
    - spikes/s02-parser/src/bin/ocr/ttq.rs

key-decisions:
  - "Utente: lock PaddleOCR rigenerato con cooldown di supply chain al 2026-10-10 con uv al posto di pip-compile (dbb615b), poi approvato"
  - "Utente, opzione A: tre librerie apt a versione fissata (libgomp1, libgl1, libglib2.0-0t64) nel Dockerfile dello sfidante (24654ca); 38 dipendenze indirette non fissate, elencate"
  - "Utente, opzione (a): PP-OCRv6 misurato con un solo tentativo, detection ridotta a 960 px e oneDNN spento, argomenti fissati prima della misura (7490cba, 106f278)"
  - "Regola «vince nettamente» applicata: PP-OCRv6 non vince (CER 13,57% / 14,19% contro 3,16% / 2,74%, tempo 7,7-8,6 ×); Tesseract confermato in ADR-0004, stato Proposed, ratifica nel consolidamento (01-17, Task 2)"
  - "Coordinatore: test1 attende uv==0.13.5 invece di pip-tools==7.4.1 (307de01); il piano non si modifica per non invalidare risk_audit_26"
  - "Coordinatore: paddle-weights.sha256 con sha256 completi a 64 caratteri (manifest d'integrità, non evidenza di misura)"
  - "Esecutore: limiti ocr_tesseract con ocr limits-from-runs, perché calibrate di 01-07 rifiuta le op con discendenti (difetto di un file chiuso, gap-closure con riapre_perimetro [1])"

patterns-established:
  - "Un'op con discendenti misura la memoria del discendente nel figlio e la riporta nel campo vm_hwm_kb che legge il padre"
  - "Una prova ostile su un nuovo percorso usa gli attesi letterali del manifest esistente e lo stesso CSV per più manifest, con prefisso del detail quando il manifest lo dichiara"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "Tesseract da CLI (tessdata_best ita+eng, TSV con bbox per parola) gira solo come job ocr_render/ocr_tesseract del lanciatore ed è misurato in CER, s/pagina e fedeltà delle bbox su scansioni buone e degradate"
    verification:
      - {test: TEST-8, risks: [], ref: "spikes/s02-parser/src/bin/ocr/main.rs:810#test8_child_cli_matches_allowlist_argv ; :837#test8_child_cli_rejects_other_forms ; :868#test8_tesseract_argv_is_tsv_ita_eng_best_with_effective_dpi ; :898#test8_no_shell_and_no_omp_override_in_ocr_sources ; tsv.rs:98, :129, :135 ; geom.rs:163, :169", status: pass, evidence: "test verdi nel runner (rapporto degli esecutori, ocr 47 + 4 ignorati) e nel gate sul main fuso d13e23a; argv letto dal bridge a main.rs:115-135, Command::new con percorso assoluto a main.rs:411, nessuna shell nel codice non di test"}
      - {test: TEST-9, risks: [], ref: "geom.rs:195, :209, :225, :253 ; score.rs:1613, :1627, :1644, :1654, :1682, :1712, :1738, :1758, :1781 ; spikes/s02-parser/results/ocr/ocr.csv ; bbox_map.csv", status: pass, evidence: "ricontato dal bridge: ocr.csv 12 righe tesseract good e 12 degraded, 12 + 12 PP-OCRv6; bbox_map.csv tesseract analytic 4 good + 4 degraded tutte pass, pdfium 31 + 31 tutte non_valutabile; PP-OCRv6 analytic 8 su 8 fail, pdfium 12 non_valutabile"}
      - {test: "TEST-9 (soglia CER ≤ 3% sulle buone)", risks: [], ref: "spikes/s02-parser/RISULTATI-ocr.md:97-107", status: non_raggiunta, evidence: "aggregato 3,16% (p-005 8,36%, p-006 4,70%); dichiarata non raggiunta, soglia invariata dal commit 40e67c3 (git diff 40e67c3..4436d90 sulle Soglie: nessuna riga tolta)"}
      - {test: "TEST-9 (riferimento PDFium)", risks: [], ref: "spikes/s02-parser/RISULTATI-ocr.md:138 ; bbox_map.csv colonna verdict", status: non_valutabile, evidence: "01-08 sotto la propria soglia IoU (E-3 77,0%); IoU mediana 0,55-0,69 accanto al plafond 0,666"}
  - must_have: "PP-OCRv6 sfidante con flag espliciti, pesi precaricati con file e SHA-256 registrati, modelli risolti registrati, rete negata, regola «vince nettamente» scritta prima; nessuna riga v6 senza il flag"
    verification:
      - {test: TEST-2, risks: [], ref: "main.rs:991#test2_build_selects_v6_explicitly_and_stops_without_fallback ; score.rs:2032#test2_v6_cli_args_are_explicit_literals ; score.rs:2055#test2_v6_measure_args_add_plan_flags_and_reduced_detection", status: pass, evidence: "test verdi; argomenti letterali in score.rs:598-633 (V6_PLAN_FIXED_ARGS, V6_REDUCED_DET_ARGS, v6_measure_args), introdotti da 7490cba prima delle righe v6 di 106f278"}
      - {test: "TEST-2 (runtime: valori letterali da v6.selection.json)", risks: [], ref: "spikes/s02-parser/RISULTATI-ocr.md:84-85", status: human_judgment, human_judgment: true, rationale: "v6.selection.json e l'aiuto della CLI stanno nell'immagine e in results/raw (ignorata da git); nel repo resta solo il loro sha256 in paddle-weights.sha256 e la trascrizione in RISULTATI"}
      - {test: TEST-3, risks: [], ref: "main.rs:1009#test3_build_asserts_v6_family_of_det_and_rec ; score.rs:2116#test3_family_check_on_resolved_det_and_rec ; spikes/docker/paddle.Dockerfile:104-127", status: pass, evidence: "test verdi; family_check nel resolver condiviso build/sessione"}
      - {test: "TEST-3 (runtime: PP-OCRv6_medium_det/_rec risolti)", risks: [], ref: "spikes/s02-parser/RISULTATI-ocr.md:86", status: human_judgment, human_judgment: true, rationale: "v6.resolved.json non è versionato; il bridge ha solo confrontato i sei sha256 a 16 caratteri di RISULTATI con le righe complete di paddle-weights.sha256 (coincidono)"}
      - {test: TEST-4, risks: [], ref: "score.rs:2145#test4_weights_manifest_detects_change_omission_and_substitution ; main.rs:1221#test4_test6_evidence_paddle_rows (ignorato) ; git log --reverse", status: pass, evidence: "verificato dal bridge: paddle-weights.sha256 42 righe, hash tutti a 64 caratteri, primo commit 654523c (10-25 13:23) prima del primo commit di righe v6 in ocr.csv 106f278 (10-25 15:45)"}
      - {test: "TEST-4 (runtime: sha256sum -c e diff a ogni sessione)", risks: [], ref: "spikes/s02-parser/RISULTATI-ocr.md:87-88 ; :113", status: human_judgment, human_judgment: true, rationale: "esiti di sessione trascritti in RISULTATI; log in results/raw/paddle (ignorata da git), non rieseguiti dal bridge"}
      - {test: TEST-5, risks: [], ref: "score.rs:2183#test5_session_must_match_build_configuration ; score.rs:693#session_matches_build", status: pass, evidence: "test verde"}
      - {test: "TEST-5 (runtime: resolved-session.json uguale a v6.resolved.json)", risks: [], ref: "spikes/s02-parser/RISULTATI-ocr.md:88 ; :113", status: human_judgment, human_judgment: true, rationale: "resolved-session.json sta in results/raw/paddle, non versionato"}
      - {test: TEST-6, risks: [], ref: "score.rs:2087#test6_unselectable_v6_or_missing_italian_stops ; spikes/docker/paddle.Dockerfile:139-148", status: pass, evidence: "test verde; il ramo selectable=false è scattato davvero alla prima build (7c85634, RISULTATI-ocr.md:80) senza ripiego su v5 e senza righe PaddleOCR in quel commit"}
      - {test: TEST-7, risks: [], ref: "main.rs:1017#test7_image_preloads_weights_and_runs_offline ; spikes/compose.yaml:208 (network_mode none) ; spikes/docker/paddle.Dockerfile:238-240", status: pass, evidence: "test statico verde; letto dal bridge: servizio paddle con network_mode none, mem_limit 2g, cpus 8, utente 10001; USER 10001 e CMD sleep infinity nel Dockerfile"}
      - {test: "TEST-7 (runtime: pesi dalla cache senza rete)", risks: [], ref: "spikes/s02-parser/RISULTATI-ocr.md:88", status: human_judgment, human_judgment: true, rationale: "il messaggio «Using cached files» è nei log di sessione, non versionati"}
      - {test: "regola «vince nettamente»", risks: [], ref: "spikes/s02-parser/RISULTATI-ocr.md:129-133 ; ocr.csv righe PP-OCRv6", status: pass, evidence: "applicata come scritta in 40e67c3: CER peggiore di 10,41 e 11,45 punti, tempo 7,7 × e 8,6 × contro il limite di 1,5 ×; esito Tesseract confermato. Sfidante con detection ridotta, svantaggio dichiarato prima (RISULTATI-ocr.md:89)"}
  - must_have: "Throughput OCR via parse-parent batch --concurrency 1, 4, 8, 16 con max_admitted reale a 2 GiB, batch giudicati col metodo di M10, dimensionamento in ADR-0004 dichiarato estrapolato (D-28)"
    verification:
      - {test: TEST-11, risks: [RISK-001], ref: "score.rs:1949#test11_reservation_is_v6_formula_with_one_descendant ; :1961#test11_m10_pass_and_each_failure ; :1995#test11_published_concurrency_is_largest_conforming_n_capped_by_admitted ; main.rs:1157#test11_test15_evidence_throughput_csv (ignorato) ; spikes/s02-parser/results/ocr/ocr_throughput.csv", status: pass, evidence: "ricontato dal bridge: 12 righe (N 1, 4, 8, 16 × 3), m10_pass true 12 su 12, max_admitted 1/2/2/2, oom_kill_delta, job_errors, leftover_procs_max e failed_jobs 0, parent_exit 0, rapporto minimo 5,999; prenotazione 823.248 KiB = 2 × 421.502.976 B / 1024, con 421.502.976 = 3 × 137.208 kB × 1024 (limits-ocr_tesseract.json)"}
      - {test: "TEST-11 (soglia 25 pagine ≤ 30 s con N = 8)", risks: [RISK-001], ref: "ocr_throughput.csv righe N=8", status: non_raggiunta, evidence: "34,461-34,867 s con 2 job ammessi; dichiarata non raggiunta"}
      - {test: TEST-15, risks: [RISK-001], ref: "score.rs:2013#test15_stale_reservation_or_other_cgroup_is_not_conforming ; score.rs:523-540#m10_pass ; main.rs:1157 (ignorato)", status: pass, evidence: "parte OCR: m10_pass confronta reservation_kb con i limiti correnti e memory.max con 2 GiB; reservation_kb 823248 su ogni riga uguale ai limiti committati"}
      - {test: "TEST-15 (parte parse)", risks: [RISK-001], ref: ".planning/phases/01-spike-tecnici/01-CONTEXT.md:80 (D-40) ; docs/adr/0004-parser-ocr-isolamento.md:48", status: human_judgment, human_judgment: true, rationale: "M10 parse resta misurata con la prenotazione di allora (803.057 KiB) e non è ripetuta con quella attuale (848.777 KiB): la copre una decisione utente (D-40) e una dichiarazione in ADR, non una misura"}
  - must_have: "Il decoder JBIG2 di PDFium si prova sul percorso ocr_render con jbig2-malformed e l'atteso del manifest di 01-18, deep-nesting nello stesso batch, contenimento verificato; l'esito chiude o lascia aperto T-09 JBIG2 (D-34)"
    verification:
      - {test: TEST-13, risks: [], ref: "score.rs:1830#test13_hostile_rows_table ; :1880#test13_parent_alive_needs_self_exit_and_live_worker ; :1898#test13_t09_closed_only_with_three_of_three ; main.rs:1049, :1063, :1120 (ignorato) ; spikes/s02-parser/results/ocr/hostile-render.csv", status: pass, evidence: "ricontato dal bridge: 15 righe, 3 per caso; parent_alive true, leftover_procs 0, oom_kill_delta 0, oom_adj_set true su 15 su 15; soglie della prova ostile in 40e67c3 prima di hostile-render.csv in be8e871"}
      - {test: "TEST-13 (chiusura di T-09 JBIG2)", risks: [], ref: "hostile-render.csv righe jbig2-malformed ; spikes/s02-parser/RISULTATI-ocr.md:167", status: non_raggiunta, evidence: "jbig2-malformed ok 3 su 3 (pass false): residuo T-09 aperto; deep-nesting ok 3 su 3, contenimento intatto"}
      - {test: TEST-14, risks: [], ref: "hostile-render.csv righe dct-sof-huge-image, jpx-siz-huge-image, jpx-siz-huge-jp2 ; score.rs:1257-1258 (prefisso del detail) ; 01.5-CONTRATTO.md «Obbligo per 01-13»", status: pass, evidence: "ricontato dal bridge: 9 righe su 9 failed/DecompressionLimit con detail dct_sof over_cap o jpx_siz over_cap, pass true, padre vivo. Il test-write l'aveva dichiarato non scrivibile (test-write-01-13.yaml); la copertura è la 1.5 più queste righe"}
  - must_have: "TTQ-parse misurato sul percorso composto reale con cronometro esterno, run_id e hash dell'input comuni, contro la quota di 5 s (D-07)"
    verification:
      - {test: TEST-10, risks: [], ref: "ttq.rs:315, :327, :334, :341, :362 ; main.rs:1074#test10_ttq_script_shape_and_exits_2_without_run_id ; spikes/s02-parser/results/ocr/ttq_parse.csv", status: pass, evidence: "ricalcolato dal bridge: p95 nearest-rank sulle run calde con run ≥ 2: 20.188 ms (n 20), 73.072 ms (n 10), 11.626 ms misto (n 10), uguali a RISULTATI; 46 righe, una fredda per documento, sha256 a 16 caratteri"}
      - {test: "TEST-10 (soglie M1)", risks: [], ref: "spikes/s02-parser/RISULTATI-ocr.md:181", status: non_raggiunta, evidence: "20,2 s contro 5 s e 73,1 s contro 20 s, dominati da Docling sulle pagine-tabella"}
      - {test: "TEST-10 (autocontrollo del ritardo)", risks: [], ref: "spikes/s02-parser/RISULTATI-ocr.md:184", status: human_judgment, human_judgment: true, rationale: "ttq-selfcheck.csv sta in results/raw, ignorata da git"}
  - must_have: "Parser, isolamento e tetto di decompressione decisi in ADR-0004, che riassume i quattro RISULTATI di S-02 e annota i Windows Job Objects fuori scope"
    verification:
      - {test: TEST-12, risks: [RISK-001], ref: "main.rs:1181#test12_evidence_adr_0004 (ignorato, host) ; docs/adr/0004-parser-ocr-isolamento.md ; spikes/s02-parser/RISULTATI.md", status: pass, evidence: "rieseguito dal bridge con grep: T-06, T-07, T-09, T-10, «estrapolat» (2), «Job Object» (2) nell'ADR; i quattro RISULTATI linkati nell'indice; stato Proposed. Il test ignorato è verde sull'host per gli esecutori"}
  - must_have: "Ogni pacchetto pip installato per PaddleOCR è approvato dall'utente prima dell'installazione"
    verification:
      - {test: TEST-1, risks: [], ref: "main.rs:931#test1_pip_installs_only_the_hashed_lock ; spikes/docker/paddle.Dockerfile:53-55 ; spikes/docker/paddle-requirements.lock:1", status: pass, evidence: "test verde dopo 307de01: un solo pip install, -r del lock con --require-hashes, FROM per digest uguale al digest del commento di testa (uv==0.13.5); 70 pacchetti con == e hash"}
      - {test: "TEST-1 (approvazione dell'utente)", risks: [], ref: "spikes/s02-parser/RISULTATI-ocr.md:80 ; :209 ; commit dbb615b", status: human_judgment, human_judgment: true, rationale: "l'approvazione è una risposta dell'utente in conversazione, riportata in RISULTATI e nei messaggi di commit; nessun artefatto la prova. La prima build (7c85634) è successiva al lock approvato (dbb615b). I pacchetti apt sono fuori dal lock: li copre la decisione separata dell'opzione A (24654ca)"}
  - must_have: "Gate d'onda e sicurezza"
    verification:
      - {test: "gate sul main fuso d13e23a", risks: [], ref: "nota dell'orchestratore", status: pass, evidence: "cargo fmt --check ok; clippy host --workspace --all-targets --locked --offline -D warnings ok; cargo test -p s02-parser nel runner con «Compiling s02-parser» visto: primo giro 3 fallimenti di temporizzazione in tests/isolation.rs (file di 01-07, non toccati), rilancio della suite isolation 37 su 37, rilancio completo 380 passati, 0 falliti, 8 ignorati"}
      - {test: "security-scanner", risks: [], ref: "python skills/security-scanner/security_scan.py --fail-on HIGH spikes/s02-parser/src/bin/ocr spikes/s02-parser/scan-gen spikes/s02-parser/ocr-run spikes/docker/paddle.Dockerfile", status: pass, evidence: "rieseguito dal bridge: exit 0, 1 MEDIUM pdf-untrusted-parse a main.rs:35 (import di pdfium_render; il render gira nel figlio isolato dopo il prescan). Col default --fail-on MEDIUM del verify del piano l'exit è 1"}

# Metrics
duration: "test rossi e322de8 (2026-10-23 17:14) poi esecuzione dal 2026-10-24 14:43 (3314f9d) al 2026-10-25 15:45 (4436d90), con le attese delle decisioni utente e due fermi di Docker Desktop; merge d13e23a 2026-10-25 15:46"
completed: 2026-10-25
---

# Fase 1 Piano 13: S-02, OCR delle scansioni, TTQ-parse, render ostile e ADR-0004 · Summary

**Tesseract 5.5.0, lanciato solo come job isolato del lanciatore, legge le scansioni con CER 3,16% sulle buone (soglia 3% mancata per l'ordine dei blocchi di due pagine) e 2,74% sulle degradate, con bbox entro 0,84 pt e 2,66 pt dal rettangolo analitico; lo sfidante PP-OCRv6, misurato con detection ridotta su 2 GiB, dà 13,57% / 14,19% e 7,7-8,6 volte il tempo, quindi non vince e Tesseract è confermato in ADR-0004 (Proposed). Il worker da 2 GiB ammette 2 job OCR (12 batch conformi a M10, 25 pagine in 34,5 s contro 30); il TTQ-parse sta a 20,2 s e 73,1 s contro 5 e 20, per colpa di Docling. Sul render i tre casi DCT/JPX della 1.5 sono respinti dal prescan, mentre `jbig2-malformed` dà `ok`: T-09 JBIG2 resta aperto, con contenimento intatto su 15 righe su 15**

## Performance

- **Duration:** test rossi del test-write `e322de8` (2026-10-23 17:14) e `df0d833`; esecuzione dal 2026-10-24 14:43 (`3314f9d`) al 2026-10-25 15:45 (`4436d90`), in quattro riprese separate da checkpoint utente; merge `d13e23a` alle 15:46.
- **Started:** 2026-10-24T14:43:14+02:00
- **Completed:** 2026-10-25T15:46:47+01:00
- **Tasks:** 3 di 3 (Task 1 checkpoint di legittimità dei pacchetti chiuso con l'approvazione dell'utente)
- **Files modified:** 23 su 23 `files_modified` del piano (`git diff --stat 05d1760 task/PLAN-010-13`), nessun file fuori elenco nel diff del merge. Nessun file di 01-07, 01-08, 01-09, 01-18 o della 1.5 toccato (`git diff --stat 05d1760 main` vuoto su `prescan.rs`, `parse-parent.rs`, `job.rs`, `isolate.rs`, `tests/`, manifest e generatori ostili, `compose.yaml`, `worker.Dockerfile`, `DEPENDENCIES.md`, threat model).

## Accomplishments

- **Soglie prima della misura** (`spikes/s02-parser/RISULTATI-ocr.md:16-61`, commit `40e67c3`): CER, variante degradata, controllo analitico a due stadi e riferimento PDFium condizionato a 01-08, throughput e conformità M10, prova ostile con gli attesi dei due manifest, regola «vince nettamente», TTQ. Nessuna riga tolta fino a `4436d90`.
- **Bin `ocr`** (`spikes/s02-parser/src/bin/ocr/`): `render` con prescan prima di PDFium e PNG con `pHYs` e sidecar, `tesseract` con DPI dal `pHYs`, argv senza shell, stdout con tetto e picco del discendente; sottocomandi di analisi `map`, `score`, `hostile-verdict`, `throughput-row`, `limits-from-runs`, `paddle-check`, `tess-jobs`, `ttq-select`, `ttq-merge`.
- **Scansioni e geometria:** `make_scans.sh` (22 render e 7 extract via `parse-parent batch`, indice per run, degradazione con `magick` e `transform.json`), `gen_geom.py` (4 PDF Helvetica con 193 parole e rettangoli d'inchiostro dalle metriche AFM).
- **Misure Tesseract:** CER e tempi per 6 pagine × 2 qualità; bbox analitiche 8 pass su 8 e stadio 1 a 7,1 × 10⁻¹³ px; throughput 12 batch conformi con `max_admitted` 2.
- **Prova ostile sul render:** 5 casi × 3 ripetizioni; contenimento 15 su 15; casi 1.5 9 su 9; `jbig2-malformed` e `deep-nesting` `ok` 3 su 3.
- **TTQ-parse composto:** 46 run con `run_id` univoco, componenti con stesso `run_id` e sha256 verificati da `ttq-merge`.
- **Sfidante PP-OCRv6:** lock con cooldown, librerie apt fissate, selezione e modelli verificati in build, pesi ancorati prima della misura, 60 invocazioni su 60 riuscite con detection ridotta, regola applicata.
- **Sintesi di S-02:** `RISULTATI.md` (indice, Esito, impatto su porte e threat model, destinazione) e `docs/adr/0004-parser-ocr-isolamento.md` (Proposed).

## Task Commits

0. **Test rossi (agente test-write, distinto dagli esecutori):** `e322de8`, `df0d833` (test). RED_EXPECTED, 42 rossi, 1 verde per costruzione (guardia), 4 ignorati (`.planning/tmp/test-write-01-13.yaml`).
1. **Task 1, lock e Dockerfile senza build:** `3314f9d` (chore); lock rigenerato con cooldown `dbb615b` (build), test1 allineato `307de01` (test, coordinatore).
2. **Task 2, soglie e bin:** `40e67c3` (docs), `3180542` (feat), `c874b42` (feat), `d67a748` (fix), `8b75a99` (feat, lettura delle uscite PP-OCRv6), `fb83602` (fix), `24654ca` (build), `7490cba` (feat, argomenti della detection ridotta).
3. **Task 3, misure, RISULTATI e ADR:** `be8e871` (test), `75d2dff` (test), `d66f502` (docs), `7c85634` (docs), `654523c` (test, manifest dei pesi), `cf591ae` (docs), `106f278` (test, misura v6), `4436d90` (docs).

Base dell'esecuzione: `05d1760` (main dopo la gap-closure 1.5 nel branch del piano). **Merge:** `d13e23a` (`--no-ff` di `task/PLAN-010-13`). **Plan metadata:** questo SUMMARY e `01-13-RACCONTO.md`, scritti dal bridge senza commit.

## Files Created/Modified

- `spikes/s02-parser/src/bin/ocr/main.rs`: CLI, `render`, `tesseract`, `map`, `tess-jobs`, `ttq-select`, `ttq-merge`, 15 test (4 ignorati di evidenza).
- `spikes/s02-parser/src/bin/ocr/score.rs`: CER, oracoli tre-verdi, bbox, verdetti, prova ostile, M10, controlli PaddleOCR, `limits-from-runs`, 22 test.
- `spikes/s02-parser/src/bin/ocr/geom.rs`: `pHYs`, trasformazione px↔pt, 6 test. `tsv.rs`: parser stretto del TSV, 3 test. `ttq.rs`: riga TTQ e `page_score` di 01-08, 5 test.
- `spikes/s02-parser/scan-gen/make_scans.sh`, `scan-gen/gen_geom.py`, `ocr-run/run_ttq.sh`, `ocr-run/run_hostile_render.sh`.
- `spikes/docker/paddle.Dockerfile`, `spikes/docker/paddle-requirements.lock`.
- `spikes/s02-parser/results/ocr/`: `hw.json`, due `limits-*.json`, `paddle-weights.sha256`, cinque CSV.
- `spikes/s02-parser/RISULTATI-ocr.md`, `spikes/s02-parser/RISULTATI.md`, `docs/adr/0004-parser-ocr-isolamento.md`.

## Decisions Made

### Decisioni dell'utente

- **Lock con cooldown** (2026-10-24): nessuna release PyPI dopo il 2026-10-10; rigenerato con `uv pip compile --exclude-newer` perché pip-compile non filtra per data (`dbb615b`, testa del lock). Poi approvato.
- **Opzione A, librerie apt a versione fissata** (2026-10-25): `libgomp1=14.2.0-19`, `libgl1=1.7.0-1+b2`, `libglib2.0-0t64=2.84.4-3~deb13u5` (`spikes/docker/paddle.Dockerfile:41-51`, `24654ca`). Le 38 dipendenze tirate da apt non hanno versione fissata nel Dockerfile; elenco in `RISULTATI-ocr.md:81` e in `/opt/paddle-tools/apt-installed.txt` dell'immagine.
- **Opzione (a), un solo tentativo con detection ridotta** (2026-10-25): `--enable_mkldnn False --text_det_limit_type max --text_det_limit_side_len 960`, fissati in `score.rs:613-620` (`7490cba`) prima della misura `106f278`.
- **D-28, D-34, D-39, D-40** (già in `01-CONTEXT.md`): worker a 2 GiB, prova JBIG2 sul render, TEST-14 coperto dalla 1.5, M10 parse dichiarata in ADR.

### Deviazioni decise dal coordinatore

- **test1 atteso `uv==0.13.5`** (`307de01`, scritto dall'orchestratore e non da un esecutore): il test del test-write chiedeva `pip-tools==7.4.1` e `pip==24.3.1` nel commento di testa; il nuovo assert controlla solo `uv==0.13.5` (`main.rs:953-957`). L'acceptance del Task 2 cita ancora pip-tools: il piano non si modifica per non invalidare risk_audit_26.
- **`paddle-weights.sha256` con sha256 completi a 64 caratteri**: è un manifest d'integrità che `weights_manifest_check` (`score.rs:673-690`) confronta per intero, non un'evidenza di misura, a cui si applica il troncamento a 16.

### Scelte degli esecutori

- **Picco del discendente nel `vm_hwm_kb` del figlio** (`main.rs:183-200`, `:443-478`): senza, il picco del job sarebbe stato quello del solo figlio `ocr`.
- **Cartella corrente di `tesseract` = config Debian** (`main.rs:106`, `:417`): l'argv del test resta invariato e `worker.Dockerfile` (01-07) non si tocca.
- **Job ostili come file** (`run_hostile_render.sh:115`): `session.sh exec` non inoltra lo stdin.
- **`ttq-select` con `page_score` copiato da 01-08** (`ttq.rs:240-262`), PDF misto con pagine JPEG (Flate di pagine bianche oltre il rapporto 100 del prescan), seme del rumore 1301, sha256 troncati a 16 caratteri nelle evidenze.
- **Casi della 1.5 nello stesso `hostile-render.csv`**, con il prefisso del detail nel pass (`score.rs:1257-1258`).

## Esiti delle soglie

| soglia | esito | numeri |
|---|---|---|
| CER Tesseract buone ≤ 3% | **non raggiunta** | 3,16% (p-005 8,36%, p-006 4,70%); degradate 2,74% senza soglia |
| bbox analitiche (≤ 1 pt / ≤ 3 pt) | pass 8/8 | 0,48-0,84 pt e 1,34-2,66 pt; stadio 1 7,1 × 10⁻¹³ px |
| bbox contro PDFium | non valutabile | IoU mediana 0,55-0,69, plafond 0,666 |
| 25 pagine ≤ 30 s con N = 8 | **non raggiunta** | 34,5-34,9 s con `max_admitted` 2; M10 conforme 12/12 |
| render ostile, casi 1.5 | pass 9/9 | `DecompressionLimit`, `dct_sof`/`jpx_siz over_cap` |
| render ostile, T-09 JBIG2 | **aperto** | `jbig2-malformed` `ok` 3/3; `deep-nesting` `ok` 3/3; contenimento 15/15 |
| TTQ-parse p95 (5 s / 20 s) | **non raggiunta** | 20.188 ms (n 20) e 73.072 ms (n 10); misto 11.626 ms |
| «vince nettamente» PP-OCRv6 | no, Tesseract confermato | CER 13,57% / 14,19%, 23,3 / 20,8 s per pagina; bbox analitiche 0/8 (rettangoli di riga) |

## Deviations from Plan

### Livello a

1. **`calibrate --op ocr_tesseract` esce 2 con `BudgetExceeded`.** Difetto del file chiuso `parse-parent.rs` di 01-07: la calibrazione prenota `capacity − 256 MiB` sia come `memory_reservation_bytes` sia come tetto (`parse-parent.rs:599-610`) e il budget moltiplica per (1 + discendenti) (`isolate.rs:120-125`): (1.792 − 256) × 2 + 256 = 3.328 MiB oltre 1.792. Ripiego: `ocr limits-from-runs` con la stessa formula su 25 job benigni (`score.rs:1471-1556`, `c874b42`). Non corretto qui: **gap-closure con `riapre_perimetro: [1]`**.
2. **Config `tsv` di Tesseract** non trovata in `tessdata_best` (`d67a748`): cartella corrente del discendente sulla cartella dei config Debian.
3. **`|| true` sull'aiuto della CLI** nel Dockerfile scambiava un import fallito per «flag assente» (`fb83602`): ora l'exit si registra in `cli-help.exit` (`paddle.Dockerfile:58-60`).
4. **`os.walk` trovava `locks/official_models`** e dava un `family_check` falso negativo (`24654ca`): cartella esatta della cache e `model_name` da `_merged_paddlex_config`, resolver unico per build e sessione.
5. **`hw.json` normalizzato** allo schema di `common::manifest`, originale in `results/raw` (difetto noto `hw-manifest.sh` / `common::manifest`).
6. **clippy `type_complexity`** sui test a tabella del test-write: `allow` a livello di modulo in `score.rs`, test non toccati.
7. **Tempo ammortizzato di PP-OCRv6 non misurato** (il piano lo chiedeva «a parte», senza effetto sulla regola): un solo tentativo.
8. **Diagnosi fuori comando** (`--enable_mkldnn False`, batch 1, `cpu_threads 1`) solo come diagnosi, nessuna riga di misura; driver della sessione v6 come comandi dell'esecutore fuori repo, copiati in `results/raw/paddle/<run>/`.
9. **Test con `bash` su host Windows** (test13/test10 exit 2) danno 127: verdi nel runner, che è il gate.

### Livello c

- **Docker Desktop fermato più volte dall'utente** durante il TTQ: serie spostate in `results/raw/invalid-ttq-docker-restart*` (ignorata da git, non verificabile dal bridge), TTQ rilanciato da capo nella sessione `01-13-20261024t143200z-384b`; le misure OCR, throughput e ostili erano già concluse. Un'altra ripresa (continuazione 1) si è fermata prima di ogni build per Docker spento.

### Scostamenti dall'acceptance scritta

- `paddle-requirements.lock` non contiene `pip-tools==7.4.1` nel commento di testa (decisione utente sul cooldown, deviazione del coordinatore).
- Il `grep -n 'Command::new("sh"\|Command::new("bash"'` dell'acceptance trova 4 righe, tutte nei test (`main.rs:903-904` letterali della guardia, `:1064` e `:1091` test che lanciano gli script); il codice non di test è pulito, e lo verifica `test8_no_shell_and_no_omp_override_in_ocr_sources`.
- Il `<verify>` del Task 2 lancia lo scanner senza `--fail-on`: col default MEDIUM esce 1 per il MEDIUM noto; gli esecutori e il bridge l'hanno eseguito con `--fail-on HIGH` (exit 0).
- `run_hostile_render.sh` salva `<job_id>.stdout` e `<job_id>.meta` invece di `<job_id>.json`; il contenuto (riga `RunReport`, exit, stato del worker) è quello richiesto.

**Total deviations:** 9 di livello a, 1 di livello c, 2 decise dal coordinatore, 3 decisioni utente; nessuna di livello b assorbita (il difetto di `calibrate` è registrato e instradato).
**Impact on plan:** nessuna estensione di perimetro (23 file su 23).

## Discrepanze fra rapporto e prove

- **Versione di setuptools.** Il rapporto principale cita `setuptools==84.2.1` aggiunto con `--allow-unsafe`: vale per il primo lock (`3314f9d`). Il lock in vigore, rigenerato con uv in `dbb615b`, ha `setuptools==84.0.0` e non dice più `--allow-unsafe`. Superato, non contraddetto.
- **Conteggio dei test.** 43 + 4 nel primo rapporto, 46 + 4 e poi 47 + 4 nelle riprese: coincide con i 3 test `test9_paddle_*` di `8b75a99` e `test2_v6_measure_args_*` di `7490cba`. Il bridge conta 51 `#[test]` di cui 4 ignorati.
- **Autocontrolli, TTQ invalidi, log PaddleOCR, `ocr-hw-raw.json`, `ttq-selfcheck.csv`:** stanno in `results/raw/`, ignorata da git; il bridge non li vede.
- **Il resto coincide** con le prove rilette o ricalcolate: commit e ordine (soglie prima delle misure, pesi prima delle righe v6), conteggi dei cinque CSV, p95 del TTQ, prenotazione OCR, sha256 dei pesi contro RISULTATI, apt a versione fissata, scanner.

## Threat Flags

- **T-01-13-SC (pacchetti pip):** lock a hash con cooldown, `--require-hashes`, approvazione utente; i pacchetti apt hanno avuto una decisione separata.
- **T-01-13-W (pesi):** manifest committato prima della misura, `diff` e configurazione risolta confrontati in sessione.
- **T-01-13-01 (render e Tesseract):** solo job del lanciatore; render ostile contenuto su 15 righe su 15; **T-09 JBIG2 resta aperto**.
- **T-01-13-02 (OCR concorrente):** ammissione dal budget, `max_admitted` 2, `oom_kill_delta` 0; prenotazione stimata e non tetto rigido (RISK-001).

## Known Stubs

Nessuno.

## User Setup Required

Nessuno.

## Punti aperti

- **Difetto di `calibrate`** in `parse-parent.rs` (01-07) per le op con discendenti: gap-closure con `riapre_perimetro: [1]`.
- **T-09 JBIG2 aperto** e `deep-nesting` non visitato dalla render.
- **Soglie mancate e dichiarate:** CER 3,16%, throughput 34,5 s, TTQ-parse 20,2 s e 73,1 s (scelta fra le idee di D-27 all'utente).
- **38 dipendenze apt indirette non fissate** nel Dockerfile dello sfidante.
- **Ratifica di ADR-0004** nel consolidamento (01-17, Task 2), come dice il campo «Decisori»; la riga di firma che diceva /verify 1 è stata allineata dall'orchestratore al bridge.
- **Scanner e hook:** lo scanner con scope vuoto esce «NON ESEGUITO [security-scope-empty]» e l'hook di pre-commit blocca i commit di soli dati (è successo con `654523c`, risolto includendo RISULTATI nel commit); segnalato anche uno scanner che esce 0 senza percorsi, che il bridge non ha riprodotto (senza argomenti la CLI esce 2): TODO per /verify 1.
- **Test di isolamento di 01-07 sensibili al carico** (`tests/isolation.rs`: `budget_admits_at_most_two_jobs_together`, `dropped_future_kills_group_and_cleans_within_one_second`, `quota_counts_job_dir_plus_out_dir`): tre rossi di temporizzazione al primo giro del gate sul main fuso, verdi al rilancio. Per /verify 1.
- **«Esito parziale» di RISULTATI-ocr.md** apre ancora con «Tesseract confermato in via provvisoria» seguito dagli aggiornamenti; la conclusione vera è in coda alla stessa voce.

## Next Phase Readiness

- **01-17 (consolidamento)** porta la riga OCR/parser di `DEPENDENCIES.md` a Decisa secondo ADR-0004 dopo la ratifica.
- **S-07 / porta `DocumentParser`:** errori tipizzati, parsing e OCR fuori processo, concorrenza decisa dal budget (`RISULTATI.md`, «Impatto su porte»).
- **Gap-closure di 01-07** su `calibrate` prima che un'altra op con discendenti debba calibrarsi.

## Self-Check: PASSED

- 23 file su 23 dei `files_modified` nel diff `05d1760..task/PLAN-010-13`, nessun file di altre fasi; `d13e23a^2` = `4436d90` = punta del branch.
- Ricontati o ricalcolati dal bridge: cinque CSV, p95 del TTQ, prenotazione `ocr_tesseract`, sha256 dei pesi contro RISULTATI, ordine dei commit di soglie, pesi e misure, invarianza delle Soglie, grep dell'acceptance, scanner con soglia HIGH (exit 0, 1 MEDIUM giustificato).
- Gate sul main fuso `d13e23a` (nota dell'orchestratore): cargo fmt --check ok; clippy host --workspace --all-targets --locked --offline -D warnings ok; cargo test -p s02-parser nel runner, «Compiling s02-parser» visto. Primo giro: 3 rossi di temporizzazione in `tests/isolation.rs` (`budget_admits_at_most_two_jobs_together` wall 17,1 s; `dropped_future_kills_group_and_cleans_within_one_second` «permesso del budget non restituito dopo il drop: Elapsed»; `quota_counts_job_dir_plus_out_dir` «quota non rilevata in tempo») con la macchina sotto carico, su file non toccati da 01-13 (`git diff 05d1760..HEAD` vuoto su `tests/`, `job.rs`, `isolate.rs`, `limits.json`). Rilancio della sola suite isolation: 37/37 in 2,12 s; rilancio completo: 380 passati, 0 rossi, 8 ignorati. Scanner con soglia HIGH su `spikes/s02-parser`, `docs/adr`, `spikes/docker`: verde con avvisi sotto soglia.
- Test del piano nel worktree, per gli esecutori: runner `ocr` 47 passati + 4 ignorati; ignorati verdi sull'host.

**Modifica:** Claude (Anthropic) · 2026-10-25 · creazione (bridge di 01-13, passi 1-3)
