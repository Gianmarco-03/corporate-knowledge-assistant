# ORCHESTRATOR-STATUS — Corporate Knowledge Assistant

Stato persistente dell'orchestratore del loop GSD+ECC. Lo scrive solo `update-orchestrator-state`.
Regole di sanità: (1) indice, non archivio — una riga + puntatore; (2) mai fatti GSD (avanzamento piani/fasi sta in STATE.md/ROADMAP.md).

## Posizione

fase 01 / onda 1 / slice 01-01, 01-02 / execute onda 1 completata · aggiornato: 2026-10-19T19:51:24Z

## Prossima azione

/execute 1 onda 2: READ-GATE PLAN-010-04, worktree 01-03 (exec/01-03) + 01-04 (task/PLAN-010-04) da main, test-write, esecutori.

## Findings

- fase 01 criticità critica, ratificata → 01-CONTEXT.md
- storage S3 Garage, chiave sha256 + GC orfani, dedup invisibile (D-01/02) · fonte Area A
- tempfile + validazione pre-persistenza; limite 25 MB; L1 + regole economiche (D-03..05)
- PDFium + Docling solo tabelle; processo figlio isolato; Tesseract CLI + PP-OCRv6 sfidante (D-07..09, D-09 emendata) · fonte Area B
- locator 2 varianti: PDF pagina+rect / testo sezione+offset (D-10)
- ricerca esatta MVP, HNSW a soglia; tsvector + RRF (D-12, D-15) · fonte Area C
- embedding locale GPU, TEI, Qwen3-0.6B + sfidanti bge-m3/e5 (D-13/14)
- corpus pubblico/sintetico; JEV pin jev-1.13.0; LLM API Haiku/Sonnet con routing JEV, fallback Sonnet (D-16..19) · fonte Area D
- risposta JSON structured output, citazioni per claim con chunk_id (D-20); reranker baseline no-rerank + bge-reranker GPU (D-21)
- decisioni utente post-audit D-23..D-30 (curl+xargs, ADR-0005, limiti PDF in doc ufficiale, tolleranze spike, Docling pagine-tabella rinviata, worker 2 GiB, DocLayNet 40 pp, split 01-07/01-18) → 01-CONTEXT.md
- plan: 18 piani, gate 6 giri, 18/18 conformi → 01-GATE.md
- wiring: nessun seam inter-fase, WIRING-MAP invariata; W17 = contratto job JSON di 01-07 → 01-WIRING.md
- audit GPT critici 02,04,07,08,09,13,16,18 tutti APPROVED_WITH_WARNINGS (07, 13 al 3° giro; 18 al 2°) → 01-NN-RISK-AUDIT*.md
- test design GPT su 8 critici (plan-gate TEST_SPEC_READY), Claude su 10 → 01-NN-TEST-DESIGN.md
- contratto emesso, MEDIUM residui per brief execute → 01-CONTRATTO.md; 11 onde, 01-12 in onda 7 (tetto 2) → 01-ONDE.md
- onda 1 chiusa: 01-01 (merge 00df603) + 01-02 (merge ad19dc6, a43a0fa), gate onda verde, wave 0 §6 verde, pgvector 0.8.7 → 01-WIRING.md §6, 01-01/01-02-SUMMARY.md
- 01-02 bridge rifiutato 1×: separatore session.sh (spazio vs IFS='|'), assert-only + rifiuto run_id altrui mai attivi; fix 37c68ee, prove negative verdi → 01-02-SUMMARY.md
- deviazioni livello a: 01-01 fmt + rinomina MANIFEST_NAME; 01-02 14 (+1 fix) es. docling-models user 0:0, mem_limit, PDF stdlib → SUMMARY
- checkpoint 01-02 Task 4: chiavi inserite, GPU VERDE, pesi Docling --all 28,6 GB tenuti · utente 2026-10-19 → DECISIONS-INDEX.md
- 01-12 spesa Anthropic dimezzata: ripasso plan prima onda 7 · utente 2026-10-19 → DECISIONS-INDEX.md
- rimbalzi livello b: nessuno

## Limiti trovati

- 2026-10-19 · Docker Desktop 29.8.2 + Compose v5.6.0, WSL2, runtime nvidia; `.wslconfig` memory=20GB processors=16 swap=0 applicato (VM: 16 CPU, 20000 MiB, swap 0); GPU check container verde · impatto: VRAM libera ~4,3 GiB (desktop Windows ~1,8 GiB su 6) → considerarlo in 01-15 VRAM e gate GPU di 01-02 · rif. HANDOFF §7

## Gate & verdetti

| Piano | Gate sul PLAN | Review ECC | Verifier GSD |
| --- | --- | --- | --- |

## UAT

*(vuoto)*

## TODO

- hardware spike: i7-13700K 16C/24T, 32 GB RAM, RTX 3050 6 GB VRAM, Windows 11.
- Docker Desktop non installato: prerequisito execute 1 (passthrough GPU WSL2, .wslconfig) → HANDOFF §7.
- S-07 porte: Embedder copre embedding query; validate_query restituisce complessità; locator in contracts.
- DEPENDENCIES.md da aggiornare a esito spike (JEV pin, A-02..A-10) → 01-17.
- da confermare utente: soglie M10/M11 (01-18); lessico motivi quarantena + exit 3 driver (01-07); pin pip 24.3.1 / pip-tools 7.4.1 non verificati.
- test_design_3, _6 (07 v2, 13 v2) senza risposta su AgentBridge: obsoleti, sostituiti da _8, _9.
- D-27 Docling pagine-tabella: scelta dopo misura 01-08 → 01-CONTEXT Deferred Ideas.
- interfaces-and-concurrency.md (split Reranker/DecisionEngine) → Fondazione (ADR-0012).

## Decisioni ratificate in corsa

- strategia provider-first + insiemi file chiusi (U-04) · utente · 2026-10-18 → PROJECT.md
- racconto didattico per slice · utente · 2026-10-18 → skills/execute/SKILL.md §bridge 3-bis

**Modifica:** Claude (Anthropic) · 2026-10-18 · creazione (preflight discuss 1)
**Modifica:** Claude (Anthropic) · 2026-10-18 · --discuss fase 01
**Modifica:** Claude (Anthropic) · 2026-10-19 · --plan fase 01
**Modifica:** Claude (Anthropic) · 2026-10-19 · --evento: Docker Desktop e GPU pronti
**Modifica:** Claude (Anthropic) · 2026-10-19 · --exec fase 01 onda 1
