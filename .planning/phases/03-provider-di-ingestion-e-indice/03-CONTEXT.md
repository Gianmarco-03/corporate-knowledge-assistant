# Phase 3: Provider di ingestion e indice - Context

**Gathered:** 2026-10-21
**Status:** Ready for planning
**Criticità:** critica — proposta da ROADMAP e ratificata dall'utente con questo CONTEXT. La fase parsa file non fidati, il connettore `DocumentSource` raggiunge la rete (SSRF) e `VectorIndex.search` è il punto in cui si applica il predicato del retrieval autorizzato. In `plan`: `implementation-risk-audit` + `test-design` di GPT via AgentBridge.

<domain>
## Phase Boundary

La fase costruisce per intero, nel Document Service, i sei provider del lato ingestion e indice, ciascuno dietro la propria porta:

- `DocumentStore` (Garage), `DocumentParser` (PDFium, Tesseract, Docling), `Chunker`, `Embedder` (TEI), `VectorIndex` (PostgreSQL + pgvector) e `DocumentSource` (connettore online);
- per ogni porta: adapter reale, fake deterministico e una suite di contract test che gira identica su entrambi;
- la tabella «Tracciabilità provider → consumer» aggiornata con le operazioni nuove decise qui.

Non costruisce casi d'uso: upload, job e worker sono della fase 6, `AccessPolicy` e le rotte di retrieval della fase 7, quarantena e scheduler del connettore della fase 12. Non decide la classificazione dei documenti (Q-02, discuss 6) né la prima fonte online (discuss 12): qui si fissa la **forma** che quei discuss riempiranno. Il `plan` della fase 3 arriva dopo entrambi (ROADMAP §Strategia di consegna, passi 2-3).

Requisiti: nessuno diretto (fase provider). Criteri di successo 1-5 del ROADMAP, fase 3.

Ricerca a supporto: `03-RICERCA-DISCUSSIONE.md`, Aree A-J, con fonti (URL e data di accesso 2026-10-21, o file e sezione del repo). Ogni decisione qui sotto cita l'area che la fonda.

**Rischio di pipeline (spec §8):** la fase 1 è all'onda 3 di 11. Hanno esiti solo S-01 (ADR-0003 `Proposed`) e S-05 (ADR-0010 `Proposed`). ADR-0004 (parser e isolamento), 0005 (locator), 0006 (indice), 0007 (embedding) e 0012 (firme) non sono ancora emessi. Le decisioni che dipendono dai loro numeri restano provvisorie. Secondo il ROADMAP, il discuss si riapre solo se le misure ne cambiano le premesse.

</domain>

<decisions>
## Implementation Decisions

### Firme delle porte (conferma S-07, Area A)
- **D-01: Base = forma S-07 del piano 01-16** (`01-CONTRATTO.md` §3): `CallContext`/`MutationContext`, `MutationOutcome` con `NotApplied` distinto da `OutcomeUnknown`, `EmbeddingSpace` completo, errori tipizzati per porta, nessun tipo di provider in firma, deadline esplicita (ADR-0010). `Embedder::embed_documents` ed `embed_query` separati: chiude la decisione del ROADMAP «embedding della query per la Fase 7». Le firme definitive si trascrivono da ADR-0012, con le variazioni D-02..D-05 e D-10.
- **D-02: Ricerca ibrida in una chiamata.** `VectorIndex::search(ctx, SearchQuery { text, embedding }, &AuthorizationPredicate, k)`: una sola query SQL, vettoriale + `tsvector` con fusione RRF k=60 e predicato in entrambe le parti. Fonte: D-15 di fase 1 (ibrida), Area A V1 (la firma candidata riceveva solo l'embedding, quindi la parte lessicale non aveva testo). Scartate: due porte separate con RRF nel chiamante (predicato ripetuto), solo vettoriale (D-15 rivista).
- **D-03: `DocumentStore::delete(MutationContext, key)`**, usato solo dal GC degli orfani, con consumatore la fase 6: il job di GC, la tabella `pending` e il protocollo `live|deleting` li costruisce la fase 6. Nuova riga nella tabella di tracciabilità: `DocumentStore (3) | delete | 6 (GC orfani)`. Fonte: ADR-0003 §Conseguenze (follow-up S-07), regola provider-first (ogni operazione ha un consumatore), D-10 di fase 2 (primitive alla fase proprietaria). Area A V2, Area G.
- **D-04: Parser su file locale.** `DocumentParser::parse(ctx, &LocalFile, MediaType)`: il worker della fase 6 scarica l'originale da `DocumentStore` in un tempfile e lo passa. Il processo figlio isolato non conosce lo store né le sue credenziali. Fonte: D-08 di fase 1 (processo figlio), spike 01-07 (job JSON con percorso validato). Area A V3. Supera la firma `parse(&ObjectRef)` di `interfaces-and-concurrency.md`.
- **D-05: Le variazioni entrano nel brief di execute di 01-16**, che non è ancora eseguito: ADR-0012 nasce già allineato a D-02, D-03, D-04, D-07, D-08 e D-10, senza re-gate del piano. Sono vincoli aggiunti, non un cambio di forma. Decisione utente. Se l'execute di 01-16 non li recepisce, il plan 3 li registra come divergenze motivate da ADR-0012.

### Autorizzazione nell'indice (Area B)
- **D-06: Predicato = insieme di etichette.** `AuthorizationPredicate { subject_id, acl_version, labels }`: un documento è visibile se le sue etichette intersecano quelle del soggetto (es. `role:manager`, `user:<uuid>`). Un insieme vuoto dà zero risultati, mai «tutto» (deny by default nel tipo). L'indice non conosce la scala delle classificazioni: la mappa classificazione → etichette la decide il discuss 6 (Q-02) e la applica chi scrive (fasi 6 e 12); un cambio della matrice ricalcola le etichette e incrementa le versioni. Fonte: Azure AI Search, security filter pattern (`group_ids` + `search.in`), https://learn.microsoft.com/en-us/azure/search/search-security-trimming-for-azure-search (consultato 2026-10-21); CLAUDE.md (deny by default); ROADMAP fase 3 criterio 3 (predicato nella query, nessun post-filtro). Chiude la decisione del ROADMAP «forma del predicato di `VectorIndex` (da Q-02)». Variazione rispetto a `{subject_id, roles, acl_version}` di 01-16.
- **D-07: L'ACL è una proiezione posseduta dall'indice.** Tabella `document_acl` nella fascia di migrazioni 03 dello schema `document`, letta in join a ogni ricerca. Nuova operazione `VectorIndex::set_acl(MutationContext, document_id, labels)`, con consumatori 6 (ACL all'upload), 7 (revoca) e 12. Una revoca è un solo UPDATE e vale dalla ricerca successiva. Fonte: ROADMAP fase 7 criterio 4 (revoca immediata); insiemi di file chiusi (U-04: l'SQL della fase 3 non legge tabelle di altre fasi); pgvector README (ricerca esatta con filtro), https://github.com/pgvector/pgvector (consultato 2026-10-21). Scartate: etichette copiate su ogni chunk (una revoca riscrive tutti i chunk), join sulle tabelle della fase 6.
- **D-08: Versione ACL per documento.** Ogni `set_acl` incrementa una versione del documento, restituita con i candidati, così il ricontrollo della fase 7 e la cache della fase 4 vedono la revoca. La versione ACL del soggetto (cambio di ruoli) resta della fase 5/10 (01-CONTRATTO §6). Decisione utente.

### Pubblicazione delle versioni (Area C)
- **D-09: `publish_version` in una sola transazione.** Embedding calcolati prima, fuori dalla transazione; la transazione inserisce i chunk della versione e sposta il puntatore alla versione corrente del documento; la ricerca legge solo le versioni correnti. Un'interruzione a metà fa rollback e non lascia nulla di visibile. Fonte: ROADMAP fase 3 criterio 3 e fase 6 criterio 3; `interfaces-and-concurrency.md` §Database (nessuna chiamata esterna in transazione); PostgreSQL Read Committed («sees only data committed before the query began»), https://www.postgresql.org/docs/current/transaction-iso.html (consultato 2026-10-21).

### Parser e chunker (Aree D, E)
- **D-10: Locator multi-span, al massimo un confine.** Un chunk può attraversare **un** confine di pagina (PDF) o di sezione (DOCX/Markdown), mai due. Entrambe le varianti del locator diventano liste di span: `Pdf { spans: [{ page, rects }] }` e `Text { spans: [{ section_path, char_range }] }`, con al massimo 2 elementi. La citazione mostra «pp. 4–5»; il viewer della fase 9 apre la prima pagina dello span ed evidenzia su entrambe. Decisione utente, **contro la raccomandazione** (chunk mai a cavallo, coerente con lo spike 01-10 e con il dato NVIDIA sul chunking per pagina: https://developer.nvidia.com/blog/finding-the-best-chunking-strategy-for-accurate-ai-responses/, consultato 2026-10-21). Motivo: non spezzare un paragrafo o una tabella a cavallo di pagina. **Emenda D-10 di fase 1** (locator a una pagina) e la forma attesa di ADR-0005: la modifica entra nel brief di execute di 01-09 e di 01-16, e D-10 di 01-CONTEXT ha ricevuto il paragrafo *Storia* (2026-10-21). Gli invarianti di D-24 di fase 1 (offset Unicode scalar su NFC, `section_path` con ordinale, `cka-text-v1`) valgono per ogni span.
- **D-11: Dimensione dei chunk in caratteri.** Come nello spike (circa 1.600 caratteri con sovrapposizione 200; valori definitivi da ADR-0007/0006), senza tokenizer nel Chunker. TEI si configura per **rifiutare** gli input oltre il limite del modello, invece di troncarli in silenzio (`--auto-truncate` è true di default): l'adapter restituisce un errore tipizzato. Fonte: `01-10-PLAN.md` Task 1; TEI README, https://github.com/huggingface/text-embeddings-inference (consultato 2026-10-21). Scartato il conteggio in token (dipendenza in più, Chunker legato al modello).
- **D-12: Docling dentro la fase 3, alle condizioni degli spike.** Il parser è PDFium (testo, bbox, pagine) + Tesseract (pagine senza testo) + Docling come sidecar HTTP su rete interna, solo per le pagine con tabelle, nella forma che sceglieranno ADR-0004 e D-27 di fase 1. Se la misura boccia Docling, il discuss si riapre. Fonte: D-07..D-09 e D-27 di fase 1; `01-CONTRATTO.md` §6 (Docling come sidecar); riferimenti di prodotto (Azure Document Intelligence, Textract: tabelle estratte come struttura). Area D.

### Connettore online (Area H)
- **D-13: Difesa SSRF nel processo, più la rete.** Client `reqwest` dedicato con:
  - resolver custom (`ClientBuilder::dns_resolver`) che scarta gli IP non globali (loopback, RFC 1918, link-local incluso 169.254.169.254, multicast) e connessione solo agli IP validati, con l'hostname mantenuto per Host, SNI e certificato;
  - allowlist degli host con confronto stretto e URL costruito dal codice;
  - redirect disattivati, oppure ricontrollati hop per hop (il default di reqwest ne segue 10);
  - `https_only`, timeout totale e di connessione, tetto di byte sul body letto in streaming.

  In più il processo che fa il fetch esce in rete solo da una rete Compose dedicata. Il formato della fonte (API, RSS o manifest), l'host e la licenza arrivano dal discuss 12. Fonte: OWASP SSRF Prevention Cheat Sheet, https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html; reqwest 0.13.5 `ClientBuilder`, https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html (consultati 2026-10-21). Scartato il proxy egress Smokescreen (un servizio Go in più). Chiude, per la parte di forma, la decisione del ROADMAP «vincoli di `DocumentSource` dalla prima fonte».

### Contract test e fake (Area I)
- **D-14: Una suite per porta, eseguita su reale e fake.** Funzioni generiche che ricevono `Arc<dyn Porta>`, chiamate da un test per adapter, sul modello di `object_store::integration`. Fonte: https://docs.rs/object_store/latest/object_store/integration/index.html (consultato 2026-10-21); ROADMAP fase 3 criterio 1.
- **D-15: Dove gira la suite sugli adapter reali.**
  - **In CI a ogni push:** PostgreSQL + pgvector, Garage, PDFium + Tesseract, TEI su CPU con un modello piccolo. La suite prova il protocollo, non la qualità.
  - **Gate locale dichiarato:** TEI su GPU con il modello scelto in ADR-0007, e Docling. Viene eseguito in `verify` sulla macchina dell'utente, con un comando documentato.

  Fonte: GitHub Actions service containers (runner Ubuntu, nessuna GPU documentata), https://docs.github.com/en/actions/tutorials/use-containerized-services/use-docker-service-containers; TEI immagine `cpu-1.9`; D-17 di fase 2 (CI a ogni push). Scartati: reali solo in locale (la CI non vede le regressioni), runner self-hosted (rischio su un PC personale).
- **D-16: Fake e suite dietro la feature `testkit`** del document-service (`#[cfg(any(test, feature = "testkit"))]`), mai nel binario di release: un controllo in CI lo verifica. Fonte: Cargo, features additive e attivazione tramite dev-dependencies, https://doc.rust-lang.org/cargo/reference/features.html (consultato 2026-10-21). Scartato un crate separato in `crates/` (oggi solo `contracts` e `telemetry`; i modelli di dominio non si condividono, CLAUDE.md).
- **D-17: Fake deterministici con copione di guasti.** Fixture iniettabili; errore tipizzato alla N-esima chiamata; `OutcomeUnknown`; latenza con il clock di tokio in pausa; conteggio delle chiamate; nessuna casualità. L'API del fake fa parte del contratto della fase 3, perché le fasi 6, 7 e 12 la usano senza poterla modificare. Fonte: CLAUDE.md (fake deterministico nei test); U-04 (insiemi di file chiusi).
- **D-18: Fixture ostili copiate con manifest.** I file di `spikes/data/hostile/` si copiano in `services/document-service/tests/fixtures/hostile/` con un manifest (esito atteso, sha256, origine o licenza). Si copiano file, non codice. Fonte: `01-CONTRATTO.md` §6 (nessuna dipendenza da `spikes/`); ROADMAP fase 3 criterio 2.

### Letture di dati non fidati (da D-41 di fase 1)
- **D-19: Lettore limitato unico e divieto Clippy** (decisione utente 2026-10-24, D-41 di fase 1). Il document-service legge ogni dato non fidato (upload, fonti online, stream decompressi di PDF e DOCX) attraverso un solo helper di dominio. L'helper:
  - riceve il reader già decodificato e una quota sui byte di **output**;
  - legge a blocchi con `Read::read` e un contatore `checked_add`, e fallisce appena supera la quota (si legge al più quota + 1);
  - alimenta un budget condiviso per documento fra tutti gli stream.

  Fuori dall'helper, `clippy.toml` vieta con `disallowed-methods` (con `reason` e sostituto) `Read::read_to_end`, `Read::read_to_string`, `std::io::copy`, `std::fs::read` e gli equivalenti async di tokio. Si aggiunge `allow_attributes_without_reason`. I test coprono: output esattamente alla quota, quota + 1, errore a metà stream, molti stream che superano il budget insieme, `take` messo prima del decoder. Fonte: ASVS 4.0.3 V12.1.2 e ASVS 5.0 V5.2.3 (limite sulla dimensione decompressa prima di consumare risorse), CWE-409/770/789, Clippy `disallowed-methods`, tramite `../01-spike-tecnici/01-DEEPSEARCH-7.md` (ChatGPT Deep Research, `deepsearch_7`, fonti non riverificate). Scartato anticipare l'helper nello spike, che si butta.

### Vincoli dalla WIRING-MAP e dalle fasi precedenti
- La WIRING-MAP non ha righe (il `plan` della fase 2 non è ancora fatto): nessun `WM-NNN` da citare. La fase 3 **riempie** gli innesti che la Fondazione pre-registrerà (02-CONTEXT §Vincoli e D-22):
  - slot dei provider nel composition root del document-service;
  - fascia di migrazioni `03NNNN_*.sql` nello schema `document`;
  - file vuoti di `crates/contracts` per `Locator`, `ContentHash` ed `EmbeddingSpace`.

  `wiring-design` del plan 2 assegnerà le righe e la `fase-proprietaria` 3.
- Configurazione di Garage (parti da 2 MiB, una in volo, bucket) e PUT idempotente con HEAD da misurare nell'adapter: carico `dedup` a c=16 e protocollo del canale laterale (ADR-0003 §Decisione; D-18 di fase 2). Garage via HTTP su rete interna, TLS in fase 15 (D-15 di fase 2).
- Fan-out dell'Embedder: deadline globale, variante A `timeout_at` + `buffer_unordered`, batch ≤ `--max-client-batch-size`, concorrenza limitata, circuit breaker (ADR-0010; `interfaces-and-concurrency.md` §Backpressure).
- Isolamento del parser: processo figlio per documento con limiti calibrati, budget aggregato, tetti di decompressione, limite di pagine, filtri LZW/RunLength rifiutati come limite noto documentato (D-08, D-25, D-28 di fase 1; ADR-0004 atteso).

### Claude's Discretion
- Immagine del worker: Tesseract e libpdfium non stanno in distroless (D-12 di fase 2). In `plan`, secondo ADR-0004: immagine debian-slim per la modalità worker oppure eccezione documentata.
- Lingua del `tsvector` (colonna `regconfig` per chunk come nello spike, o altro) secondo ADR-0006; parametri di ricerca (k, soglia per HNSW) secondo ADR-0006 e il discuss 7.
- Modello piccolo per TEI su CPU in CI; nomi delle tabelle; struttura interna dei moduli `ports`/`adapters`; forma concreta del copione di guasti.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Ricerca di fase
- `.planning/phases/03-provider-di-ingestion-e-indice/03-RICERCA-DISCUSSIONE.md` — brief Aree A-J con fonti e data di accesso.

### Decisioni di progetto
- `.planning/ROADMAP.md` §Phase 3, §Strategia di consegna, §Tracciabilità provider → consumer, §Decisioni aperte per fase.
- `.planning/phases/01-spike-tecnici/01-CONTEXT.md` — D-01..D-30 (storage, parser, OCR, locator, indice, embedding); D-10 emendata da D-10 di questa fase.
- `.planning/phases/01-spike-tecnici/01-CONTRATTO.md` §3 (firme candidate S-07), §6 (ADR attesi, vincoli verso Fondazione e Provider).
- `.planning/phases/02-fondazione/02-CONTEXT.md` — D-07 (schema per servizio), D-09 (fascia migrazioni), D-10 (primitive alle fasi proprietarie), D-12 (immagini distroless), D-15 (Garage su rete interna), D-17 (CI), D-18 (Garage nel Compose), D-22 (file vuoti in `crates/contracts`).
- `docs/adr/0003-storage-originali-garage.md` — Garage, parti 2 MiB, PUT idempotente, GC 24 h (Proposed).
- `docs/adr/0010-deadline-cancellazione-http.md` — deadline e fan-out (Proposed).
- `docs/adr/0004..0007`, `docs/adr/0012-firme-porte-candidate.md` — **non ancora emessi** (piani 01-09, 01-10, 01-13, 01-14, 01-16): fonte delle firme e dei parametri da trascrivere.
- `DEPENDENCIES.md` — righe pgvector, reqwest, originali, embedding model, OCR/parser (da aggiornare in 01-17).

### Piani della fase 1 che portano le variazioni
- `.planning/phases/01-spike-tecnici/01-16-PLAN.md` — firme S-07 (brief di execute: D-02, D-03, D-04, D-06, D-07, D-08, D-10).
- `.planning/phases/01-spike-tecnici/01-09-PLAN.md` — locator e ADR-0005 (brief di execute: D-10 multi-span).
- `.planning/phases/01-spike-tecnici/01-07-PLAN.md` — lanciatore del processo isolato (input su file locale).
- `.planning/phases/01-spike-tecnici/01-10-PLAN.md` — chunker di misura, schema `doc_acl`.

### Architettura
- `docs/architecture/interfaces-and-concurrency.md` — porte, concorrenza, race condition; superato da D-02, D-03, D-04, D-07 (riallineamento in Fondazione, 01-CONTRATTO §6).
- `docs/architecture/architecture.md` — flusso di ingestion, ownership dei dati.
- `skills/crate-boundary-check/boundaries.json` — crate proibiti nei moduli puri (criterio 5).

### Sicurezza
- `docs/security/threat-model.md` — T-05..T-11, T-23, T-24 (compilazione in 01-17); SSRF del connettore da aggiungere.
- `spikes/data/hostile/` — casi ostili da copiare come fixture (D-18).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Nessun codice di prodotto: `services/document-service` contiene solo `.gitkeep`.
- `spikes/` è materiale da consultare e trascrivere, mai da importare: adapter Garage dello spike S-01 (parti da 2 MiB, una in volo), lanciatore del processo isolato (01-07, pendente), chunker, client TEI e SQL dell'indice (01-10/01-14, pendenti), Compose con TEI `86-1.9.4`, Docling `v1.37.0`, pgvector `0.8.7-pg18-trixie`, Garage `v2.4.1`.

### Established Patterns
- Porte come `Arc<dyn Trait + Send + Sync>`, errori enum per porta, nessun tipo SDK/DB in firma (01-16; `crate-boundary-check`).
- Gate di piano: `gate-perimetro.cjs`, `gate-proprieta-file.cjs` (colonna `fase-proprietaria` della WIRING-MAP).
- Lezione di fase 1.1: le variabili d'ambiente dell'host possono prevalere su `--env-file` nel Compose; i segreti di Garage e TEI si leggono da `secrets:`/file.
- Gate su `spikes/`: partire da `cargo clean -p` dei crate toccati (binario di test stale, verify 01.2).

### Integration Points
- Composition root del document-service (slot provider della Fondazione); fascia migrazioni 03; `crates/contracts` (`Locator`, `ContentHash`, `EmbeddingSpace`).
- Consumatori: fase 6 (`put`, `get`, `delete`, `parse`, `chunk`, `embed_documents`, `publish_version`, `set_acl`), 7 (`embed_query`, `search`, `set_acl`), 9 (`get` con range), 12 (`discover`, `fetch`, `put`, `parse`, `chunk`, `embed_documents`, `publish_version`, `set_acl`).

</code_context>

<specifics>
## Specific Ideas

- L'utente vuole che un chunk non spezzi un paragrafo o una tabella a cavallo di pagina: preferisce un locator a due span alla regola «mai a cavallo» (D-10).
- Preferenza confermata per una toolchain semplice: niente tokenizer nel Chunker, niente proxy egress, niente crate di test separato.

</specifics>

<deferred>
## Deferred Ideas

- **Tombstone `DELETE /api/v1/documents/{id}` senza fase proprietaria.** Esiste in `interfaces-and-concurrency.md` e le fasi 9 (criterio 3) e 14 (race «documento rimosso») lo danno per scontato. Va assegnato nel discuss 6 o 12; quando una fase lo prende, la riconciliazione (passo 5 della strategia) aggiunge `VectorIndex::withdraw` alla fase 3.
- Re-embedding dell'intero corpus al cambio di modello: oggi un `EmbeddingSpace` diverso viene rifiutato; la migrazione resta fuori dall'MVP.
- HNSW + `iterative_scan` oltre la soglia misurata: fase 14 (già in Deferred di fase 1).

</deferred>

## TODO aperti dal discuss

- Brief di execute di **01-16**: firme con D-02 (`SearchQuery`), D-03 (`delete`), D-04 (`parse` su file locale), D-06 (predicato a etichette), D-07 (`set_acl`), D-08 (versione ACL per documento), D-10 (locator multi-span). Brief di execute di **01-09**: D-10 (ADR-0005 multi-span, al massimo un confine).
- ROADMAP, tabella «Tracciabilità provider → consumer»: aggiungere `DocumentStore delete → 6`, `VectorIndex set_acl → 6, 7, 12`; rinominare `embed_batch` in `embed_documents`/`embed_query` (via `gsd-tools`, nella riconciliazione o nel plan 3).
- Discuss 6 o 12: assegnare il tombstone (vedi Deferred).
- Discuss 6: la mappa classificazione → etichette (Q-02) deve produrre insiemi di etichette (D-06).
- Discuss 12: host, formato e licenza della prima fonte, dentro i vincoli di D-13.
- `docs/security/threat-model.md`: SSRF del connettore e DNS rebinding (D-13), in 01-17 o nel plan 3.

---

*Phase: 03-provider-di-ingestion-e-indice*
*Context gathered: 2026-10-21*

**Modifica:** Claude (Anthropic) · 2026-10-21 · creazione (discuss fase 03)
**Modifica:** Claude (Anthropic) · 2026-10-24 · D-19 (lettore limitato unico e divieto Clippy, da D-41 di fase 1)
