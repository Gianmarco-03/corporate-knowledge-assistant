# 01 — WIRING

Redazione: Claude (Anthropic) · 2026-10-18 · wiring-search + wiring-design fase 1, prima dei piani; riallineato ai 17 PLAN dopo la pianificazione (stessa data, vedi registro variazioni in coda).
Input letti (main `b5ef8c2`, prima stesura): `01-CONTEXT.md` (D-01..D-22), `01-RESEARCH.md` (non versionato, §0-§11; §1 layout harness, §2 compose, §8 evidenze, §9 S-07), goal fasi 2-15 (`roadmap get-phase N --pick goal`), `scripts/proprieta-file.whitelist.json`, `scripts/gate-proprieta-file.cjs` (parsing WIRING-MAP), `skills/onde-slice/SKILL.md` §2.
Input riallineamento (main `63da692`): `01-01-PLAN.md` … `01-17-PLAN.md` (frontmatter `files_modified`/`wave`/`depends_on`, `<interfaces>`, azioni). **PLAN = fonte**: questo file descrive i seam reali dei piani scritti.
WIRING-MAP: **assente** all'avvio → creata da wiring-design (template, zero righe seam). Riallineamento: nessun seam inter-fase nuovo → WIRING-MAP invariata.
Stato codice: niente `Cargo.toml` radice, niente `apps/web/package.json`, niente `spikes/`. Tutti i seam = create-once dall'harness (piani 01-01 + 01-02 = «wave 0» del wiring, `wave: 1` nel frontmatter), nessuno «già fatto».

### Slice reali (dai PLAN)

| piano | slice | contenuto | file propri |
|---|---|---|---|
| 01-01 | H-rust (harness) | workspace `spikes/`, 8 crate vuoti, deps pinnate, lockfile, `common` | `spikes/Cargo.toml`, `spikes/Cargo.lock`, `spikes/common/**`, `spikes/<crate>/Cargo.toml` + `src/main.rs`/`src/lib.rs` iniziali |
| 01-02 | H-infra (harness) | compose, env, gitignore, dockerignore, runner, tools, gate GPU, cartelle dati, file segreti esterno | `spikes/compose.yaml`, `.env.example`, `.gitignore`, `.dockerignore`, `docker/runner.Dockerfile`, `tools/**`, `results/gpu-check.txt`, `data/*/.gitkeep`; fuori repo `$HOME/.cka-spikes/spikes.env` |
| 01-03 | SC corpus | relazioni pubbliche + sintetici classificati, ACL, fetch | `spikes/data/corpus/**` tranne `oracles/` |
| 01-06 | SO oracoli | query etichettate + oracoli parser IT, triplo guardrail | `spikes/data/corpus/oracles/**` |
| 01-04 | S01 upload/storage | S-01 + A-02 | `spikes/s01-upload/**`, `spikes/docker/garage.toml`, ADR 0003 |
| 01-05 | S05 cancellazione | deadline/cancel h1/h2 + JEV | `spikes/s05-cancel/**`, ADR 0010 |
| 01-07 | S02a isolamento | lanciatore isolato, prescan, ostili | `spikes/s02-parser/src/{lib,protocol,prescan,isolate}.rs`, `src/bin/parse-{parent,child}.rs`, `hostile-gen/`, `RISULTATI-isolamento.md`, `results/isolation/`, `spikes/docker/worker.Dockerfile`, `spikes/data/hostile/{manifest.json,.security-scan-fixture}` |
| 01-08 | S02b estrazione | PDFium testo/bbox, tabelle, Docling GPU/CPU | `s02-parser/src/bin/{extract,docling-client}/`, `RISULTATI-estrazione.md`, `results/extract/`, `spikes/data/s02/**` |
| 01-09 | S02c locator | locator DOCX/MD/PDF rect | `s02-parser/src/bin/locator/`, `locator-gen/`, `RISULTATI-locator.md`, `results/locator/`, ADR 0005 |
| 01-13 | S02d OCR + chiusura S-02 | Tesseract vs PP-OCR, TTQ-parse | `s02-parser/src/bin/ocr/`, `scan-gen/`, `RISULTATI-ocr.md`, `RISULTATI.md` (indice S-02), `results/ocr/`, `spikes/docker/paddle.Dockerfile`, ADR 0004 |
| 01-10 | S03a embedding | 3 modelli, TEI GPU, chunk/candidati | `s03-pgvector/src/{main,lib,chunk,tei,db,metrics}.rs`, `sql/schema.sql`, `RISULTATI-embedding.md`, `results/embedding/`, ADR 0007 |
| 01-14 | S03b indice + chiusura S-03 | esatta vs HNSW, ibrida RRF, leak, ingest sotto carico | `s03-pgvector/src/bin/index-bench/`, `sql/index.sql`, `RISULTATI-indice.md`, `RISULTATI.md` (indice S-03), `results/index/`, ADR 0006 |
| 01-11 | S04a JEV | forma `rank`, concorrenza, pin | `s04-jev/src/{main,lib,jev,candidates,metrics}.rs`, `questions/`, `RISULTATI-jev.md`, `results/jev/`, ADR 0008 |
| 01-15 | S04b rerank + chiusura S-04 | reranker baseline, budget VRAM | `s04-jev/src/bin/rerank/`, `vram.sh`, `RISULTATI-rerank.md`, `RISULTATI.md` (indice S-04), `results/rerank/`, ADR 0009 |
| 01-12 | S06 LLM | Haiku/Sonnet, routing JEV, structured output | `spikes/s06-llm/**`, `spikes/data/s06/selection.json`, ADR 0011 |
| 01-16 | S07 porte | firme candidate tutte le porte | `spikes/s07-ports/{Cargo.toml,src/lib.rs,RISULTATI.md}`, `spikes/Cargo.lock` (regenerate), ADR 0012 |
| 01-17 | SD consolidamento | DEPENDENCIES, threat model, ADR AV/CDR, ratifica ADR | `DEPENDENCIES.md`, `docs/security/threat-model.md`, ADR 0013; riga `Stato` + riga `**Modifica:**` di ADR 0003-0012 |

Correzioni a RESEARCH (motivo, prima stesura, ancora valide):
- `spikes/data/manifest.json` unico (RESEARCH §1, §8.1) → **spezzato** in `data/corpus/manifest.json` (SC) + `data/<sNN>/manifest.json` per slice: file unico toccato da più slice = serializzazione inutile.
- compose RESEARCH §2 senza `docling-cpu` né `paddle` ma §4 li misura → aggiunti a W5.
- `gpu-check.ps1` «verificare PER PRIMO» (D-13) → script + esecuzione nel piano 01-02 = gate di wave 0, non task di slice.
- Codice JEV usato da S04, S05, S06: **nessun client condiviso** in `common`; ogni crate ha il proprio `src/jev.rs` (01-05, 01-11, 01-12).

## §1 — Inventario intra-fase

| id | punto (file/simbolo) | kind | contributo | piani | rischio |
|---|---|---|---|---|---|
| W1 | `spikes/Cargo.toml` `[workspace] members` | workspace-member | create-once | 01-01 (scrive); tutti consumano | ALTO se ogni slice aggiunge membro → BASSO pre-registrato |
| W2 | `spikes/Cargo.toml` `[workspace.dependencies]` | workspace-member | shared-artifact (append) | 01-01 (scrive tutto, incl. async-trait, quick-xml, unicode-normalization, image, …); nessun altro piano lo tocca | BASSO (nessun append pianificato) |
| W3 | `spikes/Cargo.lock` | workspace-member | regenerate | 01-01 (genera), 01-16 (rigenera: attiva deps nel crate s07) | MEDIO (2 contributori, onde 1 e 10) |
| W4 | `spikes/<crate>/Cargo.toml` + `src/main.rs`/`src/lib.rs` iniziali | workspace-member | create-once → innesto | 01-01 crea; `main.rs` riempito da 01-04, 01-05, 01-10, 01-11, 01-12; `s07-ports/{Cargo.toml,lib.rs}` da 01-16 | BASSO (serie via `depends_on`) |
| W5 | `spikes/compose.yaml` servizi + profili + volumi + reti | compose-service | create-once | 01-02 (unico scrittore); consumato da 01-03..01-15 | ALTO senza chiusura → BASSO chiuso |
| W6 | `spikes/.env.example` + file segreti esterno `$HOME/.cka-spikes/spikes.env` (`CKA_SPIKES_ENV`) | config-key | create-once (01-02); append ammesso ma nessuno pianificato | 01-02; letto da 01-03..01-15 | BASSO |
| W7 | `spikes/.gitignore`, `spikes/.dockerignore` | config-key (assimilato) | create-once | 01-02 | BASSO chiusi |
| W8 | `spikes/common` (env, manifest hw, CSV header fisso, hist, timer) | workspace-member (crate condiviso) | create-once | 01-01; consumato da 01-04..01-15 | ALTO se editato → BASSO chiuso |
| W9 | `spikes/tools/hw-manifest.sh` → `results[/<parte>]/hw.json` | create-once (tool) | create-once | 01-02; ogni piano di misura | BASSO (output per piano) |
| W10 | `spikes/tools/gpu-check.ps1` + `spikes/results/gpu-check.txt` | create-once (tool, gate) | create-once | 01-02; letto da 01-08, 01-10, 01-13, 01-14, 01-15 | MEDIO (gate condiviso) |
| W11 | `spikes/data/` (`corpus/`, `corpus/oracles/`, `s02/`, `s03/`, `s06/`, `hostile/`) | registry (manifest dati) | shared-artifact → spezzato per cartella | 01-02 (`.gitkeep`), 01-03, 01-06, 01-07, 01-08, 01-12 | BASSO spezzato |
| W12 | `spikes/docker/` (`runner.Dockerfile`, `garage.toml`, `worker.Dockerfile`, `paddle.Dockerfile`) | compose-service (file build/config) | create-once per file | 01-02, 01-04, 01-07, 01-13; rebuild worker da 01-08, 01-09, 01-13 | BASSO (file per piano) |
| W13 | `docs/adr/00NN-*.md` 0003-0013 | migration-sequence (assimilato: sequenza ADR) | append (file nuovi, numero riservato) + ratifica | 01-04, 01-05, 01-09, 01-10, 01-11, 01-12, 01-13, 01-14, 01-15, 01-16, 01-17 | ALTO senza prenotazione → BASSO prenotato |
| W14 | `DEPENDENCIES.md` | shared-artifact | proprietario unico | 01-17 | BASSO |
| W15 | `docs/security/threat-model.md` | shared-artifact | proprietario unico | 01-17 | BASSO |
| W16 | `spikes/s07-ports/src/lib.rs` (firme candidate) | trait-port | create-once (proprietario 01-16) | variazioni dichiarate in RISULTATI §Esito da tutti gli spike | BASSO |
| W17 | `s02-parser` lib: `protocol::ChildOutcome`, `isolate::run_child`, `prescan::check_{pdf,docx}`, bin `parse-parent run --child <bin>` | registry (assimilato: lancio figlio per nome bin) | create-once (01-07) + innesto bin | 01-07 crea; 01-08, 01-09, 01-13 aggiungono bin figli | ALTO se editato → BASSO chiuso |
| W18 | crate spike spezzati in più piani (`s02-parser`, `s03-pgvector`, `s04-jev`): `src/lib.rs`, `src/bin/<nome>/`, `RISULTATI-<parte>.md`, `results/<parte>/`, `RISULTATI.md` indice | workspace-member (assimilato: innesto per-bin) | create-once (primo piano) + file nuovi per piano | s02: 07→08→09→13; s03: 10→14; s04: 11→15 | MEDIO (stesso crate, piani in serie) |
| W19 | Postgres `spikes` condiviso (tabelle S01 vs S03) | compose-service (runtime) | create-once (01-02) | 01-04 (`s01-upload/sql/schema.sql`), 01-10/01-14 (`s03-pgvector/sql/{schema,index}.sql`) | BASSO (file SQL per crate, onde diverse) |
| W20 | finestra di misura (CPU runner, GPU RTX 3050, rate JEV/Anthropic) | risorsa condivisa (non file) | serializzazione | 01-04, 01-05, 01-07 (CPU); 01-08, 01-10, 01-13, 01-14, 01-15 (GPU); 01-05, 01-11, 01-12 (JEV) | ALTO se sovrapposte (misure falsate) |

Riepilogo dopo il design: nessun ALTO residuo; MEDIO: W3, W10, W18; resto BASSO. W20 non è un file: gestito con onde (§3) + controllo `docker ps` (§4).
Serie forzate (dai `depends_on`): harness {01-01, 01-02} → tutto; 01-02 → 01-03 → 01-06 → {01-08, 01-10, 01-11, 01-12, 01-13}; s02 07→08→09→13; s03 10→14; s04 11→15; {01-04, 01-05, 01-12, 01-13, 01-14, 01-15} → 01-16 → 01-17.

## §2 — Seam inter-fase

| rif | punto | kind | azione per 01 | fase futura o d'origine | fase proprietaria del file | motivo |
|---|---|---|---|---|---|---|
| — | nessuno | — | — | — | — | WIRING-MAP senza righe → nessuna promessa da riempire. Codice fase 1 = usa-e-getta in `spikes/`, mai importato dal prodotto (ROADMAP fase 1 criterio 5, 01-CONTEXT §code_context): nessun goal 2-15 estende codice costruito da 01 → regola (a)+(b) non soddisfatta per nessun candidato. Riallineamento ai PLAN: confermato, nessun piano crea file in `services/`, `crates/`, `apps/`, `infra/`, né `Cargo.toml` radice |

Goal futuri letti (solo goal) → esito per 01:
- 02 Fondazione «…ogni fase successiva trova già pronti i propri punti di innesto vuoti…» → nessun seam per 01: gli innesti li crea 02 nel prodotto. Consuma da 01 solo documenti (S-07, ADR, DEPENDENCIES) via CONTRATTO.
- 03 Provider ingestion/indice, 04 Provider decisione/risposta/trasversali → nessun seam per 01: le porte nascono in 02 dal documento S-07 (`spikes/s07-ports/src/lib.rs` + ADR-0012, trascritte, mai importate), non da codice spike.
- 05..15 → nessun seam per 01 (area non costruita da 01 come codice di prodotto).

Uscite inter-fase di 01 = **solo documenti** → vanno nel **CONTRATTO** (`01-CONTRATTO.md`), non nella WIRING-MAP:
- ADR 0003-0013 (W13), `DEPENDENCIES.md` aggiornato, `docs/security/threat-model.md` compilato (PLAT-07), firme candidate S-07 (`spikes/s07-ports/src/lib.rs` + ADR-0012 con sezione «Vincoli verso la Fondazione»), manifest hw + RISULTATI per spike/parte.
- Vincolo verso 02 (da CONTRATTO e ADR-0012, non seam): Cargo.toml radice con `[workspace] exclude = ["spikes"]`; CI senza `spikes/**`; nessun path dep verso `spikes/` (`grep -r "spikes" --include=Cargo.toml services crates apps` vuoto).

Segnali (prima stesura, ancora aperti):
- **S-a (perimetro file chiusi).** `DEPENDENCIES.md` e `docs/security/threat-model.md` nel perimetro di 01 e **non** in whitelist `proprieta-file`; fasi future li aggiorneranno (03/04 DEPENDENCIES; 10/14 threat model). Serve decisione utente: whitelist «riga di stato per fase» / «sezione per fase». Non toccato qui.
- **S-b (numerazione ADR fra fasi).** Nessuna convenzione di progetto per numeri ADR fra fasi parallele. Dentro 01 risolto con prenotazione (§5 W13). Proposta: «primo numero libero su `main` al `plan`, prenotato nel CONTRATTO», da ratificare. Non promesso in WIRING-MAP.
- **S-c (nuovo, riallineamento).** 01-17 modifica ADR 0003-0012 di altre slice (sola riga `Stato` + riga `**Modifica:**` di ratifica). Intra-fase → ammesso in §4. Stesso schema servirà fra fasi (ADR di fase N ratificato/superato da fase M): da coprire insieme a S-b.

## §3 — Convenzioni e plug-in contract   (wiring-design)

Principio: harness (01-01 + 01-02) crea workspace, crate vuoti con deps dichiarate e attive, lockfile, compose completo (runner incluso), env, gitignore/dockerignore, common, tools, cartelle dati, file segreti esterno. Ogni slice riempie solo propri file: cartella crate (o propri bin/moduli dentro crate spezzato), propri file dati/docker, proprio ADR a numero riservato. Niente auto-discovery magica: `members` esplicito; bin figli s02 scelti per nome con argomento esplicito `--child <bin>` (auto-discovery Cargo di `src/bin/` = convenzione standard del build, non registrazione runtime).

| id | meccanismo | plug-in contract (frase per la slice) |
|---|---|---|
| W1 | innesto pre-registrato: `members` esplicito 8 crate (01-01), ogni crate vuoto compilabile | «riempi solo i tuoi file del crate; NON editare `members`» |
| W2 | pre-dichiarazione in 01-01 di tutte le deps note (RESEARCH §0 + serde, csv, hex, uuid, rand, flate2, zip, image, async-trait, unicode-normalization, quick-xml), pin `=x.y.z`; coda append-only | «usa `<dep>.workspace = true`; dipendenza nuova = riga in **coda** a `[workspace.dependencies]` + `spikes/Cargo.toml` e `spikes/Cargo.lock` in `files_modified` (serializza). Nessun piano attuale ne ha bisogno» |
| W3 | regenerate: 01-01 genera (`cargo generate-lockfile` + `cargo check --workspace` in `spikes/`); 01-16 rigenera dopo aver attivato deps già dichiarate nel crate s07 | «in conflitto: `spikes/Cargo.lock` di `main` + `cargo check --workspace` in `spikes/`; mai fusione a mano; un solo contributore per onda» |
| W4 | crate creati da 01-01 con `[dependencies]` già attive (mappa §5) → slice non tocca `Cargo.lock`; bin aggiuntivi in `src/bin/` (auto-discovery Cargo), mai `[[bin]]` altrui | «il tuo `src/main.rs` iniziale è tuo da riempire; `Cargo.toml` del crate in sola lettura salvo 01-16 (`s07-ports`, solo deps già in workspace); dep fuori mappa → regola W2/W3» |
| W5 | create-once completo in 01-02 (9 servizi incl. `runner`; volumi incl. `cargo-target`, `cargo-registry`, `work-tmp`); YAML chiuso; parametri `${VAR:-default}`; segreti `${VAR}` da `--env-file "$CKA_SPIKES_ENV"`; niente `env_file:`. Scartato `include:` per frammenti: volumi/reti condivisi | «NON editare `spikes/compose.yaml`; varia parametri via env. Modifica inevitabile = deviazione (b) → rimbalzo a `plan`» |
| W6 | 01-02 scrive tutte le chiavi in sezioni `# common`, `# s01`…`# s06`, segreti vuoti; valori reali solo in `$HOME/.cka-spikes/spikes.env` (fuori repo e worktree), letto via `CKA_SPIKES_ENV` | «chiave nuova = coda alla tua sezione `# sNN`, valore vuoto o non segreto, `.env.example` in `files_modified`; segreti mai in repo/log/SUMMARY; compose sempre con `--env-file "$CKA_SPIKES_ENV"`, script host `set -a; . "$CKA_SPIKES_ENV"; set +a`» |
| W7 | create-once completo in 01-02 | «NON editare `spikes/.gitignore` né `spikes/.dockerignore`; grezzi sotto `results/raw/` o `data/**/raw/` (già ignorati)» |
| W8 | crate `common` create-once in 01-01, API fissa (§5), chiuso | «consuma `common`; helper mancante = modulo locale al tuo crate (duplicazione accettata); NON editare `spikes/common/**`» |
| W9 | create-once 01-02; output nella cartella risultati del piano | «lancia `tools/hw-manifest.sh <tua-cartella-results>/hw.json [--image …] [--tool …]` prima di ogni sessione; CSV senza `hw.json` gemello = rifiutato da `common::csv::Writer`» |
| W10 | create-once 01-02 + esecuzione = gate di wave 0 (`spikes/results/gpu-check.txt`, ultima riga `VERDETTO:`), decisione utente nel checkpoint di 01-02 | «misure GPU solo con `VERDETTO: VERDE`; rosso → ripiego CPU annotato in Anomalie, o stop secondo decisione utente (D-13)» |
| W11 | spezzato per cartella, un proprietario ciascuna: `corpus/` (01-03, escluso `oracles/`), `corpus/oracles/` (01-06), `hostile/` (01-07), `s02/` (01-08), `s06/` (01-12); `s03/` creata vuota, nessun proprietario (query spostate in `corpus/oracles/queries.json`); manifest stesso schema; grezzi in `raw/` ricreati da `fetch.sh` idempotente | «leggi `data/corpus/**` (incl. `oracles/`, solo voci con tre assi verdi), scrivi solo la tua cartella dati; oracoli indipendenti dalla pipeline (eval-oracle-check)» |
| W12 | un file per piano; compose li referenzia; immagine worker compila tutto il crate `s02-parser` e copia tutti i bin → i piani s02 successivi ricostruiscono senza editare | «01-02 possiede `runner.Dockerfile`; 01-04 `garage.toml`; 01-07 `worker.Dockerfile`; 01-13 `paddle.Dockerfile`. Altri: rebuild sì, edit no» |
| W13 | prenotazione numeri ADR per piano (§5), file da `docs/adr/0000-template.md`, Stato `Proposed`; numeri immutabili; numero non emesso → «non emesso» nel CONTRATTO, mai riassegnato; ratifica centralizzata in 01-17 | «scrivi solo `docs/adr/<tuo-numero>-<slug>.md` con link a `RISULTATI*.md` + `hw.json`; eccezione unica: 01-17 tocca negli ADR 0003-0012 solo la riga `Stato` e aggiunge una riga `**Modifica:**`» |
| W14 | proprietario unico 01-17 (ultima onda); le slice scrivono l'esito in RISULTATI §Esito «Destinazione: riga DEPENDENCIES …» | «NON editare `DEPENDENCIES.md`» |
| W15 | proprietario unico 01-17; le slice dichiarano in RISULTATI §Esito «Impatto threat model: T-.. + file evidenza» | «NON editare `docs/security/threat-model.md`» |
| W16 | proprietario unico 01-16 (dopo tutti gli spike); le slice dichiarano «Impatto su porte (S-07)» in RISULTATI | «NON creare trait-porta fuori dal tuo crate; variazioni di firma solo come testo in RISULTATI» |
| W17 | contratto pubblicato da 01-07 (`<interfaces>` del piano): `ChildOutcome`/`Kind` JSON su stdout, `run_child(bin, args, &Limits) -> RunReport`, `check_pdf`/`check_docx`, `parse-parent run --child <nome-bin> --input <f> [--out <d>]`; chiuso dopo 01-07 | «aggiungi il tuo figlio come `src/bin/<nome>/` che stampa una riga `ChildOutcome` e chiama `prescan` prima di PDFium; lancialo con `parse-parent run --child <nome>`; NON editare `src/lib.rs`, `protocol.rs`, `prescan.rs`, `isolate.rs`, `parse-parent.rs`, `parse-child.rs`, `worker.Dockerfile`» |
| W18 | crate spezzato: primo piano possiede `src/lib.rs` + moduli (s02: 01-07; s03: 01-10; s04: 01-11); piani successivi solo `src/bin/<nome>/` + propri file; risultati in `results/<parte>/` con `hw.json` proprio, `RISULTATI-<parte>.md` per piano; `RISULTATI.md` indice + Esito complessivo scritto dall'ultimo piano dello spike (01-13, 01-14, 01-15), che scrive anche l'ADR finale | «scrivi solo `src/bin/<tuo-bin>/`, `RISULTATI-<tua-parte>.md`, `results/<tua-parte>/`; lib del crate in sola lettura; `RISULTATI.md` indice solo se sei l'ultimo piano dello spike» |
| W19 | Postgres unico (`spikes` DB) creato da 01-02; schema SQL nel crate dello spike; S01 e S03 in onde diverse | «DDL solo nel tuo `sql/*.sql`; nessuna tabella di un altro spike» |
| W20 | onde: max 2 piani, mai due piani della stessa classe di risorsa misurata nella stessa onda (CPU-runner, GPU, rate JEV/Anthropic); in più finestra di misura `docker ps` a macchina quieta | «prima di misurare: `docker ps` senza container di altri piani, altrimenti attendi» |

Vincoli di pianificazione per `onde-slice` (applicati nei frontmatter, 11 onde):
- `spikes/Cargo.toml`: solo 01-01. `spikes/Cargo.lock`: 01-01 (onda 1), 01-16 (onda 10) → un contributore per onda.
- `spikes/compose.yaml`, `.env.example`, `.gitignore`, `.dockerignore`: solo 01-02.
- Un piano per numero ADR; `DEPENDENCIES.md`/`threat-model.md` solo 01-17; ADR altrui solo 01-17 (riga Stato + Modifica).
- Nessuna superficie API, nessuna migrazione sqlx di prodotto, nessun client TS: regole TS-client/migrazioni non applicabili.
- Classi di risorsa (W20): CPU-runner {01-04, 01-05, 01-07}, GPU {01-08, 01-10, 01-13, 01-14, 01-15}, JEV/LLM {01-05, 01-11, 01-12}: due piani della stessa classe mai nella stessa onda.
- Tetto 2 esecutori concorrenti (CLAUDE.md): onde con ≤ 2 piani.
- Onde risultanti: 1 {01-01, 01-02} · 2 {01-03, 01-04} · 3 {01-05, 01-06} · 4 {01-07, 01-11} · 5 {01-08, 01-12} · 6 {01-09, 01-10} · 7 {01-13} · 8 {01-14} · 9 {01-15} · 10 {01-16} · 11 {01-17}.

## §4 — Vincoli per gli esecutori        (wiring-design)

- Lavori SOLO dentro i file del tuo `files_modified`, che ricadono in: la tua cartella crate (o, se il crate è spezzato, i tuoi `src/bin/<nome>/`, `RISULTATI-<parte>.md`, `results/<parte>/`); la tua cartella dati (§5 W11); i tuoi file `spikes/docker/*` (§5 W12); il tuo ADR a numero riservato (§5 W13). SC (01-03): solo `spikes/data/corpus/**` tranne `oracles/`. SO (01-06): solo `spikes/data/corpus/oracles/**`. SD (01-17): solo `DEPENDENCIES.md`, `docs/security/threat-model.md`, ADR 0013, e negli ADR 0003-0012 solo riga `Stato` + una riga `**Modifica:**`.
- NON creare `Cargo.toml`, `Cargo.lock`, `.env.example` o workflow CI alla **radice** del repo. NON toccare `services/`, `crates/`, `apps/`, `infra/`, `.github/`, `.gitignore` radice.
- NON editare `spikes/Cargo.toml`, `spikes/common/**`, `spikes/.gitignore`, `spikes/.dockerignore`, `spikes/tools/**`, `spikes/compose.yaml`, `spikes/docker/runner.Dockerfile`, `spikes/results/gpu-check.txt`: harness chiuso dopo 01-01/01-02.
- Crate: `Cargo.toml` del tuo crate in sola lettura (deps già attive, mappa §5). Eccezione: 01-16 aggiunge a `s07-ports/Cargo.toml` solo deps già in `[workspace.dependencies]` e rigenera `spikes/Cargo.lock`. Bin nuovi solo in `src/bin/` (auto-discovery Cargo), mai `[[bin]]`.
- Dipendenza fuori mappa = riga in **coda** a `[workspace.dependencies]` (pin `=x.y.z`) + `cargo check --workspace` in `spikes/`; `spikes/Cargo.toml` e `spikes/Cargo.lock` in `files_modified`, altrimenti deviazione (b) → rimbalzo a `plan`.
- `spikes/Cargo.lock`: mai fuso a mano. Conflitto → versione di `main` + `cargo check --workspace` in `spikes/`.
- Crate spezzati (`s02-parser`, `s03-pgvector`, `s04-jev`): `src/lib.rs` e moduli sono del primo piano (01-07, 01-10, 01-11); dopo, sola lettura. `RISULTATI.md` indice + Esito complessivo + ADR finale: solo l'ultimo piano dello spike (01-13, 01-14, 01-15).
- `s02-parser`: figlio isolato = tuo `src/bin/<nome>/`, stampa una riga `ChildOutcome`, chiama `prescan::check_pdf`/`check_docx` prima di parsare, gira solo via `parse-parent run --child <nome>` nel worker. NON editare `lib.rs`, `protocol.rs`, `prescan.rs`, `isolate.rs`, `parse-parent.rs`, `parse-child.rs`, `worker.Dockerfile`; ricostruire l'immagine worker è ammesso.
- `spikes/compose.yaml`: NON editare. Parametri via env `${VAR:-default}`. Servizio mancante o da cambiare = deviazione (b) → rimbalzo a `plan`.
- Segreti: solo in `$HOME/.cka-spikes/spikes.env` (variabile `CKA_SPIKES_ENV`). Ogni `docker compose` con `--env-file "$CKA_SPIKES_ENV"`; script host con `set -a; . "$CKA_SPIKES_ENV"; set +a`. File assente → fermati e segnala. Mai creare `spikes/.env`. Mai valori in commit, log, SUMMARY.
- `spikes/.env.example`: chiave nuova solo in coda alla tua sezione `# sNN`, valore vuoto o non segreto, file in `files_modified`. Endpoint (`garage:3900`, `tei-embed:80`, …) **solo da env**, mai literal `http://` nel codice o nello YAML (scanner `http-url`).
- Codice Linux-only (dhat, VmHWM, `/proc`, `ss`, misure CPU): eseguilo nel servizio `runner` (`--profile runner run --rm runner …`); tempfile su volume `work-tmp` (`/work/tmp`), mai tmpfs né bind NTFS.
- Worktree pulito: dati grezzi ignorati non ci sono → rilancia `fetch.sh` idempotenti (`data/corpus/fetch.sh`, `data/s02/fetch.sh`) prima di misurare.
- Finestra di misura: prima di ogni sessione `docker ps` senza container di altri piani (build o misure); altrimenti attendi. Misure GPU solo con `VERDETTO: VERDE` in `spikes/results/gpu-check.txt`.
- Marker `.security-scan-fixture`: solo in `spikes/data/hostile/` (01-07), mai su cartelle di codice. Scanner di sicurezza su elenco esplicito di percorsi, mai su `spikes` intero.
- Prima di ogni sessione di misura: `spikes/tools/hw-manifest.sh <tua-cartella-results>/hw.json`. Soglie scritte in RISULTATI §Soglie PRIMA della misura, poi immutabili (correzioni solo in «Anomalie e deviazioni»).
- Corpus comune `spikes/data/corpus/**` (incl. `oracles/`): sola lettura per chi non ne è proprietario. Si usano solo oracoli con tre assi verdi. Grezzi in `raw/` (ignorati); si versionano CSV + manifest con sha256.
- ADR: solo il tuo numero riservato (§5 W13), da `docs/adr/0000-template.md`, Stato `Proposed`, link a `RISULTATI*.md` e `hw.json` (ADR-0012: dichiara assenza di `hw.json`). Mai un numero non tuo.
- `DEPENDENCIES.md`, `docs/security/threat-model.md`: li scrive SOLO 01-17. Le altre slice scrivono in RISULTATI §Esito: riga DEPENDENCIES e nuovo stato, minacce T-.. con file di evidenza, impatto sulle porte S-07.
- Trait-porta candidati: SOLO 01-16 in `spikes/s07-ports/src/lib.rs`. Nessun altro crate definisce firme «di prodotto».
- Nessun file di prodotto importa `spikes/` e nessun crate spike è path-dep di prodotto. Nessun contenuto per fasi future (adapter, migrazioni, trait in `crates/` o `services/`): seam inter-fase di 01 = nessuno (§2).
- Client JEV/LLM/TEI: locale al tuo crate (`src/jev.rs`, `src/anthropic.rs`, `src/tei.rs`, …), nessun client condiviso in `common` né import fra crate spike. Tetti di spesa `S04_MAX_COST_USD`, `S06_MAX_COST_USD` letti dall'env.

## §5 — Seam da creare in wave 0          (wiring-design → wiring-build)

Nessun seam inter-fase (nessuna riga `WM-NNN`). Solo intra-fase. «Wave 0» = piani 01-01 e 01-02 (`wave: 1` nel frontmatter); `wiring-build` verifica che coincidano con questo elenco. Gate di wave 0: `cargo check --workspace` + `cargo test -p common` in `spikes/` verdi; `docker compose --env-file spikes/.env.example -f spikes/compose.yaml --profile db config -q` e config con tutti i profili validi; pgvector ≥ 0.8.4 letto da `pg_extension`; build del `runner` verde; `gpu-check.txt` con `VERDETTO:` + decisione utente; scanner di sicurezza pulito su elenco esplicito di percorsi `spikes/`; nessun `Cargo.toml` in radice; `$HOME/.cka-spikes/spikes.env` presente con chiavi provider inserite dall'utente.

**W1/W4 — `spikes/Cargo.toml` + crate vuoti (01-01).**
```toml
[workspace]
resolver = "3"
members = ["common", "s01-upload", "s02-parser", "s03-pgvector", "s04-jev", "s05-cancel", "s06-llm", "s07-ports"]

[workspace.package]
edition = "2024"
publish = false

[workspace.dependencies]
# RESEARCH §0 + §1, pin esatti; coda append-only per le slice
```
Ogni crate: `Cargo.toml` (`version = "0.0.0"`, `edition.workspace = true`, `publish.workspace = true`) + `src/lib.rs` (`common`, `s07-ports`) o `src/main.rs` `fn main() {}` (s01..s06). Bin aggiuntivi in `src/bin/` (auto-discovery). Crate ratificati dall'utente (checkpoint 01-01 Task 1) prima del lockfile.
`[workspace.dependencies]` pin `=x.y.z`: axum (multipart, http2), tower-http (limit, timeout, trace), tokio (full), tokio-util (io), object_store (aws), sqlx (no default; runtime-tokio, tls-none, postgres, uuid, migrate, macros), pgvector (sqlx, halfvec), pdfium-render, reqwest (json, stream, zstd, multipart), wiremock, dhat, tempfile, sha2, rlimit, nix (signal, process), futures, hdrhistogram, serde_json, clap (derive), anyhow, sysinfo, bytes, lopdf; al max stable crates.io in esecuzione: serde (derive), csv, hex, uuid (v4, serde), rand, flate2, zip, image (png, no default), async-trait, unicode-normalization, quick-xml.
Mappa `[dependencies]` attive (`.workspace = true`):
- `common`: serde, serde_json, csv, hdrhistogram, sysinfo, anyhow
- `s01-upload`: common (path), axum, tower-http, tokio, tokio-util, object_store, sqlx, tempfile, sha2, hex, bytes, futures, zip, serde, serde_json, uuid, clap, anyhow; dev: dhat, reqwest
- `s02-parser`: common, pdfium-render, tokio, rlimit, nix, lopdf, flate2, zip, quick-xml, unicode-normalization, reqwest, serde, serde_json, clap, image, anyhow
- `s03-pgvector`: common, sqlx, pgvector, uuid, tokio, reqwest, futures, serde, serde_json, rand, unicode-normalization, clap, anyhow
- `s04-jev`: common, reqwest, tokio, futures, serde, serde_json, sha2, hex, clap, anyhow
- `s05-cancel`: common, axum, tokio, reqwest, futures, wiremock, serde_json, clap, anyhow
- `s06-llm`: common, reqwest, tokio, futures, serde, serde_json, sha2, hex, unicode-normalization, clap, anyhow
- `s07-ports`: nessuna in wave 0; 01-16 attiva solo deps già dichiarate (async-trait se scelto, futures, bytes, uuid, serde) e rigenera il lock

**W3 — `spikes/Cargo.lock`**: generato da 01-01 (`cargo generate-lockfile` + `cargo check --workspace` in `spikes/`; ripiego container `rust:1.99.1-trixie` se la toolchain C di Windows fallisce, annotato). Rigenerato una volta da 01-16.

**W5 — `spikes/compose.yaml`** (01-02), `name: cka-spikes`, porte solo `127.0.0.1`, segreti `${VAR}` da `--env-file "$CKA_SPIKES_ENV"`, parametri `${VAR:-default}`, nessun `env_file:`, nessun `http://` letterale:

| servizio | profili | note | proprietario blocco / file collegato |
|---|---|---|---|
| postgres (`pgvector/pgvector:0.8.7-pg18-trixie`) | db, s01, s03 | `127.0.0.1:55432`, volume `pgdata` su `/var/lib/postgresql` | 01-02 (chiuso; condiviso 01-04 / 01-10 / 01-14) |
| garage (`dxflrs/garage:v2.4.1`) | s01 | `127.0.0.1:3900`, `./docker/garage.toml:ro` | blocco 01-02; `garage.toml` 01-04 |
| tei-embed (`…:86-1.9.4`) | s03, gpu | `127.0.0.1:8081`, `hf-cache` | 01-02 |
| tei-rerank (`…:86-1.9.4`) | s04, gpu | `127.0.0.1:8082`, `hf-cache` | 01-02 |
| docling (`docling-serve-cu128:v1.37.0`) | s02 + gpu | nessuna porta, sola rete `parse` | 01-02 |
| docling-cpu (`docling-serve-cpu:v1.37.0`) | s02 + cpu | nessuna porta, sola rete `parse` | 01-02 |
| worker | s02 | `network_mode: none`, read_only, tmpfs `/work` 2g, mem=swap 2g, cap_drop ALL, user 10001, `./data:/data:ro`, `./s02-parser/results:/results` | blocco 01-02; `worker.Dockerfile` 01-07 |
| paddle | s02 | `./data:/data:ro`, `./s02-parser/results:/results` | blocco 01-02; `paddle.Dockerfile` 01-13 |
| runner (`docker/runner.Dockerfile`, rust 1.99.1 + oha + qpdf + poppler + imagemagick) | runner | `./:/src`, `cargo-target:/target`, `cargo-registry`, `work-tmp:/work/tmp`, reti `default` + `parse`, env = tutte le chiavi di `.env.example` come `KEY: ${KEY}` | 01-02 |

`networks: { parse: { internal: true } }`, `volumes: { pgdata, garage-meta, garage-data, hf-cache, cargo-target, cargo-registry, work-tmp }` = 01-02, chiusi.

**W6 — `spikes/.env.example`** (01-02), segreti e URL interni vuoti (forma in commento):
`# common` HW_LABEL · `# s01` POSTGRES_USER, POSTGRES_PASSWORD, DATABASE_URL, GARAGE_RPC_SECRET, GARAGE_ACCESS_KEY, GARAGE_SECRET_KEY, S3_ENDPOINT, S3_BUCKET=cka-originals, S01_TARGET_URL · `# s02` PDFIUM_DIR=/opt/pdfium/lib, DOCLING_URL, DOCLING_CPU_URL, DOCLING_NUM_THREADS=8 · `# s03` EMBED_MODEL=Qwen/Qwen3-Embedding-0.6B, TEI_EMBED_URL, TEI_EMBED_DTYPE=float16, TEI_MAX_BATCH_TOKENS=16384, TEI_MAX_CLIENT_BATCH=64 · `# s04` JEV_API_URL=https://api.typesafe.ai, TYPESAFE_API_KEY, JEV_MODEL=jev-1.13.0 (D-17), TEI_RERANK_URL, RERANK_MODEL=BAAI/bge-reranker-v2-m3, TEI_RERANK_MAX_BATCH_TOKENS=8192, S04_MAX_COST_USD=5 · `# s05` S05_JEV_CANCEL_REQUESTS=20 · `# s06` ANTHROPIC_API_KEY, LLM_MODEL_SIMPLE=claude-haiku-5-5, LLM_MODEL_COMPLEX=claude-sonnet-5-5 (D-18), S06_MAX_COST_USD=5.
`DATABASE_URL` in `# s01`, letta anche da S03; nessun duplicato. Valori reali: `$HOME/.cka-spikes/spikes.env` (fuori repo, permessi solo utente, generato da 01-02; chiavi provider inserite dall'utente al checkpoint). Nessun `spikes/.env`.

**W7 — `spikes/.gitignore`**: `target/`, `.env`, `**/results/raw/`, `data/**/raw/`, `data/hostile/generated/`. **`spikes/.dockerignore`**: `target/`, `.env`, `data/`, `**/results/`.

**W8 — `spikes/common`** (01-01, API fissa, chiusa):
- `env::required(name: &str) -> anyhow::Result<String>` (errore nomina la variabile, mai il valore)
- `manifest::HwManifest` (campi RESEARCH §8.2, opzionali) + `manifest::ensure_present(dir: &Path) -> anyhow::Result<HwManifest>`
- `csv::Writer::create(path: &Path, header: &[&str])` (chiama `ensure_present` sulla cartella) + `write_row(&mut self, fields: &[&str])` (rifiuta numero campi ≠ header)
- `hist::Recorder::new()`, `record_us(u64)`, `summary() -> Summary { n, p50_us, p95_us, p99_us, max_us }`, `Summary::csv_fields() -> [String; 5]`
- `timer::Stopwatch::start()`, `elapsed_us()`, `elapsed_ms_f64()`
Test unitari ≥ 5 (header fisso, CSV senza hw.json, riga di lunghezza errata, percentili su 1..=100, env mancante, manifest malformato).

**W9/W10 — `spikes/tools/`** (01-02): `hw-manifest.sh <out.json> [--image <ref>]… [--tool nome=versione]…` (campi RESEARCH §8.2, assenti → `null`, JSON validato), `gpu-check.ps1` (sequenza RESEARCH §6: nvidia-smi, wsl, docker, nbody, TEI embed 1024 valori). Esito in `spikes/results/gpu-check.txt` (versionato), ultima riga `VERDETTO: VERDE|ROSSO — <passo>`; riportato in §6.

**W11 — `spikes/data/`** (01-02): solo `.gitkeep` in `corpus/`, `s02/`, `s03/`, `s06/`, `hostile/`. Proprietari dei contenuti: `corpus/` 01-03 (CORPUS.md, manifest.json, acl.json, fetch.sh, synthetic/), `corpus/oracles/` 01-06, `hostile/` 01-07 (manifest.json, marker fixture; generati in `hostile/generated/` ignorato), `s02/` 01-08 (manifest.json, fetch.sh, DATASET.md), `s06/` 01-12 (selection.json), `s03/` nessuno (resta vuota).

**W12 — `spikes/docker/`**: `runner.Dockerfile` 01-02; `garage.toml` 01-04; `worker.Dockerfile` 01-07 (compila `-p s02-parser`, copia tutti i bin, PDFium + tessdata verificati sha256, `USER 10001`); `paddle.Dockerfile` 01-13 (pip pin `paddlepaddle==3.3.1`, `paddleocr==3.7.0`, non root). Wave 0 crea solo `runner.Dockerfile`.

**W13 — prenotazione numeri ADR** (fissata prima di wave 0, immutabile; confermata dai PLAN, numeri invariati):

| ADR | file | tema | piano |
|---|---|---|---|
| 0003 | `0003-storage-originali-garage.md` | storage originali (Garage, chiave sha256, GC orfani) | 01-04 |
| 0004 | `0004-parser-ocr-isolamento.md` | parser PDFium + Docling tabelle + OCR + isolamento processo + tetto decompressione | 01-13 |
| 0005 | `0005-locator-citazione.md` | locator di citazione a due varianti | 01-09 |
| 0006 | `0006-indice-vettoriale-ricerca-esatta-ibrida.md` | ricerca esatta, ibrida RRF, soglia HNSW | 01-14 |
| 0007 | `0007-embedding-tei-gpu.md` | modello di embedding + serving TEI GPU | 01-10 |
| 0008 | `0008-jev-forma-rank-concorrenza-pin.md` | JEV: forma `rank`, concorrenza, pin `jev-1.13.0` | 01-11 |
| 0009 | `0009-reranker-baseline.md` | reranker baseline + budget VRAM | 01-15 |
| 0010 | `0010-deadline-cancellazione-http.md` | deadline e cancellazione HTTP | 01-05 |
| 0011 | `0011-llm-generativo-routing-structured-output.md` | LLM, routing per complessità, fallback, structured output | 01-12 |
| 0012 | `0012-firme-porte-candidate.md` | firme candidate delle porte (+ async-trait/boxing, «Vincoli verso la Fondazione») | 01-16 |
| 0013 | `0013-rinvio-antivirus-cdr.md` | rinvio antivirus/CDR (ASVS L2) | 01-17 |
Ratifica: 01-17 porta 0003-0012 a `Accepted` (o li lascia `Proposed` con motivo utente) toccando solo la riga `Stato` + una riga `**Modifica:**`. Wave 0 non crea file ADR: solo la tabella.

### Storia

Regole della prima stesura superate dal riallineamento ai PLAN (2026-10-18), conservate qui:
- Slice S02/S03/S04 «un piano per spike» → spezzate: S-02 in 01-07/01-08/01-09/01-13, S-03 in 01-10/01-14, S-04 in 01-11/01-15 (W18 nuovo, W17 nuovo per il lanciatore isolato). Motivo: volume di lavoro e serie per risorsa GPU/CPU/JEV.
- W11 «query etichettate in `data/s03/`» → `data/corpus/oracles/` di un piano oracoli dedicato (01-06): le usano S-02, S-03, S-04, S-06; guardrail una volta sola. `data/s03/` resta vuota.
- W9 «`spikes/<sNN>/results/hw.json`» → `results/<parte>/hw.json` per i crate spezzati.
- W5 tabella a 8 servizi → 9 con `runner` (Linux per dhat/VmHWM/`/proc`/`ss`) e volumi `cargo-target`, `cargo-registry`, `work-tmp` (tempfile S-01 non su tmpfs: falserebbe la memoria del cgroup); docling senza porte (rete interna). Deviazione già dichiarata in 01-02.
- W5/W4 «modifica inevitabile = solo il tuo blocco, serializzata» → compose interamente chiuso; nessun piano lo modifica.
- W6 «segreti in `spikes/.env` ignorato» → file esterno `$HOME/.cka-spikes/spikes.env` via `CKA_SPIKES_ENV`: i worktree puliti di `execute` non vedrebbero un `.env` ignorato.
- W7 + `.dockerignore` (contesto di build di runner/worker/paddle).
- W4 mappa deps estesa (serde, csv, hex, uuid, rand, image, quick-xml, unicode-normalization, async-trait) così nessuna slice tocca `spikes/Cargo.toml`; `s07-ports` tocca solo il proprio `Cargo.toml` + lock.
- W12 `paddle.Dockerfile` da «S02» a 01-13; `worker.Dockerfile` a 01-07.
- W13 numeri invariati; aggiunta l'eccezione di ratifica di 01-17 sugli ADR altrui (prima: «mai prendere un numero non tuo» senza eccezione).
- Vincoli onde: da «serie SC → {S02,S03,S04,S06}» generica a 11 onde con classi di risorsa (W20).

## §6 — Esito della wave 0                (wiring-build)

## Registro variazioni

| data | autore | variazione |
|---|---|---|
| 2026-10-18 | Claude (Anthropic) | prima stesura §1-§5 (wiring-search + wiring-design), prima dei piani |
| 2026-10-18 | Claude (Anthropic) | §1-§5 riallineati ai 17 PLAN (fonte): split S-02/S-03/S-04 (W17, W18), oracoli in `data/corpus/oracles/`, `results/<parte>/`, `runner` + volumi, segreti esterni `CKA_SPIKES_ENV`, `.dockerignore`, mappa deps estesa, 11 onde con classi di risorsa (W20), Postgres condiviso (W19), ratifica ADR in 01-17; numeri ADR invariati; WIRING-MAP invariata (nessun seam inter-fase) |

**Modifica:** Claude (Anthropic) · 2026-10-18 · wiring-search §1-§2 e wiring-design §3-§5, fase 1
**Modifica:** Claude (Anthropic) · 2026-10-18 · riallineamento §1-§5 ai 17 PLAN della fase 1 (PLAN = fonte), Storia delle regole superate
