---
phase: 01-spike-tecnici
plan: 04
subsystem: spike-upload-storage
tags: [axum, multipart, streaming, object_store, garage, pgbytea, sqlx, dhat, zip, admission-control, gc, dedup-side-channel, adr]

# Dependency graph
requires:
  - phase: 01-01
    provides: "workspace degli spike, crate spikes/common (env, manifest, csv, hist, timer) e Cargo.toml di s01-upload con le dipendenze fissate"
  - phase: 01-02
    provides: "harness Compose chiuso, session.sh (acquire/up/run/oneshot/exec/stop/down/release), hw-manifest.sh, runner Linux uid 10001, file segreti esterno"
provides:
  - "Spike S-01: server di upload multipart in streaming su Axum 0.8.9 (field.chunk() su tempfile, sha256 in streaming, limite di 25 MiB su tre livelli), validazione prima di storage e DB, admission control con semafori, budget e deadline"
  - "Trait BlobSink locale allo spike con tre adapter (GarageSink su object_store, PgByteaSink a chunk da 1 MiB, NullSink) e GC degli orfani con protocollo per hash live|deleting + pending"
  - "Evidenze con soglie scritte prima: dhat del puro streaming, VmHWM del percorso completo, matrice di 288 righe sui tre adapter, correttezza 20/20, admission, canale laterale della deduplica con protocollo statistico pre-registrato, storage, GC e gc_race"
  - "ADR-0003 Proposed: Garage dietro DocumentStore con parti da 2 MiB e una in volo, PUT idempotente proposto e non misurato, N = 24 h; PG bytea scartato"
  - "Impatto su porte (S-07) e threat model (T-05, T-06, T-07, T-08, T-11, T-23) scritti in RISULTATI §Esito per 01-16 e 01-17"
affects: [01-05, 01-07, 01-16, 01-17, 1.1]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Soglie e protocollo statistico committati prima di qualsiasi misura e mai più toccati; le correzioni stanno in «Anomalie e deviazioni»"
    - "Mitigazioni misurate dopo la matrice come opzioni del server con default invariato, con etichetta propria nella colonna adapter degli stessi CSV"
    - "Permesso di validazione e tempfile spostati dentro la closure di spawn_blocking: il lavoro già partito tiene le risorse fino alla fine anche se il client se ne va"
    - "Protocollo a stati per hash fra upload e GC: adozione A prima del PUT, adozione B nella transazione dei metadati, GC in tre passi con I/O fuori transazione"
    - "Build del runner offline con crate vendorizzate dall'host (spikes/target/vendor-s01, ignorata), perché il DNS del runner non risolve index.crates.io"

key-files:
  created:
    - spikes/s01-upload/RISULTATI.md
    - spikes/s01-upload/src/lib.rs
    - spikes/s01-upload/src/upload.rs
    - spikes/s01-upload/src/admission.rs
    - spikes/s01-upload/src/validate.rs
    - spikes/s01-upload/src/sink.rs
    - spikes/s01-upload/src/gc.rs
    - spikes/s01-upload/src/pdfgen.rs
    - spikes/s01-upload/src/bin/probe.rs
    - spikes/s01-upload/sql/schema.sql
    - spikes/s01-upload/bench.sh
    - spikes/s01-upload/fixtures.sh
    - spikes/docker/garage.toml
    - spikes/s01-upload/tests/dhat_1m.rs
    - spikes/s01-upload/tests/dhat_10m.rs
    - spikes/s01-upload/tests/dhat_25m.rs
    - spikes/s01-upload/tests/admission.rs
    - spikes/s01-upload/tests/gc_race.rs
    - spikes/s01-upload/tests/pdfgen.rs
    - spikes/s01-upload/results/hw.json
    - spikes/s01-upload/results/runs.csv
    - spikes/s01-upload/results/mem_by_size.csv
    - spikes/s01-upload/results/adapters.csv
    - spikes/s01-upload/results/dedup_timing.csv
    - spikes/s01-upload/results/correctness.csv
    - spikes/s01-upload/results/admission.csv
    - spikes/s01-upload/results/storage.csv
    - spikes/s01-upload/results/gc.csv
    - spikes/s01-upload/results/gc_race.csv
    - spikes/s01-upload/results/dhat_1m.json
    - spikes/s01-upload/results/dhat_10m.json
    - spikes/s01-upload/results/dhat_25m.json
    - docs/adr/0003-storage-originali-garage.md
  modified:
    - spikes/s01-upload/src/main.rs

key-decisions:
  - "Garage dietro DocumentStore con parti da 2 MiB e una in volo: con il default di object_store (parti da 10 MiB, fino a 8 in volo) VmHWM cresce di 42,6 / 48,1 MiB fra 1 e 25 MiB, oltre la soglia di 8 MiB"
  - "PG bytea scartato: canale laterale della deduplica fuori soglia in 0 sessioni su 3 superate, anche con ON CONFLICT DO UPDATE; latenza 10-20 volte Garage sotto concorrenza"
  - "PUT idempotente sulla chiave di contenuto (HEAD di verifica dopo un errore) proposto per i 500 della deduplica concorrente su Garage, non misurato: va provato nell'adapter del prodotto"
  - "N = 24 h per il GC degli orfani"
  - "Configurazione mitigata del canale laterale di Garage spostata da parti da 5 MiB a parti da 2 MiB (83720bb) dopo la misura di memoria e prima di raccogliere i tempi"

patterns-established:
  - "Protocollo pre-registrato del canale laterale: 3 sessioni pulite, blocchi alternati new/dedup con ordine da sha256, esclusione solo dei non-2xx, IC 95 % bootstrap, decisione a sicurezza chiusa"
  - "Ogni comando con container parte dall'host dentro una prenotazione session.sh; bench.sh host toglie dall'ambiente le chiavi del file segreti prima di chiamare compose"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "Il puro streaming ha picco di heap dhat costante fra 1, 10 e 25 MiB; il percorso completo con PDF validi ha VmHWM costante, misurato in un processo server nuovo per configurazione; soglie scritte prima della misura (criterio 1; D-03)"
    verification:
      - {test: TEST-1 (puro streaming), risks: [RISK-001], ref: "spikes/s01-upload/tests/dhat_25m.rs:98#test1_puro_streaming_dhat_25m_e_soglia ; spikes/s01-upload/results/dhat_1m.json, dhat_10m.json, dhat_25m.json", status: pass, evidence: "max_bytes 1_486_181 / 2_334_796 / 2_564_162, delta 25 − 1 = 1_077_981 B < 2 MiB; JSON versionati nel commit 15ea345 (sessione 01-04-20261020t113319z-9379). Il bridge ha riletto i JSON; non ha rieseguito il test, che è ignorato fuori Linux"}
      - {test: TEST-1 (percorso completo, default di object_store), risks: [RISK-001], ref: "spikes/s01-upload/results/mem_by_size.csv righe adapter=garage mode=full, run 2 e 3", status: fail, evidence: "ricalcolato dal bridge: Garage 68_544 − 24_952 = 43_592 KiB (42,6 MiB) e 69_656 − 20_392 = 49_264 KiB (48,1 MiB), soglia < 8 MiB. Fallimento registrato in RISULTATI §Esito come da piano; PG bytea (−0,4 / 1,6) e nullo (0,8 / 0,4) superano"}
      - {test: TEST-1 (percorso completo, garage-p2k1), risks: [RISK-001], ref: "spikes/s01-upload/results/mem_by_size.csv righe adapter=garage-p2k1, run 2 e 3", status: pass, evidence: "ricalcolato dal bridge: 26_460 − 25_240 = 1_220 KiB e 25_608 − 23_660 = 1_948 KiB (1,2 / 1,9 MiB). Configurazione misurata dopo la matrice (codice 2bc8a00), non quella del piano: il criterio 1 è chiuso solo a questa condizione. garage-p5k1 non supera (9,4 / 14,6 MiB)"}
      - {test: Task-3-acceptance (soglie prima dei CSV), ref: "git log --diff-filter=A -- spikes/s01-upload/results/adapters.csv ; git show 922cf42 vs f0f61d3 della sezione Soglie", status: pass, evidence: "rieseguito dal bridge: primo commit di adapters.csv cc24b09 (ct 1792512851) dopo 922cf42 (ct 1792449349); sezione «Soglie (scritte PRIMA della misura)» identica byte per byte fra 922cf42 e f0f61d3"}
  - must_have: "Oltre 25 MiB (26_214_401 byte) la risposta è 413 senza leggere il corpo intero, con il limite su tre livelli; 26_214_399 e 26_214_400 byte sono accettati (D-04)"
    verification:
      - {test: TEST-2, risks: [RISK-001], ref: "spikes/s01-upload/tests/admission.rs:434#test2_confini_25mib_accettati_e_piu_uno_413 ; spikes/s01-upload/tests/pdfgen.rs:73,90", status: pass, evidence: "rieseguito dal bridge sull'host: admission 4 passed 1 ignored, pdfgen 13 passed 1 ignored"}
      - {test: TEST-2 (qpdf), risks: [RISK-001], ref: "spikes/s01-upload/tests/pdfgen.rs:111#test2_pdfgen_qpdf_check_verde", status: human_judgment, human_judgment: true, rationale: "ignorato fuori Linux; «pdfgen 14 test verdi con qpdf --check» è attestato dal messaggio del commit 15ea345 e dal rapporto, senza log versionato del runner"}
      - {test: TEST-2 (100 MiB e byte letti), risks: [RISK-001], ref: "spikes/s01-upload/results/correctness.csv righe 25MiB-1, 25MiB, 25MiB+1, 100MiB ; spikes/s01-upload/src/upload.rs:42,163,172,354", status: pass, evidence: "25MiB+1 → 413 con 26_214_401 byte letti (soglia 26_345_472); 100MiB → 413 con 0 byte del campo letti; confini accettati. Tre livelli nel codice: RequestBodyLimitLayer globale, DefaultBodyLimit sulla route, contatore manuale"}
  - must_have: "Prima di storage e DB girano magic bytes, struttura e limiti DOCX e un prefiltro byte-level dei PDF; il controllo autorevole dei PDF resta al processo isolato di S-02 (D-03, D-05, D-08)"
    verification:
      - {test: TEST-3, risks: [RISK-001], ref: "spikes/s01-upload/tests/pdfgen.rs:134-289 (test3_*) ; spikes/s01-upload/tests/admission.rs:532#test3_file_non_valido_non_raggiunge_il_sink", status: pass, evidence: "rieseguito dal bridge sull'host: verdi. Flake noto di test3_magic_mismatch_e_polyglot_415 sull'host (1 su 4 giri il 2026-10-20, probabile scansione antivirus di Windows sul tempfile ZIP) non riprodotto oggi"}
      - {test: TEST-3 (end-to-end), risks: [RISK-001], ref: "spikes/s01-upload/results/correctness.csv", status: pass, evidence: "20 righe, 20 pass=true (polyglot, magic-mismatch → 415; zip-bomb, dtd, encrypted, troncato → 422; javascript, javascript-offuscato, launch, embedded → quarantined), sessione 01-04-20261020t160904z-c0f9 a 34e73f6"}
  - must_have: "Upload e validazioni hanno admission control: sotto carico ostile le validazioni attive non superano il tetto e disco e FD tornano al baseline (T-05)"
    verification:
      - {test: TEST-4 (tetto upload, 503, 408, baseline), risks: [RISK-001], ref: "spikes/s01-upload/tests/admission.rs:224#test4_admission_sotto_carico_ostile ; spikes/s01-upload/results/admission.csv", status: pass, evidence: "slow12_docx6_half_closed: peak_active_uploads 4 = max_inflight, 14 × 503 tutti con Retry-After, 3 × 408, FD 13 → 14, 0 byte temporanei; test eseguito nel runner al 15ea345"}
      - {test: TEST-4 (tetto validazioni), risks: [RISK-001], ref: "spikes/s01-upload/results/admission.csv colonna peak_active_validations", status: human_judgment, human_judgment: true, rationale: "picco 1 con max_validations = 2: il tetto non è mai stato raggiunto, quindi la prova mostra che non è superato ma non che il semaforo di validazione limiti davvero sotto saturazione. Il codice (admission.rs:165-176) è corretto alla lettura"}
      - {test: TEST-4 (deadline e cancellazione), risks: [RISK-001], ref: "spikes/s01-upload/tests/pdfgen.rs:291,307", status: pass, evidence: "rieseguiti dal bridge sull'host: verdi"}
  - must_have: "Garage, PG bytea a chunk da 1 MiB e adapter nullo sono misurati sugli stessi insiemi di corpi, con carichi separati, reset dello storage e conteggio delle scritture effettive; la scelta di Garage è motivata da numeri in ADR-0003 (D-01, D-06; A-02)"
    verification:
      - {test: TEST-6, risks: [RISK-001], ref: "spikes/s01-upload/results/adapters.csv, runs.csv ; docs/adr/0003-storage-originali-garage.md §Opzioni e §Evidenze", status: pass, evidence: "ricontato dal bridge: 288 righe con adapter ∈ {garage, pgbytea, null}; 0 righe new di Garage/PG con distinct_sha ≠ objects_after ≠ req; 0 righe dedup con objects_after ≠ 1; 0 righe 5xx fuori da Garage"}
      - {test: TEST-6 (500 di Garage), risks: [RISK-001], ref: "spikes/s01-upload/results/adapters.csv righe garage* workload=dedup", status: fail, evidence: "500 nella deduplica concorrente: default a c = 16 54-78 su 200 a 24-25 MiB e 6-14 a 10 MiB; garage-p2k1 2-7 a 10 MiB e 0 a 25 MiB. Ridotti, non eliminati. Fuori dalle soglie scritte (nessuna soglia sui 5xx), registrati in RISULTATI"}
      - {test: TEST-6 (diagnosi dei 500), risks: [RISK-001], ref: "spikes/s01-upload/RISULTATI.md «Anomalie e deviazioni», ripresa 4", status: human_judgment, human_judgment: true, rationale: "la causa (abort del multipart → 404 NoSuchUpload) viene dai log del server in results/raw/, ignorati da git e spariti con il worktree 01-04; che Garage chiuda i multipart concorrenti sulla stessa chiave è dedotto dai log, non verificato sul codice di Garage"}
  - must_have: "Chiave = sha256 del contenuto, PUT prima della transazione breve metadati+outbox, GC degli orfani con N=24 h coordinato con l'adozione concorrente da un protocollo a stati provato sulle interleaving enumerate; risposta identica con o senza deduplica anche nei tempi (D-02, T-11)"
    verification:
      - {test: TEST-5, risks: [RISK-001], ref: "spikes/s01-upload/tests/gc_race.rs:331#test5_gc_race_interleavings_enumerate ; spikes/s01-upload/results/gc_race.csv", status: pass, evidence: "8 righe (a, b, c, d × garage, pgbytea), unreadable 0, pass=true, runner al 15ea345. Prova delle sole interleaving enumerate, non dell'assenza di ogni race"}
      - {test: TEST-7, risks: [RISK-001], ref: "spikes/s01-upload/results/dedup_timing.csv ; spikes/s01-upload/RISULTATI.md «Protocollo della misura del canale laterale» (922cf42)", status: pass, evidence: "ricalcolato dal bridge: 200 + 200 righe per adapter e sessione; mediane identiche a RISULTATI (Garage d = 0,170 / −0,075 / 0,027 ms; PG −194,3 / −45,1 / −31,9; garage-p2k1 −9,6 / −8,5 / −1,3; pgbytea-update −109,9 / 16,9 / 7,3). Esito: Garage e garage-p2k1 3/3 superano; PG bytea e pgbytea-update 0/3. Gli IC bootstrap non sono stati ricalcolati"}
      - {test: TEST-8, risks: [RISK-001], ref: "spikes/s01-upload/tests/admission.rs:564#test8_risposta_identica_con_o_senza_deduplica ; correctness.csv riga duplicato", status: pass, evidence: "rieseguito dal bridge sull'host: verde (stessi campi, upload_id diversi, 2 PUT). Correttezza: 2 righe, 1 oggetto, campi sha256,size_bytes,status,upload_id"}
      - {test: TEST-9, risks: [RISK-001], ref: "spikes/s01-upload/tests/admission.rs:474#test9_upload_interrotto_nessun_put_nessun_tempfile ; gc_race casi c e d ; correctness.csv righe interrotto e crash-fra-put-e-tx ; gc.csv", status: pass, evidence: "test9 rieseguito dal bridge sull'host: verde. interrotto 0/0/0; crash-fra-put-e-tx 0 righe, orfano raccolto con N = 0, 1 tempfile lasciato dall'abort (atteso, registrato). gc.csv: 100 orfani di 48 h raccolti, 100 di 1 h protetti con N = 24 h"}
  - must_have: "Nessun segreto versionato, nessuna porta del server pubblicata, garage.toml senza segreti (T-01-04-05, T-23)"
    verification:
      - {test: TEST-10, risks: [RISK-001], ref: "python skills/security-scanner/security_scan.py s01-upload docker/garage.toml ; grep http:// in src ; grep rpc_secret|admin_token|metrics_token in garage.toml", status: pass, evidence: "rieseguito dal bridge sul main: «OK - nessun problema di sicurezza rilevato su 25 file» exit 0; 0 http:// in src; 0 segreti in garage.toml; 0 field.bytes(); results/raw/ ignorata da spikes/.gitignore:4"}

# Metrics
duration: "circa 35 h di calendario (2026-10-20 00:35 → 2026-10-21 11:35), con quattro riprese da agenti nuovi"
completed: 2026-10-21
---

# Fase 1 Piano 04: S-01 + A-02, upload in streaming e storage degli originali · Summary

**Upload multipart in streaming su Axum con heap costante, limite di 25 MiB su tre livelli, validazione prima dello storage e admission control; Garage proposto per gli originali solo con parti del multipart da 2 MiB e una in volo (il default di object_store non regge la soglia di memoria), PG `bytea` scartato per il canale laterale della deduplica, 500 di Garage sulla deduplica concorrente ridotti ma non risolti**

## Performance

- **Duration:** circa 35 ore di calendario, dal commit delle soglie `922cf42` (2026-10-20 00:35) al summary dell'esecutore `f0f61d3` (2026-10-21 11:35). I test rossi di `c9085b7` (00:26) li ha scritti un agente separato. Il tempo comprende quattro riprese da agenti nuovi: tre dopo interruzioni di rete e una dopo un riavvio di Claude Code. Il merge su main è `cc202a5` (11:37).
- **Started:** 2026-10-20T00:35:49+02:00
- **Completed:** 2026-10-21T11:35:48+02:00
- **Tasks:** 4 di 4
- **Files modified:** 34, esattamente i `files_modified` del piano: 33 creati, più `spikes/s01-upload/src/main.rs` modificato. Il SUMMARY sta fuori elenco.

## Accomplishments

- Server `s01-upload` con un unico handler `POST /upload` (`spikes/s01-upload/src/upload.rs:431`). L'ordine è: permesso di upload, ricezione con `field.chunk()` su tempfile e sha256 in streaming, permesso di validazione, `validate::check` dentro `spawn_blocking`, adozione A, PUT sempre, transazione breve `upload` + `outbox` + adozione B. La risposta ha la stessa forma con e senza deduplica.
- Validazione byte-level (`validate.rs`). Magic bytes per PDF, DOCX e Markdown. Per i DOCX: al più 1.000 voci, somma dichiarata ≤ 200 MiB, rapporto di compressione, macro, DTD ed entità. Per i PDF un prefiltro con decodifica `#hh`: `/Encrypt` porta al rifiuto, `/JavaScript`, `/Launch` ed `/EmbeddedFile` alla quarantena. È dichiarato come prefiltro: il controllo autorevole resta in S-02.
- Admission control (`admission.rs`): due semafori, un budget di byte temporanei e deadline di ricezione, di inattività e di validazione. Il runtime ha `max_blocking_threads` esplicito.
- Trait `BlobSink` locale con `GarageSink`, `PgByteaSink` e `NullSink` (`sink.rs`). Il GC usa il protocollo `live|deleting` + `pending` con barriere `GcHooks` per i test (`gc.rs`). C'è un generatore di PDF validi di dimensione esatta (`pdfgen.rs`) e `probe` per reset, conteggi, aggregati, IC bootstrap, GC, range e dimensioni di PG.
- `bench.sh host …` orchestra dall'host tutte le sessioni con `session.sh`: test, matrice in tre run, correttezza, canale laterale in tre sessioni, storage/GC, mitigazioni e `finalize`. `bench.sh client` gira nel runner con `curl` e `xargs -P`.
- Evidenze: le soglie sono state committate prima di tutto e mai più toccate. Sono stati misurati 288 righe di matrice, 20/20 casi di correttezza, admission, 3.600 + 2.400 tempi del canale laterale, storage, GC e 8 interleaving di `gc_race`.
- ADR-0003 `Proposed` con opzioni, numeri, decisione condizionata, conseguenze ed evidenze. In RISULTATI §Esito ci sono l'impatto sulle porte di S-07 e sul threat model.

## Esito delle soglie, in breve

| Soglia | Esito |
|--------|-------|
| Puro streaming, delta dhat 25 − 1 MiB < 2 MiB | supera (1,03 MiB) |
| Percorso completo, delta `VmHWM` < 8 MiB | Garage con il default di `object_store` **non supera** (42,6 / 48,1 MiB). Supera solo con parti da 2 MiB e 1 in volo (1,2 / 1,9). Con parti da 5 MiB non supera (9,4 / 14,6). PG `bytea` e nullo superano |
| Ack p95 ≤ 500 ms, 10 MiB, Garage, `new`, c = 1 e 4 | supera, con il default e con le parti da 2 MiB |
| Confini, correttezza, scritture effettive | supera (20/20; `new` 200/200/200; `dedup` 1 oggetto) |
| Admission control | supera; il tetto delle validazioni però non è stato saturato (picco 1 su 2) |
| Canale laterale della deduplica | Garage supera 3/3 (default e parti da 2 MiB). PG `bytea` 0/3, anche con `ON CONFLICT DO UPDATE` |
| Range 64 KiB p95 ≤ 50 ms | supera (Garage 3,267 ms, PG 0,742 ms; non rimisurato con le parti da 2 MiB) |
| GC contro adozione, interleaving enumerate | supera (8/8, solo quelle enumerate) |
| Fuori soglia | 500 di Garage nella deduplica concorrente, ridotti ma non eliminati |

## Task Commits

1. **Test rossi (agente test-write, distinto dall'esecutore):** `c9085b7` (test). Stato RED_EXPECTED sull'host: 17 falliti per `todo!`, 7 ignorati. Il rosso nel runner non è stato verificato per il DNS.
2. **Task 1: soglie, poi server, admission, validazione, adapter, GC e generatore:** `922cf42` (docs: soglie e protocollo del canale laterale, prima di qualsiasi codice), `bbd3a4d` (feat), `8e6f2ee` (fix: SIGTERM con PID 1 e valore `after_put` di clap).
3. **Task 2: test Linux nel runner:** `15ea345` (test: evidenze dhat, admission, gc_race; pdfgen verde con qpdf).
4. **Task 3: fixture, benchmark e misure:** `03b8f1e` (feat), `db62725` (fix: risoluzione dei nomi del compose con tentativi), `9341b43` (fix: le variabili dell'host non scavalcano il file segreti), `34e73f6` (test: storage, GC, provenienza delle misure ereditate), `cc24b09` (test: matrice, correttezza, canale laterale), `2bc8a00` (feat: opzioni mitigate con default invariato), `83720bb` (fix: canale laterale mitigato su parti da 2 MiB), `690b42b` (test: misure delle mitigazioni e diagnosi dei 500).
5. **Task 4: esito e ADR:** `bfd77e9` (docs: Esito in RISULTATI), `d068c8f` (docs: ADR-0003).

**Plan metadata:** `f0f61d3` (summary dell'esecutore, riscritto da questo bridge). Il merge su main è `cc202a5`.

## Files Created/Modified

- `spikes/s01-upload/src/upload.rs`: router con i tre livelli del limite, ricezione in streaming, handler, transazione dei metadati, endpoint di bench.
- `spikes/s01-upload/src/admission.rs`: configurazione, semafori, permessi RAII, contatori e `/bench/stats` (FD, `VmHWM`, `memory.peak`).
- `spikes/s01-upload/src/validate.rs`: magic bytes, limiti DOCX, prefiltro PDF con decodifica `#hh`, guardia di deadline e cancellazione.
- `spikes/s01-upload/src/sink.rs`: trait `BlobSink` e tre adapter. Il tempfile si legge con `tokio::fs::File`, mai con `std::fs` nell'async.
- `spikes/s01-upload/src/gc.rs`: adozione A/B, GC in tre passi, `GcHooks`, `apply_schema`.
- `spikes/s01-upload/src/pdfgen.rs`: PDF valido e deterministico di dimensione esatta.
- `spikes/s01-upload/src/bin/probe.rs`: strumenti di misura senza HTTP.
- `spikes/s01-upload/src/main.rs`, `spikes/s01-upload/src/lib.rs`: CLI (`--adapter`, `--validate`, `--bench`, `--fault`, admission, opzioni mitigate) e moduli.
- `spikes/s01-upload/sql/schema.sql`: `upload`, `outbox`, `blob`, `blob_chunk`.
- `spikes/s01-upload/bench.sh`, `fixtures.sh`: orchestrazione dall'host e fixture nel runner.
- `spikes/docker/garage.toml`: configurazione di Garage senza segreti letterali.
- `spikes/s01-upload/tests/*.rs`: i sei file di test di `c9085b7`, invariati dopo quel commit.
- `spikes/s01-upload/results/*`: CSV a header fisso, JSON dhat e `hw.json` della sessione `c0f9`.
- `spikes/s01-upload/RISULTATI.md`, `docs/adr/0003-storage-originali-garage.md`: esito e decisione proposta.

## Decisions Made

- **Garage dietro `DocumentStore`, condizionato** a tre cose: parti da 2 MiB con una in volo, PUT idempotente per chiave (proposto, **non misurato**) e N = 24 h. La sola configurazione misurata che supera insieme la soglia di memoria e quella del canale laterale è `garage-p2k1`.
- **PG `bytea` scartato.** Il canale laterale non supera in nessuna sessione (0/3), nemmeno mitigato con la riscrittura dei chunk. A c = 4 e c = 16 la latenza è 10-20 volte quella di Garage. Restano a suo favore memoria costante, nessun 500 e range più veloce, ma non compensano un requisito di sicurezza (T-11).
- **Scelta della mitigazione dopo una misura.** Il canale laterale mitigato di Garage doveva usare parti da 5 MiB. L'esecutore lo ha spostato a 2 MiB (`83720bb`) dopo aver visto che `garage-p5k1` non supera la soglia di `VmHWM`, e prima di raccogliere i tempi del canale laterale mitigato. È una scelta post-misura dichiarata, che non tocca soglie né protocollo.
- **Misure ereditate tenute.** Le tre sessioni del canale laterale prodotte da un esecutore interrotto (`git_rev` 15ea345, codice identico a 34e73f6) non sono state ripetute: una seconda estrazione è vietata dal protocollo pre-registrato. La correttezza è stata invece ripetuta a 34e73f6.

## Rischi del risk audit e trattamento

- **risk_audit_9 (v2), RISK-001 MEDIUM:** la soglia del canale laterale poteva essere applicata dopo aver raccolto i tempi. Mitigato con il protocollo statistico committato in `922cf42` prima di qualsiasi tempo: 3 sessioni pulite, blocchi alternati, esclusione dei soli non-2xx, IC bootstrap, decisione a sicurezza chiusa. Il protocollo è stato applicato senza esclusioni e senza seconde estrazioni. Unica scelta dopo una misura: la configurazione mitigata da misurare (vedi sopra), dichiarata.
- **risk_audit_2 (v1), RISK-001..005**, chiusi dalla v2 del piano e verificati nel codice: orchestrazione Docker solo dall'host (RISK-001), carichi `new`/`dedup` separati con pool distinti (RISK-002), admission control (RISK-003, con il limite di prova sul semaforo di validazione detto sopra), protocollo GC/adozione (RISK-004, solo interleaving enumerate), PDF validi di dimensione esatta controllati con `qpdf` (RISK-005).

## Deviations from Plan

### Auto-fixed Issues (livello a)

**1. [Rule 3 - Blocking] Build del runner offline con crate vendorizzate.** Il DNS di Docker non risolve `index.crates.io`: 35 `cargo fetch` falliti fra test-write ed esecutore. `bench.sh host vendor` vendorizza dall'host in `spikes/target/vendor-s01` (ignorata) e il runner compila con `--offline` (`bench.sh:96-100`). Nessun file di 01-02 è stato toccato e le crate sono quelle del `Cargo.lock`.

**2. [Rule 1 - Bug] clap e SIGTERM** (`8e6f2ee`). `--fault after_put` veniva reso come `after-put`, e il server come PID 1 non gestiva SIGTERM. Corretti con `#[value(name = "after_put")]` e `with_graceful_shutdown` (`main.rs:34,190-197`).

**3. [Rule 1 - Bug] Risoluzione dei nomi del compose** (`db62725`). `pin_env` con 20 tentativi da 0,5 s lasciava `postgres` irrisolto senza errore. Ora fa 120 tentativi e si ferma con un errore se il nome non si risolve.

**4. [Rule 3 - Blocking] L'hook pre-commit rifiuta i commit di soli CSV.** Con uno scope vuoto lo scanner esce con rc = 2. I CSV sono stati committati insieme a `RISULTATI.md`. È lo stesso comportamento già visto in 01-02 con i `.txt`.

**5. [Deviazione a] Mitigazioni negli stessi CSV del piano** con etichetta propria nella colonna `adapter` (`garage-diag`, `garage-p5k1`, `garage-p2k1`, `pgbytea-update`). Non ci sono CSV nuovi, e i conteggi della matrice si leggono filtrando l'adapter.

**6. [Deviazione a] `results/hw.json`** riportato al manifest della sessione `c0f9` dopo le sessioni di mitigazione. Gli altri manifest stavano in `results/raw/`, che non è versionata.

**7. [Deviazione a] Matrice in tre sessioni**, una per run, invece di una sola. Ordine delle dimensioni crescente, decrescente, crescente; run 1 di warm-up.

### Segnalazione su file chiuso di 01-02 (livello b-segnalazione, non corretto qui)

**8. `session.sh`/compose: l'ambiente dell'host vince su `--env-file`** nell'interpolazione. Una `DATABASE_URL` globale dell'host (database cloud Neon di un altro progetto, con `sslmode=require`) è arrivata al runner. `gc_race`, che fa `TRUNCATE`, è caduto al primo giro per DNS e al secondo per il TLS assente in sqlx. Secondo il rapporto non c'è stata nessuna connessione riuscita e nessun dato esterno è stato toccato: il bridge non ha modo di verificarlo oltre il rapporto. Workaround nel file del piano (`9341b43`): `bench.sh host` toglie dall'ambiente le chiavi del file segreti (`bench.sh:380-388`), e `pin_env` si ferma se `DATABASE_URL` o `S3_ENDPOINT` non puntano ai servizi del compose (`bench.sh:65-72`). La correzione vera va nella gap-closure **fase 1.1** (`riapre_perimetro: [01-02]`), già annotata in `e1fd014`. Fino ad allora ogni altro piano che usa `session.sh` senza un workaround analogo resta esposto.

---

**Total deviations:** 8 (3 auto-fix di bug o blocchi, 4 di processo o di evidenza, 1 segnalazione su file chiuso); nessuna di livello b vero né c, a parte le interruzioni esterne.
**Impact on plan:** nessuna estensione di perimetro (34 file su 34 dichiarati). La parte che conta è il risultato. Il criterio 1 è chiuso solo con una configurazione dell'adapter diversa dal default, misurata dopo la matrice.

## Issues Encountered

- **Interruzioni esterne (livello c).** Ci sono state quattro riprese da agenti nuovi: tre dopo interruzioni di rete (l'ultima con ENOTFOUND, ripresa da `2bc8a00`) e una dopo un riavvio di Claude Code (da `34e73f6`). Anche il test-write è ripartito dopo una caduta di rete, rileggendo i file non committati. A ogni ripresa le misure prodotte da codice non committato sono state rifatte, quelle tracciabili per `git_rev` tenute.
- **Test Linux non ripetuti dopo `2bc8a00`.** dhat, `pdfgen::test2_pdfgen_qpdf_check_verde`, `admission::test4` e `gc_race` sono stati eseguiti nel runner a `15ea345`. `2bc8a00` aggiunge opzioni con default invariato a `main.rs`, `sink.rs` e `upload.rs` (`git diff --stat 15ea345 2bc8a00 -- src`: 3 file, +100 −17). Gli eseguibili su Linux non sono stati rilanciati. Sull'host fmt, clippy `-D warnings` e `cargo test -p s01-upload --locked` sono verdi (rieseguiti dal bridge: lib 2, admission 4 + 1 ignorato, pdfgen 13 + 1 ignorato, dhat e gc_race ignorati).
- **Flake sull'host di `pdfgen::test3_magic_mismatch_e_polyglot_415`.** È fallito 1 volta su 4 giri il 2026-10-20 e non si è riprodotto nei giri successivi né nel bridge. La causa probabile è la scansione antivirus di Windows sul tempfile ZIP, ma non è stata verificata.
- **Istogrammi per fase saturati** (`GET /bench/phases`, p50 di 2 µs): non sono stati usati. Le latenze vengono tutte da `time_total` di `curl`.

## Discrepanze fra rapporto, SUMMARY dell'esecutore e prove

- Il SUMMARY dell'esecutore non aveva il blocco `coverage`, `patterns-established` né le sezioni del template. Elencava i CSV come `modified` con un glob, mentre sono tutti nuovi, e dava una durata di «2 giorni» invece di circa 35 ore. Corretto qui.
- «Admission control: tetti rispettati»: è vero per gli upload in corso. Il tetto delle validazioni però non è mai stato raggiunto (picco 1 su 2), quindi non è dimostrato sotto saturazione. Qui è `human_judgment`.
- RISULTATI §Esito dice «ancora 2-13 su 200 a 10 MiB con le parti piccole»: 13 è un valore di `garage-p5k1`. Per `garage-p2k1`, la configurazione proposta, i valori sono 2-7 (`adapters.csv`), come scrive correttamente l'ADR. È un refuso del documento e non cambia la decisione.
- Le prove della diagnosi dei 500 (log del server) e i manifest delle sessioni stavano in `results/raw/`, non versionata, e sono spariti con il worktree 01-04. La diagnosi resta attestata solo da RISULTATI e dal rapporto.

## Threat Flags

- **T-11, nuovo segnale:** i 500 della deduplica concorrente rivelano che qualcuno sta caricando lo stesso contenuto nello stesso momento. Resta aperto finché il PUT idempotente non viene implementato e misurato nell'adapter del prodotto.
- **T-11, d sistematicamente negativo con le parti da 2 MiB** (−1,3 … −9,6 ms in tre sessioni su tre): è entro T = 20 ms, ma va rimisurato nel prodotto.
- **T-01-04-07 accettato:** `with_allow_http(true)` verso Garage solo sulla rete compose; il TLS resta alla Fondazione.

## Known Stubs

Nessuno nel codice dello spike. Il PUT idempotente esiste solo come proposta in RISULTATI e nell'ADR.

## User Setup Required

Nessuno. Il file segreti esterno è quello di 01-02. Attenzione però a una `DATABASE_URL` globale nell'ambiente dell'host (vedi deviazione 8).

## Punti aperti

- Il PUT idempotente con HEAD di verifica va provato nell'adapter del prodotto con il carico `dedup` a c = 16 e con il protocollo del canale laterale.
- **Parti da 2 MiB sotto il minimo S3.** Il commento di `sink.rs:130` ricorda che S3 chiede parti di almeno 5 MiB, tranne l'ultima. Garage le ha accettate, ma la configurazione proposta potrebbe non essere portabile su un altro storage S3-compatibile, e l'ADR non lo dice. Da valutare in 01-16/01-17.
- Lettura per range con `garage-p2k1` non misurata: il p95 di 3,267 ms è del default.
- Il tetto di validazione va provato sotto saturazione.
- Al riavvio il servizio deve ripulire i tempfile lasciati da un crash.
- Gap-closure 1.1 su `session.sh`.

## Next Phase Readiness

- 01-16 (S-07) può trascrivere le firme di `DocumentStore` da RISULTATI §Esito «Impatto su porte (S-07)»: `put(ContentHash, percorso validato)` idempotente, `get` con range, `delete` solo dal GC, nessun tipo `object_store` in firma.
- 01-17 ratifica ADR-0003, porta la riga «Originali PDF/DOCX» di `DEPENDENCIES.md` a Decisa e aggiorna il threat model per T-05, T-06, T-07, T-08, T-11 e T-23.
- 01-07 (S-02) eredita il ruolo di controllo autorevole dei PDF che il prefiltro qui non copre.
- La fase 1.1 va chiusa prima dell'onda 3.

## Self-Check: PASSED

- 34 file su 34 dichiarati presenti nel diff `ecd61ed..f0f61d3`, nessuno fuori elenco tranne il SUMMARY. Test invariati dopo `c9085b7`. Sezione Soglie identica fra `922cf42` e `f0f61d3`.
- Host verde (fmt, clippy `-D warnings`, test), scanner exit 0. I numeri principali (VmHWM, 5xx, mediane del canale laterale, conteggi della matrice) sono stati ricalcolati dal bridge sui CSV.

**Modifica:** Claude (gsd-executor 01-04, ripresa 4) · 2026-10-21 · summary del piano
**Modifica:** Claude (Anthropic) · 2026-10-21 · bridge del piano 01-04
