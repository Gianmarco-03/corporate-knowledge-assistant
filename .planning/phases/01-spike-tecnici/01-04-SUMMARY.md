---
phase: 01-spike-tecnici
plan: 04
subsystem: spike-upload-storage
tags: [axum, multipart, streaming, object_store, garage, pgbytea, dhat, admission-control, gc, dedup-side-channel]

requires:
  - phase: 01-01
    provides: "crate spikes/common (env, manifest, csv, hist, timer) e workspace degli spike"
  - phase: 01-02
    provides: "harness Compose, session.sh, hw-manifest.sh, runner Linux, file segreti esterno"
provides:
  - "Spike S-01: upload multipart in streaming su Axum 0.8.9 con limite di 25 MiB su tre livelli, validazione prima dello storage, admission control, tre adapter BlobSink (Garage, PG bytea, nullo), GC con protocollo live|deleting + pending"
  - "Misure di memoria, latenza, correttezza, deduplica, storage e GC con soglie scritte prima, in spikes/s01-upload/results/"
  - "Decisione proposta A-02 in ADR-0003: Garage dietro DocumentStore con buffer di multipart limitato e PUT idempotente"
  - "Impatto su porte S-07 e threat model dichiarati in RISULTATI §Esito"
affects: [01-16, 01-17, 01-07]

tech-stack:
  added: []
  patterns:
    - "Soglie e protocollo statistico committati prima di qualsiasi misura; mitigazioni misurate dopo con etichetta propria negli stessi CSV, default invariato"
    - "Build del runner offline con crate vendorizzate dall'host (spikes/target/vendor-s01, ignorata)"

key-files:
  created:
    - spikes/s01-upload/RISULTATI.md
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
    - docs/adr/0003-storage-originali-garage.md
  modified:
    - spikes/s01-upload/src/main.rs
    - spikes/s01-upload/src/lib.rs
    - spikes/s01-upload/results/*.csv, *.json

key-decisions:
  - "Garage dietro DocumentStore con parti da 2 MiB e una in volo: la configurazione di default di object_store non supera la soglia di VmHWM"
  - "PG bytea scartato: canale laterale della deduplica fuori soglia anche con ON CONFLICT DO UPDATE, latenza 10-20 volte Garage sotto concorrenza"
  - "PUT idempotente sulla chiave di contenuto (HEAD di verifica dopo un errore) proposto per i 500 della deduplica concorrente, non misurato"
  - "N = 24 h per il GC degli orfani"

requirements-completed: [PLAT-07]

duration: "2 giorni (2026-10-20 → 2026-10-21), con quattro riprese dopo interruzioni di rete"
completed: 2026-10-21
---

# Fase 1 Piano 04: S-01 + A-02, upload in streaming e storage degli originali · Summary

**Modifica:** Claude (gsd-executor 01-04, ripresa 4) · 2026-10-21 · summary del piano

**Upload multipart in streaming su Axum con memoria costante, limiti su tre livelli, validazione prima dello storage e admission control; Garage proposto come storage degli originali con buffer di multipart limitato, PG `bytea` scartato per il canale laterale della deduplica.**

## Risultato

- **Puro streaming:** heap dhat +1,03 MiB fra 1 e 25 MiB (soglia 2 MiB), supera.
- **Percorso completo:** `VmHWM` costante per PG `bytea` e nullo; Garage con il default di `object_store` cresce di 42,6 / 48,1 MiB (soglia 8 MiB, non supera), con parti da 2 MiB e una in volo di 1,2 / 1,9 MiB (supera). Il criterio 1 è chiuso a condizione di usare il buffer limitato.
- **Ack TTQ:** Garage a 10 MiB, p95 ≤ 500 ms a c = 1 e c = 4 con entrambe le configurazioni.
- **Correttezza:** 20 casi su 20 (confini di 25 MiB, polyglot, zip bomb, DTD, cifrato, JavaScript anche offuscato, interrotto, crash fra PUT e transazione, duplicato).
- **Admission control:** tetti rispettati, 503 con `Retry-After`, 408, risorse al baseline.
- **Canale laterale (protocollo pre-registrato, 3 sessioni):** Garage supera (default e parti da 2 MiB); PG `bytea` non supera, nemmeno mitigato.
- **GC:** 8 interleaving enumerate verdi; N = 24 h.
- **Aperto:** 500 di Garage nella deduplica concorrente (abort di multipart concorrenti sulla stessa chiave, `NoSuchUpload`), ridotti ma non eliminati dalle parti piccole; correzione proposta per l'adapter del prodotto, non misurata.

## Commit

`922cf42`, `bbd3a4d`, `8e6f2ee`, `03b8f1e`, `db62725`, `9341b43`, `15ea345`, `34e73f6`, `cc24b09`, `2bc8a00`, `83720bb`, `690b42b`, `bfd77e9`, `d068c8f` (dettaglio in `git log c9085b7..d068c8f`).

## Deviazioni dal piano

- **[Rule 3] Build del runner offline** con crate vendorizzate dall'host: il DNS di Docker non risolve `index.crates.io`. Nessun file di 01-02 toccato, crate identiche al `Cargo.lock`.
- **[Segnalazione, file chiuso 01-02] Interpolazione di `session.sh`/compose:** le variabili dell'ambiente host vincono su `--env-file`; un `DATABASE_URL` globale dell'host (database cloud di un altro progetto) arrivava al runner. Nessuna connessione stabilita, nessun dato esterno toccato. Aggirato in `bench.sh` (commit `9341b43`); serve una gap-closure di 01-02.
- **[Rule 1] Correzioni:** risoluzione dei nomi del compose con tentativi e arresto (`db62725`); nome del valore `after_put` di clap e arresto ordinato su SIGTERM (`8e6f2ee`).
- **Mitigazioni:** opzioni del server con default invariato (`2bc8a00`); il canale laterale mitigato di Garage misurato su parti da 2 MiB invece che da 5 MiB (`83720bb`), deciso dopo la misura di memoria e prima di raccogliere i tempi; righe mitigate con etichetta propria negli stessi CSV del piano.
- **Misure ereditate** da esecutori interrotti, tracciabili per `git_rev` e tenute (il canale laterale non si ripete: seconda estrazione vietata). Dettaglio in RISULTATI «Anomalie e deviazioni».
- **Test Linux nel runner** (dhat, pdfgen, admission, gc_race) eseguiti al commit `15ea345` e non ripetuti dopo `2bc8a00`, che aggiunge solo opzioni con default invariato; sull'host fmt, clippy `-D warnings` e `cargo test -p s01-upload --locked` sono verdi al commit finale.

## Known Stubs

Nessuno.

## Self-Check: PASSED

File creati presenti (`RISULTATI.md`, ADR-0003, CSV e JSON in `results/`); commit presenti nel log del branch `task/PLAN-010-04`.
