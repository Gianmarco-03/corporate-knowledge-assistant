# Indice delle decisioni

Una riga per decisione, con puntatore all'artefatto che porta il rationale. Le stesse righe stanno in `STATE.md` (scritte con `gsd-tools state add-decision`).

## Fase 1 — Spike tecnici

- 2026-10-19 · Spesa Anthropic di 01-12 dimezzata (Sonnet solo su complex/no_answer, fallback 15 query, tetto 2 USD); ripasso di plan prima dell'onda 7 → `phases/01-spike-tecnici/01-02-SUMMARY.md#punti-aperti`
- 2026-10-19 · Pesi Docling tenuti con --all (28,6 GB); chiavi provider inserite; gate GPU verde → `phases/01-spike-tecnici/01-02-SUMMARY.md#task-4-checkpoint-umano`
- 2026-10-21 · Fonte IT unica Banca d'Italia; niente SEC EDGAR (UA con recapito personale), 10-K dal PDF Berkshire → `phases/01-spike-tecnici/01-03-SUMMARY.md#decisions-made`
- 2026-10-21 · Manifest del corpus immutabile: fetch.sh e build.sh non riscrivono gli hash; sostituzione = voce nuova con motivo → `phases/01-spike-tecnici/01-03-SUMMARY.md#decisions-made`
- 2026-10-21 · Trappola «Progetto Cormorano» al posto di «Aurora», che collide con un Project Aurora reale nel BIS AER 2025 → `phases/01-spike-tecnici/01-03-SUMMARY.md#decisions-made`
- 2026-10-21 · Garage dietro DocumentStore solo con parti da 2 MiB e 1 in volo: il default di object_store supera la soglia VmHWM → `../spikes/s01-upload/RISULTATI.md#decisione-proposta`
- 2026-10-21 · PG bytea scartato per gli originali: canale laterale della deduplica 0/3 anche con ON CONFLICT DO UPDATE → `../docs/adr/0003-storage-originali-garage.md#decisione`
- 2026-10-21 · PUT idempotente con HEAD proposto per i 500 di Garage sulla dedup concorrente, non misurato: da provare nell'adapter → `../spikes/s01-upload/RISULTATI.md#decisione-proposta`
- 2026-10-21 · GC orfani con N = 24 h e protocollo live|deleting + pending, provato sulle sole interleaving enumerate → `../docs/adr/0003-storage-originali-garage.md#conseguenze`
- 2026-10-21 · Mitigazione del canale laterale su Garage spostata da parti 5 a 2 MiB dopo la misura di memoria, prima dei tempi → `../spikes/s01-upload/RISULTATI.md#anomalie-e-deviazioni`
- 2026-10-21 · Parti da 2 MiB sotto il minimo S3 di 5 MiB: portabilità non verificata, da valutare in 01-16/01-17 → `phases/01-spike-tecnici/01-04-SUMMARY.md#punti-aperti`
- 2026-10-21 · DATABASE_URL di sistema (altro progetto) eliminata dall'utente; difetto di session.sh (env host vince) → gap-closure fase 1.1 prima dell'onda 3 → `phases/01-spike-tecnici/01-04-SUMMARY.md#punti-aperti`
- 2026-10-21 · Utente 2026-10-21: ratifica-tutti sugli 85 oracoli CONFIRMED (74 query, 11 parser nelle misure) → `phases/01-spike-tecnici/01-06-SUMMARY.md#decisions-made`
- 2026-10-21 · Derivatore degli oracoli = esecutore del piano: deviazione accettata dall'utente, TEST-10 non verde → `../spikes/data/corpus/oracles/ORACOLI.md#da-dove-vengono-i-valori`
- 2026-10-21 · TODO-01 P-006: «teneteli», restano riquadro WEB e didascalia nell'oracolo → `../spikes/data/corpus/oracles/ORACOLI.md#to-do-di-dominio`
- 2026-10-21 · TODO-02: resta la regola di complessità; Q-008/010/012/040/048/056 fuori dalle misure (GPT non CONFIRMED), reinviabili → `../spikes/data/corpus/oracles/ORACOLI.md#to-do-di-dominio`
- 2026-10-21 · Deadline globale tokio::time::Instant esplicita su ogni porta verso provider; cancellazione = drop del future → `../docs/adr/0010-deadline-cancellazione-http.md#decisione`
- 2026-10-21 · Fan-out: variante A timeout_at+buffer_unordered di default, B JoinSet+Semaphore solo per task indipendenti; HTTP/2 preferito → `../docs/adr/0010-deadline-cancellazione-http.md#decisione`
- 2026-10-21 · Addebito JEV dopo il drop non determinabile (console vuota 2026-10-21): cancellate contate come addebitate, lettura da ripetere → `../spikes/s05-cancel/RISULTATI.md#esito`
- 2026-10-21 · Difetto hist.rs (01-01): Recorder satura a 2047 µs; gap-closure riapre_perimetro [1], verificare s01-upload → `phases/01-spike-tecnici/01-05-SUMMARY.md#deviations-from-plan`
- 2026-10-21 · Hook pre-commit blocca solo CRITICAL/HIGH (--fail-on HIGH); i MEDIUM sono avvisi da motivare nel SUMMARY; gate di plan/verify invariati → `../skills/security-scanner/SKILL.md`
- 2026-10-21 · Gap-closure 1.3: tmpfs /work del worker scrivibile da uid 10001 in compose.yaml (file chiuso 01-02) + commento s05-cancel → `../.planning/ORCHESTRATOR-STATUS.md#limiti-trovati`
- 2026-10-21 · Ripasso di plan 01-07: object stream su somma reale decompressa ≤512 MiB, PDF con password utente vuota ammessi, allegati ignorati e registrati, memoria su VmHWM → `../.planning/ORCHESTRATOR-STATUS.md#limiti-trovati`
- 2026-10-21 · Forma A per il rank JEV: nDCG@5 1,0 contro 0,729 (C) e 0,679 (B), p95 606 ms ≤ 2.000 → `../spikes/s04-jev/RISULTATI-jev.md#esito`
- 2026-10-21 · Concorrenza JEV 16 (massimo misurato con 0 risposte 429, non limite trovato); retry unico su 429/529 entro la deadline → `../docs/adr/0008-jev-forma-rank-concorrenza-pin.md#decisione`
- 2026-10-21 · Pin jev-1.13.0 da JEV_MODEL senza default; jev-latest mobile, jev-preview già 1.14.0 → `../docs/adr/0008-jev-forma-rank-concorrenza-pin.md#decisione`
- 2026-10-21 · Varianza Noul oltre soglia per 1 candidato su 600 (std 0,0954): margine di calibrazione ≥ ±0,1 attorno al taglio → `../spikes/s04-jev/RISULTATI-jev.md#esito`
- 2026-10-21 · 01-11 fuso prima di 01-07 contro 01-ONDE (decisione utente: 01-07 fermo su tre blocchi esterni) → `phases/01-spike-tecnici/01-11-SUMMARY.md#integrazione-fuori-ordine`
- 2026-10-21 · instructs_model misurato solo sui falsi positivi (0/1.800): il rilevamento sul testo ostile va a 01-12 o a un oracolo nuovo → `phases/01-spike-tecnici/01-11-SUMMARY.md#issues-encountered`

- 2026-10-22 · Sonnet 5.5 anche sulle query simple (D-33): regola D-31 fallita, misura D-32 Sonnet 1,00 contro Haiku 0,72 → `phases/01-spike-tecnici/01-12-SUMMARY.md#decisions-made`

- 2026-10-22 · Prescan v6 (cifrati a password vuota ammessi, allegati contati, object stream sulla somma reale) e prenotazione 3×VmHWM: 10/10 PDF benigni, 2 job parse nel budget → `phases/01-spike-tecnici/01-07-SUMMARY.md#decisions-made`
- 2026-10-22 · M8 di 01-18 dichiarata non raggiunta 24/26 (D-34, JBIG2 a 01-13); driver budget/overhead di 01-07 alla gap-closure 1.4 (D-35) → `phases/01-spike-tecnici/01-18-SUMMARY.md#decisions-made`
- 2026-10-22 · D-27 chiusa con l'opzione 4 (D-36), riavvio Docling e precisione pagine-tabella aperti per l'ADR di ingestion; falso positivo del prescan alla 1.4 (D-37) → `phases/01-spike-tecnici/01-CONTEXT.md#implementation-decisions`
- 2026-10-23 · Modello di embedding bge-m3 su TEI CUDA (D-38, emenda D-14): nDCG@10 0,723 contro Qwen3 0,660 → `phases/01-spike-tecnici/01-CONTEXT.md#implementation-decisions`
- 2026-10-24 · Scanner upload-in-memory: niente HIGH su read_to_end con .take(, nuova regola su io::copy verso Vec senza take; prescan torna a read_to_end; in /verify 1 → `phases/01-spike-tecnici/01-CONTEXT.md#decisioni`
- 2026-10-24 · D-41 corretta su deepsearch_7: eccezione solo con take dopo il decoder, casi ambigui «da verificare», test dello scanner; helper limitato alla fase 3 → `phases/01-spike-tecnici/01-CONTEXT.md#implementation-decisions`
- 2026-10-25 · 01-13: lock PaddleOCR con cooldown al 2026-10-10 (uv), librerie apt fissate, PP-OCRv6 misurato una volta a detection ridotta: non vince, Tesseract in ADR-0004 → `phases/01-spike-tecnici/01-13-SUMMARY.md`
- 2026-10-25 · 01-14: ricerca esatta con ACL nella stessa query nell'MVP, coseno, ibrida RRF non adottata, HNSW da rivalutare oltre 50k (recall filtrata ≥0,95 solo a 10k) → `phases/01-spike-tecnici/01-14-SUMMARY.md`
- 2026-10-26 · 01-15: baseline rerank no-rerank; bge-reranker-v2-m3 su TEI misurato e non adottato (p95 360 ms); JEV forma A resta il rank; picco VRAM 5.562 MiB, margine 70 MiB → `phases/01-spike-tecnici/01-15-SUMMARY.md`

## Fase 1.1 — Gap-closure di session.sh

- 2026-10-21 · dc() toglie con env -u, dal solo processo compose, le variabili dell'host omonime (in maiuscolo) delle chiavi di spikes.env; la shell non cambia → `phases/01.1-gap-closure-session-sh-segreti-di-spikes-env-prevalgono-su-e/01.1-01-SUMMARY.md#decisions-made`
- 2026-10-21 · Id di piano 01-1-01 nelle sessioni di session.sh: le regex rifiutano il punto; fixture TEST-11/12 allineate, non indebolite → `phases/01.1-gap-closure-session-sh-segreti-di-spikes-env-prevalgono-su-e/01.1-01-SUMMARY.md#deviations-from-plan`
- 2026-10-21 · Commit unico RED+GREEN+regressione: l'hook rifiuta i commit di soli .txt (security-scope-empty); regola dell'hook da decidere → `phases/01.1-gap-closure-session-sh-segreti-di-spikes-env-prevalgono-su-e/01.1-01-SUMMARY.md#punti-aperti`

- 2026-10-21 · Hook pre-commit invariato: con evidenze solo .txt/.csv il RED entra nel commit della correzione, in sezione distinta (decisione utente) → `../.planning/ORCHESTRATOR-STATUS.md#decisioni-ratificate-in-corsa`

## Fase 1.2 — Gap-closure di hist.rs

- 2026-10-21 · Il Recorder registra con record (auto-resize); il difetto reale saturava a 2 µs, non a 2047 → `phases/01.2-gap-closure-hist-rs-il-recorder-delle-latenze-non-satura-sop/01.2-01-SUMMARY.md#il-difetto-reale-più-grave-di-quello-del-context`
- 2026-10-21 · Rifiuto di record contato da rejected() (metodo additivo), ripiego saturato e avviso unico su stderr, firme invariate → `phases/01.2-gap-closure-hist-rs-il-recorder-delle-latenze-non-satura-sop/01.2-01-SUMMARY.md#decisions-made`
- 2026-10-21 · TEST-15/16 adottati su proposta del test design: ramo Err provato con new_with_max(2047, 3) dal mod tests → `phases/01.2-gap-closure-hist-rs-il-recorder-delle-latenze-non-satura-sop/01.2-01-SUMMARY.md#decisions-made`

## Fase 1.3 — Gap-closure di compose.yaml

- 2026-10-21 · tmpfs /work del worker con uid/gid 10001 e mode=0700 in sintassi corta; scartati 1777, /work nell'immagine e sintassi lunga → `phases/01.3-gap-closure-compose-yaml-work-del-worker-scrivibile-dall-ute/01.3-01-SUMMARY.md#decisions-made`
- 2026-10-21 · Docker monta il tmpfs con noexec/nosuid/nodev: per 01-07 nessun binario si esegue da /work → `phases/01.3-gap-closure-compose-yaml-work-del-worker-scrivibile-dall-ute/01.3-01-SUMMARY.md#punti-aperti`
- 2026-10-21 · TEST-5 e TEST-13 del design 01.3-01 difettosi (regex non ancorata, conflitto su «nessun down -v»): da correggere se riusati → `phases/01.3-gap-closure-compose-yaml-work-del-worker-scrivibile-dall-ute/01.3-01-SUMMARY.md#deviations-from-plan`

## Fase 1.4 — Gap-closure dei driver di misura e del prescan di 01-07
- 2026-10-23 · 1.4: DOCX ostili non deterministici per timestamp zip (D-11); M10 non ripetuta dopo ricalibrazione, 2 job con margine 8% (D-12) → `phases/01.4-gap-closure-driver-di-misura-di-01-07-pulizia-di-budget-sul-/01.4-CONTEXT.md#decisioni`

## Fase 1.5 — Gap-closure del prescan sulle dimensioni codificate delle immagini
- 2026-10-23 · 1.5: tetto delle immagini DCT e JPX sulle dimensioni codificate SOF/SIZ (D-39, D-09); inline debito fase 3; ocr_render obbligo di 01-13 → `phases/01.5-gap-closure-prescan-tetto-delle-immagini-dct-sul-sof-del-jpe/01.5-CONTEXT.md#decisioni`

## Fase 2 — Fondazione (discuss)

- 2026-10-21 · Contract-first: YAML OpenAPI per servizio, trait Contract (from_json + validate) in crates/contracts, classi Rust a mano, nessun generatore → `phases/02-fondazione/02-CONTEXT.md#contratti-api-a-01-plat-03`
- 2026-10-21 · Contract test per servizio: oasdiff breaking + Schemathesis --mode all contro servizio avviato + unit test su validate(), a ogni push → `phases/02-fondazione/02-CONTEXT.md#contratti-api-a-01-plat-03`
- 2026-10-21 · Errori problem+json RFC 9457 con code/message/request_id; 503+Retry-After per saturazione, 429 solo rate limit → `phases/02-fondazione/02-CONTEXT.md#contratti-api-a-01-plat-03`
- 2026-10-21 · Schema per servizio in un database (contro raccomandazione), search_path per ruolo, test 42501, cataloghi visibili accettati → `phases/02-fondazione/02-CONTEXT.md#dati-e-migrazioni-plat-02-criteri-3-e-5`
- 2026-10-21 · Migrazioni a fascia PPNNNN_nome.sql con gate CI (sqlx: versione = prefisso prima del primo _, fuori ordine applicato in silenzio) → `phases/02-fondazione/02-CONTEXT.md#dati-e-migrazioni-plat-02-criteri-3-e-5`
- 2026-10-21 · Primitive idempotency/outbox/lease alle fasi proprietarie, nessun seam in Fondazione (contro raccomandazione) → `phases/02-fondazione/02-CONTEXT.md#dati-e-migrazioni-plat-02-criteri-3-e-5`
- 2026-10-21 · Proposte runtime Decise alle versioni del 2026-10-21; reqwest-tracing escluso; crate config; distroless debian13 → `phases/02-fondazione/02-CONTEXT.md#ambiente-build-e-ci-plat-06-criteri-1-e-2`
- 2026-10-21 · Caddy unico origin, /internal/v1 bloccate e solo su rete interna; Garage HTTP su rete interna, TLS in fase 15; niente bind mount → `phases/02-fondazione/02-CONTEXT.md#ambiente-build-e-ci-plat-06-criteri-1-e-2`
- 2026-10-21 · OTLP opzionale + Jaeger v2 in override (passa a fase 14); redazione di default; /livez e /readyz → `phases/02-fondazione/02-CONTEXT.md#telemetria-log-e-health-criterio-4`
- 2026-10-21 · crates/contracts: tipi trasversali subito, tipi di dominio file vuoti per le fasi 3 e 7 → `phases/02-fondazione/02-CONTEXT.md#tipi-condivisi-area-e2`
- 2026-10-21 · React Router 8 data mode, TS 7 se il lint regge, Tailwind 4 + shadcn, registro route esplicito → `phases/02-fondazione/02-CONTEXT.md#shell-frontend-criterio-1-e-5`

## Fase 3 — Provider di ingestion e indice (discuss)

- 2026-10-21 · Firme S-07 confermate con variazioni: search(SearchQuery{text, embedding}) per l'ibrida, DocumentStore::delete per il GC della fase 6, parse su file locale; nel brief di execute di 01-16 → `phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md#firme-delle-porte-conferma-s-07-area-a`
- 2026-10-21 · Predicato = insieme di etichette, indice agnostico da Q-02; deny by default nel tipo → `phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md#autorizzazione-nellindice-area-b`
- 2026-10-21 · ACL in proiezione document_acl posseduta dalla fase 3, VectorIndex::set_acl (consumatori 6, 7, 12), versione ACL per documento → `phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md#autorizzazione-nellindice-area-b`
- 2026-10-21 · publish_version in una sola transazione con puntatore alla versione corrente → `phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md#pubblicazione-delle-versioni-area-c`
- 2026-10-21 · Locator multi-span con al massimo un confine (PDF e testo), contro raccomandazione; emenda D-10 di fase 1 → `phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md#parser-e-chunker-aree-d-e`
- 2026-10-21 · Chunk in caratteri, TEI rifiuta invece di troncare; Docling in fase 3 condizionato ad ADR-0004/D-27 → `phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md#parser-e-chunker-aree-d-e`
- 2026-10-21 · SSRF: resolver che scarta IP non globali, redirect controllati, https, tetti, rete Compose dedicata; niente proxy egress → `phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md#connettore-online-area-h`
- 2026-10-21 · Contract suite unica su reale e fake; reali in CI (PG, Garage, PDFium+Tesseract, TEI CPU) + gate locale GPU in verify → `phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md#contract-test-e-fake-area-i`
- 2026-10-21 · Fake deterministici con copione di guasti dietro feature testkit, mai in release; fixture ostili copiate con manifest → `phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md#contract-test-e-fake-area-i`
- 2026-10-24 · D-19: lettore limitato unico (quota sull'output, budget per documento) e divieto Clippy di read_to_end/io::copy fuori da esso (da D-41) → `phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md#implementation-decisions`

## Fase 4 — Provider di decisione, risposta e trasversali (discuss)

- 2026-10-21 · Split S-07 Reranker/DecisionEngine; baseline solo sul Reranker, guasto JEV = errore tipizzato; criterio 1 ROADMAP da riformulare → `phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md#porte-e-baseline-aree-a-b-d-e`
- 2026-10-21 · Adapter solo segnali, soglie nel caso d'uso della fase 8 (calibrate 13/14); generate con Complexity di dominio, single-turn fino a Q-05 → `phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md#porte-e-baseline-aree-a-b-d-e`
- 2026-10-21 · Circuit breaker scritto a mano, uno per provider, solo guasti del provider; semaforo globale + limite per query → `phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md#resilienza-area-c`
- 2026-10-21 · Snapshot OpenAPI JEV con hash; minimizzazione verso i provider; inference_geo global, nessun dato utente → `phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md#adapter-jev-area-b`
- 2026-10-21 · AuditSink PG append-only per servizio, evento tipizzato, IdempotencyConflict (forma provvisoria fino a Q-04) → `phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md#auditsink-area-f-forma-provvisoria-fino-a-q-04`
- 2026-10-21 · Hash chain dell'audit già in fase 4 (contro raccomandazione), riga di testa FOR UPDATE, nessun ancoraggio, verify_chain → fase 11 → `phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md#auditsink-area-f-forma-provvisoria-fino-a-q-04`
- 2026-10-21 · NoOpCache + fake in memoria con scope; replay wiremock in CI + gate locale a 0,50 USD; prova no-leak a sentinelle → `phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md#contract-test-prova-no-leak-e-adapter-reali-aree-h-i`

**Modifica:** Claude (Anthropic) · 2026-10-19 · creazione dell'indice con le decisioni dell'onda 1 della fase 1
**Modifica:** Claude (Anthropic) · 2026-10-21 · decisioni del discuss della fase 3
**Modifica:** Claude (Anthropic) · 2026-10-21 · decisioni del discuss della fase 4
