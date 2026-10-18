---
phase: 1
slug: spike-tecnici
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-10-18
---

# Phase 1 — Validation Strategy

**Modifica:** Claude (Anthropic) · 2026-10-18 · creato da gsd-plan-phase dal template GSD, contenuto derivato da `01-RESEARCH.md` §8 (Validation architecture).
**Modifica:** Claude (gsd-planner) · 2026-10-18 · revisione 1 dei piani: mappa per task, percorsi delle evidenze allineati al layout `results/<parte>/` dei piani, onde ricalcolate (≤ 2 piani per onda), segreti fuori dal repo.

> Contratto di validazione per fase di spike: la "verifica" di uno spike non è una suite di test di prodotto ma un file di evidenza machine-readable, rigenerabile con un comando, confrontato con una soglia scritta PRIMA della misura (`RISULTATI*.md` §Soglie, committata prima dei CSV).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` / `cargo check` nel workspace isolato `spikes/` (nessun workspace di prodotto, nessuna CI) + script di evidenza |
| **Config file** | `spikes/Cargo.toml` (piano 01-01) |
| **Segreti** | file esterno al repo `$HOME/.cka-spikes/spikes.env` (piano 01-02), passato con `docker compose --env-file "$CKA_SPIKES_ENV"` |
| **Quick run command** | `cd spikes && cargo check --workspace` |
| **Full suite command** | `cd spikes && cargo test --workspace --release -- --test-threads=1` + controllo evidenze (CSV con header fisso + `hw.json` gemello nella stessa cartella + soglia dichiarata) |
| **Estimated runtime** | quick ~60 s; misure complete: ore (fuori dal ciclo di feedback, per spike) |

---

## Sampling Rate

- **Dopo ogni task che tocca codice spike:** `cd spikes && cargo check --workspace`
- **Dopo ogni onda:** `cargo test --workspace` in `spikes/` + verifica presenza evidenze dell'onda
- **Prima di `verify`:** ogni criterio ROADMAP ha la sua evidenza, il suo `hw.json` e la decisione registrata in ADR/DEPENDENCIES con link
- **Max feedback latency:** 120 s per il quick check

---

## Per-Criterion Evidence Map

| Criterio ROADMAP | Evidenza (path) | Piano |
|---|---|---|
| 1 S-01 memoria costante | `spikes/s01-upload/results/{mem_by_size.csv,dhat_*.json,oha_*.json,correctness.csv}` | 01-04 |
| 4 A-02 storage | `spikes/s01-upload/results/{adapters.csv,storage.csv,gc.csv,dedup_timing.csv}`, ADR-0003 | 01-04 |
| 2 file ostili + isolamento | `spikes/s02-parser/results/isolation/{calibration.csv,limits.json}` (01-07); `{hostile.csv,budget.csv,overhead.csv}` (01-18) | 01-07, 01-18 |
| 2 PDF digitali, tabelle | `spikes/s02-parser/results/extract/{text_cer.csv,bbox_iou.csv,tables.csv,table_pages.csv,docling.csv}` | 01-08 |
| 2 pagina DOCX/MD | `spikes/s02-parser/results/locator/{locator_samples.json,roundtrip.csv,rect_transform.csv}`, ADR-0005 | 01-09 |
| 2 scansioni, TTQ-parse, decisione parser | `spikes/s02-parser/results/ocr/{ocr.csv,ocr_throughput.csv,ttq_parse.csv}`, `spikes/s02-parser/RISULTATI.md`, ADR-0004 | 01-13 |
| 3 embedding | `spikes/s03-pgvector/results/embedding/{models.csv,query_instruction.csv,throughput.csv}`, ADR-0007 | 01-10 |
| 3 pgvector | `spikes/s03-pgvector/results/index/{threshold.csv,latency.csv,recall.csv,hybrid.csv,leak.csv,ingest_under_load.csv,plans/*.json}`, ADR-0006 | 01-14 |
| 4 JEV forma + 429 | `spikes/s04-jev/results/jev/{forms.csv,concurrency.csv,aliases.csv}`, ADR-0008 | 01-11 |
| 4 cancellazione | `spikes/s05-cancel/results/{cancel.csv,deadline.csv,jev_cancel.csv}`, ADR-0010 | 01-05 |
| 4 LLM | `spikes/s06-llm/results/{llm.csv,routing.csv,fallback.csv,injection.csv}`, ADR-0011 | 01-12 |
| 4 reranker + VRAM | `spikes/s04-jev/results/rerank/{rerank.csv,vram.csv}`, ADR-0009 | 01-15 |
| 4 S-07 firme | `spikes/s07-ports/src/lib.rs` (`cargo check -p s07-ports`, `cargo test -p s07-ports`), ADR-0012 | 01-16 |
| 5 hw + registrazione | `hw.json` in ogni cartella di evidenza + link negli ADR 0003-0011 (ciclo di controllo del piano 01-17) | tutti, 01-17 |
| 5 threat model (PLAT-07) | `docs/security/threat-model.md` con T-01..T-28+, requisiti ed evidenze | 01-17 |
| 5 isolamento spike | `test ! -f Cargo.toml`; `grep -r "spikes" --include=Cargo.toml services crates apps` vuoto; nessun workflow con `spikes/` | 01-17 |

---

## Per-Task Verification Map

Requisito di fase: PLAT-07 per tutti i task. «Threat» = ID del registro `<threat_model>` del piano.

| Task ID | Piano | Onda | Tipo | Threat | Comando automatico (sintesi; testo completo nel piano) |
|---|---|---|---|---|---|
| 01-01-T1 | 01-01 | 1 | checkpoint legittimità crate | T-01-01-SC | conferma umana |
| 01-01-T2 | 01-01 | 1 | auto | T-01-01-01 | `cd spikes && cargo check --workspace && test ! -f ../Cargo.toml` |
| 01-01-T3 | 01-01 | 1 | auto tdd | T-01-01-02, -03 | `cd spikes && cargo test -p common` |
| 01-02-T1 | 01-02 | 1 | auto | T-01-02-01..04 | `docker compose --env-file .env.example ... config -q` + scanner su elenco esplicito |
| 01-02-T2 | 01-02 | 1 | auto (gate GPU) | T-01-02-03 | `tail -1 spikes/results/gpu-check.txt \| grep -E '^VERDETTO: (VERDE\|ROSSO)'` |
| 01-02-T3 | 01-02 | 1 | checkpoint chiavi + gate | T-01-02-05 | `grep -cE '^(TYPESAFE_API_KEY\|ANTHROPIC_API_KEY)=.+' "$HOME/.cka-spikes/spikes.env"` |
| 01-03-T1 | 01-03 | 2 | auto | T-01-03-01, -04 | `bash spikes/data/corpus/fetch.sh` + controllo manifest con `node -e` |
| 01-03-T2 | 01-03 | 2 | auto | T-01-03-02, -03 | `bash spikes/data/corpus/synthetic/build.sh` + controllo manifest/ACL |
| 01-04-T1 | 01-04 | 2 | auto | T-01-04-01..05 | `cargo check -p s01-upload --all-targets` + Soglie presenti + scanner |
| 01-04-T2 | 01-04 | 2 | auto (misura) | T-01-04-01, -03, -06 | header CSV esatti + `hw.json` valido |
| 01-04-T3 | 01-04 | 2 | auto | — | link ADR-0003 → RISULTATI e hw.json; T-ID in Contesto |
| 01-05-T1 | 01-05 | 3 | auto | T-01-05-01, -05 | `cargo check -p s05-cancel` + Soglie + scanner |
| 01-05-T2 | 01-05 | 3 | auto (misura) | T-01-05-02..04 | header CSV + `hw.json` + link ADR-0010 |
| 01-05-T3 | 01-05 | 3 | checkpoint console JEV | T-01-05-03 | `grep -q "Addebito JEV dopo il drop"` senza «in lettura» |
| 01-06-T1 | 01-06 | 3 | auto (agente separato) | T-01-06-01 | controllo `node -e` di `queries.json` |
| 01-06-T2 | 01-06 | 3 | auto (GPT double-check) | T-01-06-02, -03 | guardrail.gpt valorizzato, TO-DO sui dissensi |
| 01-06-T3 | 01-06 | 3 | checkpoint ratifica | T-01-06-02 | ≥ 60 query con tre assi verdi |
| 01-07-T1 | 01-07 | 4 | auto | T-01-07-04, -06 | build worker + `tesseract --version` |
| 01-07-T2 | 01-07 | 4 | auto | T-01-07-01..03, -05 | `cargo check -p s02-parser --all-targets` + grep `pre_exec`, `killpg`, `take(` |
| 01-07-T3 | 01-07 | 4 | auto | T-01-07-01, -02 | prescan e figlio PDFium/DOCX |
| 01-07-T4 | 01-07 | 4 | auto (misura) | T-01-07-01, -02 | `calibration.csv` + `limits.json` + `hw.json` |
| 01-18-T1/T2 | 01-18 | 5 | auto (misura) | T-01-18-01..06 | manifest ostili prima dei CSV; header di `hostile.csv`, `budget.csv`, `overhead.csv` |
| 01-11-T1 | 01-11 | 4 | auto | T-01-11-01, -02, -05 | `cargo check -p s04-jev --all-targets` + `assert_visible` + scanner |
| 01-11-T2 | 01-11 | 4 | auto (misura) | T-01-11-01, -03, -04 | header di `results/jev/*.csv`; `unauthorized_sent` = 0 |
| 01-08-T1 | 01-08 | 5 | auto | T-01-08-03, -04 | `cargo check -p s02-parser` + `data/s02/fetch.sh` + scanner |
| 01-08-T2 | 01-08 | 5 | auto (misura) | T-01-08-01, -02 | header di `results/extract/*.csv` + `hw.json` |
| 01-12-T1 | 01-12 | 5 | auto | T-01-12-01..05, -07 | `cargo check -p s06-llm` + schema + selection + scanner |
| 01-12-T2 | 01-12 | 5 | auto (misura) | T-01-12-01, -04, -06 | header CSV; `unauthorized_in_prompt` = 0; nessun `validated` nel fallback |
| 01-09-T1 | 01-09 | 6 | auto | T-01-09-01, -03 | `cargo test -p s02-parser --bin locator` + scanner |
| 01-09-T2 | 01-09 | 6 | auto (misura) | T-01-09-02 | header di `results/locator/*` + link ADR-0005 + T-ID in Contesto |
| 01-10-T1 | 01-10 | 6 | auto | T-01-10-01, -02, -05 | `cargo check -p s03-pgvector --all-targets` + grep `doc_acl` + scanner |
| 01-10-T2 | 01-10 | 6 | auto (misura) | T-01-10-01, -03, -04 | header di `results/embedding/*.csv`; `leaked_chunks` = 0 |
| 01-13-T1 | 01-13 | 7 | checkpoint legittimità pip | T-01-13-SC | conferma umana |
| 01-13-T2 | 01-13 | 7 | auto | T-01-13-01 | `cargo check -p s02-parser --all-targets` + pin pip + scanner |
| 01-13-T3 | 01-13 | 7 | auto (misura) | T-01-13-01..03 | header di `results/ocr/*.csv`; RISULTATI.md di S-02; T-ID in ADR-0004 |
| 01-14-T1 | 01-14 | 8 | auto | T-01-14-01, -03 | `cargo check -p s03-pgvector --all-targets` + CTE `allowed` ≥ 2 |
| 01-14-T2 | 01-14 | 8 | auto (misura) | T-01-14-01, -02, -04 | header di `results/index/*.csv`; `denied_in_topk` = 0 |
| 01-15-T1 | 01-15 | 9 | auto | T-01-15-01, -02, -04 | `cargo check -p s04-jev --all-targets` + `assert_visible` + scanner |
| 01-15-T2 | 01-15 | 9 | auto (misura) | T-01-15-01, -03 | header di `results/rerank/*.csv`; `unauthorized_sent` = 0 |
| 01-16-T1 | 01-16 | 10 | auto | T-01-16-01 | RISULTATI nomina le 11 porte |
| 01-16-T2 | 01-16 | 10 | auto | T-01-16-01..04 | `cargo check --workspace && cargo test -p s07-ports`; nessun crate proibito |
| 01-17-T1 | 01-17 | 11 | auto | T-01-17-03 | T-01..T-28 presenti + scanner |
| 01-17-T2 | 01-17 | 11 | checkpoint ratifica ADR | T-01-17-01 | conferma umana |
| 01-17-T3 | 01-17 | 11 | auto | T-01-17-01, -02, -04 | ciclo dei link degli ADR + controlli di isolamento + scanner |

---

## Wave 0 Requirements

- [ ] `spikes/Cargo.toml` workspace con membri espliciti e `spikes/Cargo.lock` generato (01-01)
- [ ] `spikes/common` (hw manifest, CSV writer a header fisso, istogrammi) (01-01)
- [ ] `spikes/tools/hw-manifest.sh` e `spikes/tools/gpu-check.ps1` (01-02)
- [ ] `spikes/compose.yaml`, `spikes/.env.example`, `spikes/.gitignore`, `spikes/.dockerignore`, file segreti esterno (01-02)
- [ ] Gate GPU (Docker Desktop/WSL2 passthrough, poi avvio TEI) superato prima di ogni misura GPU (01-02)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Legittimità dei pacchetti (crate, pip) | PLAT-07 / supply chain | nessuna sezione «Package Legitimacy Audit» in RESEARCH | checkpoint 01-01-T1 e 01-13-T1 |
| Oracoli indipendenti (query etichettate, oracolo IT del parser) | PLAT-07 / eval-oracle-check | provenienza indipendente dalla pipeline, triplo guardrail | checkpoint 01-06-T3 su estratti e derivazioni |
| Addebito JEV dopo il drop | D-22 | nessuna API di billing nota | checkpoint 01-05-T3 in console |
| Scelta finale di parser, LLM, embedding (ADR) | criteri 2-4 | decisione con giudizio sulle misure | checkpoint 01-17-T2 |

---

## Validation Sign-Off

- [x] Ogni task ha verifica automatica o è un checkpoint umano dichiarato
- [x] Mai 3 task consecutivi senza verifica automatica
- [x] Wave 0 copre tutti i riferimenti mancanti
- [x] Nessun flag watch-mode
- [x] Feedback latency < 120 s (quick check)
- [x] `nyquist_compliant: true` in frontmatter

**Approval:** pending

**Modifica:** Claude (Anthropic) · 2026-10-19 · righe di 01-07 riallineate alla divisione 01-07/01-18
