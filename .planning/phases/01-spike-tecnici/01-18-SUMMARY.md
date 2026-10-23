---
phase: 01-spike-tecnici
plan: 18
subsystem: spike-isolamento-parser-misure-casi-ostili-budget-oom
tags: [s02-parser, isolamento, casi-ostili, decompression-bomb, object-stream, cifratura, allegati, quarantena, budget-aggregato, limiter, cgroup-v2, memory-peak, memory-events, oom, oom_score_adj, subreaper, pdfium, overhead, qpdf, gen_hostile, validazione-01-07, M8, M9, M10, M11]

# Dependency graph
requires:
  - phase: 01-07
    provides: "lanciatore isolato e contratto W17 (fuso con 4a9129e, SUMMARY 6428995): job JSON chiuso, run_child, Limits con memory_reservation_bytes, Budget::fixed, driver hostile/budget/overhead e sottocomando batch di parse-parent, limits.json calibrato, immagine worker"
  - phase: 01-02
    provides: "harness Compose chiuso, session.sh (with, up, stop, cid, exec, ps --assert-only, oneshot, down), hw-manifest.sh, immagine cka-spikes-runner con qpdf"
  - phase: 01-03
    provides: "corpus benigno spikes/data/corpus/raw (10 PDF, sha256 verificati da fetch.sh), derivato da 25 pagine per M9"
provides:
  - "Soglie M8-M11, portata del selftest ed esiti attesi di 26 casi ostili committati prima di ogni misura (4a28697), immutati fino al merge 3fb03bc"
  - "Generatore senza rete dei casi ostili (gen_hostile.py, standard library più qpdf, password a runtime) e manifest con sha256 della generazione misurata"
  - "Misure nel worker: hostile.csv (78 righe), budget.csv (selftest, 4 righe), budget-real.csv (8 batch reali), budget-real-jobs.csv (178 job), overhead.csv (106 righe), hw-measure.json (7c63561)"
  - "Test OOM controllato tests/oom.rs con classificatore puro di M11 e prova in container da 256 MiB (run_oom.sh): oom.json verificato (ef4efe2)"
  - "RISULTATI-isolamento.md con Setup, Risultati, Validazione di 01-07 (33 voci), Anomalie ed Esito parziale (2f60680), più le decisioni D-34 e D-35 dell'utente (c69f4f7)"
  - "Concorrenza massima per l'op parse pubblicata per 01-13: 2 job insieme nel worker da 2 GiB, dai job reali e non dal selftest"
affects: [01-13, 01.4, ADR-0004, porta DocumentParser, worker di ingestion del Document Service]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Soglie ed esiti attesi committati prima della misura e confrontati con git: un esito diverso resta pass=false o m10_pass=false, mai un ritocco"
    - "Classificatore puro dell'esito tristato (verificato / fallito / non_esercitato) separato dalla misura che richiede il container, così il contratto si prova con test non ignorati"
    - "Riavvio del worker prima di ogni batch per azzerare memory.peak del cgroup (uid 10001 non può scriverlo), letture del cgroup prima e dopo"
    - "Container di prova fuori compose solo con etichetta cka.run_id, volume trovato per etichetta e mai con compose config"
    - "Fixture ostili costruite a byte con controlli di costruzione (taglie, rapporti, JavaScript non in chiaro) che fanno fallire il generatore invece di produrre un caso diverso da quello dichiarato"

key-files:
  created:
    - spikes/s02-parser/RISULTATI-isolamento.md
    - spikes/s02-parser/tests/oom.rs
    - spikes/s02-parser/hostile-gen/gen_hostile.py
    - spikes/s02-parser/measure/budget_real.py
    - spikes/s02-parser/measure/run_budget_real.sh
    - spikes/s02-parser/measure/run_oom.sh
    - spikes/data/hostile/manifest.json
    - spikes/data/hostile/.security-scan-fixture
    - spikes/s02-parser/results/isolation/hostile.csv
    - spikes/s02-parser/results/isolation/budget.csv
    - spikes/s02-parser/results/isolation/budget-real.csv
    - spikes/s02-parser/results/isolation/budget-real-jobs.csv
    - spikes/s02-parser/results/isolation/overhead.csv
    - spikes/s02-parser/results/isolation/oom.json
    - spikes/s02-parser/results/isolation/hw-measure.json
  modified: []

key-decisions:
  - "D-34 (utente, 2026-10-22): M8 resta «non raggiunta», 24 casi su 26; deep-nesting e jbig2-malformed danno ok con contenimento intatto; attesi non ritoccati; il caso JBIG2 si riprova in 01-13 con la rasterizzazione"
  - "D-35 (utente, 2026-10-22): i due difetti dei driver di 01-07 (cleanup di budget sul percorso d'errore, interruzione di overhead su FormatError in-process) vanno nella gap-closure 1.4, che rimisura overhead.csv con una colonna status"
  - "Classe benign = i 10 PDF di corpus/raw: i DOCX di 01-03 stanno in synthetic/, quindi fuori dalla regola del piano"
  - "11 test puri non ignorati in tests/oom.rs accanto all'unico test #[ignore] (scelta del coordinatore: il criterio letto come «un solo test ignorato»)"

patterns-established:
  - "Un consumatore che misura una concorrenza (01-13 per le op OCR) usa lo stesso metodo di M10: batch reali per classe e N, worker riavviato, cgroup prima e dopo, m10_pass calcolato dallo script"
  - "Il percorso OOM si dichiara verificato solo da un oom.json tristato con vittima dedotta da memory.events, segnale e oom_score_adj, mai dal solo SIGKILL"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "Ogni file ostile termina con un esito tipizzato entro 5 s senza far cadere il processo padre (criterio 2; D-05, D-08)"
    verification:
      - {test: TEST-2, risks: [], ref: "spikes/s02-parser/hostile-gen/gen_hostile.py:682#generate ; :729#verify_manifest ; :271#runtime_secret ; spikes/data/hostile/manifest.json (26 casi, 26 sha256)", status: pass, evidence: "conteggio del bridge: 26 casi e 26 sha256 nel manifest; import solo di moduli standard (gen_hostile.py:29-43); scanner rieseguito dal bridge exit 0. La generazione con docker run --network none e la verifica 26/26 prima delle misure sono dichiarate in RISULTATI:51 e :49, non ripetute dal bridge"}
      - {test: TEST-3, risks: [], ref: "spikes/s02-parser/results/isolation/hostile.csv", status: pass, evidence: "ricontato dal bridge: 78 righe, 3 per caso, parent_alive true e leftover_procs 0 su 78, oom_kill_delta 0, ms_to_fail massimo 238 ≤ 5000, picco RSS massimo 45.952 KiB contro RLIMIT_AS 1.835.008 KiB. Contenimento 26 casi su 26"}
      - {test: "TEST-3 (M8, esiti uguali all'atteso)", risks: [], ref: "spikes/s02-parser/results/isolation/hostile.csv colonna pass ; spikes/s02-parser/RISULTATI-isolamento.md:90", status: non_raggiunta, evidence: "ricontato dal bridge: 72 pass su 78, 6 false tutte su deep-nesting (3) e jbig2-malformed (3), ottenuto ok contro atteso failed Corrupt|Crashed. Soglia 100% non raggiunta, dichiarata tale; D-34 dell'utente"}
      - {test: "TEST-3 (causa dei due ok)", ref: "spikes/s02-parser/RISULTATI-isolamento.md:91-92", status: human_judgment, human_judgment: true, rationale: "per jbig2-malformed la lettura (l'op parse non decodifica immagini) è coerente col codice ma non provata da una misura; per deep-nesting RISULTATI stesso dice che non è misurato se lopdf carichi o scarti l'oggetto annidato. Il rinvio di JBIG2 a 01-13 è D-34; l'annidamento profondo resta senza piano"}
  - must_have: "Le regole di 01-07 v4 e la tabella «Contratto unico degli esiti» di 01-07 v6 sono misurate in hostile.csv: cifrati, allegati, object stream sulla somma reale, quarantena anche in object stream e verso allegati"
    verification:
      - {test: TEST-4, risks: [], ref: "spikes/s02-parser/results/isolation/hostile.csv (righe encrypted-*, objstm-*, pdf-embedded-*, pdf-objstm-hidden-js)", status: pass, evidence: "riletto dal bridge: encrypted-user, encrypted-pubsec → Encrypted; encrypted-object-dropped → Corrupt; encrypted-objstm-quota, objstm-sum-bomb, pdf-embedded-bomb → DecompressionLimit; encrypted-owner-only, objstm-many, pdf-embedded-file → ok; pdf-embedded-launch → launch; pdf-objstm-hidden-js → javascript; tutti 3/3; extra_reasons vuota su 78 righe, nessun embedded_file"}
      - {test: "TEST-4 (detail, controlli (d) ed (e))", ref: "spikes/s02-parser/RISULTATI-isolamento.md:177 ; :179 ; :180", status: human_judgment, human_judgment: true, rationale: "i detail password_required, object_dropped, total_decompressed (counted 536.885.397), objstm_dropped, empty_password e il marcatore CKA-EMBEDDED-MARKER assente dall'out_dir stanno solo in RISULTATI: i log grezzi in results/raw/isolation-validate (ignorata da git) non esistono più dopo la rimozione del worktree, e il bridge non ha una sessione per rieseguirli"}
  - must_have: "Esiti attesi e soglie M8, M9 scritti e committati prima di qualsiasi misura e non ritoccati sui risultati"
    verification:
      - {test: TEST-1, risks: [], ref: "git: 4a28697 (18:13:52, manifest + Soglie) prima di 7c63561 (18:26:23, CSV) ed ef4efe2 (18:27:40, oom.json)", status: pass, evidence: "git show --stat del bridge: 4a28697 contiene manifest, marcatore, RISULTATI (40 righe) e generatore, nessun CSV; diff della sezione Soglie fra 4a28697 e il merge: solo l'aggiunta che segue la sezione, nessuna riga delle soglie cambiata"}
  - must_have: "Il driver budget (selftest) è misurato a 1, 4, 8, 16 con padre vivo e prova solo la correttezza aritmetica del limiter"
    verification:
      - {test: TEST-5, risks: [RISK-001 di risk_audit_17], ref: "spikes/s02-parser/results/isolation/budget.csv ; spikes/s02-parser/RISULTATI-isolamento.md:20 ; :107", status: pass, evidence: "riletto dal bridge: 4 righe, max_admitted 1/4/4/4 = ⌊1.792/448⌋, padre vivo, oom_kill_delta 0, VmHWM del padre ≤ 6.144 KiB; RISULTATI dichiara «sola correttezza aritmetica»"}
  - must_have: "La protezione del parser reale si misura solo con job parse reali via batch, classi benign e hostile, concorrenza 1, 4, 8, 16; la concorrenza per 01-13 viene solo da budget-real.csv (M10)"
    verification:
      - {test: TEST-6, risks: [RISK-001 di risk_audit_25, RISK-001 di risk_audit_17], ref: "spikes/s02-parser/results/isolation/budget-real.csv ; budget-real-jobs.csv", status: pass, evidence: "riletto dal bridge: 8 righe (benign e hostile × 1, 4, 8, 16), header esatti, 178 job (68 benigni, 110 ostili)"}
      - {test: TEST-7, risks: [RISK-001 di risk_audit_25], ref: "spikes/s02-parser/measure/budget_real.py:259-269#m10 ; budget-real.csv colonne margin_kb, memory_max_events_delta, m10_pass", status: pass, evidence: "m10_pass true 8/8; il rapporto ≥ 1 entra solo per la classe benign (budget_real.py:268); margine e delta dell'evento max registrati e non usati in m10 (budget_real.py:259-269)"}
      - {test: TEST-8, risks: [RISK-001 di risk_audit_25], ref: "spikes/s02-parser/measure/budget_real.py:125-133#reservation_bytes ; :160-164#p95 ; :249-258", status: pass, evidence: "ricalcolato dal bridge: 820.961.280 × (1 + 0) + 1.369.666 = 822.330.946 byte = 803.057 KiB, uguale a reservation_kb; p95 nearest-rank; oom_kill_delta e margine da cg-after meno cg-before"}
      - {test: TEST-9, risks: [], ref: "spikes/s02-parser/measure/run_budget_real.sh:29-32 ; :54-56 ; :59-74", status: pass, evidence: "letto dal bridge: validazione di classe, N e tag prima di ogni comando (exit 2), stop via session.sh cid (:54), nessun stop del servizio per nome, cg-before/after, exit_code e parent_alive scritti anche con batch non zero (:65 con || code=$?), nessun valore dell'env stampato"}
      - {test: "TEST-6 (concorrenza pubblicata)", risks: [RISK-001 di risk_audit_25], ref: "spikes/s02-parser/RISULTATI-isolamento.md:124 ; :217", status: pass, evidence: "max_admitted 2 per N ≥ 4 in entrambe le classi; ostili oltre la prenotazione 0 su 110 (ricontato dal bridge); concorrenza massima per parse = 2"}
  - must_have: "Il percorso OOM è esercitato da tests/oom.rs in un container a memoria nota: vittima il figlio, padre vivo, gruppo raccolto, oom_kill_delta ≥ 1; senza OOM l'esito è non_esercitato (M11)"
    verification:
      - {test: TEST-10, risks: [RISK-002 di risk_audit_17], ref: "spikes/s02-parser/tests/oom.rs:227#oom_kills_child_not_parent ; :101#classify_m11 ; spikes/s02-parser/results/isolation/oom.json", status: pass, evidence: "oom.json riletto dal bridge: esito verificato, memory_reservation_bytes 67.108.864 < memory_max_bytes 268.435.456, failed/ResourceLimit/cgroup_oom, exit_signal 9, oom_kill_delta 1, oom_adj_set true, parent_oom_score_adj 0, leftover_procs 0, unreaped_children 0, job_dir_removed true, container_exit 0. Il test ignorato l'ha eseguito l'esecutore nel container; non rieseguito né dall'integratore né dal bridge"}
      - {test: "TEST-10 (test puri)", ref: "spikes/s02-parser/tests/oom.rs:423 ; :433 ; :440 ; :481 ; :508", status: pass, evidence: "gate d'onda dell'integratore nel runner: cargo test -p s02-parser 180 pass, 0 fail, 1 ignored"}
      - {test: TEST-11, risks: [RISK-002 di risk_audit_17], ref: "spikes/s02-parser/tests/oom.rs:449 ; :458 ; :469 ; :533 ; :544 ; spikes/s02-parser/measure/run_oom.sh:41", status: pass, evidence: "test puri verdi nel gate d'onda; oom.json rimosso prima del lancio (run_oom.sh:41); senza riga OOM_RESULT si scrive fallito/parent_killed_or_no_result ed exit 1 (run_oom.sh:90-98)"}
      - {test: TEST-12, risks: [], ref: "spikes/s02-parser/measure/run_oom.sh:71-74 ; spikes/s02-parser/tests/oom.rs:569#result_json_is_one_line_without_environment_values", status: pass, evidence: "letto dal bridge: --network none, --memory 256m --memory-swap 256m, --pids-limit 256, --cap-drop ALL, no-new-privileges, --read-only, --user 10001:10001, volume per etichetta (:61-66), etichetta cka.run_id; nessun compose config"}
  - must_have: "RISULTATI contiene «Validazione di 01-07» (gate di 01-07-RISK-AUDIT-v3): driver, header, job_id, codici d'uscita, Kind e cleanup confrontati col contratto; una difformità va a gap-closure di 01-07"
    verification:
      - {test: TEST-14, risks: [RISK-001 di 01-07-RISK-AUDIT-v3], ref: "spikes/s02-parser/RISULTATI-isolamento.md:151-191", status: pass, evidence: "ricontrollato dal bridge sulle voci leggibili dai file: header dei tre CSV dei driver, Kind osservati ⊆ insieme del contratto, motivi ⊆ quattro fissi, aritmetica di limits.json (3 × 267.240 KiB = 820.961.280; min(2.325.123.072, 1.879.048.192)), difetti dei driver citati a parse-parent.rs:1049-1050, :1116 e :1255 come nel codice"}
      - {test: "TEST-14 (voci non esercitate)", ref: "spikes/s02-parser/RISULTATI-isolamento.md:170-172 ; :174 ; :178", status: human_judgment, human_judgment: true, rationale: "sei voci del contratto non si sono presentate: exit 3 di budget/overhead, exit 3 di batch con parent_reserve_exceeded, exit 2 subreaper_unavailable, detail encrypt_state e work_bound, pdfium_password. Restano lette nel codice di 01-07, non esercitate"}
      - {test: "TEST-14 (difformità)", ref: "spikes/s02-parser/RISULTATI-isolamento.md:184-185 ; spikes/s02-parser/src/bin/parse-parent.rs:1049 ; :1116 ; :1255", status: pass, evidence: "registrate e non corrette qui (nessun file di 01-07 nel diff del merge); instradate dall'utente alla gap-closure 1.4 (D-35), che il gate proprieta-file ammette come riapertura"}
      - {test: TEST-13, risks: [RISK-001 di 01-07-RISK-AUDIT-v3], ref: "spikes/s02-parser/RISULTATI-isolamento.md:167-169", status: human_judgment, human_judgment: true, rationale: "UnknownOp exit 2, out_dir_exists exit 2 e manifest non valido exit 2 senza cartelle sono trascritti in RISULTATI; i file di prova e gli stdout stavano in results/raw/isolation-validate, ignorata da git e non più presente"}
      - {test: TEST-17, risks: [RISK-001 di risk_audit_23], ref: "git log b6f36e9 --grep='(01-07)' ; git show b6f36e9:.planning/phases/01-spike-tecnici/01-07-PLAN.md", status: pass, evidence: "rieseguito dal bridge sul main di partenza del merge: 6428995 docs(01-07) presente; sha256 del PLAN di 01-07 = 5c9f942d…f9fd5b9, uguale al pin"}
      - {test: TEST-18, risks: [RISK-001 di risk_audit_23], ref: "spikes/s02-parser/RISULTATI-isolamento.md:49 (job val-schema-3dffa106)", status: human_judgment, human_judgment: true, rationale: "ok con i sei campi di data (objstm_count 80, xref_streams 1, decompress_work_bytes 134.221.765) è trascritto in RISULTATI; il log grezzo non è più disponibile. Indizio indiretto coerente: objstm-many ok in hostile.csv 3/3"}
      - {test: TEST-19, risks: [RISK-001 di risk_audit_23], ref: "nessun file versionato (il rapporto cita measure.sh con exit 30-33)", status: human_judgment, human_judgment: true, rationale: "il ramo di arresto del preflight non si è presentato (preflight conforme) e lo script che lo implementa non è nel repo né fra i files_modified: non è verificabile dal bridge"}
  - must_have: "Tutte le misure passano dal lanciatore di 01-07 tramite job JSON, driver, batch o API pubblica, senza modificarne il codice"
    verification:
      - {test: "proprietà dei file", ref: "git diff --name-only 3fb03bc^1 3fb03bc ; node scripts/gate-proprieta-file.cjs --fase 01", status: pass, evidence: "rieseguito dal bridge: 0 file sotto src/, tests/isolation.rs, Cargo.toml o spikes/docker; 15 file su 15 dei files_modified nel diff, più 5 Markdown di pianificazione; gate proprieta-file: conforme"}
  - must_have: "Soglia M9 e overhead del processo per documento (artifact overhead.csv)"
    verification:
      - {test: TEST-15, risks: [], ref: "spikes/s02-parser/results/isolation/overhead.csv ; spikes/s02-parser/RISULTATI-isolamento.md:143-145", status: pass, evidence: "ricalcolato dal bridge: spawn_init_ms medio 4,97 ms su 2.948,65 / 16 = 184,3 ms per documento a c1, 2,7% ≤ 10%; 20 ripetizioni per modo e concorrenza salvo in-process-mutex c8 (6)"}
      - {test: "TEST-15 (riserva su M9)", ref: "spikes/s02-parser/RISULTATI-isolamento.md:143 ; :186", status: human_judgment, human_judgment: true, rationale: "spawn_init_ms è la parete meno parse_ms, e parse_ms comprende il bind di PDFium: la colonna non misura «spawn + init PDFium» come dice M9. In più il modo in-process a c8 è troncato. La gap-closure 1.4 rimisura overhead.csv (D-35); la lettura di M9 resta da confermare"}
  - must_have: "RISULTATI limita le conclusioni ai carichi misurati, riporta margine e pressione OOM e chiede di ripetere M10 se cambiano workload, memoria o limiti"
    verification:
      - {test: TEST-16, risks: [RISK-001 di risk_audit_25], ref: "spikes/s02-parser/RISULTATI-isolamento.md:126 ; :217", status: pass, evidence: "letto dal bridge: margine per classe e N, caso peggiore teorico 1.868.258 KiB (89% di memory.max), portata limitata al corpus di 01-03 e ai 26 casi, «M10 va ripetuta se cambiano workload, memoria del worker o limiti»"}
  - must_have: "Scanner di sicurezza verde sul generatore, sugli script di misura, su tests/oom.rs e su RISULTATI"
    verification:
      - {test: "security-scanner", ref: "python skills/security-scanner/security_scan.py spikes/s02-parser/hostile-gen spikes/s02-parser/measure spikes/s02-parser/tests/oom.rs spikes/s02-parser/RISULTATI-isolamento.md", status: pass, evidence: "rieseguito dal bridge: exit 0, nessun problema su 6 file"}
  - must_have: "Gate d'onda Rust sull'intero workspace"
    verification:
      - {test: "cargo test --workspace nel runner", ref: "gate d'onda dell'integratore", status: human_judgment, human_judgment: true, rationale: "nel runner gira solo cargo test -p s02-parser --locked --offline (180 pass); --workspace non è eseguibile perché al registry cargo del runner manca addr2line, dipendenza di dhat di s01-upload. Sull'host il resto del workspace dà 96 pass. Deviazione d'ambiente, non difetto del piano"}

# Metrics
duration: "circa 30 min di esecuzione (test rossi 4f4391c 18:02:41 → RISULTATI 2f60680 18:32:56), più c69f4f7 18:54:26 con le decisioni D-34 e D-35; merge su main 3fb03bc 20:22:01"
completed: 2026-10-22
---

# Fase 1 Piano 18: S-02, misure di isolamento, casi ostili, budget e OOM · Summary

**I 26 casi ostili sono tutti contenuti dal lanciatore di 01-07 (padre vivo su 78 righe su 78, nessun processo residuo, fallimento entro 238 ms, RSS massimo 45 MiB), ma M8 non è raggiunta: 24 casi su 26 danno l'esito atteso, mentre `deep-nesting` e `jbig2-malformed` danno `ok` (D-34). Sui job `parse` reali il budget ammette 2 job insieme, gli 8 batch sono conformi a M10 e questa è la concorrenza pubblicata per 01-13. L'OOM del cgroup, provocato in un container da 256 MiB, uccide il figlio e lascia vivo il padre (M11 verificato). L'overhead del figlio per documento è del 2,7% (M9 conforme con riserva). Il contratto di 01-07 risulta conforme su tutte le voci esercitate; i due difetti dei driver di misura vanno alla gap-closure 1.4 (D-35)**

## Performance

- **Duration:** circa 30 minuti di esecuzione. I test rossi `4f4391c` (18:02:41) precedono le soglie `4a28697` (18:13:52), le misure `7c63561` (18:26:23), il test OOM `ef4efe2` (18:27:40) e RISULTATI `2f60680` (18:32:56). Le decisioni dell'utente sono in `c69f4f7` (18:54:26), il merge su main è `3fb03bc` (20:22:01).
- **Started:** 2026-10-22T18:02:41+02:00
- **Completed:** 2026-10-22T20:22:01+02:00
- **Tasks:** 4 di 4
- **Files modified:** 15 su 15 `files_modified` del piano (`git diff --stat 3fb03bc^1 3fb03bc`), tutti nuovi, nessun file di codice fuori elenco. In più cinque Markdown di pianificazione (PLAN, TEST-DESIGN, RISK-AUDIT v4 e v5, GATE), ammessi da qualsiasi fase. Nessun file di 01-07 toccato; `scripts/gate-proprieta-file.cjs --fase 01`: conforme.

## Accomplishments

- **Soglie prima della misura.** `spikes/s02-parser/RISULTATI-isolamento.md:9-40` fissa M8, M9, M10, M11, la portata del selftest e le soglie del lanciatore riportate da 01-07 v6. `spikes/data/hostile/manifest.json` ha 26 casi con esito atteso come array di stringhe e sha256. Tutto è nel commit `4a28697`, prima di ogni CSV.
- **Generatore dei casi ostili** (`spikes/s02-parser/hostile-gen/gen_hostile.py`). PDF scritti a mano con xref, xref stream e object stream veri; cifratura con `qpdf` e password generate a runtime (`:271-278`); controlli di costruzione che fanno fallire il generatore se il caso non è quello dichiarato (per esempio `objstm-sum-bomb`, `:405-410`, e `pdf-objstm-hidden-js`, `:525-526`); `--verify-only` confronta i file con il manifest (`:729-740`).
- **Casi ostili** (`hostile.csv`): 78 righe, 72 `pass`. Contenimento su 26 casi su 26.
- **Limiter** (`budget.csv`): `max_admitted` 1, 4, 4, 4 a concorrenza richiesta 1, 4, 8, 16, sempre ≤ ⌊1.792 / 448⌋.
- **Budget sui job reali** (`budget_real.py`, `run_budget_real.sh`, `budget-real.csv`, `budget-real-jobs.csv`): 8 batch conformi, `max_admitted` 2 per N ≥ 4, `oom_kill_delta` 0, nessun `job_error`, rapporto prenotazione/RSS minimo 2,33 sui benigni e 16,9 sugli ostili, 0 job ostili su 110 oltre la prenotazione.
- **Overhead** (`overhead.csv`): `spawn_init_ms` medio 4,97 ms contro 184,3 ms per documento a c1. Il modo in-process (PDFium nel padre) si interrompe a c8 con `FormatError`.
- **Test OOM** (`spikes/s02-parser/tests/oom.rs`, `measure/run_oom.sh`, `oom.json`): esito `verificato`.
- **Validazione di 01-07** (`RISULTATI-isolamento.md:151-191`): 33 voci; conforme sulle esercitate, 2 difformità minori dei driver, 6 voci non esercitate.

## Task Commits

1. **Test rossi di M11 (agente test-write, distinto dall'esecutore):** `4f4391c` (test). RED_EXPECTED: 11 falliti su `todo!` di `classify_m11` e `result_json`, 1 ignorato, regressione di 01-07 verde.
2. **Task 1: soglie, esiti attesi, generatore:** `4a28697` (feat).
3. **Task 2: casi ostili, budget del limiter, budget reale, overhead:** `7c63561` (test).
4. **Task 3: test OOM controllato:** `ef4efe2` (feat).
5. **Task 4: validazione di 01-07 e RISULTATI:** `2f60680` (docs).
6. **Decisioni dell'utente D-34 e D-35 nell'esito:** `c69f4f7` (docs).

Commit di pianificazione nello stesso range: `ba93204` (v3), `0f46860` (v4), `9f3d78d` (v5), `9ebbb31` (risk_audit_25 e test_design_12 agli atti), `8872793` (main fuso nel branch).

**Merge:** `3fb03bc`. **Plan metadata:** questo SUMMARY e `01-18-RACCONTO.md`, scritti dal bridge senza commit.

## Files Created/Modified

- `spikes/s02-parser/RISULTATI-isolamento.md`: domanda, soglie, setup, risultati, validazione di 01-07, anomalie, esito parziale, decisioni D-34 e D-35.
- `spikes/s02-parser/tests/oom.rs`: classificatore puro di M11, riga `OOM_RESULT`, test di misura ignorato, 11 test puri.
- `spikes/s02-parser/hostile-gen/gen_hostile.py`: generatore dei 26 casi.
- `spikes/s02-parser/measure/budget_real.py`: sottocomandi `jobs` e `summarize`, calcolo di `m10_pass`.
- `spikes/s02-parser/measure/run_budget_real.sh`: ciclo di un batch (riavvio del worker, cgroup prima e dopo, `batch`, `ps`).
- `spikes/s02-parser/measure/run_oom.sh`: compilazione nel runner e container a memoria nota.
- `spikes/data/hostile/manifest.json`, `spikes/data/hostile/.security-scan-fixture`.
- `spikes/s02-parser/results/isolation/{hostile,budget,budget-real,budget-real-jobs,overhead}.csv`, `oom.json`, `hw-measure.json`.

## Decisions Made

### Decisioni dell'utente

- **D-34.** M8 resta «non raggiunta», 24 casi su 26. Gli attesi non si ritoccano; il caso JBIG2 si riprova in 01-13, dove le pagine si rasterizzano per l'OCR (`01-CONTEXT.md:72`).
- **D-35.** I difetti dei driver `budget` e `overhead` di 01-07 si correggono nella gap-closure 1.4, che aggiunge a `overhead.csv` una colonna `status` e lo rimisura (`01-CONTEXT.md:73`; fase 01.4 nel ROADMAP).
- **Deviazioni accettate** (comunicate dal coordinatore): gli 11 test puri non ignorati in `tests/oom.rs`; gli 8 letterali `memory_reservation_bytes` nei test del budget di 01-07 (`73f448c`), con le asserzioni invariate. Il secondo punto chiude il punto aperto del SUMMARY di 01-07; 01-18 non tocca `tests/isolation.rs`.

### Scelte dell'esecutore

- **Classe `benign` = 10 PDF di `corpus/raw/`.** I DOCX del manifest di 01-03 stanno in `synthetic/`, e il piano prende i documenti «presenti in `raw/`».
- **`completed`, `job_errors` e `parent_alive` in `summarize`** (`budget_real.py:192-209`): `completed` conta i `RunReport` scritti con qualunque esito; `job_errors` somma le righe `job_error` e i `launch_errors` del `batch_summary`; `parent_alive` vuole `ps --assert-only` 0 ed exit di `batch` sotto 128.
- **`generator_args` = `["--only", case_id]`**: riproduce il singolo caso.
- **Costruzione dei casi:** `pdf-objstm-hidden-js` usa l'azione aggiuntiva `/AA` della pagina verso un'azione JS dentro un object stream (`gen_hostile.py:519-527`), così non compare il motivo in più `open_action_js`; `encrypted-object-dropped` rompe l'intestazione (`obj` → `obx`) di un oggetto citato solo da `/Extra` del Catalog, cercando il Catalog fino a `endobj` perché quello di qpdf ha dizionari annidati (`:300-324`); `objstm-sum-bomb` usa 9 object stream da 60 MiB a bassa entropia (rapporto circa 34, `:391-411`).
- **Overhead non ripetuto nel CSV ufficiale:** la seconda corsa (tag `ad080a07`) è servita solo a riprodurre il `FormatError` ed è rimasta fra i grezzi.

## Esiti delle soglie

| soglia | esito | numeri |
|---|---|---|
| M8 | **non raggiunta** (D-34) | 72/78 righe, 24/26 casi; scarti `deep-nesting` e `jbig2-malformed` (`ok` contro `failed`); contenimento 26/26 |
| M9 | conforme con riserva | 4,97 ms su 184,3 ms = 2,7% ≤ 10%; la colonna non isola l'init di PDFium; in-process c8 troncato (6/20) |
| M10 | conforme | 8/8 batch; `max_admitted` 2 per N ≥ 4; concorrenza massima per `parse` = 2 |
| M11 | verificato | `oom_kill_delta` 1, figlio `ResourceLimit`/`cgroup_oom` segnale 9, padre vivo con `oom_score_adj` 0 |

## Deviations from Plan

### Auto-fixed Issues (livello a)

**1. [Deviazione a] Verifica del Task 2 non eseguibile come scritta.**
- **Issue:** il `<verify>` lancia `parse-parent budget … --out /dev/null`, ma `common::csv::Writer` (01-01) rifiuta un CSV senza `hw.json` accanto: exit 1.
- **Fix:** stessa verifica con `--out` in `results/raw/isolation-verify/budget.csv` accanto a una copia dell'`hw.json` di 01-07: `id -u` 10001, `budget` exit 0, `/work` 0 (`RISULTATI-isolamento.md:208`). La parte statica del verify è verde.

**2. [Deviazione a] CSV dei driver validati contro `hw.json` di 01-07.** `common::csv::Writer` trova solo quello accanto ai CSV; `hw-measure.json` resta il manifest grezzo della sessione (`RISULTATI-isolamento.md:209`). È il difetto noto di livello b fra `hw-manifest.sh` (01-02) e `common::manifest` (01-01).

**3. [Deviazione a] Parsing di `OOM_RESULT` in `run_oom.sh`.**
- **Issue:** libtest con `--nocapture` stampa `test oom_kills_child_not_parent ... ` sulla stessa riga del marcatore; la prima corsa ha scritto `fallito`/`parent_killed_or_no_result` con il test in realtà verificato.
- **Fix:** marcatore cercato ovunque nella riga, occorrenza unica (`run_oom.sh:83-92`); `oom.json` rimosso e rigenerato dalla verifica del Task 3. **Committed in:** `ef4efe2`

**4. [Deviazione a, accettata] 11 test puri non ignorati in `tests/oom.rs`.** Il piano dice «un solo test `oom_kills_child_not_parent`» e «ha un solo test `#[ignore]`». Il test-write ha messo nello stesso file, l'unico ammesso, gli 11 test del classificatore (`4f4391c`). Il criterio è stato letto come «un solo test ignorato».

**5. [Deviazione a, accettata] Letterali `memory_reservation_bytes` negli 8 test del budget** di 01-07 (`73f448c`), asserzioni invariate: confermata come completamento del campo, non come indebolimento.

**6. [Incoerenza nominale] `plan_version` del PLAN.** Il frontmatter di `01-18-PLAN.md` dice `plan_version: 4`, mentre il contenuto è la v5 (Storia «plan_version 5», risk_audit_25 e test_design_12 sulla v5, sha256 `c373101a…`). Nessuno l'ha corretta: va allineata dal `plan`, non dal bridge.

### Deviazione d'ambiente

**7. Gate `--workspace` non eseguibile nel runner.** Al registry cargo del runner manca `addr2line`, dipendenza di `dhat` di `s01-upload`. Il gate d'onda ha quindi girato `cargo test -p s02-parser --locked --offline` nel runner (180 pass, 0 fail, 1 ignorato) e il resto del workspace sull'host (96 pass). Il test ignorato è `oom_kills_child_not_parent`, per costruzione: l'ha eseguito l'esecutore nel container a memoria bassa (M11).

### Segnalazioni su file chiusi (livello b, non corrette qui)

**8. Driver `budget` di 01-07, cleanup sul percorso d'errore.** `selftest_input` crea il file prima del CSV (`parse-parent.rs:1049-1050`) e `remove_file` gira solo sul percorso riuscito (`:1116`): su errore resta `/work/selftest-input-<tag>`. → gap-closure 1.4 (D-35).

**9. Driver `overhead` di 01-07, interruzione su errore in-process.** Il `?` su `in_process_run` (`parse-parent.rs:1255`) fa uscire il driver con 1 al primo `FormatError` di PDFium: CSV a 6 righe su 20 per c8 (16 su 20 nella seconda corsa). → gap-closure 1.4 (D-35).

**10. `hw-manifest.sh` (01-02) / `common::manifest` (01-01).** Difetto già noto da 01-04, 01-05, 01-07, 01-11 e 01-12; nessun file chiuso toccato.

---

**Total deviations:** 5 di livello a (2 accettate dall'utente), 1 incoerenza nominale, 1 d'ambiente, 3 segnalazioni di livello b instradate (2 alla gap-closure 1.4, 1 già nota); nessuna di livello c.
**Impact on plan:** nessuna estensione di perimetro (15 file su 15).

## Discrepanze fra rapporto e prove

- **Commit.** Il rapporto elenca quattro commit di task; git ne mostra anche `4f4391c` (test rossi, dal rapporto del test-write) e `c69f4f7` (D-34 e D-35, scritto dopo il rapporto). Coincidono con le prove.
- **Conteggio dei test.** Il rapporto dice nel runner lib 84, parse-child 1, parse-parent 7, isolation 37, oom 11 più 1 ignorato: 140. Il gate d'onda dell'integratore dice 180 pass. La differenza è compatibile con i test di 01-08, fuso quattro secondi dopo (`a1241d4`, 18 file nuovi sotto `spikes/s02-parser`), ma il bridge non l'ha scomposta per target.
- **Preflight.** Il rapporto rimanda TEST-17 a `results/raw/isolation-validate/preflight-git.txt` e TEST-19 a uno script `measure.sh` con exit 30-33: nessuno dei due è versionato. TEST-17 l'ha rieseguito il bridge su `b6f36e9`; TEST-19 resta `human_judgment`.
- **Controlli (a)-(e) e schema.** Gli esiti stanno solo in RISULTATI; i grezzi, ignorati da git, non esistono più dopo la rimozione del worktree (`human_judgment` in coverage).
- **Il resto coincide** con le prove rilette o ricalcolate dal bridge: conteggi e massimi di `hostile.csv`, `budget.csv`, `budget-real*.csv` e `overhead.csv`, prenotazione 822.330.946 byte, 0 ostili oltre la prenotazione su 110, M9 al 2,7%, `oom.json`, ordine dei commit di soglie e misure, scanner, proprietà dei file.

## Threat Flags

- **T-01-18-02, T-01-18-03 (file ostili, DoS):** contenuti su 26 casi su 26 in `hostile.csv`.
- **T-01-18-07 (limiter tarato sul selftest):** concorrenza da `budget-real.csv`, selftest limitato all'aritmetica.
- **T-01-18-08 (OOM che uccide il padre):** mitigata e provata da M11. Chiude anche **T-01-07-06**, aperta dal SUMMARY di 01-07 fino a un `oom.json` verificato.
- **T-01-18-11 (allegati), T-01-18-12 (cifrati ammessi):** casi in `hostile.csv` verdi; detail e marcatore dell'allegato solo in RISULTATI.
- **T-09 del threat model di prodotto:** restano scoperti il decoder JBIG2 (a 01-13, D-34) e l'annidamento profondo, che l'op `parse` non raggiunge.

## Known Stubs

Nessuno.

## User Setup Required

Nessuno.

## Punti aperti

- **Annidamento profondo:** non si sa se lopdf carichi o scarti l'oggetto da 100.000 livelli; nessun piano lo riprende.
- **M9:** `spawn_init_ms` non isola l'init di PDFium; la gap-closure 1.4 rimisura `overhead.csv`.
- **Sei voci del contratto di 01-07 non esercitate** (exit 3 di `budget`/`overhead` e di `batch`, `subreaper_unavailable`, `encrypt_state`, `work_bound`, `pdfium_password`).
- **Caso limite PDFium** (proprietario vuota, utente non vuota) del SUMMARY di 01-07: ancora da decidere, nessun caso del manifest lo costruisce.
- **Picco RSS benigno sopra la calibrazione:** `pub-it-01` fino a 344.588 KiB contro 267.240 della calibrazione (+29%); il margine 3× regge a 2,33×, da considerare nell'ADR-0004.
- **`selftest` e `fake-child` nell'immagine del worker:** da togliere o mettere dietro una feature di test in produzione (nota per l'ADR-0004).
- **clippy su `tests/oom.rs`:** il file è solo Linux, l'host Windows lo esclude e nel runner clippy non è installato.
- **`plan_version` del frontmatter** da allineare alla v5.

## Next Phase Readiness

- **01-13** legge da `RISULTATI-isolamento.md` la concorrenza di 2 job `parse`, l'esito OOM e la validazione di 01-07; misura le op OCR con lo stesso metodo di M10 e riprova il caso JBIG2 con la rasterizzazione; scrive l'Esito complessivo di S-02 e l'ADR-0004.
- **01.4 (gap-closure)** corregge i due driver di 01-07 con `riapre_perimetro` e rimisura `overhead.csv`.

## Self-Check: PASSED

- 15 file su 15 dei `files_modified` nel diff del merge `3fb03bc`, nessun file di 01-07; gate `proprieta-file --fase 01` conforme.
- Rieseguiti o ricalcolati dal bridge: conteggi dei cinque CSV, `oom.json` contro i controlli di M11, prenotazione e aritmetica di `limits.json`, M9, ordine dei commit di soglie e misure, sha256 del PLAN di 01-07 su `b6f36e9`, scanner di sicurezza (exit 0).
- Test: gate d'onda dell'integratore nel runner, `cargo test -p s02-parser --locked --offline` 180 pass e 1 ignorato per costruzione; host 96 pass sul resto del workspace. Non ripetuti dal bridge.

**Modifica:** Claude (Anthropic) · 2026-10-22 · creazione (bridge di 01-18)
