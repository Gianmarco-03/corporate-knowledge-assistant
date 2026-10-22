---
phase: 01-spike-tecnici
plan: 07
subsystem: spike-isolamento-parser-prescan-calibrazione
tags: [s02-parser, isolamento, processo-figlio, pre_exec, setrlimit, rlimit-as, oom_score_adj, process-group, killpg, subreaper, tokio, semaforo, budget-aggregato, job-json, allowlist, deny_unknown_fields, toctou, lstat, prescan, lopdf, filter-func, object-stream, decompression-bomb, flate2, take, pdfium, chromium-8100, password-vuota, allegati, docx, zip, tesseract, tessdata_best, calibrazione, vmhwm, vmpeak]

# Dependency graph
requires:
  - phase: 01-01
    provides: "workspace degli spike, crate s02-parser con le dipendenze dichiarate (pdfium-render, rlimit, nix, lopdf, flate2, zip, tokio, serde, clap, anyhow, common), crate common (csv::Writer, manifest)"
  - phase: 01-02
    provides: "harness Compose chiuso con il servizio worker (network none, read_only, tmpfs /work, mem 2g, pids 256, cap_drop ALL, utente 10001), session.sh, hw-manifest.sh, runner Linux"
  - phase: 01-03
    provides: "corpus benigno spikes/data/corpus/raw scaricato da fetch.sh con sha256 verificati"
  - phase: 01-05
    provides: "ordine delle onde: la calibrazione CPU di 01-07 non si sovrappone alle misure di 01-05"
  - phase: 01.3
    provides: "gap-closure del tmpfs /work del worker (uid/gid 10001, modo 0700), fusa sul branch prima della ripresa"
provides:
  - "Contratto W17 del lanciatore isolato (test rossi a6cd619 e 056911a, implementazione f25b3b9, ce35b04, 73f448c): job JSON chiuso con allowlist op → bin fisso, esiti tipizzati ChildOutcome/Kind, RunReport del padre, Limits con memory_reservation_bytes, Budget a semaforo pesato, classify pura"
  - "Lanciatore run_child: pre_exec con oom_score_adj 1000 e setrlimit AS/CPU/FSIZE/NOFILE/CORE, process group proprio, GroupGuard con killpg, reaping asincrono e rimozione della cartella su ogni uscita, quota cartella del job + out_dir, stdout a 1 MiB, stderr a 4 KiB, subreaper nel padre"
  - "Prescan v6: scansione a byte, prima passata lopdf con defer_objstm, contatore unico I1 (512 MiB distinti con arresto), lavoro I2 con limite B, ramo cifrato a password vuota con riconciliazione della xref, allegati contati e mai estratti, DOCX con limiti zip"
  - "Figlio parse-child PDFium/DOCX con i sei campi dello schema unico in data; driver hostile, budget e overhead in parse-parent per 01-18; immagine worker con PDFium chromium/8100, Tesseract 5.5.0 e tessdata_best verificati (46fd539)"
  - "Calibrazione su 10 PDF benigni (535e3a2): 10/10 ok, nessuna anomalia; limits.json profilo default e parse con prenotazione 782,9 MiB e AS 1.792 MiB; 2 job parse insieme nel budget del worker da 2 GiB"
affects: [01-08, 01-09, 01-13, 01-18, ADR-0004, porta DocumentParser, worker di ingestion del Document Service]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Figlio per job lanciato solo da un job JSON tipizzato: enum serde adiacente (op + params) con deny_unknown_fields, bin da un registro chiuso di letterali, argv costruito dal padre, mai una shell"
    - "Spec sigillata: ChildSpec ha un campo privato Sealed, quindi si costruisce solo con resolve, e run_child rifiuta uno spec i cui campi pubblici non coincidono con la copia sigillata"
    - "pre_exec solo con syscall async-signal-safe su valori copiati: oom_score_adj alzato dal figlio prima dell'exec, setrlimit con errno, _exit senza exec se qualcosa non si applica"
    - "Guardia RAII del gruppo: Drop invia killpg subito e affida reaping, rimozione della cartella e rilascio del permesso a un task sul runtime, oppure ripiega su un percorso sincrono senza attese"
    - "Budget aggregato come semaforo pesato in MiB: la prenotazione è 3 × VmHWM benigno × (1 + discendenti) + quota d'uscita e si rilascia solo dopo il cleanup"
    - "Prescan in due passate sul ramo non cifrato: FilterFunc che rinomina il /Type degli object stream per rinviarne l'espansione, decodifica in streaming con take(cap + 1) e un contatore unico, caricamento vero solo se il totale regge"
    - "Fallire chiusi sulle perdite silenziose di lopdf: riconciliazione di ogni voce in uso della xref sul ramo cifrato, strict prima del tollerante sul ramo non cifrato, Flate non decodificabile → Corrupt"

key-files:
  created:
    - spikes/s02-parser/src/protocol.rs
    - spikes/s02-parser/src/job.rs
    - spikes/s02-parser/src/isolate.rs
    - spikes/s02-parser/src/prescan.rs
    - spikes/s02-parser/src/bin/parse-parent.rs
    - spikes/s02-parser/src/bin/parse-child.rs
    - spikes/s02-parser/src/bin/fake-child.rs
    - spikes/s02-parser/tests/isolation.rs
    - spikes/docker/worker.Dockerfile
    - spikes/s02-parser/results/isolation/hw.json
    - spikes/s02-parser/results/isolation/limits.json
    - spikes/s02-parser/results/isolation/calibration.csv
  modified:
    - spikes/s02-parser/src/lib.rs

key-decisions:
  - "Ripasso dell'utente del 2026-10-21 (D-05, D-08, D-25 emendate): object stream sulla somma reale ≤ 512 MiB e non più sul numero, PDF con password utente vuota ammessi, allegati contati e mai estratti, prenotazione del budget su 3 × VmHWM con RLIMIT_AS da VmPeak solo come tetto rigido"
  - "Quarto giro di audit del ripasso, oltre il limite di tre, autorizzato dall'utente il 2026-10-22 (plan_version 6, risk_audit_24 APPROVED_WITH_WARNINGS)"
  - "oom_score_adj alzato dal figlio in pre_exec invece che dal padre dopo lo spawn: sui figli brevi la scrittura del padre dava EACCES"
  - "Ramo non cifrato caricato prima in strict: in modalità tollerante lopdf 0.45.0 scarta in silenzio un object stream oltre il tetto per stream"
  - "PDFium apre il PDF dagli stessi byte già passati dal prescan (load_pdf_from_byte_slice)"
  - "S02_BUDGET_TOTAL_BYTES e S02_PARENT_RESERVE_BYTES, lette solo dal padre, danno un Budget::fixed nei test del runner, che non ha memory.max"

patterns-established:
  - "Un consumatore del lanciatore (01-08, 01-09, 01-13) aggiunge il proprio bin solo come variante già elencata dell'allowlist e ricostruisce l'immagine; i limiti propri li calcola con parse-parent calibrate --op, con la stessa formula"
  - "Ogni esito del figlio è una riga JSON ChildOutcome; un job non valido è job_error con exit 2 e non lancia nulla; una misura non isolata esce 3"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "Il parsing avviene in un processo figlio per job con limite di memoria, timeout rigido, tetto ai byte decompressi, in un container senza rete, read-only, non root (D-08)"
    verification:
      - {test: TEST-3, risks: [RISK-003 di risk_audit_3], ref: "spikes/s02-parser/tests/isolation.rs:887#alloc_over_address_space_is_resource_limit ; :737#dpi_out_of_domain_is_invalid_params ; :775#reservation_over_budget_is_budget_exceeded_without_spawn", status: pass, evidence: "gate del coordinatore nel runner Linux sul main fuso: isolation 37/37 in quattro corse; non rieseguito dal bridge"}
      - {test: TEST-5, ref: "spikes/s02-parser/tests/isolation.rs:960#timeout_kills_grandchild_group_and_removes_job_dir ; :1054#externally_killed_child_is_crashed", status: pass, evidence: "gate del coordinatore nel runner: verdi"}
      - {test: TEST-10, ref: "spikes/docker/worker.Dockerfile:56 USER 10001:10001 ; :58 CMD sleep infinity", status: pass, evidence: "grep del bridge: USER 10001 una volta, CMD una volta; rete, read_only e cap_drop vengono dal compose.yaml di 01-02"}
      - {test: "TEST-11 (proprietà del container a runtime)", ref: "piano 01-18, misura sull'artefatto di 01-07", status: human_judgment, human_judgment: true, rationale: "UID 10001 effettivo, rete assente e filesystem read-only durante un job ostile non sono provati da un test di questo piano: li misura 01-18 (risk_audit_15 RISK-001 MEDIUM)"}
  - must_have: "Un figlio si lancia solo tramite un job JSON tipizzato con operazione in allowlist: nessun nome di bin né argomento libero arriva dal chiamante (contratto per 01-08, 01-09, 01-13)"
    verification:
      - {test: TEST-1, ref: "spikes/s02-parser/tests/isolation.rs:286#argv_echo_passes_path_with_spaces_and_non_ascii_unchanged ; :312#unknown_op_is_unknown_op ; :324 ; :333 ; :345 ; :351 ; :357 ; :372#resolve_builds_fixed_argv_per_op ; :459#missing_bin_is_child_missing ; :476#parse_parent_prints_job_error_and_exits_2 ; spikes/s02-parser/src/job.rs:173", status: pass, evidence: "gate del coordinatore nel runner: verdi; grep del bridge: deny_unknown_fields 7 volte in job.rs, nessun --child in parse-parent.rs, nessun Command::new di sh o bash"}
      - {test: TEST-2, ref: "spikes/s02-parser/tests/isolation.rs:549 ; :562 ; :574 ; :588#out_dir_wrong_forms_are_path_not_allowed ; :614 ; :647#existing_out_dir_is_out_dir_exists_and_launches_nothing ; :673#out_dir_symlink_planted_after_resolve_is_rejected_before_spawn ; :702#input_component_swapped_for_symlink_after_resolve_is_rejected", status: pass, evidence: "gate del coordinatore nel runner: verdi"}
  - must_have: "La somma delle prenotazioni (3 × VmHWM di picco benigno più la quota) più la riserva del padre non supera il cgroup; ogni figlio ha RLIMIT_AS come tetto rigido e oom_score_adj 1000, così un OOM del cgroup sceglie un figlio e non il padre"
    verification:
      - {test: TEST-3, ref: "spikes/s02-parser/tests/isolation.rs:833#budget_admits_alloc_jobs_and_all_complete ; :857#budget_admits_at_most_two_jobs_together ; spikes/s02-parser/src/isolate.rs:120#reservation_bytes", status: pass, evidence: "gate del coordinatore nel runner: verdi; diff 056911a..535e3a2 di isolation.rs controllato dal coordinatore e dal bridge: 9 righe memory_reservation_bytes (1 prevista dal test-write in limits(), 8 nei letterali del budget), nessuna asserzione toccata"}
      - {test: TEST-11, risks: [RISK-001 di risk_audit_24, RISK-004 di risk_audit_19], ref: "spikes/s02-parser/tests/isolation.rs:1263#child_is_the_designated_oom_victim ; :1348#batch_over_parent_reserve_exits_3_after_reports ; spikes/s02-parser/src/bin/parse-parent.rs:446-452", status: pass, evidence: "gate del coordinatore nel runner: verdi (oom_score_adj del figlio 1000, del processo del test 0; exit 3 e parent_reserve_exceeded dopo i rapporti)"}
      - {test: "TEST-11 (OOM vero del cgroup)", risks: [RISK-001 di risk_audit_24], ref: "piano 01-18, tests/oom.rs e soglia M10/M11", status: human_judgment, human_judgment: true, rationale: "la prenotazione non è un tetto rigido per costruzione (decisione utente del 2026-10-21); che all'OOM muoia il figlio e il padre resti vivo lo prova solo l'OOM esercitato in 01-18. T-01-07-06 resta aperta finché oom.json non vale verificato"}
      - {test: "calibrazione del budget", ref: "spikes/s02-parser/results/isolation/limits.json", status: pass, evidence: "ricalcolato dal bridge: memory_reservation_bytes 820.961.280 = 3 × 267.240 KiB; address_space_bytes 1.879.048.192 = 2 GiB − 256 MiB, perché 3 × 756.876 KiB = 2,17 GiB; prenotazione di un job 822.330.946 byte, due job 1,53 GiB entro 1.792 MiB, tre no"}
  - must_have: "Il gruppo di processi del job è ucciso e raccolto, e la cartella del job rimossa, su ogni uscita: successo, errore, timeout, eccesso di output, cancellazione del chiamante"
    verification:
      - {test: TEST-5, ref: "spikes/s02-parser/tests/isolation.rs:960 ; :988#dropped_future_kills_group_and_cleans_within_one_second ; :1035#flood_on_both_pipes_is_output_limit_within_two_seconds ; spikes/s02-parser/src/isolate.rs:463-503#Drop", status: pass, evidence: "gate del coordinatore nel runner: verdi, isolation ripetuto tre volte senza instabilità; grep del bridge: nessun std::thread::sleep in isolate.rs, nessun unwrap né expect nel Drop di GroupGuard"}
      - {test: TEST-9, ref: "spikes/s02-parser/tests/isolation.rs:1109 ; :1136#quota_counts_job_dir_plus_out_dir ; :1178 ; :1206#quota_scan_entry_cap_is_out_quota ; :1239#successful_parse_keeps_out_dir_and_removes_job_dir", status: pass, evidence: "gate del coordinatore nel runner: verdi"}
      - {test: TEST-6, ref: "spikes/s02-parser/src/isolate.rs:935-1056 (16 casi di classify) ; spikes/s02-parser/src/bin/parse-parent.rs:1309-1365 (7 casi di hostile_verdict) ; spikes/s02-parser/tests/isolation.rs:1090", status: pass, evidence: "gate del coordinatore nel runner: lib 84 e parse-parent 7 verdi"}
      - {test: TEST-4, risks: [RISK-001 di risk_audit_10], ref: "spikes/s02-parser/tests/isolation.rs:910#unapplicable_setrlimit_fails_the_spawn_with_errno ; spikes/s02-parser/src/isolate.rs:430-444", status: pass, evidence: "gate del coordinatore nel runner: verde; grep del bridge: last_os_error 2, errno 21 in isolate.rs, subreaper_unavailable errno in parse-parent.rs"}
      - {test: "TEST-4 (oom_adj e subreaper non applicabili)", risks: [RISK-001 di risk_audit_10], ref: "spikes/s02-parser/src/isolate.rs:420-427, :847-850, :882 ; spikes/s02-parser/src/bin/parse-parent.rs:158-161", status: human_judgment, human_judgment: true, rationale: "un fallimento di prctl o della scrittura di oom_score_adj non si provoca in un test: il percorso è letto nel codice (marcatore oom_adj_failed, oom_adj_set=false, esito ResourceLimit/oom_adj_unavailable; subreaper_unavailable ed exit 2), non eseguito"}
  - must_have: "Il prescan applica i tetti di decompressione già al caricamento (per stream, somma reale con arresto oltre 512 MiB, catene di filtri delimitate, 2.000 pagine) e ogni rifiuto è un esito tipizzato; la misura sui file ostili la fa 01-18 (D-05, D-08, D-25)"
    verification:
      - {test: TEST-8, ref: "spikes/s02-parser/src/prescan.rs:2043#many_small_objstm_are_not_rejected_by_count (k1) ; :2087#objstm_sum_over_total_stops_at_total_plus_one_block (k2) ; :2128 (k1e) ; :2150 (k2e) ; :2074 ; :2058 ; :2068 ; :2103 ; :2115", status: pass, evidence: "gate del coordinatore nel runner: lib 84 verdi; k1e e k2e usano qpdf, presente solo nel runner"}
      - {test: TEST-12, ref: "spikes/s02-parser/src/prescan.rs:2175#i1_shared_stream_is_counted_once ; :2184#i1_objstm_reexpansion_is_not_counted_twice ; :2200#i1_encrypted_counts_distinct_streams_once", status: pass, evidence: "gate del coordinatore nel runner: verdi"}
      - {test: TEST-13, ref: "spikes/s02-parser/src/prescan.rs:2231#i2_work_counts_every_xref_stream ; :342#work_bound", status: pass, evidence: "gate del coordinatore nel runner: verde; limiti I2 asseriti anche in k1, k1e, k2, k2e e nei test i1"}
      - {test: "TEST-7 (filtri e Flate)", ref: "spikes/s02-parser/src/prescan.rs:2316-2439 (filtri, immagini, Flate per stream, rapporto, totale)", status: pass, evidence: "gate del coordinatore nel runner: verdi"}
      - {test: "TEST-14 c2, c7", ref: "spikes/s02-parser/src/prescan.rs:2491 ; :2533 ; :2851#c7_undecodable_flate_stream_is_corrupt", status: pass, evidence: "gate del coordinatore nel runner: verdi"}
      - {test: "misura dei casi ostili", ref: "piano 01-18 (objstm-bomb, filter-chain-bomb, objstm-sum-bomb, 10k pagine)", status: human_judgment, human_judgment: true, rationale: "il must_have stesso rinvia la misura sui file ostili a 01-18; qui ci sono solo PDF costruiti nei test"}
  - must_have: "PDF che richiedono una password → Encrypted; cifrati solo per i permessi ammessi e analizzati decifrati; /JavaScript, /Launch in quarantena anche in object stream compressi; allegati ignorati, mai estratti, contati (D-05 emendata il 2026-10-21)"
    verification:
      - {test: TEST-7, ref: "spikes/s02-parser/src/prescan.rs:1905 (k3) ; :1915 (k4) ; :1926 ; :1943 ; :1964 ; :1997 (k5) ; :2011 (k6) ; :2023 ; :2244 ; :2257 ; :2275 ; :2288 ; :2295 ; :2302", status: pass, evidence: "gate del coordinatore nel runner: verdi; grep del bridge: nessuna stringa \"embedded_file\" in prescan.rs, nessun attachment in parse-child.rs"}
      - {test: "TEST-14 c3-c6", ref: "spikes/s02-parser/src/prescan.rs:2546 ; :2585 (c3) ; :2640 (c4) ; :2710 ; :2738 (c5) ; :2791 ; :2829 (c6)", status: pass, evidence: "gate del coordinatore nel runner: verdi; sull'host Windows i 12 test del ramo cifrato si fermano solo perché manca qpdf, condizione d'ambiente e non difetto"}
      - {test: "PDFium e password vuota (Task 4)", ref: "job pwd-k3, pwd-k4, pwd-edge nel worker (results/raw/isolation, ignorata da git, nel worktree di 01-07)", status: pass, evidence: "letto dal bridge nel log grezzo: k3 ok con encryption empty_password, 1 pagina, 4 caratteri; k4 failed Encrypted/password_required"}
      - {test: "PDFium, caso proprietario vuota e utente non vuota", ref: "spikes/s02-parser/src/bin/parse-child.rs:117-124 ; job pwd-edge", status: human_judgment, human_judgment: true, rationale: "esito failed Encrypted/pdfium_password: lopdf autentica la stringa vuota come password del proprietario, PDFium chromium/8100 senza password risponde PasswordError. Esito chiuso, mai ok, ma diverso dall'assunzione del piano (punto 3-bis: «si apre senza chiedere nulla sia in lopdf sia in PDFium»): la regola va confermata dall'utente"}
  - must_have: "PDFium chromium/8100 e tessdata_best ita+eng installati con hash verificato; Tesseract 5.5.0 di Debian trixie (D-07, D-09)"
    verification:
      - {test: TEST-10, ref: "spikes/docker/worker.Dockerfile:23 ; :33 ; :36-45 ; spikes/s02-parser/results/isolation/hw.json tools", status: pass, evidence: "grep del bridge: sha256sum -c sul tgz di PDFium e sui tre traineddata; hw.json registra tesseract 5.5.0 e pdfium chromium/8100, immagine cka-spikes-worker@sha256:cc54ac3f…; fallback chromium/7881 non usato"}
  - must_have: "Limiti per figlio calibrati sugli input benigni con la formula delle Soglie e pubblicati in results/isolation/limits.json (profilo default, letto da parse-parent e da 01-09); calibrate --op applica la stessa formula per 01-08 e 01-13"
    verification:
      - {test: TEST-10, ref: "spikes/s02-parser/results/isolation/limits.json ; calibration.csv ; spikes/s02-parser/src/bin/parse-parent.rs:556-756#cmd_calibrate", status: pass, evidence: "verify del Task 4 rieseguito dal bridge: VERIFY-OK (JSON validi, profilo default, header esatto con fine riga LF); 10 righe, una per PDF di /data/corpus/raw"}
      - {test: TEST-15, ref: "spikes/s02-parser/tests/isolation.rs:1611#parse_child_data_schema ; spikes/s02-parser/src/prescan.rs:2909#benign_docx_passes_with_none_and_zero_schema ; spikes/s02-parser/src/bin/parse-child.rs:57-70", status: pass, evidence: "gate del coordinatore nel runner: verde, con PDFium chromium/8100 copiato in spikes/target/pdfium (ignorato da git), perché l'immagine del runner non lo ha; grep del bridge: i sei nomi in parse-child.rs"}
      - {test: "calibrate --op per altre op", ref: "spikes/s02-parser/src/bin/parse-parent.rs:556-756", status: human_judgment, human_judgment: true, rationale: "calibrate è stato eseguito solo con --op parse: i bin extract, locator e ocr non esistono ancora. Che la formula valga identica per le altre op si legge nel codice (stessa funzione, profilo per op in limits.json), non è esercitato"}
  - must_have: "Scanner di sicurezza con exit 0 e MEDIUM motivati (acceptance dei Task 1-3)"
    verification:
      - {test: "security-scanner", ref: "python skills/security-scanner/security_scan.py --fail-on HIGH spikes/s02-parser/src spikes/docker", status: pass, evidence: "rieseguito dal bridge: exit 0, 0 CRITICAL, 0 HIGH, 31 MEDIUM (25 pdf-untrusted-parse, 6 archive-decompress), motivati nella sezione Scanner di sicurezza"}

# Metrics
duration: "circa 41 min il 2026-10-21 (test rossi a6cd619 17:20 → immagine worker 46fd539 18:01, poi il fermo B1-B4) e circa 45 min il 2026-10-22 (ripresa ce35b04 16:57 → calibrazione 535e3a2 17:40); merge su main 4a9129e 17:42"
completed: 2026-10-22
---

# Fase 1 Piano 07: S-02, lanciatore isolato, prescan e calibrazione · Summary

**Il parsing dei documenti gira in un figlio per job, lanciato solo da un job JSON con allowlist chiusa, con `oom_score_adj` 1000 e `setrlimit` applicati prima dell'exec e il gruppo di processi ucciso, raccolto e ripulito su ogni uscita. Il prescan v6 ferma la somma reale dei byte decompressi a 512 MiB, ammette i PDF a password vuota con riconciliazione della xref e conta gli allegati senza estrarli. Sui 10 PDF benigni del corpus la calibrazione dà 10 ok su 10, nessuna anomalia, prenotazione 782,9 MiB e 2 job `parse` insieme nel worker da 2 GiB. Un caso resta all'utente: con password del proprietario vuota e utente non vuota PDFium rifiuta (`Encrypted`/`pdfium_password`), contro l'assunzione del piano**

## Performance

- **Duration:** due blocchi. Il 2026-10-21 i test rossi `a6cd619` (17:20:45) precedono il lanciatore `f25b3b9` (17:58:47) e l'immagine `46fd539` (18:01:05). Poi il piano si ferma sui blocchi B1-B4 (vedi Decisioni). Il 2026-10-22, dopo ripasso e audit, la ripresa va da `ce35b04` (16:57:44) alla calibrazione `535e3a2` (17:40:34). Il merge su main è `4a9129e` (17:42:52).
- **Started:** 2026-10-21T17:20:45+02:00
- **Completed:** 2026-10-22T17:42:52+02:00
- **Tasks:** 4 di 4 (piano v6)
- **Files modified:** 13 su 13 `files_modified` del piano (`git diff --stat 4a9129e^1 4a9129e`), nessun file di codice fuori elenco. In più sette Markdown di pianificazione (PLAN, TEST-DESIGN, RISK-AUDIT v4-v6, DEEPSEARCH-6, GATE), ammessi da qualsiasi fase. il gate `scripts/gate-proprieta-file.cjs`: conforme. `Cargo.toml` e `Cargo.lock` non toccati.

## Accomplishments

- **Job JSON chiuso** (`spikes/s02-parser/src/job.rs`). `JobWire` con `deny_unknown_fields` (`job.rs:235-244`) legge l'`op` come stringa, che si confronta con `OPS` (`:306`) per dare `UnknownOp`. Poi l'enum `Op` adiacente `op` + `params` (`:172-187`), anch'esso chiuso, valida i parametri. `parse_out_dir` (`:349`) accetta solo la forma esatta `/results/raw/<area>/<job_id>`. `resolve_in` (`:458-557`) costruisce argv ed env per op, canonicalizza `input` ed `excerpt_file` entro le radici e registra `(dev, inode)` per il ricontrollo. `ChildSpec` porta un campo privato `Sealed` (`:405-433`).
- **Lanciatore** (`spikes/s02-parser/src/isolate.rs`). `run_child` (`isolate.rs:207-220`) rifiuta uno spec manomesso e prenota il budget. `imp::run` (`:689-894`) fa i controlli `lstat` e la creazione esclusiva in `spawn_blocking`, lancia il bin del registro con `env_clear`, `process_group(0)`, `kill_on_drop` e `pre_exec(child_setup)` (`:713-734`), poi un `select!` fra uscita del figlio, overflow dello stdout, scadenza di parete e scansione della quota ogni 250 ms (`:787-820`). `classify` è pura (`:230-266`).
- **Budget** (`isolate.rs:64-158`): semaforo `tokio` pesato in MiB con capacità totale − riserva. La prenotazione è `memory_reservation_bytes × (1 + discendenti) + out_quota_bytes` (`:120-125`) e si rilascia solo quando la guardia del gruppo lascia andare il permesso.
- **Padre** (`spikes/s02-parser/src/bin/parse-parent.rs`): subreaper all'avvio (`:158-162`), `run`, `batch` con `parent_reserve_exceeded` ed exit 3 (`:444-452`), `calibrate` (`:556-756`), `hostile_verdict` (`:765-790`) e i driver `hostile`, `budget`, `overhead` per 01-18.
- **Prescan v6** (`spikes/s02-parser/src/prescan.rs`): scansione a byte (`:247-286`), contatore `Meter` di I1 e I2 (`:292-339`), ramo non cifrato `check_plain` (`:916-961`), ramo cifrato `check_encrypted` (`:964-1030`) con `reconcile` (`:429-465`), allegati (`:846-857`), DOCX (`:1105`).
- **Figlio** (`spikes/s02-parser/src/bin/parse-child.rs`): magic bytes, prescan, PDFium dagli stessi byte (`parse-child.rs:117-124`), testo in `text.txt`, i sei campi di `data` (`:57-70`).
- **Immagine worker** (`spikes/docker/worker.Dockerfile`): build `rust:1.99.1-slim-trixie`, PDFium `chromium/8100` e tessdata_best verificati con `sha256sum -c`, `USER 10001:10001`, `CMD ["sleep","infinity"]`.
- **Calibrazione** (`spikes/s02-parser/results/isolation/`): `limits.json`, `calibration.csv` (10 righe), `hw.json` normalizzato.

## Task Commits

1. **Test rossi Task 1-3, plan_version 3 (agente test-write, distinto dall'esecutore):** `a6cd619` (test). RED_EXPECTED: lib 54 falliti, parse-parent 7, isolation 34, tutti su `todo!`.
2. **Task 2: job JSON e lanciatore:** `f25b3b9` (feat).
3. **Task 1: immagine worker:** `46fd539` (feat).
4. **Fermo B1-B4 e ripasso:** `66be9b6` (plan v4, decisione utente), `d0d1113` (v5 da risk_audit_19), `7ce2183` (v6 da risk_audit_21), `d13b527` (risk_audit_24 e test_design_11 agli atti), `8b35978` (main fuso nel branch, con la gap-closure 1.3).
5. **Task 3, stato intermedio con le regole v3:** `ce35b04` (feat).
6. **Test rossi v6 (test-write separato):** `056911a` (test). RED_EXPECTED: lib 23 falliti su 84, isolation 2 su 37.
7. **Task 3 riallineato alla v6 (tocca anche file del Task 2):** `73f448c` (feat).
8. **Task 4: calibrazione:** `535e3a2` (test).

**Merge:** `4a9129e`. **Plan metadata:** questo SUMMARY e `01-07-RACCONTO.md`, scritti dal bridge senza commit.

## Files Created/Modified

- `spikes/s02-parser/src/protocol.rs`: `Kind`, `ChildOutcome`, `emit` con `vm_hwm_kb`, `parse_line`.
- `spikes/s02-parser/src/job.rs`: job JSON, allowlist, `ChildSpec` sigillato, `resolve`; test di forma.
- `spikes/s02-parser/src/isolate.rs`: `Limits`, `Budget`, `RunReport`, `run_child`, `classify`, `GroupGuard`; tabella (i) di `classify`.
- `spikes/s02-parser/src/prescan.rs`: prescan PDF e DOCX; TEST-7, 8, 12, 13, 14 e parte di 15.
- `spikes/s02-parser/src/bin/parse-parent.rs`: CLI del padre, calibrazione, driver di 01-18; tabella (j) di `hostile_verdict`.
- `spikes/s02-parser/src/bin/parse-child.rs`: figlio PDFium/DOCX.
- `spikes/s02-parser/src/bin/fake-child.rs`: figlio di prova (`alloc`, `spawn-grandchild`, `flood`, `argv-echo`, `sleep`, `write`).
- `spikes/s02-parser/tests/isolation.rs`: 37 test di integrazione, solo Linux.
- `spikes/s02-parser/src/lib.rs`: i quattro moduli.
- `spikes/docker/worker.Dockerfile`, `spikes/s02-parser/results/isolation/{hw.json,limits.json,calibration.csv}`.

## Decisions Made

### Decisioni dell'utente

- **Ripasso del 2026-10-21.** Il piano si era fermato dopo lo smoke nel worker: il prescan v3 rifiutava 8 PDF benigni su 8, e 3 × `VmPeak` di un PDF da una pagina valeva circa 1,8 GiB, quindi nel budget entrava un job alla volta. L'utente ha deciso quattro regole, che emendano D-05, D-08 e D-25 (`01-CONTEXT.md:26`, `:32`, `:60`):
  1. gli object stream non si rifiutano per numero: il tetto è la somma reale dei byte decompressi, ≤ 512 MiB, con arresto appena la supera; sul ramo cifrato vale in più una quota per stream min(64 MiB, ⌊512 MiB / n⌋);
  2. i PDF cifrati che si aprono con la password utente vuota si ammettono e si analizzano decifrati; quelli che chiedono una password restano `Encrypted`;
  3. gli allegati incorporati si contano e non si estraggono mai; non sono più un motivo di quarantena, ma i loro stream passano dagli stessi tetti;
  4. la prenotazione del budget si fa su 3 × `VmHWM` (memoria residente di picco); `RLIMIT_AS` = min(3 × `VmPeak`, budget − riserva) resta solo tetto rigido.
  La regola «soglie mai ritoccate sui risultati» resta: questo ritocco l'ha deciso l'utente, non l'esecutore (`01-07-PLAN.md:139`).
- **Quarto giro di audit, 2026-10-22.** La revisione v6 (schema unico dei sei campi di `data`, dopo il REVISE di risk_audit_21) è il quarto giro di audit del ripasso, oltre il limite di tre. L'ha autorizzato l'utente (`01-07-PLAN.md:357`, `01-GATE.md` giro 10). Esito: risk_audit_24 APPROVED_WITH_WARNINGS, un MEDIUM.

### Scelte dell'esecutore

- **`oom_score_adj` nel figlio**, in `pre_exec`, invece che dal padre dopo lo spawn. Un figlio breve può già essere uscito: la sua voce di `/proc` è di root e la scrittura del padre dava `EACCES` (errno 13). Il contratto osservabile non cambia: `oom_adj_set`, esito `ResourceLimit`/`oom_adj_unavailable`, marcatore `oom_adj_failed errno=<n>` (`isolate.rs:412-427`).
- **Bin dal registro chiuso con `PATH` = `S02_BIN_DIR`** (`isolate.rs:324-332`, `job.rs:477`) invece di `Command::new(spec.bin)`, che lo scanner segnalava HIGH `dynamic-exec`.
- **`Sealed` privato** invece di `#[non_exhaustive]`, che non impedisce di modificare i campi pubblici.
- **Strict prima del tollerante sul ramo non cifrato** (`prescan.rs:377-414`): in modalità tollerante lopdf 0.45.0 scarta in silenzio un object stream oltre il tetto. In strict l'errore arriva come `Error::Decompress(DecompressError::MemoryLimitExceeded)`, e questo chiude la [NV] del piano sulla variante d'errore.
- **I1 sul documento della prima passata**, dove ogni stream compare una volta; pagine, quarantena e allegati sul documento vero, con i membri espansi.
- **Lavoro analitico del ramo cifrato su max(conteggio a byte, contenitori caricati)** (`prescan.rs:993-999`): se il loader espandesse più contenitori di quelli contati, il limite n × quota non reggerebbe, e si fallisce chiusi con `work_bound`.
- **Id del dizionario `/Encrypt` da `encryption_state.encrypt_object_id()`**, API pubblica di lopdf 0.45.0 (`prescan.rs:430-433`), non dai byte.
- **Limiti fuori formula fissati prima della calibrazione:** CPU 60 s, parete 120 s, NOFILE 64, `file_size_bytes` = max(quota, 1 MiB).
- **SIGXFSZ → `ResourceLimit`/`rlimit_fsize`** in `classify` (`isolate.rs:247`), regola aggiunta.
- **PDFium dagli stessi byte del prescan** (`load_pdf_from_byte_slice`, `parse-child.rs:118`) invece di `load_pdf_from_file` del piano: PDFium vede esattamente il contenuto validato, senza una seconda lettura del file.

## Anomalie di calibrazione

**Nessun PDF benigno rifiutato: 10 ok su 10.** Per categoria: filtro 0, somma decompressa 0, quota per stream del ramo cifrato 0, password richiesta 0, oggetti scartati da lopdf 0. `_meta_parse.anomalies` di `limits.json` è vuoto e `ok_docs` vale 10.

Con le regole v3 lo smoke nel worker del 2026-10-21 rifiutava 8 PDF benigni su 8: per il conteggio o la stima degli object stream (12-312 per file), per la cifratura dei soli permessi del report della Bank of England, per l'allegato del report Berkshire. È stato quel risultato a fermare il piano (blocchi B3 e B4) e a portare al ripasso dell'utente.

Dati della calibrazione, da `calibration.csv` e dal log grezzo:

- **Corpus:** 10 PDF invece degli 8 attesi. `fetch.sh` ha completato il report BIS `aer-2025` (prima HTTP 403), e in `raw/` c'è anche l'estratto `der-it-01` di 25 pagine.
- **Cifratura:** `pub-en-03` (Bank of England) è cifrato solo per i permessi: ammesso con `empty_password`, PDFium ne legge 220 pagine. Ha 164 object stream, quindi la quota del ramo cifrato è 3,1 MiB per stream; il lavoro I2 vale 681.256.883 byte, entro B.
- **Allegati:** `pub-en-01` (Berkshire) ha 1 allegato, contato e non estratto, nessuna quarantena.
- **Object stream:** da 0 a 312 per file (massimo `pub-it-01`); somma reale massima 9,1 MB (`pub-it-01`).
- **Stream:** massimo 12,4 MB espansi da un solo stream. Rapporti Flate fino a 901 (BIS) e 834 (`pub-it-02`), su stream sotto 16 MiB: la regola del rapporto vale solo oltre 16 MiB, quindi sono ammessi.
- **Memoria:** `VmHWM` massimo 267.240 kB (`pub-it-01`), `VmPeak` massimo 756.876 kB (`der-it-01`). 3 × `VmPeak` = 2,17 GiB supera 1.792 MiB, quindi `address_space_bytes` vale il tetto del budget, 1.879.048.192 byte. La prenotazione è 782,9 MiB più 1,3 MiB di quota: **2 job `parse` insieme** nel budget da 1.792 MiB, non 3.
- **NV chiusi:** variante d'errore di lopdf sul tetto per stream (`Error::Decompress(DecompressError::MemoryLimitExceeded)`, solo in strict); errore PDFium su password (`PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError)`); ABI `chromium/8100` compatibile, fallback `chromium/7881` non usato.

## PDFium e password vuota

Tre job `parse` nel worker sulla build `chromium/8100`, con fixture generate da `qpdf` (job `pwd-k3`, `pwd-k4`, `pwd-edge`, run tag `842z5359`):

| fixture | esito | chi decide |
|---|---|---|
| (k3) password utente vuota, solo permessi | `ok`, `encryption = empty_password`, 1 pagina, 4 caratteri | lopdf autentica `""`, PDFium apre senza password |
| (k4) password utente non vuota | `failed`/`Encrypted`, detail `password_required` | il prescan, prima di PDFium |
| caso limite: proprietario vuota, utente non vuota | `failed`/`Encrypted`, detail `pdfium_password` | PDFium (`PasswordError`) |

**Il caso limite non va come il piano assumeva.** Il punto 3-bis del Task 3 (`01-07-PLAN.md:209`) dice che quel PDF «si apre senza chiedere nulla sia in lopdf sia in PDFium, quindi è ammesso». In realtà lopdf autentica la stringa vuota come password del proprietario e lo ammette, ma PDFium `chromium/8100`, aperto senza password, risponde con un errore di password. L'esito è chiuso, mai `ok`: il documento non entra. Ma la regola dichiarata e il comportamento divergono, e la scelta spetta all'utente: accettare che quei PDF restino rifiutati (e correggere la frase del piano e di D-05), oppure chiedere che il figlio provi PDFium con una password vuota esplicita. È un punto da portare all'utente, non un difetto da correggere in silenzio.

## Caratterizzazione di lopdf 0.45.0 (c1)-(c7)

| caso | esito | dato |
|---|---|---|
| (c1) versione | verde | `Cargo.lock` ha solo `lopdf 0.45.0`; il test legge il lockfile (`prescan.rs:2473`), `cargo tree -i lopdf` sull'host stampa `lopdf v0.45.0` |
| (c2) filtro sul ramo non cifrato | verde | con `defer_objstm` i membri non entrano in `Document::objects`, il contenitore resta compresso e Flate; gli oggetti normali sono uguali al caricamento senza filtro |
| (c3) ramo cifrato | verde | filtro ignorato, membri espansi, contenitori rimasti 3 su 3; un object stream oltre la quota (512 KiB con 8 object stream) → `DecompressionLimit`/`objstm_dropped` |
| (c4) forme della xref | verde con nota | xref in tabella (`xref_streams` 0), in stream (1), due revisioni classiche (0), due revisioni con xref stream non cifrate (2) |
| (c5) perdite silenziose | verde | oggetto cifrato illeggibile → `Corrupt`/`object_dropped`; object stream con AES alterato → `DecompressionLimit`/`objstm_dropped` |
| (c6) password richiesta | verde | `load_mem_with_options` restituisce `Ok` con `is_encrypted() = true` e un solo oggetto → `Encrypted`/`password_required`; `/Adobe.PubSec` → `password_required` |
| (c7) Flate rotto | verde | `Corrupt`/`flate_decode`; prima `drain_counted` ingoiava l'errore e dava `ok` |

**Nota su (c4).** Nell'aggiornamento incrementale, la voce liberata nella revisione 2 lopdf la vede ancora come `Normal` con l'offset della revisione 1: unendo le revisioni, tiene la voce vecchia. La riconciliazione controlla quindi anche una voce che non è più in uso, ma l'oggetto vecchio si carica e non c'è nessun falso `object_dropped`. Un falso drop è possibile solo se il corpo vecchio fosse illeggibile, e anche allora l'esito sarebbe chiuso (`Corrupt`), mai `ok`. Non è stato osservato e non ha fermato nulla. Il rischio residuo è un PDF benigno rifiutato, non un PDF ostile ammesso.

## Rischi degli audit e trattamento

- **risk_audit_3** (v1, 3 HIGH e 1 MEDIUM: CLI chiusa dei figli, tetto dopo il loader, cgroup condiviso, cleanup solo su timeout): risolti nella v2 e verificati da TEST-1, 3, 5, 8 e 9.
- **risk_audit_10** (v2, MEDIUM RISK-001: `oom_score_adj` e subreaper non applicabili non devono dare un job «isolato»): logica implementata (marcatore ed errno, `oom_adj_set=false`, `subreaper_unavailable` exit 2, `pass=false` in `hostile_verdict`). Il percorso d'errore non è eseguibile in test (`human_judgment` in coverage).
- **risk_audit_15** (v3, MEDIUM: evidenza runtime demandata a 01-18): resta aperto per costruzione, 01-18 è il gate di validazione di questo piano.
- **risk_audit_19** (v4, HIGH contratto con 01-18, MEDIUM I1/I2, lopdf, OOM): tabella «Contratto unico degli esiti», invarianti I1/I2, caratterizzazione (c1)-(c7), test (f2) e (f3), tutti verdi.
- **risk_audit_21** (v5, MEDIUM schema dei campi): schema unico, `parse_child_data_schema` verde.
- **risk_audit_24** (v6, APPROVED_WITH_WARNINGS, MEDIUM RISK-001: la prenotazione RSS non è un tetto rigido): rispettato senza aggiungere tetti. **Rischio residuo:** un figlio ostile può superare la prenotazione fino al proprio `RLIMIT_AS`; che il cgroup scelga il figlio e il padre resti vivo lo prova solo l'OOM vero di 01-18 (`tests/oom.rs`, M10/M11). Fino ad allora T-01-07-06 è aperta e `verify` la porta all'utente come rischio non provato, mai come verde.

## Scanner di sicurezza

`python skills/security-scanner/security_scan.py --fail-on HIGH spikes/s02-parser/src spikes/docker`, rieseguito dal bridge: exit 0, 0 CRITICAL, 0 HIGH, **31 MEDIUM** (25 `pdf-untrusted-parse`, 6 `archive-decompress`). L'hook pre-commit li tratta come avvisi da motivare qui (B1, `4441aef`).

- **22 nel codice del figlio e del prescan** (`prescan.rs` 20 righe di produzione, `parse-child.rs:18` e `:150`): parsing PDF, decompressione Flate e lettura zip girano solo nel figlio isolato, con `RLIMIT_AS`, CPU, FSIZE, NOFILE, timeout di parete, `oom_score_adj` 1000, container senza rete, read-only e non root, e con tetti `take(cap + 1)` su ogni lettura.
- **5 nei test del prescan** (`prescan.rs` sotto `#[cfg(test)]`): PDF costruiti nel test.
- **4 in `parse-parent.rs`** (`:1129`, `:1247`, `:1248`, `:1251`): sono il modo `in-process-mutex` del driver `overhead`, che apre PDFium **nel padre** di proposito. Il piano lo prevede (Task 2) solo per il confronto M9 di 01-18, su un input fidato che sceglie 01-18. Non è un percorso del prodotto: è l'unica eccezione alla regola «parsing solo nel figlio».

## Deviations from Plan

### Auto-fixed Issues (livello a)

**1. [Deviazione a] Aggiunte del test-write al seam W17.**
- **Found during:** test rossi `a6cd619`
- **Issue:** i test avevano bisogno di `job::parse(bytes)` per mappare op ignota e campo sconosciuto nei codici del contratto, e di campi pubblici di `ChildSpec` e `Caps` da ispezionare.
- **Fix:** aggiunte additive; `ChildSpec` resta non costruibile fuori da `resolve` grazie a `Sealed`.
- **Committed in:** `f25b3b9`

**2. [Deviazione a] `oom_score_adj` dal figlio in `pre_exec`** (vedi Scelte). Anche `open`, `write`, `close` e `_exit` nella closure, tutte async-signal-safe. **Committed in:** `f25b3b9`

**3. [Deviazione a] `PATH` nell'env del figlio, metodo `ChildBin::program`, `peak_rss_kb` senza `ru_maxrss`** (ripiego sull'ultimo `VmHWM` campionato durante la scansione della quota, `isolate.rs:811`, `:888`). **Committed in:** `f25b3b9`

**4. [Deviazione a] `memory_reservation_bytes` negli 8 letterali dei test del budget.**
- **Found during:** Task 3 v6
- **Issue:** il test-write v6 prevedeva una sola riga in `limits()`. Con quella sola, `reservation_over_budget_is_budget_exceeded_without_spawn` era insoddisfacibile: i casi pilotavano la prenotazione con l'AS (semantica v3), e `tw-bud-as` vuole più di 256 MiB mentre `tw-bud-fits` ne vuole al più 256.
- **Fix:** `memory_reservation_bytes` uguale ad `address_space_bytes` in 8 letterali (`reservation_over_budget` ×4, (f) ×2, `setrlimit`, `dropped_future`) più 3 commenti. Nessuna asserzione toccata; la (f) del piano v6 fissa proprio «`memory_reservation_bytes` 128 MiB, AS 128 MiB».
- **Verification:** il coordinatore ha controllato `git diff 056911a 535e3a2 -- spikes/s02-parser/tests/isolation.rs`, il bridge l'ha riletto: 9 righe aggiunte di `memory_reservation_bytes` (1 in `limits()`, 8 nei letterali), solo commenti e letterali cambiati.
- **Committed in:** `73f448c`. Lettura come completamento del campo delegato, non come indebolimento: da confermare all'utente.

**5. [Deviazione a] File del Task 2 toccati nel commit del Task 3.** `isolate.rs` e `parse-parent.rs` cambiano in `73f448c`: `memory_reservation_bytes`, `Budget::fixed` da env, `parent_reserve_exceeded` (il grep di accettazione del Task 2 era a 0, ora è 1), `calibrate` v6. Sono file del piano.

**6. [Deviazione a] Variabili `S02_BUDGET_TOTAL_BYTES` e `S02_PARENT_RESERVE_BYTES`** (`parse-parent.rs:61-62`, `:333-341`), lette solo dal padre come `S02_BIN_DIR`: convenzione imposta dai test di (f3) e dello schema, perché il runner non ha `memory.max`. Senza la prima vale `Budget::from_cgroup`.

**7. [Deviazione a] Fixture della prova PDFium da uno script.** I PDF di (k3), (k4) e del caso limite li ha generati uno script del runner con le stesse ricette `qpdf` dei test, non i test stessi. Test non modificati; fixture in `results/raw/isolation-fixtures/`, ignorata da git. **Committed in:** `535e3a2`

**8. [Deviazione a] Commit di stato intermedio `ce35b04`** con prescan, figlio e driver secondo le regole v3, poi riallineati dalla v6 in `73f448c`. Serviva a salvare il lavoro del 2026-10-21 (prima tenuto in una patch fuori repo) prima dei test rossi v6.

**9. [Deviazione a] `hw.json` normalizzato** allo schema di `common::manifest` con la ricetta di `s01-upload/bench.sh`, come in 01-05 e 01-11; l'originale è in `results/raw/isolation/`. **Committed in:** `535e3a2`

### Segnalazione su file chiuso (livello b)

**10. [Deviazione b, segnalazione] `hw-manifest.sh` (01-02) incompatibile con `common::manifest` (01-01).** `common::csv::Writer` rifiuta l'`hw.json` grezzo («invalid type: map, expected a string»). Stesso difetto già registrato da 01-04, 01-05, 01-11 e 01-12; nessun file chiuso toccato. La correzione resta per una fase decimale che riapra il perimetro di 01-01 o 01-02.

### Blocchi del 2026-10-21 e come sono stati risolti

Il 2026-10-21 l'esecutore si è fermato su quattro blocchi esterni al proprio perimetro (`ORCHESTRATOR-STATUS.md:84`):

- **B1, scanner:** l'hook pre-commit bloccava sui MEDIUM `pdf-untrusted-parse` e `archive-decompress`, inevitabili in un prescan PDF. Risolto con `4441aef`: l'hook blocca solo CRITICAL e HIGH, i MEDIUM si motivano nel SUMMARY (sezione Scanner).
- **B2, tmpfs `/work`:** nel `compose.yaml` chiuso di 01-02 il tmpfs era di root con modo 755, non scrivibile dal worker di uid 10001. Risolto con la gap-closure 1.3 (`5d695eb`, uid/gid 10001 e modo 0700), fusa nel branch con `8b35978`.
- **B3, soglie del prescan:** 8 PDF benigni rifiutati su 8. **B4, formula su `VmPeak`:** un solo job nel budget. Risolti dal ripasso dell'utente (v4, `66be9b6`), poi da tre giri di audit GPT fino alla v6.

---

**Total deviations:** 9 di livello a (3 di contratto o firma, 2 di processo, 2 d'ambiente o fixture, 2 di implementazione), 1 segnalazione di livello b su un difetto già noto; nessuna di livello c. I blocchi B1-B4 sono stati risolti fuori dal piano (hook, gap-closure, decisione utente).
**Impact on plan:** nessuna estensione di perimetro (13 file su 13).

## Issues Encountered

- **Condizioni d'ambiente, non difetti.** Sull'host Windows i 12 test del ramo cifrato si fermano perché `qpdf` non c'è: servono il runner. `tests/isolation.rs` è solo Linux. Nel runner, `parse_child_data_schema` cerca PDFium in `S02_TEST_PDFIUM_DIR`, `PDFIUM_DIR`, `/opt/pdfium/lib` o `/src/target/pdfium/lib` (`isolation.rs:1435-1454`): l'immagine del runner non ha PDFium, e la build `chromium/8100` è stata estratta e verificata in `spikes/target/pdfium`, ignorata da git.
- **clippy nel runner assente** (`cargo-clippy` non installato per 1.99.1): il codice `cfg(linux)` non è lintato da clippy, compila pulito. Sull'host clippy `-D warnings` è verde.
- **`VmPeak` della build.** Un PDF da una pagina ha `VmPeak` 15.248 kB in questa build, contro i 613.756 kB dello smoke del 2026-10-21; i PDF del corpus stanno fra 616 e 757 MB.

## Discrepanze fra rapporto e prove

- **Motivazione dei 31 MEDIUM.** Il rapporto dice «parsing e decompressione solo nel figlio isolato». Vale per 22 righe di produzione; 5 sono nei test, e 4 (`parse-parent.rs:1129`, `:1247-1251`) sono il modo `in-process-mutex` del driver `overhead`, che apre PDFium nel padre per scelta del piano. Il SUMMARY lo dichiara come eccezione.
- **`load_pdf_from_byte_slice` invece di `load_pdf_from_file`** in `parse-child.rs:118`: scostamento dal testo del piano non elencato nel rapporto. È innocuo, anzi toglie una seconda lettura del file fra prescan e PDFium.
- **«Storia del 2026-10-21».** Il rapporto attuale nomina solo B1 e B2 fra le deviazioni; B3 e B4 stanno in `ORCHESTRATOR-STATUS.md:84` e nella Storia del piano (`01-07-PLAN.md:139`). I fatti coincidono.
- **`rustc` in `hw.json`** è `1.95.0`, la toolchain dell'host che ha scritto il manifest; l'immagine compila con `rust:1.99.1-slim-trixie`. Il binario misurato è quello dell'immagine.
- **Ripetizioni del gate.** Il rapporto dice «isolation 37 + 3 ripetizioni da 37»; il log del gate mostra la corsa completa più tre ripetizioni di `isolation`, tutte 37/37, come dice anche il coordinatore.
- **Lavoro I2 di `pub-en-03`** («681 MB»): non è in `calibration.csv`, che non ha la colonna; il bridge l'ha letto nel log grezzo della calibrazione (681.256.883 byte).
- Il resto coincide con le prove: commit e ordine, `files_modified`, conteggi e massimi del CSV, formula di `limits.json`, esiti dei tre job PDFium, grep di accettazione dei Task 2-4, scanner.

## Threat Flags

- **T-01-07-05 (lanciatore usato da 01-08/09/13):** allowlist chiusa, argv dal padre, nessuna shell, spec sigillata; TEST-1, 2.
- **T-01-07-06 (figli concorrenti nel cgroup):** budget, `RLIMIT_AS`, `oom_score_adj`; aperta fino all'OOM vero di 01-18.
- **T-01-07-11 (allegati):** nessuna API degli allegati nel figlio, PDFium non-V8, stream degli allegati sotto i tetti; k5, k6.
- **T-01-07-12 (cifrati ammessi):** solo password vuota, riconciliazione, ordine fisso dei controlli; il caso limite proprietario vuota + utente non vuota è oggi rifiutato da PDFium.
- **Residuo TOCTOU** fra `lstat` del padre e `open` del figlio, accettato dal piano e da annotare in RISULTATI di 01-18.

## Known Stubs

Nessuno: `src/main.rs` del crate è il bin vuoto di 01-01, fuori perimetro.

## User Setup Required

Nessuno.

## Punti aperti

- **Caso limite di PDFium** (proprietario vuota, utente non vuota): decisione dell'utente fra rifiuto confermato e tentativo con password vuota esplicita.
- **8 letterali dei test del budget:** conferma dell'utente che si tratta di completamento del campo, non di indebolimento.
- **OOM vero, UID effettivo, rete e read-only a runtime, percorsi d'errore di `oom_score_adj` e subreaper:** in 01-18.
- **Difetto `hw-manifest.sh` / `common::manifest`:** aperto, da fase decimale.
- **D-25 nella documentazione ufficiale:** il limite sulla somma degli object stream va dichiarato in ADR-0004 e nella sezione limiti noti del README del prodotto.

## Next Phase Readiness

- **01-18, controllo d'ingresso.** Verificare su main che `01-07-PLAN.md` abbia sha256 `5c9f942d1ca2ae16ed21792af7ee00868ba27b325561c04c911520b9df9fd5b9` (controllato dal bridge), che il figlio emetta i sei campi di `data` (`encryption`, `embedded_files`, `objstm_count`, `objstm_decompressed_bytes`, `xref_streams`, `decompress_work_bytes`) e che i driver `hostile`, `budget` e `overhead` di `parse-parent.rs` abbiano le CLI e gli header del Task 2. 01-18 riallinea il manifest alla tabella «Contratto unico degli esiti», misura i casi ostili, il budget reale e l'OOM, e trascrive in `RISULTATI-isolamento.md` le sezioni «Anomalie di calibrazione» e «PDFium e password vuota» di questo SUMMARY.
- **01-08, 01-09, 01-13.** Consumano il contratto W17 senza modificarlo: job JSON con le op già in allowlist (`extract_pdf`, `locate`, `ocr_render`, `ocr_tesseract`), `limits.json` profilo `default` (01-09), `parse-parent calibrate --op <op>` per i limiti propri (01-08, 01-13), bin aggiunti ricostruendo l'immagine.
- **Concorrenza del worker:** con i limiti calibrati entrano **2 job `parse` insieme** nel budget del worker da 2 GiB (riserva del padre 256 MiB). È il dato di partenza per il dimensionamento di ADR-0004.

## Self-Check: PASSED

- 13 file su 13 dei `files_modified` nel diff del merge `4a9129e`, nessun file di codice fuori elenco; il gate `scripts/gate-proprieta-file.cjs`: conforme.
- Rieseguiti dal bridge: verify del Task 4 (VERIFY-OK), grep di accettazione dei Task 2 e 3, scanner con soglia HIGH (exit 0), aritmetica di `limits.json`, diff dei letterali di `isolation.rs`, sha256 del piano, esiti dei tre job PDFium nel log grezzo.
- Test: gate del coordinatore nel runner Linux sul main fuso, quattro corse verdi (lib 84, isolation 37, parse-child 1, parse-parent 7), non ripetute dal bridge.

**Modifica:** Claude (Anthropic) · 2026-10-22 · bridge del piano 01-07
