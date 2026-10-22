---
phase: 01-spike-tecnici
plan: 08
subsystem: spike-estrazione-pdf-tabelle-docling
tags: [s02-parser, pdfium, pdfium-render, loose-bounds, mediabox, cropbox, rotate, cer, levenshtein, iou, abbinamento-uno-a-uno, tat-dqa, doclaynet, financebench, oracolo-it, f1-celle, euristica-pagine-tabella, holdout, docling, docling-serve, page-range, document-timeout, gpu, rtx-3050, vram, riavvio-per-documento, reqwest, multipart, allow-list-host]

# Dependency graph
requires:
  - phase: 01-01
    provides: "workspace degli spike, crate s02-parser con le dipendenze dichiarate (pdfium-render 0.9.4, unicode-normalization, reqwest, tokio, clap, serde), crate common"
  - phase: 01-02
    provides: "harness Compose con i servizi docling e docling-cpu (docling-serve v1.37.0, rete interna parse, 8g), worker, session.sh (with, up, stop, cid, exec, oneshot, ps --assert-only), hw-manifest.sh, gate GPU"
  - phase: 01-03
    provides: "corpus benigno spikes/data/corpus/raw con sha256 verificati (documenti IT, der-it-01 da 25 pagine)"
  - phase: 01-06
    provides: "oracolo IT ratificato parser-it.json (6 pagine di testo, 4 tavole, pagine-tabella del documento da 25 pagine), tutte le voci con i tre assi verdi"
  - phase: 01-07
    provides: "contratto W17: job JSON op extract_pdf → argv extract pdf --input --out, prescan::check_pdf, protocol::emit, parse-parent run/batch/calibrate --op, budget del worker; immagine worker con PDFium chromium/8100"
provides:
  - "Figlio isolato extract_pdf (bin extract, sottocomando pdf): prescan prima di PDFium, un JSON per pagina con testo, caratteri, parole e righe in punti PDF origine in basso a sinistra nella pagina ruotata, MediaBox, CropBox e rotazione"
  - "Punteggi con contratti di abbinamento fissati prima della misura (score.rs): CER con scomposizione S/D/I, IoU di parola uno a uno con matched/merged/missed/spurious/unscorable, F1 di cella a multinsieme, recall delle regioni tabella"
  - "Euristica pagine-tabella con parametri solo da file e banda di incertezza (tables.rs), tarata sulle pagine tune e misurata una volta sull'holdout"
  - "Client Docling (bin docling-client) che invia solo input con RunReport ok di extract_pdf, con allow-list degli host, tetti di upload e risposta, esiti ok/error/client_timeout/client_abort/not_prescanned distinti"
  - "Script host docling-bench.sh: riavvio stop+up per documento e dopo ogni fallimento, VRAM campionata, prove di confine timeout/disconnect/outside_range/state_marker"
  - "Dataset S-02 registrati con licenza e sha256 (84 voci, split 20+20 per documento) ed evidenze in results/extract; esito parziale in RISULTATI-estrazione.md per l'ADR-0004"
affects: [01-09, 01-13, 01-17, 01.4, ADR-0004, ADR di ingestion, porta DocumentParser, worker di ingestion del Document Service]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Soglie e contratti di misura committati prima di qualunque risultato (335d6bc), e ordine dei commit controllato con git merge-base: split prima dei risultati, parametri dell'euristica prima di table_pages.csv"
    - "Taratura su split tune e verdetto solo sulle righe holdout cumulate (holdout_recall); parametri senza default nel codice (deny_unknown_fields, load_params che fallisce chiuso)"
    - "Abbinamento uno a uno greedy per IoU decrescente con tie-break deterministico, seconda passata solo per le attese libere; ogni attesa valutabile resta nel denominatore"
    - "Servizio esterno non supervisionato (Docling) trattato come non cancellabile: timeout e disconnessione sono segnali di riavvio, il riavvio stop+up è la mitigazione"
    - "URL di un servizio interno costruibile solo da un tipo Endpoint validato (schema, host in allow-list, niente credenziali, query o frammento)"

key-files:
  created:
    - spikes/s02-parser/src/bin/extract/main.rs
    - spikes/s02-parser/src/bin/extract/pdf.rs
    - spikes/s02-parser/src/bin/extract/score.rs
    - spikes/s02-parser/src/bin/extract/tables.rs
    - spikes/s02-parser/src/bin/docling-client/main.rs
    - spikes/s02-parser/docling-bench/docling-bench.sh
    - spikes/s02-parser/RISULTATI-estrazione.md
    - spikes/data/s02/manifest.json
    - spikes/data/s02/splits.json
    - spikes/data/s02/fetch.sh
    - spikes/data/s02/DATASET.md
    - spikes/s02-parser/results/extract/hw.json
    - spikes/s02-parser/results/extract/limits-extract_pdf.json
    - spikes/s02-parser/results/extract/invocation.txt
    - spikes/s02-parser/results/extract/table_heuristic_params.json
    - spikes/s02-parser/results/extract/text_cer.csv
    - spikes/s02-parser/results/extract/bbox_iou.csv
    - spikes/s02-parser/results/extract/iou_examples.json
    - spikes/s02-parser/results/extract/tables.csv
    - spikes/s02-parser/results/extract/table_pages.csv
    - spikes/s02-parser/results/extract/docling.csv
    - spikes/s02-parser/results/extract/docling_isolation.csv
  modified: []

key-decisions:
  - "Decisione utente 2026-10-22, D-36: D-27 chiusa con l'opzione 4 (lasciare com'è), perché page_range riduce il costo; punto aperto per l'ADR di ingestion: riavvio di Docling ~19 s per documento e precisione dell'euristica 0,435"
  - "Decisione utente 2026-10-22, D-37: il rifiuto di OTC_NSANY_2004 (Unsupported/image_filter_on_non_image) è un falso positivo del prescan di 01-07 e si corregge nella gap-closure 1.4, piano 01.4-02"
  - "Banda dell'euristica: select_from = punteggio minimo delle pagine-tabella vere di taratura estratte (0,3057), uncertain_from = 0,5 × select_from (0,1528); le pagine senza job ok non fissano la banda"
  - "Oracolo IT senza bbox: la tavola attesa si abbina alla predetta della stessa pagina con più celle abbinate; solo celle del corpo"
  - "Lo sha256 «stesso input» lo verifica docling-bench.sh con sha256sum, non il client: nessuna dipendenza sha2 nel crate e Cargo.toml fuori dai file del piano"
  - "Endpoint di Docling con allow-list di host (docling, docling-cpu, loopback), schema http, upload ≤ 25 MiB e risposta letta in streaming con tetto di 256 MiB, per chiudere gli HIGH SSRF/upload dello scanner"

patterns-established:
  - "Un consumatore del lanciatore aggiunge il proprio figlio come src/bin/<nome>/ con la CLI dell'op già in allowlist, calcola i limiti con calibrate --op e non tocca la libreria di 01-07"
  - "Un esito sotto soglia si riporta col numero del contratto; una diagnostica calcolata dopo aver visto i risultati si scrive accanto e non sostituisce l'esito"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "PDFium (pdfium-render 0.9.4) estrae testo, bbox e numero di pagina su tutte le pagine dei PDF digitali, misurato con CER e IoU contro TAT-DQA e contro l'oracolo IT ratificato, con un contratto di abbinamento fissato prima della misura: uno a uno, ogni parola attesa nel denominatore, omissioni contate come miss"
    verification:
      - {test: TEST-5, risks: [RISK-001 di risk_audit_11], ref: "spikes/s02-parser/src/bin/extract/score.rs:1609#token_normalization_is_nfc_and_trim_only ; :1618#cer_is_levenshtein_over_expected_chars ; :1628#iou_basics ; :1639#tatdqa_pixels_top_left_to_pdf_points_bottom_left (d) ; :1656#missing_column_counts_as_missed_and_lowers_share (a) ; :1685#identical_numbers_are_matched_one_to_one (b) ; :1716#split_token_is_merged_with_union_bbox (c) ; :1736 ; :1759 ; :1768 ; :1790 ; :1805#property_counts_and_uniqueness_hold_on_random_pages ; :1872 ; :1894", status: pass, evidence: "gate dell'integratore nel runner: extract 31/31 verdi; il bridge ha controllato che i blocchi #[cfg(test)] dei cinque file sono identici fra i test rossi 60521e9 e il merge a1241d4 (nessun test tolto o indebolito)"}
      - {test: "TEST-1 (contratto delle coordinate)", ref: "spikes/s02-parser/src/bin/extract/pdf.rs:475#benign_pdf_gives_one_json_per_page_in_pdf_points_bottom_left ; :531#rotation_is_reported_in_degrees ; spikes/s02-parser/src/bin/extract/pdf.rs:55-81#Frame", status: pass, evidence: "gate dell'integratore nel runner: verdi con PDFium reale"}
      - {test: "contratti prima della misura", ref: "git diff 335d6bc a24c88c -- spikes/s02-parser/RISULTATI-estrazione.md", status: pass, evidence: "rieseguito dal bridge: il primo hunk parte dalla riga 90 (Setup); Soglie e Contratti (righe 16-89) invariati dopo 335d6bc"}
      - {test: "misure E-1, E-2, E-3", ref: "spikes/s02-parser/results/extract/text_cer.csv ; bbox_iou.csv", status: pass, evidence: "ricalcolato dal bridge dai CSV: CER TAT-DQA 0,0948 su 67.735 caratteri, CER IT 0,2158 su 15.581, quota IoU ≥ 0,5 = 7.514/9.759 = 0,770; matched 9.405, missed 354, spurious 1.374, unscorable 901; 3 righe two_column=true"}
      - {test: "esito delle soglie E-1, E-2, E-3", ref: "spikes/s02-parser/RISULTATI-estrazione.md#Esito parziale", status: human_judgment, human_judgment: true, rationale: "tutte e tre sotto soglia (9,48% e 21,58% contro ≤ 1%; 77,0% contro ≥ 95%). Il piano è uno spike e dichiara l'esito; se PDFium resta il parser per testo e coordinate con ordine di lettura ricostruito e tolleranza geometrica da fissare lo decide l'ADR-0004 (01-13) con l'utente"}
  - must_have: "Docling è misurato solo sulle pagine con tabelle, su CPU e su GPU RTX 3050, con qualità delle celle e secondi per pagina; l'euristica pagine-tabella è tarata su un insieme e misurata su un holdout separato, con parametri congelati e committati prima di aprire gli attesi dell'holdout"
    verification:
      - {test: TEST-6, risks: [RISK-001 di risk_audit_11, RISK-004 di risk_audit_4], ref: "spikes/s02-parser/src/bin/extract/tables.rs:687#params_never_fall_back_to_defaults ; :702#band_boundaries_and_uncertain_pages_go_to_docling ; :721#recall_precision_per_document ; :734#verdict_uses_only_holdout_rows_pooled ; :754#splits_are_disjoint_by_item_and_document", status: pass, evidence: "gate dell'integratore nel runner: verdi"}
      - {test: "TEST-6 (ordine dei commit)", ref: "git merge-base --is-ancestor 335d6bc ce26184 ; git merge-base --is-ancestor 5d2d65e f24507b", status: pass, evidence: "rieseguito dal bridge: splits.json entra in 335d6bc, il primo file di results/extract in ce26184; table_heuristic_params.json entra da solo in 5d2d65e (1 file), table_pages.csv in f24507b; entrambi gli ordini corretti"}
      - {test: TEST-7, ref: "spikes/s02-parser/src/bin/extract/score.rs:1954 ; :1992 ; :2024 ; spikes/s02-parser/src/bin/extract/tables.rs:777", status: pass, evidence: "gate dell'integratore: verdi; il bridge ha controllato gli header dei sette CSV committati contro il piano"}
      - {test: "misure E-4…E-11", ref: "spikes/s02-parser/results/extract/tables.csv ; table_pages.csv ; docling.csv", status: pass, evidence: "ricalcolato dal bridge: F1 micro Docling 140 abbinate / 246 predette / 237 attese = 0,580 su GPU e CPU; baseline PDFium 0,011; holdout 10/11 = 0,909, precisione 10/23 = 0,435; taratura 8/10, 8/11; p95 nearest-rank su 78 richieste utili per dispositivo: GPU 2,054 s, CPU 8,016 s; VRAM massima 1.569 MiB; 234 righe tutte ok, nessuna riga NSANY"}
      - {test: "esito delle soglie E-4, E-6, E-7, E-9", ref: "spikes/s02-parser/RISULTATI-estrazione.md#Risultati", status: human_judgment, human_judgment: true, rationale: "tutte sotto soglia: F1 celle 0,580 (≥ 0,90; la cifra 0,965 con spostamento di riga è diagnostica a posteriori e non è l'esito), recall regioni 0,595 (≥ 0,90), recall holdout 0,909 (≥ 0,95), GPU p95 2,054 s (≤ 1,0 s, tempo del client quantizzato a ~2 s). La scelta che ne segue spetta all'ADR-0004 e, per i tempi, all'ADR di ingestion (D-36)"}
  - must_have: "TAT-DQA (CC BY 4.0), DocLayNet (CDLA-Permissive) e FinanceBench (CC BY-NC 4.0) sono registrati con licenza e sha256 e non ridistribuiti"
    verification:
      - {test: "controllo node del piano", ref: "spikes/data/s02/manifest.json ; spikes/data/s02/splits.json ; spikes/data/s02/DATASET.md:34", status: pass, evidence: "rieseguito dal bridge: 84 voci, i tre dataset con licenza; split 20 + 20 disgiunti; git status --porcelain spikes/data/s02/raw vuoto; DATASET.md dichiara che lo split non usa select-tables"}
      - {test: "TEST-1 (download)", ref: "spikes/data/s02/fetch.sh", status: human_judgment, human_judgment: true, rationale: "fetch.sh non è stato rieseguito dal bridge (scarica archivi via HTTP Range da host esterni); la verifica degli sha256 dei membri è dichiarata dal rapporto e letta nello script, non osservata"}
  - must_have: "L'estrazione gira solo come job op: extract_pdf del contratto di 01-07, con limiti calibrati con parse-parent calibrate --op extract_pdf"
    verification:
      - {test: TEST-1, risks: [RISK-001 di risk_audit_11, RISK-001 di risk_audit_4], ref: "spikes/s02-parser/src/bin/extract/main.rs:170#pdf_accepts_exactly_the_resolve_argv ; :189#pdf_rejects_extra_unknown_and_missing_args ; spikes/s02-parser/src/bin/extract/pdf.rs:424#javascript_pdf_is_quarantined_before_pdfium_and_writes_nothing ; :442#garbage_is_failed_by_prescan_and_writes_nothing ; :459#caps_from_caller_are_enforced_before_pdfium", status: pass, evidence: "gate dell'integratore nel runner: verdi; grep del bridge: nessun --child né http:// in src/bin/extract e docling-bench; nessun file di 01-07 nel diff del merge"}
      - {test: "prova d'invocazione", ref: "spikes/s02-parser/results/extract/invocation.txt", status: pass, evidence: "letto dal bridge: inv-ok con outcome ok, pages 1, 496 parole, 2.659 caratteri; inv-params job_error InvalidParams; inv-path job_error PathNotAllowed/input_outside_roots"}
      - {test: "limiti", ref: "spikes/s02-parser/results/extract/limits-extract_pdf.json", status: pass, evidence: "JSON valido, profilo extract_pdf committato in ce26184; i 4 rifiuti NSANY dichiarati fra le anomalie"}
  - must_have: "Docling riceve solo PDF passati dal prescan nel job extract_pdf; il servizio si ferma e si ricrea fra un documento e il successivo e dopo ogni timeout, disconnessione o errore; cancellazione effettiva ed effetto di page_range sono misurati"
    verification:
      - {test: "TEST-1 (lato Docling)", risks: [RISK-001 di risk_audit_11], ref: "spikes/s02-parser/src/bin/docling-client/main.rs:665#only_extract_pdf_jobs_with_ok_report_are_admitted ; :690#not_prescanned_input_never_reaches_docling", status: pass, evidence: "gate dell'integratore: docling-client 9/9 verdi; docling.csv senza righe per le 4 pagine NSANY non estratte"}
      - {test: TEST-2, risks: [RISK-001 di risk_audit_11, RISK-002 di risk_audit_4], ref: "spikes/s02-parser/src/bin/docling-client/main.rs:635#caller_timeout_is_document_timeout_plus_15s ; :645#statuses_and_exit_codes_are_distinct ; :747#caller_timeout_is_client_timeout_exit_3 ; :760#abort_after_is_client_abort_exit_3 ; :772#service_error_is_error_not_timeout ; spikes/s02-parser/results/extract/docling_isolation.csv (timeout, disconnect, gpu e cpu, run 1 e 2)", status: pass, evidence: "test verdi nel gate; letto dal bridge nel CSV: 8 righe timeout/disconnect, tutte restarted=true e next_benign_status=ok; il lavoro dopo la disconnessione continua (CPU 210-862% a 10 s)"}
      - {test: TEST-3, risks: [RISK-001 di risk_audit_11], ref: "spikes/s02-parser/src/bin/docling-client/main.rs:712#request_carries_page_range_and_fixed_options ; docling_isolation.csv outside_range", status: pass, evidence: "test verde; righe outside_range 1 e 400 pagine su entrambi i dispositivi, picco del cgroup +11..34 MiB"}
      - {test: "TEST-3 (processing_time)", ref: "results/raw/docling/*/probe-outside-*-r2.json (ignorati da git)", status: human_judgment, human_judgment: true, rationale: "i valori di processing_time (+0,13 s GPU, −0,12 s CPU) stanno nei JSON grezzi, assenti su main e nel worktree ripulito: il bridge non li ha potuti rileggere. D-36 li ha già usati con decisione dell'utente"}
      - {test: TEST-4, risks: [RISK-001 di risk_audit_11], ref: "docling_isolation.csv righe state_marker gpu e cpu, run 1 e 2 ; spikes/s02-parser/docling-bench/docling-bench.sh:284-305", status: pass, evidence: "letto dal bridge: 4 righe marker_absent, restarted=true, pagina benigna ok"}
      - {test: "TEST-4 (processi, mount, stato fuori dal container)", risks: [RISK-001 di risk_audit_11], ref: "results/raw/docling/*/state-marker.tsv (ignorato da git)", status: human_judgment, human_judgment: true, rationale: "processi 4→4, cid diverso e unico mount docling-models in sola lettura sono nel TSV grezzo, non rileggibile dal bridge; lo stato non visibile da dentro il container (driver GPU dell'host) resta fuori prova per costruzione: MEDIUM residuo di risk_audit_11"}
      - {test: TEST-8, risks: [RISK-001 di risk_audit_11], ref: "spikes/s02-parser/src/bin/docling-client/main.rs:782#bench_script_restarts_with_stop_and_up_never_down_or_profiles", status: pass, evidence: "gate dell'integratore: verde; grep del bridge: session.sh stop presente, nessun session.sh down, compose run, --profile"}
      - {test: "costo del riavvio", ref: "results/raw/docling/*/restarts.tsv (ignorato da git)", status: human_judgment, human_judgment: true, rationale: "p50 19,1 s GPU e 20,8 s CPU vengono dal TSV grezzo, non rileggibile dal bridge; il numero è già alla base del punto aperto di D-36"}
      - {test: "E-12", ref: "docling_isolation.csv next_benign_s", status: pass, evidence: "ricalcolato dal bridge: massimo 2,021 s su GPU (p95 2,054) e 4,025 s su CPU (p95 8,016)"}
  - must_have: "Scanner di sicurezza con exit 0 e MEDIUM motivati"
    verification:
      - {test: "security-scanner", ref: "python skills/security-scanner/security_scan.py --fail-on HIGH spikes/s02-parser/src/bin spikes/s02-parser/docling-bench spikes/data/s02", status: pass, evidence: "rieseguito dal bridge: exit 0, 0 CRITICAL, 0 HIGH, 8 MEDIUM; 2 nei file del piano (extract/pdf.rs:18 e :129, pdf-untrusted-parse), 6 in file di 01-07 già motivati nel suo SUMMARY"}

# Metrics
duration: "circa 2 h 12 min il 2026-10-22: test rossi 60521e9 18:07 → documentazione a24c88c 20:19; merge su main a1241d4 20:22"
completed: 2026-10-22
---

# Fase 1 Piano 08: S-02, estrazione da PDF digitale, tabelle e confine di Docling · Summary

**PDFium, lanciato solo come figlio isolato `extract_pdf`, dà per ogni pagina testo e rettangoli delle parole, ma manca le tre soglie: CER 9,48% su TAT-DQA e 21,58% sull'oracolo IT per l'ordine dei blocchi, 77,0% di parole con IoU ≥ 0,5 per il box del font. Docling ha F1 di cella 0,580 e recall delle regioni 0,595 identici su GPU e CPU, con 2,05 s e 8,02 s per pagina al p95. L'euristica pagine-tabella perde 1 tabella su 11 nell'holdout (0,909), con precisione 0,435. Le prove di confine mostrano che la disconnessione del client non ferma il lavoro di Docling, che `page_range` non elabora le pagine fuori intervallo e che il riavvio per documento non lascia stato osservabile. L'esito di molte soglie è negativo e resta dichiarato: è uno spike, i numeri vanno all'ADR-0004**

## Performance

- **Duration:** circa 2 h 12 min. I test rossi `60521e9` (18:07:52) precedono il Task 1 `335d6bc` (18:51:28). Il Task 2, con cinque correzioni dello strumento, va da `3918aa8` (19:09:23) alla documentazione `a24c88c` (20:19:56). Il merge su main è `a1241d4` (20:22:05).
- **Started:** 2026-10-22T18:07:52+02:00
- **Completed:** 2026-10-22T20:22:05+02:00
- **Tasks:** 2 di 2
- **Files modified:** 22 su 22 `files_modified` del piano (`git diff --name-only a1241d4^1 a1241d4`, confrontato riga per riga con il frontmatter del PLAN: identici). Nessun file di 01-07, 01-02 o 01-01 nel diff; `Cargo.toml` e `Cargo.lock` non toccati.

## Accomplishments

- **Figlio `extract_pdf`** (`spikes/s02-parser/src/bin/extract/pdf.rs`). `run_pdf` (`pdf.rs:205-264`) legge i byte, chiama `prescan::check_pdf` (`:212`) e solo dopo lega PDFium (`:216-221`) e apre il documento dagli stessi byte (`:222`). `page_json` (`:129-202`) scrive per pagina testo, caratteri, parole e righe con `loose_bounds()` portati nella pagina ruotata da `Frame` (`:55-81`), più `media_box`, `crop_box`, `crop_differs` e rotazione. La CLI accetta solo l'argv di `resolve` (`main.rs:31-48`).
- **Punteggi** (`spikes/s02-parser/src/bin/extract/score.rs`): CER con scomposizione S/D/I (`score.rs:158-213`), regole meccaniche dell'oracolo IT (`:217-246`), conversione TAT-DQA (`:292-307`), abbinamento di parole a due passate (`:324-483`), F1 di cella a multinsieme (`:487-537`), tavola senza bbox (`:578-587`), recall delle regioni (`:590-620`).
- **Euristica pagine-tabella** (`spikes/s02-parser/src/bin/extract/tables.rs`): parametri chiusi con `deny_unknown_fields` (`tables.rs:16-61`), banda (`:114-122`), verdetto solo sulle righe holdout (`:143-155`), split disgiunti per documento (`:159-184`), punteggio di pagina (`:227-258`), taratura (`:477-511`).
- **Client Docling** (`spikes/s02-parser/src/bin/docling-client/main.rs`): esiti e codici d'uscita distinti (`main.rs:26-55`), timeout del chiamante = `document_timeout` + 15 s (`:76`), input ammessi da job + `RunReport` (`:92-106`), `Endpoint` con allow-list (`:110-151`), richiesta multipart con `page_range` (`:166-219`), timeout e abbandono volontario (`:252-261`), normalizzazione delle tabelle (`:304-364`).
- **Banco Docling** (`spikes/s02-parser/docling-bench/docling-bench.sh`): controllo sha256 (`:84-91`), riavvio stop+up (`:166-178`), misura con VRAM e lettura delle pagine dal descrittore 3 (`:195-243`), prove di confine (`:247-270`, `:307-317`), marcatore di stato (`:284-305`).
- **Dataset** (`spikes/data/s02/`): 84 voci con licenza e sha256, split 20 + 20 per documento, `fetch.sh` per membro d'archivio, `DATASET.md` con attribuzioni.
- **Evidenze** (`spikes/s02-parser/results/extract/`): 7 CSV con header del piano, `hw.json`, `limits-extract_pdf.json`, `invocation.txt`, `table_heuristic_params.json`, `iou_examples.json`; esito parziale in `spikes/s02-parser/RISULTATI-estrazione.md`.

## Task Commits

1. **Test rossi (agente test-write, distinto dall'esecutore):** `60521e9` (test). RED_EXPECTED: 40 nuovi test rossi (39 su `todo!()`, 1 per `docling-bench.sh` assente), verdi quelli di 01-07.
2. **Task 1: soglie, contratti, dataset, bin `extract`, `docling-client`, `docling-bench`:** `335d6bc` (feat).
3. **Task 2, correzioni dello strumento prima delle misure registrate:** `3918aa8` (fix, scrittura bufferizzata e banda sulle sole pagine estratte), `6d36335` (fix, larghezza e altezza dal MediaBox ruotato).
4. **Task 2, misure di testo e coordinate:** `ce26184` (test).
5. **Task 2, parametri congelati prima dell'holdout:** `5d2d65e` (test, un solo file).
6. **Task 2, correzioni del banco Docling:** `1097fd2`, `dfa11b2`, `bd501f5`, `a16b99b` (fix).
7. **Task 2, tabelle, Docling, prove di confine ed esito parziale:** `f24507b` (test).
8. **Documentazione della corsa GPU interrotta:** `a24c88c` (docs).

**Merge:** `a1241d4`. **Plan metadata:** questo SUMMARY e `01-08-RACCONTO.md`, scritti dal bridge senza commit.

## Files Created/Modified

- `spikes/s02-parser/src/bin/extract/main.rs`: CLI del bin (`pdf`, `score`, `select-tables`, `jobs`), generatore del JSONL dei job.
- `spikes/s02-parser/src/bin/extract/pdf.rs`: figlio `extract_pdf`; TEST-1 lato estrazione.
- `spikes/s02-parser/src/bin/extract/score.rs`: CER, IoU, celle, regioni, CSV, esempi; TEST-5 e TEST-7.
- `spikes/s02-parser/src/bin/extract/tables.rs`: euristica, banda, split, `table_pages.csv`, baseline PDFium a celle; TEST-6 e TEST-7.
- `spikes/s02-parser/src/bin/docling-client/main.rs`: client Docling; TEST-1 lato Docling, TEST-2, TEST-3, TEST-8.
- `spikes/s02-parser/docling-bench/docling-bench.sh`: banco host (cartella-strumento del piano, W18).
- `spikes/data/s02/{manifest.json,splits.json,fetch.sh,DATASET.md}`, `spikes/s02-parser/results/extract/*` (11 file), `spikes/s02-parser/RISULTATI-estrazione.md`.

## Esiti delle soglie

Riportati come li dà il contratto fissato prima della misura. Molte soglie non sono raggiunte: lo spike lo dichiara, non lo nasconde.

| Id | Misura | Valore | Soglia | Esito |
|----|--------|--------|--------|-------|
| E-1 | CER PDFium su TAT-DQA (EN) | 9,48% (15 pagine su 20 ≤ 1%; due pagine con annotazioni a larghezza zero fanno l'aggregato) | ≤ 1% | non raggiunta |
| E-2 | CER PDFium sull'oracolo IT | 21,58% (S = 0 ovunque, D ≈ I: ordine dei blocchi) | ≤ 1% | non raggiunta |
| E-3 | Parole TAT-DQA con match e IoU ≥ 0,5 | 77,0% (match di testo 96,4%) | ≥ 95% | non raggiunta |
| E-4 | F1 di cella Docling, 4 tavole IT | 0,580 GPU e CPU | ≥ 0,90 | non raggiunta |
| E-5 | F1 di cella baseline PDFium | 0,011 | riportata | — |
| E-6 | Recall regioni DocLayNet | 0,595 GPU e CPU | ≥ 0,90 | non raggiunta |
| E-7 | Recall pagine-tabella holdout | 0,909 (10/11) | ≥ 0,95 | non raggiunta |
| E-8 | Precisione holdout; recall e precisione taratura | 0,435; 0,800 e 0,727 | riportate | — |
| E-9 | Docling GPU, s/pagina p95 | 2,054 s (client quantizzato a ~2 s) | ≤ 1,0 s | non raggiunta |
| E-10 | Docling CPU, s/pagina p95 | 8,016 s | riportata | — |
| E-11 | VRAM di picco sopra il riposo | 1.569 MiB | registrata | — |
| E-12 | Pagina benigna dopo il riavvio entro il p95 del dispositivo | 2,01-2,02 s GPU, 4,01-4,02 s CPU | ≤ p95 | raggiunta |

Su E-4 la cifra 0,965, ottenuta spostando di una riga le tavole P-008 e P-009 dove Docling non marca la seconda riga d'intestazione, è una diagnostica calcolata dopo aver visto i risultati: **non è l'esito**.

**Prove di confine** (`docling_isolation.csv`, due ripetizioni per dispositivo): il `timeout` (400 pagine, `document_timeout` 5) dà `error` con exit 1, il container torna a riposo ma tiene 2,6-4,6 GB fino al riavvio; la `disconnect` (chiusura dopo 3 s) dà `client_abort` con exit 3 e **non ferma il lavoro** (CPU 210-862% e memoria in crescita a 10 s); `outside_range` mostra che una pagina dentro un PDF da 400 costa quasi come la pagina sola (+11..34 MiB di picco); `state_marker` è assente nel container ricreato su entrambi i dispositivi.

## Decisions Made

### Decisioni dell'utente (2026-10-22, già in `01-CONTEXT.md`)

- **D-36** (`01-CONTEXT.md:74`): D-27 si chiude con l'opzione 4, «lasciare com'è», perché la sua condizione («`page_range` non riduce il costo») non si è verificata. Resta un punto aperto per l'ADR di ingestion: il riavvio di Docling per documento (~19 s su GPU, ~21 s su CPU) e la precisione dell'euristica (0,435; nel documento IT 13 pagine inviate per 1 tabella vera) portano a ~26 s su GPU contro un TTQ di 15 s.
- **D-37** (`01-CONTEXT.md:75`): il rifiuto di 4 pagine DocLayNet benigne di `OTC_NSANY_2004` (`Unsupported`/`image_filter_on_non_image`, `spikes/s02-parser/src/prescan.rs:684-686`) è un falso positivo del prescan di 01-07. Si corregge nella gap-closure 1.4, allargata con il piano 01.4-02, non qui.

### Scelte dell'esecutore

- **Banda tarata sulle sole pagine estratte** (`tables.rs:487-494`): le 2 pagine-tabella NSANY senza job ok restano nei punteggi di taratura (`extracted=false`) ma non fissano `select_from`, che altrimenti sarebbe 0.
- **Larghezza e altezza dal MediaBox ruotato** (`pdf.rs:152-159`): PDFium misura la pagina sul CropBox, mentre i rettangoli sono riferiti al MediaBox; con la correzione `6d36335` pagina e parole stanno nello stesso riferimento.
- **Scrittura bufferizzata** (`pdf.rs:239-245`): senza `BufWriter` una pagina costava da 2 a 29 s sul bind mount di Docker Desktop, con il buffer circa 45 ms (dato del rapporto e di RISULTATI, non rimisurato dal bridge).
- **Oracolo IT senza bbox** (`score.rs:578-587`): la tavola attesa si abbina alla predetta della stessa pagina con più celle abbinate. La funzione col bbox, `score_tables` (`score.rs:548`), resta per il contratto e per TEST-5 ma non entra nelle misure, e lo dice l'attributo `cfg_attr` (`:541-547`).
- **`page_range` come due campi omonimi del multipart** (`docling-client/main.rs:185-189`), letto sull'OpenAPI del servizio; il fallback `/v1/convert/source` non è servito.
- **Allow-list degli host e tetti** (`docling-client/main.rs:110-151`, `:154-163`, `:225-234`) per chiudere gli HIGH SSRF e upload dello scanner senza esenzioni.
- **Codici d'uscita del client**: 0 ok, 1 error, 3 timeout o abbandono, 4 not_prescanned, 2 uso (`docling-client/main.rs:47-54`, `:491-517`).

## Deviations from Plan

### Livello a (dentro il piano)

1. **`docling-client --extract-jobs`.** Il `RunReport` di 01-07 non porta `input`: l'insieme ammesso si ricava unendo il JSONL dei job (`op: extract_pdf`, `job_id` → `input`) ai `RunReport` (`main.rs:92-106`). Flag in più rispetto al piano, previsto già dal test-write.
2. **sha256 «stesso input» nello script, non nel client** (`docling-bench.sh:84-91`, chiamato a `:206` per ogni documento): nessuna dipendenza `sha2` nel crate, `Cargo.toml` fuori dai file del piano.
3. **Job da file nella prova d'invocazione**: `session.sh exec` non inoltra lo stdin, quindi `--job <file>` invece di `--job -` (`invocation.txt`, righe di commento in testa).
4. **`fetch.sh` per membro d'archivio con HTTP Range**: gli archivi (155 MB-30 GB) superano il tetto di 100 MiB per file; sha256 sul membro; risolutore DNS-over-HTTPS facoltativo per l'host di DocLayNet.
5. **Campi additivi nel manifest** (`doc_id`, `role`, `member`, `coco_image_id`, `page_no`, `page_kind`).
6. **`hw.json` normalizzato** allo schema di `common::manifest`, come in 01-07; originale in `results/raw`.
7. **Corpus comune scaricato fuori sessione**: dentro `session.sh with`, `curl` riceveva percorsi MSYS non convertiti.
8. **Prove di confine ripetute due volte**: la ripetizione 1 non ha tempo della prova né `processing_time` (difetti `bd501f5`, `a16b99b`) ed è tenuta nel CSV.
9. **Corsa GPU interrotta e rifatta** (`a24c88c`): la prima corsa misurava una sola pagina per documento per il difetto dello stdin (`dfa11b2`); le sue righe sono state scartate e non compaiono in `docling.csv`.

### Livello b (segnalazioni su file chiusi, nessun file toccato)

10. **`hw-manifest.sh` (01-02) incompatibile con `common::manifest` (01-01)**: lo stesso difetto già registrato da 01-04, 01-05, 01-07, 01-11 e 01-12; correzione in una fase decimale.
11. **Falso positivo del prescan v6 di 01-07** su `OTC_NSANY_2004`: ora deciso dall'utente (D-37), gap-closure 1.4, piano 01.4-02.

### Deviazione d'ambiente del gate

12. **`cargo test --workspace` nel runner non eseguibile**: il registry cargo del runner è incompleto per gli altri membri. Il gate d'onda dell'integratore ha quindi eseguito `cargo test -p s02-parser --locked --offline` nel runner (180 verdi: lib 84, docling-client 9, extract 31, parse-child 1, parse-parent 7, isolation 37, oom 11 più 1 ignorato per progetto) e il resto del workspace sull'host (96 verdi).

---

**Total deviations:** 9 di livello a, 2 segnalazioni di livello b, 1 d'ambiente; nessuna di livello c. Nessun test tolto o indebolito: i blocchi `#[cfg(test)]` dei cinque file sono identici fra `60521e9` e `a1241d4`.
**Impact on plan:** nessuna estensione di perimetro (22 file su 22).

## Scanner di sicurezza

`python skills/security-scanner/security_scan.py --fail-on HIGH spikes/s02-parser/src/bin spikes/s02-parser/docling-bench spikes/data/s02`, rieseguito dal bridge: exit 0, 0 CRITICAL, 0 HIGH, 8 MEDIUM.

- **2 nei file del piano**, `pdf-untrusted-parse` in `extract/pdf.rs:18` (import di `pdfium_render`) e `:129` (`page_json`): il parsing PDF avviene solo nel figlio isolato `extract_pdf`, dopo il prescan, con i limiti calibrati del lanciatore di 01-07 (`RLIMIT_AS`, CPU, parete, quota, container senza rete, read-only, non root).
- **6 in file di 01-07** (`parse-child.rs:18`, `:150`; `parse-parent.rs:1129`, `:1247-1251`): compaiono perché la scansione copre `src/bin`, sono già motivati nel SUMMARY di 01-07.

## Discrepanze fra rapporto e prove

- **Evidenze grezze non rileggibili.** `results/raw` è ignorata da git e il worktree `.claude/worktrees/01-08` è vuoto: il bridge non ha potuto rileggere `restarts.tsv` (riavvio 19,1 s e 20,8 s), `probes.tsv`, `state-marker.tsv` (processi 4→4, mount in sola lettura) né i JSON grezzi con `processing_time`. Quei numeri sono marcati `human_judgment` in coverage; tutto ciò che sta nei CSV committati è stato ricalcolato.
- **p50 CPU**: il rapporto e RISULTATI dicono 4,015 s; il nearest-rank sulle 78 richieste utili dà 4,014 s. Differenza di definizione o arrotondamento, senza effetto sugli esiti (il p95 8,016 coincide).
- **Scanner**: il rapporto conta 2 MEDIUM; la scansione dei percorsi del piano ne trova 8, di cui 6 in file di 01-07. I 2 del piano coincidono.
- **Timeout**: il rapporto lo descrive come prova del timeout; l'esito osservato è una risposta d'errore del servizio (`error`, exit 1) prima del timeout del chiamante (5 + 15 s), non un `client_timeout`. Coerente con il contratto del client (`main.rs:200`, `:215`).
- Il resto coincide con le prove: commit e ordine, 22 file, invarianza di Soglie e Contratti dopo `335d6bc`, ordine split → risultati e parametri → `table_pages.csv`, conteggi dei test (31 + 9 nuovi), valori di E-1…E-12 dai CSV, header, `invocation.txt`, manifest e split.

## Threat Flags

- **T-01-08-01** (estrazione PDFium): solo job `extract_pdf`, prescan prima di PDFium; `invocation.txt`, `limits-extract_pdf.json`, TEST-1.
- **T-01-08-02** (Docling fuori dal supervisore): solo input con `RunReport` ok, host in allow-list, upload ≤ 25 MiB; `docling.csv` senza righe NSANY.
- **T-01-08-03** (lavoro residuo): la disconnessione non ferma il lavoro, dimostrato; il riavvio per documento è la mitigazione, non un'ottimizzazione.
- **T-01-08-04** (dataset): HTTPS e sha256 per membro; `raw/` ignorata.
- **T-01-08-05** (oracoli e metriche): contratti, split e parametri committati prima delle misure, verificato con `git merge-base`.
- **Residuo MEDIUM di risk_audit_11 (RISK-001)**: lo stato non visibile da dentro il container (driver GPU dell'host) resta fuori prova.

## Known Stubs

Nessuno nei file del piano. `score_tables` con bbox (`score.rs:548`) non è usata dalle misure perché gli oracoli tabella non hanno rettangoli: è dichiarata con `cfg_attr` e coperta dai test.

## User Setup Required

Nessuno.

## Punti aperti

- **Esiti sotto soglia** (E-1, E-2, E-3, E-4, E-6, E-7, E-9): li pesa l'ADR-0004 (01-13). In particolare l'ordine di lettura non può essere quello del flusso di PDFium, e la tolleranza geometrica di parola va decisa.
- **Costo di Docling per documento** (D-36): riavvio ~19-21 s e precisione 0,435 dell'euristica contro un TTQ di 15 s; punto aperto per l'ADR di ingestion.
- **Prescan su `OTC_NSANY_2004`** (D-37): gap-closure 1.4, piano 01.4-02, con nuova calibrazione dei limiti.
- **`hw-manifest.sh` / `common::manifest`**: difetto noto, fase decimale.
- **Stato fuori dal container** dopo il riavvio di Docling: non osservabile da questo banco.

## Next Phase Readiness

- **01-13 (ADR-0004)** riceve `spikes/s02-parser/RISULTATI-estrazione.md` con soglie, risultati, anomalie ed esito parziale, e l'impatto sulla porta `DocumentParser`: per i PDF, pagina per pagina, testo e rettangoli delle parole in punti PDF con origine in basso a sinistra, tabelle come blocchi con pagina, rettangolo e celle `(riga, colonna, testo)`.
- **01-09 e 01-13** consumano il contratto W17 come ha fatto questo piano: figlio in `src/bin/<nome>/`, limiti con `calibrate --op`, nessuna modifica alla libreria.
- **01.4-02** deve ricalibrare anche `limits-extract_pdf.json` se il prescan corretto ammette le 4 pagine NSANY; i numeri di taratura dell'euristica di questo piano sono stati calcolati senza di loro.

## Self-Check: PASSED

- 22 file su 22 dei `files_modified` nel diff del merge `a1241d4`, nessun file di altri piani.
- Rieseguiti dal bridge: ordine dei commit con `git merge-base`, invarianza di Soglie e Contratti, identità dei blocchi di test fra test rossi e merge, grep di accettazione, controllo node di manifest e split, header dei CSV, ricalcolo di E-1…E-12 dai CSV, scanner con soglia HIGH (exit 0).
- Test: gate dell'integratore nel runner Linux, 180 verdi su `s02-parser` e 96 sull'host per il resto del workspace, non ripetuti dal bridge.

**Modifica:** Claude (Anthropic) · 2026-10-22 · creazione (bridge di 01-08)
