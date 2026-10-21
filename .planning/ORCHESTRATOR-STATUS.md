# ORCHESTRATOR-STATUS — Corporate Knowledge Assistant

Stato persistente dell'orchestratore del loop GSD+ECC. Lo scrive solo `update-orchestrator-state`.
Regole di sanità: (1) indice, non archivio — una riga + puntatore; (2) mai fatti GSD (avanzamento piani/fasi sta in STATE.md/ROADMAP.md).

## Posizione

fase 03 / — / — / discuss completato (traccia discuss; traccia Spike: fase 01 onda 4 parziale, 01-11 fuso, 01-07 fermo su 3 blocchi) · aggiornato: 2026-10-21

## Prossima azione

Traccia Spike: gap-closure 1.3 (compose.yaml: tmpfs /work uid 10001; commento s05-cancel), poi ripasso /plan di 01-07 (soglie prescan nuove, audit+test design GPT), poi ripresa execute 01-07 dal Task 3 (patch in TEMP/cka-0107-wip); bridge 01-11 in corso; traccia discuss: /discuss 4 (plan 2/3 solo dopo discuss 4-15 e fine Spike, ROADMAP «Strategia di consegna»).

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
- fase 02 criticità normale, ratificata utente 2026-10-21 → 02-CONTEXT.md
- 02 contract-first: YAML per servizio, trait Contract (from_json+validate), classi a mano, nessun generatore; oasdiff + Schemathesis --mode all + unit (D-01..D-04) · fonte Area A → 02-CONTEXT.md
- 02 errori problem+json RFC 9457 + code/message/request_id; 503 saturazione, 429 rate limit (D-05/06) · fonte Area B
- 02 schema per servizio (contro raccomandazione), search_path per ruolo, test 42501, cataloghi visibili accettati (D-07/08) · fonte Area C
- 02 migrazioni PPNNNN_nome.sql + gate (sqlx: versione prima del primo _, fuori ordine silenzioso) (D-09) · fonte Area C
- 02 primitive idempotency/outbox/lease alle fasi proprietarie, nessun seam (contro raccomandazione, allineato 01-CONTRATTO §6) (D-10)
- 02 stack runtime Deciso 2026-10-21, reqwest-tracing escluso, crate config, distroless debian13, Caddy unico origin, /internal bloccate, Garage HTTP rete interna (D-11..D-18) · fonte Area B/D/E
- 02 OTLP opzionale + Jaeger v2 override (→ fase 14), redazione default, /livez /readyz; contracts: trasversali ora, dominio vuoti (D-19..D-22)
- 02 frontend React Router 8 data mode, TS 7 se lint regge, Tailwind 4 + shadcn, registro route esplicito (D-23..D-25) · fonte Area D
- fase 03 criticità critica, ratificata utente 2026-10-21 → 03-CONTEXT.md
- 03 firme S-07 base + search(SearchQuery{text, embedding}), DocumentStore::delete (consumer GC fase 6), parse su file locale; variazioni → brief execute 01-16 (D-01..D-05) · fonte Area A
- 03 predicato = insieme etichette (agnostico Q-02, vuoto = zero); proiezione document_acl fase 3 + set_acl (6,7,12) + versione ACL per documento (D-06..D-08) · fonte Area B (Azure security filter, pgvector)
- 03 publish_version una tx + puntatore versione corrente (D-09) · fonte Area C (PG Read Committed)
- 03 locator multi-span max un confine, PDF e testo, contro raccomandazione; emenda D-10 fase 1 (Storia scritta) (D-10) · decisione utente
- 03 chunk in caratteri + TEI rifiuta (no troncamento); Docling in fase 3 condizionato ADR-0004/D-27 (D-11, D-12) · fonte Area D/E
- 03 SSRF in-process (resolver IP globali, redirect controllati, https, tetti) + rete Compose dedicata; no Smokescreen (D-13) · fonte Area H (OWASP)
- 03 contract suite unica reale/fake; CI PG+Garage+PDFium/Tesseract+TEI CPU, gate locale GPU (TEI Qwen3, Docling) in verify; testkit mai in release; fake con copione guasti; fixture ostili copiate con manifest (D-14..D-18) · fonte Area I

## Limiti trovati

- 2026-10-19 · Docker Desktop 29.8.2 + Compose v5.6.0, WSL2, runtime nvidia; `.wslconfig` memory=20GB processors=16 swap=0 applicato (VM: 16 CPU, 20000 MiB, swap 0); GPU check container verde · impatto: VRAM libera ~4,3 GiB (desktop Windows ~1,8 GiB su 6) → considerarlo in 01-15 VRAM e gate GPU di 01-02 · rif. HANDOFF §7
- 2026-10-20 · API Claude irraggiungibile (ENOTFOUND, DNS) durante onda 2: esecutore 01-03 morto senza commit, test-write 01-04 morto con test non committati · impatto: ripresa con agenti nuovi da primo task non committato · rif. 01-ONDE.md onda 2
- 2026-10-20 · DATABASE_URL di sistema Windows (DB cloud Neon altro progetto) vince su --env-file nell'interpolazione compose di session.sh: gc_race (TRUNCATE) tentò 2× quel DB, nessuna connessione riuscita (DNS, poi TLS assente) · impatto: workaround in bench.sh 9341b43; difetto file chiuso 01-02 → gap-closure fase 1.1 prima onda 3 (decisione utente; utente toglie la variabile) · rif. .planning/tmp/rapporto-01-04.yaml → 01-04-SUMMARY
- 2026-10-20 · DNS intermittente: 2 schede attive (Fritz!Box 192.168.178.1 + hotspot 172.20.10.1); cargo fetch runner 35/35 falliti, 2° esecutore 01-04 morto ENOTFOUND · impatto: build runner offline con vendor host; riprese agenti; utente lascia rete com'è
- 2026-10-21 · 01-07 fermo su 3 blocchi esterni al piano: B1 scanner MEDIUM bloccante (pdf-untrusted-parse, archive-decompress) → hook --fail-on HIGH 4441aef; B2 tmpfs /work root 755 nel compose.yaml chiuso (worker uid 10001) → gap-closure 1.3; B3 soglie prescan rifiutano 8/8 PDF benigni (object stream conteggio×64 MiB, /Encrypt solo permessi, allegato) + B4 formula su VmPeak → ripasso plan 01-07 · utente 2026-10-21 · impatto: 01-11 fuso prima di 01-07; lavoro 01-07 Task 3 in worktree + patch in TEMP/cka-0107-wip · rif. .planning/tmp/rapporto-01-07.yaml

## Gate & verdetti

| Piano | Gate sul PLAN | Review ECC | Verifier GSD |
| --- | --- | --- | --- |
| 01.2-01 | conforme 6/6 (1° giro) + 2 ritocchi post-gate (TEST-15/16, errata RED) → 01.2-GATE.md | verde: lint-stato verde; cargo fmt/clippy/test spikes verdi dopo `cargo clean -p common` (common 17/17); security HIGH «3 FAILED» = falso allarme da binario di test stale del RED (decaduto, confermato da rust-reviewer); LOW accettati: assert `max_us <= 2047` debole in TEST-15 (rejected==3 e n==4 già asseriti), tolleranza 0,1 % vicina a 1/1024 con valori scelti lontani dal caso peggiore, doc «anche u64::MAX» poggia su test upstream, `saturating_add` superfluo, commento storico s05-cancel/src/main.rs:280 (fuori perimetro D-03) | passed 7/7, 0 gap → 01.2-VERIFICATION.md |
| 01.1-01 | conforme 6/6 (1° giro) → 01.1-GATE.md | verde: Rust/web/e2e/eval n/a (no manifest, no retrieval); lint-stato verde; code-review 0 gravi, 2 LOW accettati (diagnostica ripetuta per dc(), chiavi con `.`/`-` non esportabili da bash); security 0 gravi, 3 LOW accettati (chiavi compose assenti dal file = RISK-14 già accettato; DC_ENV_KEEP interpolata in futuro = fragilità; COMPOSE_PROJECT_NAME/DOCKER_HOST host ancora passate, preesistente) → TODO | passed 7/7, 0 gap → 01.1-VERIFICATION.md |

## UAT

- 01.1 accettata · utente 2026-10-21 · TEST-8: letterale non ripetibile (sentinelle generate dal driver cancellato), controlli di forma su session-env-precedence.txt tutti 0 → 01.1-VERIFICATION.md
- 01.1 accettata · utente 2026-10-21 · RISK-14 vuoto (manca solo CKA_RUN_ID, impostato da dc()); RISK-15 0 collisioni; RISK-16 0 nomi non ASCII/non validi → 01.1-01-SUMMARY.md
- 01.2 accettata · utente 2026-10-21 · RISK-12 memoria auto-resize ≤ ~704 KiB allocati/Recorder (≈3,4 MiB caso peggiore con 5 Recorder, ≈880 KiB a 60 s), calcolo da sorgente, misura diretta fallita (LNK1104); RISK-13 commento s05 accettato, correzione alla prossima riapertura fase 1 → 01.2-01-SUMMARY.md

## TODO

- hardware spike: i7-13700K 16C/24T, 32 GB RAM, RTX 3050 6 GB VRAM, Windows 11.
- Docker Desktop non installato: prerequisito execute 1 (passthrough GPU WSL2, .wslconfig) → HANDOFF §7.
- S-07 porte: Embedder copre embedding query; validate_query restituisce complessità; locator in contracts.
- DEPENDENCIES.md da aggiornare a esito spike (JEV pin, A-02..A-10) → 01-17.
- da confermare utente: soglie M10/M11 (01-18); lessico motivi quarantena + exit 3 driver (01-07); pin pip 24.3.1 / pip-tools 7.4.1 non verificati.
- test_design_3, _6 (07 v2, 13 v2) senza risposta su AgentBridge: obsoleti, sostituiti da _8, _9.
- D-27 Docling pagine-tabella: scelta dopo misura 01-08 → 01-CONTEXT Deferred Ideas.
- interfaces-and-concurrency.md (split Reranker/DecisionEngine) → Fondazione (ADR-0012); + 503/429 (02 D-06) → 02-CONTEXT TODO.
- 02: DEPENDENCIES.md Proposte→Decise + contract-first, React Router 8, Caddy, Jaeger, Schemathesis/oasdiff, con Storia → plan 02.
- 02: ROADMAP/STATE «primitive raccomandate in Fondazione» superata da D-10 → annotare via gsd-tools.
- 02: IdempotencyKey in contracts o interno per servizio? assunzione provvisoria file vuoto → fase 6; decide wiring-design.
- 02: rischio pipeline: D-18/D-22 provvisorie fino a ADR-0003 ratificato e ADR-0012 emesso (01-16/01-17).
- 02: tag Jaeger v2 / Garage / cargo-chef e TS 7 + typescript-eslint da verificare in plan.
- gate/verify su spikes/: target cache condivisa fra worktree può lasciare un binario di test stale (falsi rossi in verify 01.2) → partire da `cargo clean -p <crate toccato>` nei gate d'onda e in verify
- 03: brief execute 01-16 ← D-02, D-03, D-04, D-06, D-07, D-08, D-10; brief execute 01-09 ← D-10 (ADR-0005 multi-span) → 03-CONTEXT.md §TODO
- 03: ROADMAP tabella tracciabilità: delete→6, set_acl→6,7,12, embed_batch→embed_documents/embed_query (gsd-tools, riconciliazione o plan 3)
- 03: tombstone DELETE /api/v1/documents/{id} senza fase proprietaria → assegnare in discuss 6 o 12; poi VectorIndex::withdraw in riconciliazione
- 03: discuss 6 Q-02 deve produrre insiemi di etichette (D-06); discuss 12 prima fonte entro D-13
- 03: threat model SSRF connettore + DNS rebinding (01-17 o plan 3)
- 03: rischio pipeline: ADR 0004-0007, 0012 non emessi → D-01..D-04, D-10..D-12 provvisorie fino a fine Spike
- difetto commento spikes/s05-cancel/src/main.rs:279-281 (file chiuso 01-05): dice «satura a 2047 µs» al presente, reale era 2 µs e oggi corretto → correggere nella prossima gap-closure che riapre la fase 1 · utente 2026-10-21
- residuo security 01.1 (LOW, preesistente): session.sh passa a compose COMPOSE_PROJECT_NAME/COMPOSE_FILE/DOCKER_HOST/DOCKER_CONTEXT dell'host (fix: -p "$PROJECT" o -u) → candidato prossima gap-closure di 01-02

## Decisioni ratificate in corsa

- strategia provider-first + insiemi file chiusi (U-04) · utente · 2026-10-18 → PROJECT.md
- racconto didattico per slice · utente · 2026-10-18 → skills/execute/SKILL.md §bridge 3-bis
- hook pre-commit invariato: evidenze solo .txt/.csv → RED nel commit della correzione, sezione distinta · utente · 2026-10-21 → 01.1-01-SUMMARY.md#punti-aperti

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
**Modifica:** Claude (Anthropic) · 2026-10-21 · --evento: decisione utente sull'hook pre-commit
**Modifica:** Claude (Anthropic) · 2026-10-21 · --discuss fase 02
**Modifica:** Claude (Anthropic) · 2026-10-21 · --discuss fase 03
**Modifica:** Claude (Anthropic) · 2026-10-21 · --uat fasi 01.1 e 01.2 accettate
**Modifica:** Claude (Anthropic) · 2026-10-21 · --evento: 01-07 fermo su tre blocchi, decisioni utente
