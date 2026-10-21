# ORCHESTRATOR-STATUS — Corporate Knowledge Assistant

Stato persistente dell'orchestratore del loop GSD+ECC. Lo scrive solo `update-orchestrator-state`.
Regole di sanità: (1) indice, non archivio — una riga + puntatore; (2) mai fatti GSD (avanzamento piani/fasi sta in STATE.md/ROADMAP.md).

## Posizione

fase 01.2 / — / slice 01.2-01 / verify completato (passed), fase chiusa · aggiornato: 2026-10-21

## Prossima azione

/execute 1 onda 4 (01-07 contratto job/lanciatore, 01-11 JEV rank; gate con `cargo clean -p` dei crate toccati); pendenti utente: UAT 01.1 e 01.2, hook pre-commit (TODO).

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
- onda 2 chiusa: 01-03 (merge 0d0d630) + 01-04 (merge cc202a5), gate onda verde (fmt/clippy/test spikes, compose config, proprieta-file conforme, segreti 0) → 01-03/01-04-SUMMARY.md
- 01-04 esito: Garage proposto solo parti 2 MiB/1 in volo (default supera VmHWM); 500 dedup concorrente ridotti non eliminati; PG bytea scartato (canale laterale 0/3); PUT idempotente non misurato → ADR-0003 Proposed, s01-upload/RISULTATI.md
- deviazioni livello a onda 2: 01-03 container esplorativo fuori sessione; 01-04 SIGTERM/after_put, pin_env, bench.sh toglie env host, p5k1→p2k1 dopo misura memoria (83720bb) → SUMMARY
- rimbalzi livello b: nessuno; segnalazione b su file chiuso 01-02 (session.sh env host) → gap-closure 1.1
- fase 01.1 inserita (gsd-phase), criticità normale · utente 2026-10-21; CONTEXT D-01..D-04 → 01.1-CONTEXT.md
- plan 01.1: 1 piano (checker PASSED 1° giro), gate 6/6 conformi o n/a dichiarati, test design Claude 16 RISK / 14 TEST → 01.1-GATE.md, 01.1-01-TEST-DESIGN.md, 01.1-CONTRATTO.md, 01.1-ONDE.md
- onda 3 chiusa: 01-06 (merge af705f3, bridge b50f307) + 01-05 (merge 4dccede, bridge 612200f); gate onda verde (fmt/clippy/test spikes, compose, proprieta-file, segreti 0, triplo guardrail 74 query ≥ 60) → 01-05/01-06-SUMMARY.md
- 01-06: GPT 91/91 (85 CONFIRMED, 6 PARTIALLY su complessità); ratifica utente ratifica-tutti, TODO-01 teneteli, TODO-02 regola invariata (6 fuori misure); derivatore = esecutore accettato · utente 2026-10-21 → ORACOLI.md
- 01-05: leak 0 su 9×10k cicli, deadline p99 2,1/1,8 ms; addebito JEV non determinabile (console vuota) · utente 2026-10-21; ADR-0010 Proposed → s05-cancel/RISULTATI.md
- deviazione b-segnalazione onda 3: spikes/common/src/hist.rs (file chiuso 01-01) Recorder satura 2047 µs (saturating_record senza resize) → gap-closure 1.2; usato da 01-04/07/10/11/12 → 01-05-SUMMARY.md
- fase 01.2 chiusa: hist.rs registra con record (auto-resize), difetto reale saturava a 2 µs; RED dc42c83, fix dc113c4, merge e693a85, bridge 8f1ea24, verify passed 7/7 → 01.2-VERIFICATION.md
- verify 01.1: passed 7/7; contratto onorato; WIRING-MAP invariata (nessun seam); PLAT-07 non spuntato; STATE riportato a fase 1 dopo phase complete (che puntava a fase 2); goal ROADMAP 01.1 ancora segnaposto → 01.1-VERIFICATION.md
- onda 1 di 01.1 chiusa: 01.1-01 (fix 20c5e6f, merge 598022d, bridge 94f2753); test-write RED_EXPECTED; gate main verde (bash -n, compose config, proprieta-file conforme con riapertura, segreti 0, scanner ok) → 01.1-01-SUMMARY.md
- deviazioni livello a 01.1-01: id piano 01-1-01 (regex senza punto); commit unico RED+GREEN (hook su soli .txt); testo «down -v» riformulato per TEST-10; PLAN riga 114 senza «:» (falso positivo hook) → SUMMARY

## Limiti trovati

- 2026-10-19 · Docker Desktop 29.8.2 + Compose v5.6.0, WSL2, runtime nvidia; `.wslconfig` memory=20GB processors=16 swap=0 applicato (VM: 16 CPU, 20000 MiB, swap 0); GPU check container verde · impatto: VRAM libera ~4,3 GiB (desktop Windows ~1,8 GiB su 6) → considerarlo in 01-15 VRAM e gate GPU di 01-02 · rif. HANDOFF §7
- 2026-10-20 · API Claude irraggiungibile (ENOTFOUND, DNS) durante onda 2: esecutore 01-03 morto senza commit, test-write 01-04 morto con test non committati · impatto: ripresa con agenti nuovi da primo task non committato · rif. 01-ONDE.md onda 2
- 2026-10-20 · DATABASE_URL di sistema Windows (DB cloud Neon altro progetto) vince su --env-file nell'interpolazione compose di session.sh: gc_race (TRUNCATE) tentò 2× quel DB, nessuna connessione riuscita (DNS, poi TLS assente) · impatto: workaround in bench.sh 9341b43; difetto file chiuso 01-02 → gap-closure fase 1.1 prima onda 3 (decisione utente; utente toglie la variabile) · rif. .planning/tmp/rapporto-01-04.yaml → 01-04-SUMMARY
- 2026-10-20 · DNS intermittente: 2 schede attive (Fritz!Box 192.168.178.1 + hotspot 172.20.10.1); cargo fetch runner 35/35 falliti, 2° esecutore 01-04 morto ENOTFOUND · impatto: build runner offline con vendor host; riprese agenti; utente lascia rete com'è

## Gate & verdetti

| Piano | Gate sul PLAN | Review ECC | Verifier GSD |
| --- | --- | --- | --- |
| 01.2-01 | conforme 6/6 (1° giro) + 2 ritocchi post-gate (TEST-15/16, errata RED) → 01.2-GATE.md | verde: lint-stato verde; cargo fmt/clippy/test spikes verdi dopo `cargo clean -p common` (common 17/17); security HIGH «3 FAILED» = falso allarme da binario di test stale del RED (decaduto, confermato da rust-reviewer); LOW accettati: assert `max_us <= 2047` debole in TEST-15 (rejected==3 e n==4 già asseriti), tolleranza 0,1 % vicina a 1/1024 con valori scelti lontani dal caso peggiore, doc «anche u64::MAX» poggia su test upstream, `saturating_add` superfluo, commento storico s05-cancel/src/main.rs:280 (fuori perimetro D-03) | passed 7/7, 0 gap → 01.2-VERIFICATION.md |
| 01.1-01 | conforme 6/6 (1° giro) → 01.1-GATE.md | verde: Rust/web/e2e/eval n/a (no manifest, no retrieval); lint-stato verde; code-review 0 gravi, 2 LOW accettati (diagnostica ripetuta per dc(), chiavi con `.`/`-` non esportabili da bash); security 0 gravi, 3 LOW accettati (chiavi compose assenti dal file = RISK-14 già accettato; DC_ENV_KEEP interpolata in futuro = fragilità; COMPOSE_PROJECT_NAME/DOCKER_HOST host ancora passate, preesistente) → TODO | passed 7/7, 0 gap → 01.1-VERIFICATION.md |

## UAT

- 01.1 pendente · TEST-8 conteggio grep -F sentinelle letterali attestato solo da rapporti (driver cancellato per piano) → 01.1-VERIFICATION.md
- 01.1 pendente · RISK-14/15/16 LOW da confermare accettati (RISK-14 elenco reale vuoto) → 01.1-01-SUMMARY.md
- 01.2 pendente · RISK-12 memoria auto-resize ≤ ~450 KB/Recorder senza benchmark; RISK-13 commento s05 «2047» storico (01-05-SUMMARY annotato) → 01.2-VERIFICATION.md

## TODO

- hardware spike: i7-13700K 16C/24T, 32 GB RAM, RTX 3050 6 GB VRAM, Windows 11.
- Docker Desktop non installato: prerequisito execute 1 (passthrough GPU WSL2, .wslconfig) → HANDOFF §7.
- S-07 porte: Embedder copre embedding query; validate_query restituisce complessità; locator in contracts.
- DEPENDENCIES.md da aggiornare a esito spike (JEV pin, A-02..A-10) → 01-17.
- da confermare utente: soglie M10/M11 (01-18); lessico motivi quarantena + exit 3 driver (01-07); pin pip 24.3.1 / pip-tools 7.4.1 non verificati.
- test_design_3, _6 (07 v2, 13 v2) senza risposta su AgentBridge: obsoleti, sostituiti da _8, _9.
- D-27 Docling pagine-tabella: scelta dopo misura 01-08 → 01-CONTEXT Deferred Ideas.
- interfaces-and-concurrency.md (split Reranker/DecisionEngine) → Fondazione (ADR-0012).
- da decidere utente: hook pre-commit rifiuta commit di sole evidenze .txt/.csv (security-scope-empty rc=2) → commit RED separato impossibile (01-04, 01.1-01) → 01.1-01-SUMMARY.md#punti-aperti
- gate/verify su spikes/: target cache condivisa fra worktree può lasciare un binario di test stale (falsi rossi in verify 01.2) → partire da `cargo clean -p <crate toccato>` nei gate d'onda e in verify
- residuo security 01.1 (LOW, preesistente): session.sh passa a compose COMPOSE_PROJECT_NAME/COMPOSE_FILE/DOCKER_HOST/DOCKER_CONTEXT dell'host (fix: -p "$PROJECT" o -u) → candidato prossima gap-closure di 01-02

## Decisioni ratificate in corsa

- strategia provider-first + insiemi file chiusi (U-04) · utente · 2026-10-18 → PROJECT.md
- racconto didattico per slice · utente · 2026-10-18 → skills/execute/SKILL.md §bridge 3-bis

**Modifica:** Claude (Anthropic) · 2026-10-18 · creazione (preflight discuss 1)
**Modifica:** Claude (Anthropic) · 2026-10-18 · --discuss fase 01
**Modifica:** Claude (Anthropic) · 2026-10-19 · --plan fase 01
**Modifica:** Claude (Anthropic) · 2026-10-19 · --evento: Docker Desktop e GPU pronti
**Modifica:** Claude (Anthropic) · 2026-10-19 · --exec fase 01 onda 1
**Modifica:** Claude (Anthropic) · 2026-10-20 · --evento: interruzione di rete in onda 2
**Modifica:** Claude (Anthropic) · 2026-10-20 · --evento: DATABASE_URL host nel runner, DNS su due schede
**Modifica:** Claude (Anthropic) · 2026-10-21 · --exec fase 01 onda 2
**Modifica:** Claude (Anthropic) · 2026-10-21 · --plan fase 01.1
**Modifica:** Claude (Anthropic) · 2026-10-21 · --exec fase 01.1 onda 1
**Modifica:** Claude (Anthropic) · 2026-10-21 · --verify-ecc fase 01.1
**Modifica:** Claude (Anthropic) · 2026-10-21 · --verify fase 01.1
**Modifica:** Claude (Anthropic) · 2026-10-21 · --exec fase 01 onda 3
**Modifica:** Claude (Anthropic) · 2026-10-21 · --verify-ecc fase 01.2
**Modifica:** Claude (Anthropic) · 2026-10-21 · --verify fase 01.2
