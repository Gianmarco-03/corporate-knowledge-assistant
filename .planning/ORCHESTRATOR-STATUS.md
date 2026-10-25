# ORCHESTRATOR-STATUS — Corporate Knowledge Assistant

Stato persistente dell'orchestratore del loop GSD+ECC. Lo scrive solo `update-orchestrator-state`.
Regole di sanità: (1) indice, non archivio — una riga + puntatore; (2) mai fatti GSD (avanzamento piani/fasi sta in STATE.md/ROADMAP.md).

## Posizione

fase 01 / onda 8 / slice 01-14 / execute completato (bridge) · aggiornato: 2026-10-25

## Prossima azione

onda 9: preflight 01-15 (reranker GPU; verificare criticità e test design, worktree da main), test-write fresco, esecutore; poi 01-16 (critico, READ-GATE PLAN-010-16) e 01-17.

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
- fase 04 criticità critica, ratificata utente 2026-10-21 → 04-CONTEXT.md
- 04 split S-07 Reranker/DecisionEngine; baseline solo Reranker, guasto JEV = errore tipizzato; adapter solo segnali, soglie fase 8 (±0,1 ADR-0008); generate con Complexity, single-turn fino Q-05; structured output validato lato client (D-01..D-06) · fonte Area A/B/D/E
- 04 breaker a mano, uno per provider, solo guasti provider; semaforo globale + 16 per query (ADR-0008); retry JEV 429/529 (D-07..D-10) · fonte Area C
- 04 snapshot OpenAPI JEV sha256 ee599512…; forma A rank + guardia ACL (ADR-0008); inference_geo global, nessun dato utente (D-11..D-13) · fonte Area B/D
- 04 AuditSink PG append-only per servizio (identity/document/rag), evento tipizzato, IdempotencyConflict; hash chain contro raccomandazione, testa FOR UPDATE, no ancoraggio, verify_chain → 11 (D-14..D-19) · fonte Area F
- 04 NoOpCache + fake con scope (fase 10); replay wiremock CI + gate locale 0,50 USD; no-leak a sentinelle, OTel senza contenuti (D-20..D-23) · fonte Area G/H/I
- 01-12 v2 ripasso D-31: Sonnet solo complex/no_answer, fallback 15 fallback_ids, tetto 2 USD (locale portato 5→2); routing = Haiku ≥ Sonnet−0,05 su complex + Haiku ≥ 0,85 su simple, altrimenti utente; gate giro 9 verdi; test design Claude RISK-13/14, TEST-20..23; anticipato da onda 7 · utente 2026-10-22 → 01-CONTEXT D-31, 01-GATE.md, bdc3d8f

- onda 4-bis chiusa: 01-12 (merge 6d1f090, bridge d8b6227), gate onda verde (fmt/clippy/35 test s06-llm, proprieta-file, scanner); regola D-31 → utente, D-32 misura Sonnet simple 1,00 vs Haiku 0,72, D-33 Sonnet su tutto (emenda D-18); spesa 0,598/2 USD; p95 Haiku 3,744 s > 3 s dichiarata → 01-12-SUMMARY.md, ADR-0011
- deviazioni 01-12: b-segnalazione hw-manifest.sh (01-02) vs common::manifest (01-01) già nota, normalizzazione come 01-04/05/11; da verify: format! per URL http nei test (anthropic.rs:651, jev.rs:466), loopback per prefisso con userinfo, unauthorized_in_prompt 0 per costruzione (garanzia vera TEST-1) → 01-12-SUMMARY.md
- 01-07 v6 (7ce2183) e 01-18 v5 (9f3d78d) APPROVED_WITH_WARNINGS (risk_audit_24/25, MEDIUM prenotazione memoria non rigida → prova OOM 01-18); 4° giro audit 01-07 autorizzato utente → 01-07-RISK-AUDIT-v6.md, 01-18-RISK-AUDIT-v5.md
- onda 5 chiusa: 01-18 (merge 3fb03bc, bridge 3815b5e) + 01-08 (merge a1241d4, bridge ce6584a); gate runner -p s02-parser 180 + host 96 (--workspace runner impossibile: registry cargo incompleto) → 01-18/01-08-SUMMARY.md
- 01-18: M8 24/26 (D-34), M9 conforme con riserva, M10 2 job parse, M11 OOM figlio; driver 01-07 → 1.4 (D-35) → 01-18-SUMMARY.md
- 01-08: soglie E-1..E-9 quasi tutte fail (CER IT 21,58%, F1 celle Docling 0,580, GPU 2,05 s/p); D-36 D-27 chiusa opz. 4, punto aperto ADR ingestion (riavvio Docling ~19 s, precisione pagine-tabella 0,435) · utente 2026-10-22 → 01-CONTEXT D-36
- D-37 falso positivo prescan image_filter_on_non_image (OTC_NSANY_2004) → 1.4 allargata, piano 01.4-02 (gate 1 giro, test design Claude 16 RISK/17 TEST) · utente 2026-10-22 → 01.4-CONTEXT D-07..D-10, 01.4-GATE.md
- 01.4-01 chiuso: budget guardia Drop, overhead colonna status 120 righe, M9 2,4%, in-process c8 11/20 pdfium_format (merge 7c443f4, bridge b3fb3ac) → 01.4-01-SUMMARY.md
- onda 6 chiusa: 01-09 (merge 45083a6, bridge b0645bc; L-4 8/11 fail apostrofi pandoc, ADR-0005 multi-span Proposed) + 01-10 (merge 69af2c9, bridge f675e56); gate runner s02 257 + db s03 5/5 → 01-09/01-10-SUMMARY.md
- D-38 embedding bge-m3 (emenda D-14, nDCG@10 0,723 vs Qwen3 0,660) · utente 2026-10-23 → 01-CONTEXT D-38, ADR-0007 Proposed
- 01.4-02 T1 diagnosi: forma T (/Thumb DCT, ColorSpace indiretto) uguale su 4 file (bf0ecc1); test-write RED_EXPECTED 5c07320, TEST-8 non scritto (P2, rischio residuo accettato in 01.4-GATE)
- difetti fuori perimetro registrati per verify: isolate.rs:534 (01-07) results_raw mancante → detail «symlink» fuorviante; anomalia ORACOLI.md:39 vs queries.json (H1 nel section_path) per 01-06

- verify 1.4: passed 16/16, contratto onorato (aggiornato in verify con 01.4-02, mancava), WIRING-MAP invariata; phase complete spostava STATE a fase 2 → riportato a fase 1 (13/18); REQUIREMENTS: solo riga vuota, ripristinato → 01.4-VERIFICATION.md

- verify 1.5: passed 11/11, contratto onorato (2 formulazioni corrette in verify), WIRING-MAP invariata; io::copy giudicato equivalente dalla security review, commento 5239f0e; STATE riportato a fase 1 dopo phase complete → 01.5-VERIFICATION.md
- onda 7 chiusa: 01-13 (merge d13e23a, bridge); gate main fmt/clippy ok, runner s02-parser 380 pass 0 fail 8 ign al rilancio (1° giro 3 rossi temporizzazione isolation.rs, flaky sotto carico, file non toccati); decisioni utente lock Paddle cooldown dbb615b, apt fissati 24654ca, PP-OCRv6 1 tentativo detection ridotta non vince → Tesseract in ADR-0004 (Proposed, ratifica 01-17); deviazioni a: test1 uv 307de01, manifest pesi sha 64; difetto file chiuso parse-parent.rs calibrate BudgetExceeded → gap-closure riapre [1]; T-09 jbig2 aperto → 01-13-SUMMARY.md
- D-41 scanner upload-in-memory opzione B corretta su deepsearch_7 (take dopo decoder, ambigui «da verificare») + D-19 fase 3 lettore limitato → 01-CONTEXT.md, 01-DEEPSEARCH-7.md
- onda 8 chiusa: 01-14 (merge 050eb8f, bridge); gate main fmt/clippy ok, s03-pgvector host 28 pass, scanner 0/118; integrazione Postgres solo nel runner dell'esecutore (40 verdi) → human_judgment; leak 0/2.067, soglia esatto→HNSW 50k, I-2 recall ≥0,95 solo 10k, ibrida non adottata, ADR-0006 Proposed; deviazioni a: maintenance_work_mem 768MB (misure HNSW con modifica non committata, git_rev 22d6d5f), query HNSW EXISTS OFFSET 0; testi RISULTATI/ADR corretti al bridge; aperto 23,7 vs 27,7 ms → 01-14-SUMMARY.md
- pulizia disco 2026-10-25 (utente ok): build cache 23,1 GB, 419 dir s02-* residue test 9,6 GB, 8 modelli VLM docling ~27 GB; vhdx da compattare a mano (admin); TODO verify: test s02 senza pulizia tmp, compose.yaml:175 docling --all, results/docling-models.txt elenca modelli rimossi

## Limiti trovati

- 2026-10-19 · Docker Desktop 29.8.2 + Compose v5.6.0, WSL2, runtime nvidia; `.wslconfig` memory=20GB processors=16 swap=0 applicato (VM: 16 CPU, 20000 MiB, swap 0); GPU check container verde · impatto: VRAM libera ~4,3 GiB (desktop Windows ~1,8 GiB su 6) → considerarlo in 01-15 VRAM e gate GPU di 01-02 · rif. HANDOFF §7
- 2026-10-20 · API Claude irraggiungibile (ENOTFOUND, DNS) durante onda 2: esecutore 01-03 morto senza commit, test-write 01-04 morto con test non committati · impatto: ripresa con agenti nuovi da primo task non committato · rif. 01-ONDE.md onda 2
- 2026-10-20 · DATABASE_URL di sistema Windows (DB cloud Neon altro progetto) vince su --env-file nell'interpolazione compose di session.sh: gc_race (TRUNCATE) tentò 2× quel DB, nessuna connessione riuscita (DNS, poi TLS assente) · impatto: workaround in bench.sh 9341b43; difetto file chiuso 01-02 → gap-closure fase 1.1 prima onda 3 (decisione utente; utente toglie la variabile) · rif. .planning/tmp/rapporto-01-04.yaml → 01-04-SUMMARY
- 2026-10-20 · DNS intermittente: 2 schede attive (Fritz!Box 192.168.178.1 + hotspot 172.20.10.1); cargo fetch runner 35/35 falliti, 2° esecutore 01-04 morto ENOTFOUND · impatto: build runner offline con vendor host; riprese agenti; utente lascia rete com'è
- 2026-10-21 · 01-07 fermo su 3 blocchi esterni al piano: B1 scanner MEDIUM bloccante (pdf-untrusted-parse, archive-decompress) → hook --fail-on HIGH 4441aef; B2 tmpfs /work root 755 nel compose.yaml chiuso (worker uid 10001) → gap-closure 1.3; B3 soglie prescan rifiutano 8/8 PDF benigni (object stream conteggio×64 MiB, /Encrypt solo permessi, allegato) + B4 formula su VmPeak → ripasso plan 01-07 · utente 2026-10-21 · impatto: 01-11 fuso prima di 01-07; lavoro 01-07 Task 3 in worktree + patch in TEMP/cka-0107-wip · rif. .planning/tmp/rapporto-01-07.yaml
- 2026-10-22 · rate limit sessione (429) a metà di 01-09 e 01-10 · ripresi via SendMessage, nessun lavoro perso · rif. rapporto-01-09/01-10
- 2026-10-23 · sessione chiusa con integratore onda 6 e 01.4-02 T1 a metà; Docker Desktop spento · ripresi, Docker riavviato dall'agente T1
- 2026-10-23 · volume cargo-target condiviso fra worktree: binario parse-parent vecchio (base senza fix) più recente dei sorgenti di main → 3 rossi falsi su drivers.rs; touch + rebuild → verde · impatto: ogni gate dopo merge va verificato con «Compiling s02-parser»
- 2026-10-24/25 · Docker Desktop spento dall'utente 2 volte durante TTQ 01-13 e una volta fra due giri paddle · serie TTQ invalidate in results/raw/invalid-ttq-docker-restart*, rilancio da capo; esecutore fermo, Docker riavviato dall'orchestratore su richiesta · rif. rapporto-01-13 (c)
- 2026-10-25 · gsd-tools roadmap update-plan-progress 01 13 scrive «Plans» nella sezione 01.5 (fase 1 ha «**Plans**: TBD») · hunk annullato con git apply -R, solo tabella aggiornata · rif. commit bridge 01-13

## Gate & verdetti

| Piano | Gate sul PLAN | Review ECC | Verifier GSD |
| --- | --- | --- | --- |
| 01.5-01 | conforme 6/6 (3 giri sul test design: D-09 JPX, D-10, residui a-c) → 01.5-GATE.md | verde: fmt/clippy host, runner -p s02-parser 333 pass (rebuild verificato), lint-stato, scanner --fail-on HIGH; rust-review Warning 0 gravi: MEDIUM un byte per campione (12/16 bit, JPX Ssiz>8) = RISK-14 accettato (dimensioni codificate ≤ dichiarate, RLIMIT_AS), LOW fail-closed accettati; security PASS con riserve: io::copy equivalente a read_to_end su Take (falso positivo lessicale upload-in-memory), deviazione non dichiarata → commento corretto 5239f0e e SUMMARY; follow-up scanner → TODO | passed 11/11, 0 gap; UAT 3/3 accettate → 01.5-VERIFICATION.md |
| 01.4-02 | conforme 6/6 (1° giro) → 01.4-GATE.md | verde: fmt/clippy host, runner -p s02-parser 269 pass (rebuild verificato), lint-stato, scanner --fail-on HIGH; rust-review APPROVE 0 gravi, LOW accettati (test overflow accetta 2 Kind; forma T non legata a /Thumb = RISK-7); security APPROVATO 0 gravi: forma T più stretta del ramo /Image; debito preesistente 01-07 tetto su /Width /Height non su SOF JPEG (RISK-16) → verify fase 1 | passed 16/16 (fase), 0 gap; UAT 4/4 accettate → 01.4-VERIFICATION.md |
| 01.4-01 | conforme 6/6 (1° giro) → 01.4-GATE.md | verde (stesso gate); rust-review APPROVE, LOW: status misto maiuscole/minuscole (error:Corrupt vs error:pdfium_*) e riepilogo stderr solo in-process → nota per consumatori 01-18; remove_file su run_tag preesistente teorico (run_tag univoco); security: Drop + valid_run_tag senza traversal | passed 16/16 (fase), 0 gap → 01.4-VERIFICATION.md |
| 01.3-01 | conforme 6/6 (1° giro) → 01.3-GATE.md | verde: compose config, fmt/clippy/test spikes, lint-stato; security 0 gravi (0700+10001 restringe, isolamento invariato, evidenza pulita, noexec coerente con 01-07); code review non applicabile (1 riga YAML + commento) | passed 7/7, 0 gap → 01.3-VERIFICATION.md |
| 01.2-01 | conforme 6/6 (1° giro) + 2 ritocchi post-gate (TEST-15/16, errata RED) → 01.2-GATE.md | verde: lint-stato verde; cargo fmt/clippy/test spikes verdi dopo `cargo clean -p common` (common 17/17); security HIGH «3 FAILED» = falso allarme da binario di test stale del RED (decaduto, confermato da rust-reviewer); LOW accettati: assert `max_us <= 2047` debole in TEST-15 (rejected==3 e n==4 già asseriti), tolleranza 0,1 % vicina a 1/1024 con valori scelti lontani dal caso peggiore, doc «anche u64::MAX» poggia su test upstream, `saturating_add` superfluo, commento storico s05-cancel/src/main.rs:280 (fuori perimetro D-03) | passed 7/7, 0 gap → 01.2-VERIFICATION.md |
| 01.1-01 | conforme 6/6 (1° giro) → 01.1-GATE.md | verde: Rust/web/e2e/eval n/a (no manifest, no retrieval); lint-stato verde; code-review 0 gravi, 2 LOW accettati (diagnostica ripetuta per dc(), chiavi con `.`/`-` non esportabili da bash); security 0 gravi, 3 LOW accettati (chiavi compose assenti dal file = RISK-14 già accettato; DC_ENV_KEEP interpolata in futuro = fragilità; COMPOSE_PROJECT_NAME/DOCKER_HOST host ancora passate, preesistente) → TODO | passed 7/7, 0 gap → 01.1-VERIFICATION.md |

## UAT

- 01.5 accettata · utente 2026-10-23 · JPX solo sintetico (RISK-5), immagini inline debito fase 3/ADR-0004 (D-09), residui RISK-14/17 e T-01.5-09 accettati → 01.5-VERIFICATION.md
- 01.4 accettata · utente 2026-10-23 · gate runner come prova 01.4-01, TEST-8 non scritto (RISK-7), docker version fuori session.sh: accettati; exit 0 di overhead con errori in status confermato (D-02); RISK-16 → verify fase 1 / 01-13 → 01.4-VERIFICATION.md
- 01.1 accettata · utente 2026-10-21 · TEST-8: letterale non ripetibile (sentinelle generate dal driver cancellato), controlli di forma su session-env-precedence.txt tutti 0 → 01.1-VERIFICATION.md
- 01.1 accettata · utente 2026-10-21 · RISK-14 vuoto (manca solo CKA_RUN_ID, impostato da dc()); RISK-15 0 collisioni; RISK-16 0 nomi non ASCII/non validi → 01.1-01-SUMMARY.md
- 01.2 accettata · utente 2026-10-21 · RISK-12 memoria auto-resize ≤ ~704 KiB allocati/Recorder (≈3,4 MiB caso peggiore con 5 Recorder, ≈880 KiB a 60 s), calcolo da sorgente, misura diretta fallita (LNK1104); RISK-13 commento s05 accettato, correzione alla prossima riapertura fase 1 → 01.2-01-SUMMARY.md
- 01.3 accettata · utente 2026-10-21 · RISK-15 noexec/nosuid/nodev del tmpfs non misurato (default Docker, passa a 01-07); RISK-13 uid scollegato da user:, RISK-14 tmpfs nel cgroup: accettati → 01.3-VERIFICATION.md

## TODO

- scanner security_scan.py: regola upload-in-memory segnala read_to_end anche su reader limitato da .take( (falso positivo, fase 1.5) → ignorare con take o waiver motivato · da proporre all'utente
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
- 04: ROADMAP criterio 1 riformulato (3 op DecisionEngine + Reranker::rank, baseline solo Reranker) + tracciabilità (Reranker→8,13; verify_chain→11; fake Cache→10) via gsd-tools → 04-CONTEXT.md §TODO
- 04: brief execute 01-16 ← D-03, D-04, D-06, D-15, D-16, D-19; brief 01-12 ← fixture replay (01-11 già fuso: fixture da recuperare dagli artefatti) → 04-CONTEXT.md §TODO
- 04: discuss 8 (policy guasto JEV, soglie, Q-05), 11 (forma AuditSink, query nell'audit, verify_chain), 13 (Q-07 processo) ricevono vincoli da 04
- 04: plan 2 → slot AuditSink + fascia 04 anche in identity e document-service; threat model ← D-13, D-18, D-20
- 04: rischio pipeline: ADR 0009, 0011, 0012 non emessi → D-02, D-04, D-06 provvisorie fino a fine Spike
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
**Modifica:** Claude (Anthropic) · 2026-10-21 · --verify-ecc fase 01.3
**Modifica:** Claude (Anthropic) · 2026-10-21 · --verify fase 01.3
**Modifica:** Claude (Anthropic) · 2026-10-21 · --uat fase 01.3 accettata
**Modifica:** Claude (Anthropic) · 2026-10-23 · --exec: onde 5 e 6 chiuse, 01.4-01 chiuso, D-36..D-38, limiti (rate limit, cache cargo-target)
**Modifica:** Claude (Anthropic) · 2026-10-23 · --verify-ecc: fase 1.4
**Modifica:** Claude (Anthropic) · 2026-10-23 · --verify e --uat: fase 1.4 chiusa
**Modifica:** Claude (Anthropic) · 2026-10-23 · --verify-ecc: fase 1.5
**Modifica:** Claude (Anthropic) · 2026-10-24 · --verify e --uat: fase 1.5 chiusa
