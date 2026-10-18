# HANDOFF — sessione 2026-10-18 (plan fase 1 in corso)

> Per la sessione successiva di Claude. Leggere prima `CLAUDE.md` (regole del loop, strategia provider-first, insiemi chiusi), poi questo file, poi `.planning/ORCHESTRATOR-STATUS.md`. Comunicazione con l'utente in **italiano**.

## 0. Aggiornamento 2026-10-19 (prevale sulle sezioni sotto)

- §6 punti 1–4 **fatti**: audit 1–4 e deepsearch 1–5 salvati + ACK (`3a2f37f`); giro di correzioni sui 7 piani (plan_version 2) + allineamento a `session.sh` di 03, 05, 10, 11, 12, 14, 15; re-gate giro 4 = 17/17 conformi (`01-GATE.md`); commit `3ac5650`, branch `task/PLAN-010-{02,04,07,08,09,13,16}` = `3ac5650` pushati; **inviati `risk_audit_8..14`** (mappa: 8=02, 9=04, 10=07, 11=08, 12=09, 13=13, 14=16), giro 2 di 3; `status_2` sul tracker.
- **Aggiornamento:** audit v2 rientrati 7/7 APPROVED_WITH_WARNINGS, salvati come `01-NN-RISK-AUDIT-v2.md` + ACK (`187f184`); plan-gate RISK_APPROVED; inviati `test_design_1..7` (1=02, 2=04, 3=07, 4=08, 5=09, 6=13, 7=16). **Prossimo:** salvare i test design come `01-NN-TEST-DESIGN.md` + ACK, `READ-GATE` = TEST_SPEC_READY, poi §6 punto 6.
- **Decisioni utente pendenti** (non bloccano l'audit, sì l'execute): oha→curl in 04; piani 35–54 KB (spezzare?); memoria worker 2 GiB vs 16 OCR; PP-OCRv6 condizionale; DocLayNet 20→40 pagine; proposte ADR-0005 (offset Unicode scalar, ordinale sezioni omonime, testo di sezione senza sottosezioni, `cka-text-v1`); rifiuto PDF con filtri LZW/RunLength e limite ObjStm; riavvio Docling per documento e `page_range`; tolleranze del controllo analitico bbox (13); pin pip 24.3.1 / pip-tools 7.4.1 da verificare.

## 1. Dove siamo in una riga

Fase 1 «Spike tecnici» (critica): discuss chiuso, 17 PLAN scritti e conformi ai 6 gate (tre giri, `01-GATE.md`), **audit GPT al primo giro: 7/7 REVISE** — tocca al giro di correzioni (passo 6-bis di `skills/plan/SKILL.md`).

## 2. Cosa è fatto (commit su `main`, pushato su GitHub)

- Repo pubblico: https://github.com/Gianmarco-03/corporate-knowledge-assistant (`gh` autenticato come `Gianmarco-03`, binario `C:\Program Files\GitHub CLI\gh.exe`, non nel PATH di Git Bash).
- Loop GSD+ECC portato da Gestionale Light: skill in `skills/` (+ mirror `.claude/skills/`), gate, script, hook git attivi (`core.hooksPath=.githooks`), ECC installato a scope progetto.
- Bootstrap GSD con `/gsd-ingest-docs`: 15 fasi, 37 requisiti. Strategia U-04 provider-first (con emendamento: plan 1 subito, spike eseguiti in parallelo ai discuss 2–15).
- Decisioni utente rilevanti oltre a CLAUDE.md: racconto didattico per slice (`0X-YY-RACCONTO.md`); insiemi chiusi solo per codice/config, **Markdown sempre modificabili** (whitelist `**/*.md`); controllo GPU = gate di wave 0; audit GPT della fase 1 **solo sui piani a rischio** 01-02, 04, 07, 08, 09, 13, 16 — gli altri 10 hanno test design di Claude (fatto).
- `01-CONTEXT.md`: 21 decisioni D-01..D-22 (Garage S3, tempfile, 25 MB, PDFium + Docling solo tabelle, parser in processo figlio, Tesseract CLI, locator a due varianti, ricerca esatta + tsvector/RRF, embedding locale GPU via TEI Qwen3-0.6B, JEV pin `jev-1.13.0`, LLM Haiku/Sonnet 5.5 con routing di complessità deciso da JEV e fallback Sonnet, risposta JSON con citazioni per claim, reranker baseline bge-reranker su GPU).
- Ultimo commit: `180d637 docs(01): piani della fase 1 conformi ai gate`.

## 3. Lavoro NON committato nel working tree (da committare)

- 7 piani fuori audit corretti dalle note del test design: 01-01, 06, 10, 12, 14, 15, 17 (sha nuovi già riportati nei rispettivi TEST-DESIGN).
- 10 `01-NN-TEST-DESIGN.md` (01, 03, 05, 06, 10, 11, 12, 14, 15, 17), designer Claude separato; nessun CRITICAL/HIGH scoperto.
- 3 `01-NN-RISK-AUDIT.md` salvati verbatim (09 ← risk_audit_5, 13 ← risk_audit_6, 16 ← risk_audit_7), **già ACK** (archiviati in `Solved/`).
- Questo HANDOFF.

I piani sotto audit (02, 04, 07, 08, 09, 13, 16) **non sono stati toccati**: i loro sha corrispondono agli snapshot inviati.

## 4. AgentBridge — stato preciso

Controller `http://127.0.0.1:8765`, progetto `CorporateKnowledgeAssistant`, file in `C:\Agent Bridge\CorporateKnowledgeAssistant\`. Comandi: `POST /api/projects/CorporateKnowledgeAssistant/commands` con JSON (schemi in `C:\Agent Bridge\docs\contracts.md`). Branch di snapshot pushati: `task/PLAN-010-{02,04,07,08,09,13,16}` = `180d637`.

Mappa content_id → piano: risk_audit_1 = 01-02, _2 = 01-04, _3 = 01-07, _4 = 01-08, _5 = 01-09, _6 = 01-13, _7 = 01-16.

**Da fare subito:**
1. Per `risk_audit_1..4`: `READ-RESPONSE` → salvare `markdown` verbatim in `.planning/phases/01-spike-tecnici/01-NN-RISK-AUDIT.md` con l'intestazione di attribuzione (vedi i 3 già salvati) → `ACK-RESPONSE` con il `delivery_token`. Le risposte sono già presenti (`response.risk_audit_1..4.md`).
2. `ask-deepsearch` `deepsearch_1..5` (aperte automaticamente dagli audit, tutte con risposta): READ-RESPONSE, salvare verbatim (es. `01-RICERCA-AUDIT-deepsearch_N.md`, attribuite a GPT con content_id; non riverificare le fonti), ACK. Bloccanti: 2 (01-08), 3 (01-07), 4 (01-02).
3. Lezione: i file su disco del bridge arrivano a pochi secondi l'uno dall'altro; controllare la cartella (`ls`) prima di dichiarare «non pronto».

## 5. Esito degli audit (sintesi, tutti REVISE, nessun CRITICAL)

- **01-02** (1H/3M): profili Compose = **unione** (deepsearch_4) → avvio per nomi di servizio espliciti + `docker compose config --services` prima di `up`; ownership dei volumi per uid 10001 con smoke test; pesi Docling preparati offline (deepsearch_5: `docling-tools models download` in `/models`, `DOCLING_SERVE_ARTIFACTS_PATH`, SHA registrati); prenotazione della sessione di misura con run_id e cleanup solo dei propri container.
- **01-04** (3H/2M): `bench.sh` non può usare Docker dal runner → orchestrazione Compose sull'host; carico che separa nuovi/deduplicati con corpi distinti; admission control/budget per le validazioni ostili oltre a `spawn_blocking`; protocollo GC vs adozione concorrente; PDF validi di dimensione esatta separati dal test di puro streaming.
- **01-07** (3H/1M) — **chiave**: il lanciatore `parse-parent` accetta solo `--child/--input/--out` → serve un **job JSON tipizzato** (operazione + parametri, allowlist dei bin) per i figli di 08/09/13; tetto di decompressione **prima** del load (deepsearch_3: lopdf 0.45 `LoadOptions.max_decompressed_size`, `ObjectStream::new_with_limit`; PDFium senza tetto globale → solo confinamento nel figlio); budget di memoria aggregato coerente col cgroup (padre e figli condividono il limite); cleanup del gruppo su ogni uscita, stdout/stderr letti con limite.
- **01-08** (2H/2M): `extract pdf` invocabile solo tramite il nuovo contratto della 07; Docling fuori dal supervisore → kill/riavvio per job (deepsearch_2: `document_timeout` e disconnessione non cancellano il lavoro); IoU con contratto di abbinamento esplicito; pagine di taratura separate da holdout.
- **01-09** (1H/4M): lanciatore chiuso; titoli ripetuti → section_path non univoci; unità di offset (Unicode scalar vs UTF-16 del viewer JS); testo canonico DOCX/MD; marcatore vs bbox estratta.
- **01-13** (3H/2M): lanciatore chiuso; 8–16 processi OCR oltre 2 GiB; TTQ-parse va misurato sul percorso composto reale; variante degradata; lock pip non fissa i pesi (deepsearch_1: sempre `--ocr_version PP-OCRv5 --lang it`, registrare file+SHA dei pesi, rete negata).
- **01-16** (1H/3M): tipo `AuthorizedEvidence` ottenibile solo da `VectorIndex::search` con predicato, unico input di `Reranker`/`AnswerGenerator`; `CacheKey` con scope autorizzativo obbligatorio; contesto delle operazioni mutanti oltre alla deadline; identità dello spazio di embedding oltre alla versione del modello.

## 6. Prossimi passi (proposta già presentata all'utente, in attesa del suo via)

1. Salvare + ACK audit 1–4 e deepsearch 1–5 (§4).
2. Un giro di correzioni sui 7 piani (prima 01-07, che sblocca 08/09/13), in un subagente fresco con brief da §5; aggiornare `01-WIRING.md` se cambia il contratto del lanciatore (W17).
3. Re-gate sui piani toccati (solid, crate-boundary, security, perimetro, proprieta-file, eval-oracle dove serve) → aggiornare `01-GATE.md`.
4. Commit dei PLAN, nuovo branch head, push; secondo `implementation-risk-audit` con `plan_version: 2` (sha = `git show <head>:<plan> | sha256sum`, file in LF). Limite 3 giri d'audit.
5. Con verdetto APPROVED*: `test-design` GPT per i 7 piani → salvare `01-NN-TEST-DESIGN.md`, ACK, `READ-GATE` = `TEST_SPEC_READY`.
6. Passo 7: `01-CONTRATTO.md`, `onde-slice` → `01-ONDE.md`, `update-orchestrator-state --plan`, commit `docs(01): fase pianificata, conforme ai gate`.
7. In parallelo l'utente può avviare `/discuss 2`.

## 7. Prerequisiti utente ancora aperti

- **Docker Desktop** da installare (istruzioni date: `winget install -e --id Docker.DockerDesktop`, `.wslconfig` con `memory=20GB processors=16 swap=0`, verifica GPU `docker run --rm --gpus all nvidia/cuda:12.8.0-base-ubuntu24.04 nvidia-smi`). WSL2 Ubuntu presente, driver NVIDIA 610.88 ok. Serve prima di `execute 1`.

## 8. Trappole note

- Errori di rete (`ENOTFOUND`) interrompono i subagenti: riprenderli con SendMessage invece di rilanciarli.
- Il classificatore dei permessi blocca: riscrittura della storia git (l'utente l'ha eseguita con `!`), spostamenti in `~/.claude` (self-modification). Proporre all'utente il comando con `!`.
- `session_memory.json` (cache del plugin token-reducer) è in `.gitignore`: non deve mai entrare nel repo pubblico.
- Piani in CRLF nel working tree, LF nell'indice (`.gitattributes eol=lf`): gli sha per AgentBridge si calcolano sul blob committato.
- `ORCHESTRATOR-STATUS.md` va aggiornato via `update-orchestrator-state` (non è aggiornato dopo il plan: posizione ancora «discuss completato»).
- Modello: per i subagenti usare gli ID Claude correnti (Opus 5.5 `claude-opus-5-5`, Sonnet 5.5, Haiku 5.5).

**Modifica:** Claude (Anthropic) · 2026-10-18 · handoff di fine sessione (contesto in saturazione, richiesta utente)
**Modifica:** Claude (Anthropic) · 2026-10-19 · §0: giro di correzioni e audit v2 inviati
