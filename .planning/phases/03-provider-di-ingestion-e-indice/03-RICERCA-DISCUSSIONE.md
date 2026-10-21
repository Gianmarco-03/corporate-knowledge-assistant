# 03 — Ricerca decisionale (discuss fase 3: Provider di ingestion e indice)

Brief per `gsd-discuss-phase 3`. Stile caveman. Fonti: URL + accesso 2026-10-21, o file repo + sezione. Canale: Claude (WebFetch/WebSearch). Nessuna parte GPT.

## 0. Preflight

- Fase 3 goal: 6 porte (DocumentStore, DocumentParser, Chunker, Embedder, VectorIndex, DocumentSource) complete dietro porta + fake + contract suite comune. Criticità ROADMAP: **critica** (parser input non fidato; connettore online in rete) → GPT `implementation-risk-audit` + `test-design` in plan.
- Decisioni ROADMAP da chiudere qui: (1) conferma firme S-07 fase 3, embedding query per fase 7; (2) forma predicato VectorIndex (Q-02); (3) vincoli DocumentSource da prima fonte (discuss 12).
- Punti ereditati: nessun `deferred-items.md` nomina fase 3. Da 02-CONTEXT: D-18 (config Garage = adapter fase 3), D-22 (`Locator`, `ContentHash`, `EmbeddingSpace` file vuoti in `crates/contracts` → fase 3; `RetrievalSearch*` → fase 7).
- WIRING-MAP: 0 righe (plan 2 non fatto). Nessun seam da riempire con `WM-NNN`. Seam attesi da Fondazione (02-CONTEXT §Vincoli): slot provider nel composition root di document-service, fascia migrazioni `03NNNN_*.sql` schema `document`, file vuoti contracts sopra.
- **Rischio pipeline (spec §8)**: fase 1 a onda 3 di 11. Esiti presenti solo S-01 (ADR-0003 Proposed) e S-05 (ADR-0010 Proposed). Pendenti: ADR-0004 parser/isolamento (01-07/08/09/13/18), 0005 locator (01-09), 0006 indice (01-14), 0007 embedding (01-10), 0012 firme (01-16). Decisioni qui su numeri spike = provvisorie; plan 3 parte dopo fine Spike (ROADMAP §Strategia passo 3).
- Ordine strategia: plan 3 DOPO discuss 6 (Q-01/Q-02) e discuss 12 (prima fonte). Quindi qui: forma generica; valori da quei discuss.

## A. Firme S-07 per fase 3

Fonte autorevole repo:
- `01-16-PLAN.md` Task 1 (forma attesa), `01-CONTRATTO.md` §3: `DocumentStore::put(MutationContext, ContentHash, ValidatedFile)`, `get(CallContext, key, range)`; `DocumentParser::parse` → blocchi + `Locator{Pdf{page,rects}|Text{section_path,char_range}}`, errori `Encrypted|DecompressionLimit|Timeout|ResourceLimit|OutputLimit|Crashed{signal}|Corrupt|Unsupported` + esito `Quarantined{reasons}`; `Embedder::embed_documents` + `embed_query` → `Embedding{space,values}`; `VectorIndex::search(ctx, Embedding, &AuthorizationPredicate, k)`, `publish_version(MutationContext, IndexVersionDraft)`, `space()`; `DocumentSource`/`Chunker` da firme attuali + errori tipizzati + contesto con deadline.
- `docs/architecture/interfaces-and-concurrency.md` §Porte: firme concettuali, «validate con spike».
- ADR-0003 §Conseguenze follow-up: `put` idempotente, `get` range, **`delete` solo dal GC**, nessun tipo `object_store`.
- ADR-0010 §Decisione: deadline `tokio::time::Instant` esplicita su ogni porta verso provider; cancellazione = drop.

Lacune trovate confrontando forma S-07 con decisioni già prese (01-16 non eseguito: si possono ancora portare nel brief di execute di 01-16 o registrare come variazione di plan 3):
- **V1 ricerca ibrida senza testo.** D-15 (01-CONTEXT) = `tsvector` + RRF k=60, ACL in entrambe le parti. Firma `search(ctx, Embedding, predicate, k)` non riceve testo query → parte lessicale impossibile. Opzione: `search(ctx, SearchQuery{text, embedding}, &predicate, k)`.
- **V2 `delete` senza consumatore in tabella.** ADR-0003 vuole `delete` solo dal GC. Tabella «Tracciabilità» ROADMAP ha solo `put`/`get`. Regola provider-first: op senza consumatore non si scrive → aggiungere riga `delete` → 6 (GC orfani) o rinunciare.
- **V3 `parse(&ObjectRef)`** (interfaces) accoppia parser a store. Parser gira in processo figlio su file locale (D-08; spike 01-07 job JSON con `input` path validato). Opzione: `parse(ctx, &LocalFile, MediaType)`; worker (fase 6) scarica da store a tempfile.
- **V4 ACL nell'indice**: vedi §B; possibili op nuove `set_acl` / `withdraw_document`.
- **V5 embedding query** già coperto da `embed_query` (01-16). Chiude decisione ROADMAP «embedding della query per la Fase 7».

## B. Predicato di autorizzazione e ACL nell'indice

Fonte autorevole:
- CLAUDE.md: deny by default; nessun chunk non autorizzato raggiunge reranker/JEV/LLM/cache/log.
- ROADMAP fase 3 criterio 3: predicato applicato nella query stessa, niente post-filtro. Fase 7 criterio 4: `authorize` riflette **subito** revoca ACL.
- 01-16 forma: `AuthorizationPredicate{subject_id, roles, acl_version}` prodotto da `AccessPolicy::predicate_for` (fase 7).
- Q-02 (classificazione) chiusa in discuss 6; vincola fase 3 (ROADMAP §Decisioni aperte).
- Spike 01-10 schema: `doc_acl(doc_id, role)` + join; filtro `exact_search(pool, table, vec, roles, k)` (`01-10-PLAN.md` Task 1).

Pratica consolidata:
- Azure AI Search «security filter pattern»: campo `group_ids` (collezione stringhe, filterable, non retrievable) sul documento; query con `group_ids/any(g:search.in(g,'id1,id2'))` = gruppi del chiamante; cambio permessi = `mergeOrUpload` del solo campo. https://learn.microsoft.com/en-us/azure/search/search-security-trimming-for-azure-search (2026-10-21). → ACL = insieme di etichette principal sul documento, match per intersezione.
- pgvector README: con indice approssimato «filtering is applied _after_ the index is scanned»; ricerca esatta + indice su colonna filtro buona per condizioni selettive; iterative scan / partizioni per molti valori. https://github.com/pgvector/pgvector (2026-10-21). Conferma D-12 (esatta in MVP).

Implicazioni:
- Etichette generiche (`role:manager`, `user:<uuid>`, futuro `group:`) rendono indice **agnostico da Q-02**: la mappa classificazione → ruoli si materializza alla scrittura (fase 6/12), non nello schema dell'indice. Cambio matrice = ricalcolo etichette + `acl_version`++.
- ACL **joinata** a query time da tabella per-documento (non copiata su ogni chunk) → revoca immediata, un UPDATE per documento; copia sui chunk → riscrittura di tutti i chunk.
- Proprietà tabelle: se indice fa join su tabella ACL di fase 6 → SQL di fase 3 dipende da schema altrui. Alternativa: indice possiede proiezione ACL propria (`document_acl` in fascia 03) + op `set_acl(ctx: MutationContext, document_id, labels)` → consumer 6 (ACL a upload), 7 (revoca), 12.
- Deny by default nel tipo: predicato con insieme etichette vuoto → zero risultati, mai «tutto».

## C. publish_version atomico, snapshot, ritiro

Fonte autorevole:
- ROADMAP fase 3 criterio 3 + fase 6 criterio 3 («pubblica la versione in una sola transazione»; interruzione a metà → nulla visibile).
- interfaces-and-concurrency §Concorrenza: nessuna chiamata esterna in transazione; «Nuova versione durante query → snapshot coerente».
- PostgreSQL Read Committed: «a SELECT query … sees only data committed before the query began; it never sees … uncommitted data». https://www.postgresql.org/docs/current/transaction-iso.html (2026-10-21). Transazione non committata a disconnessione = abort (comportamento generale PG, non in quella pagina).

Pratica:
- Embedding calcolati prima (fuori tx); una tx inserisce chunk della versione + sposta puntatore `current_version` del documento. Search filtra su versione corrente. Interruzione = rollback → niente visibile.
- Tombstone `DELETE /api/v1/documents/{id}` (interfaces §Document): search deve escluderlo → op `withdraw`(documento) o stato nel puntatore. Consumer da assegnare (fase 6 o 12).
- Lingua `tsvector`: spike usa colonna `lang regconfig` per chunk (`italian`/`english`) (`01-10-PLAN.md` schema). Esito in ADR-0006.

## D. Parser: processo, ambito, immagine

Fonte autorevole repo:
- D-07 PDFium testo/bbox; Docling solo pagine-tabella; D-08 processo figlio per documento con limiti; D-09 Tesseract CLI (PP-OCRv6 sfidante); D-25 filtri LZW/RunLength rifiutati; D-27 Docling pagine-tabella rinviata dopo misura 01-08; D-28 worker 2 GiB (01-CONTEXT).
- 01-07 (pendente): lanciatore `parse-parent` + figli, job JSON `deny_unknown_fields`, subreaper, `killpg`, `oom_score_adj`, quota output, path validati.
- 02-CONTEXT D-12: immagini distroless `cc-debian13:nonroot`. Tesseract (CLI + tessdata) e libpdfium non presenti in distroless → immagine worker diversa da API, o eccezione documentata. Spike: Docling come container separato `docling-serve-cpu`/`-cu128` v1.37.0 (`spikes/compose.yaml`).
- ROADMAP fase 3 criterio 2: file ostili spike → errore tipizzato entro limiti, host vivo; validi → testo con pagina e coordinate quando disponibili.
- 01-CONTRATTO §6: «Docling come sidecar su rete interna» atteso in ADR-0004.

Pratica:
- Docling = servizio HTTP separato (docling-serve) → adapter HTTP con deadline; GPU condivisa con TEI (budget VRAM 6 GB, 01-CONTEXT Claude's Discretion).
- Fixture ostili: `spikes/data/hostile/` esiste; prodotto non importa codice da `spikes/` (01-CONTRATTO §6) ma file fixture si possono copiare (con manifest + hash) in `services/document-service/tests/fixtures/`.

Scelta di scope aperta: PDFium + OCR + Docling tutti in fase 3, o Docling rinviato (fase 14). Riferimenti prodotto: servizi documentali (Azure AI Document Intelligence layout, AWS Textract) estraggono tabelle come struttura; corpus CKA = bilanci, tabelle centrali (D-07 motivazione).

## E. Chunker

Fonte:
- Spike 01-10 chunker di misura: ~1.600 caratteri, overlap 200, **mai a cavallo di due pagine o due sezioni**, conserva page/section_path/char range/lingua (`01-10-PLAN.md` Task 1). «La scelta del chunker di prodotto non è materia di questo spike.»
- Locator D-10: `Pdf{page, rects}` = **una** pagina → chunk multi-pagina non rappresentabile senza cambiare locator.
- NVIDIA (2024/25): page-level chunking accuratezza media 0,648, varianza minima; token 128–2.048 con overlap 15 % tra 0,603 e 0,645; FinanceBench migliore a 1.024 token. https://developer.nvidia.com/blog/finding-the-best-chunking-strategy-for-accurate-ai-responses/ (2026-10-21; letta via sintesi di ricerca, ranking direzionale).
- TEI `--auto-truncate` default true: input oltre max del modello troncati in silenzio. https://github.com/huggingface/text-embeddings-inference (2026-10-21) → chunker deve stare sotto limite token o errore esplicito.

Pratica: chunk strutturale (pagina/sezione) + split interno a dimensione fissa con overlap; tabella come unità. Unità di misura: caratteri (semplice, spike) vs token del tokenizer del modello (preciso, dipendenza in più).

## F. Embedder

Fonte:
- D-13/D-14 TEI GPU, Qwen3-Embedding-0.6B default (ADR-0007 pendente). 01-16: `EmbeddingSpace{model_id, revision, dimension, normalization, pooling, query_instruction, document_instruction}`; mismatch rifiutato anche a parità di dimensione.
- TEI: `--max-client-batch-size` default 32, `--max-batch-tokens` default 16384; immagini `cpu-1.9`, `86-1.9` (Ampere 8.6; RTX 3050 = 8.6), Qwen3-Embedding supportato. https://github.com/huggingface/text-embeddings-inference (2026-10-21). Spike pinna `86-1.9.4`.
- ADR-0010: deadline globale, fan-out variante A `timeout_at` + `buffer_unordered`; interfaces: circuit breaker su embedding, batch bounded con backpressure.

Pratica: adapter spezza in batch ≤ max-client-batch-size, concorrenza limitata, deadline unica, errore tipizzato su 413/429/5xx; nessun retry oltre deadline.

## G. DocumentStore

Fonte: ADR-0003 §Decisione (Proposed): Garage + `object_store`, parti 2 MiB, 1 in volo; PUT idempotente con HEAD (non misurato → da provare nell'adapter, carico `dedup` c=16 + protocollo canale laterale); GC N = 24 h, protocollo `live|deleting` + `pending`; tempfile ripuliti all'avvio. 02-CONTEXT D-15: HTTP su rete interna, TLS fase 15; D-18 config parti/bucket = fase 3.
- Riga DECISIONS-INDEX: parti 2 MiB sotto minimo S3 5 MiB, portabilità non verificata (01-04-SUMMARY punti aperti). → vale per Garage; per altro S3 va riconsiderato.
- Pratica contract suite: `object_store::integration` = test generici (`put_get_delete_list`, `get_opts`, `multipart_race_condition`, …) eseguiti su ogni backend; feature `integration`; «NB: These tests will delete everything present in the provided DynObjectStore». https://docs.rs/object_store/latest/object_store/integration/index.html (2026-10-21).

Divisione GC: protocollo di adozione tocca metadati documento (fase 6). Op `delete` in porta (fase 3), job GC e tabella `pending` → fase 6 (D-10 di 02: primitive alla fase proprietaria).

## H. DocumentSource e SSRF

Fonte autorevole — OWASP SSRF Prevention Cheat Sheet https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html (2026-10-21):
- «Match the host against an allowlist, and build the request yourself»; confronto stringa stretto.
- Risolvi DNS, valida IP (non globali = rifiuto: loopback, RFC 1918, link-local incl. 169.254.169.254, multicast); «Domain allowlisting alone does not prevent DNS rebinding»; «A second, unchecked DNS lookup between validation and connection can bypass these checks» → connettersi solo agli IP validati, mantenere hostname per Host/SNI/cert.
- «Disable the support for the following of the redirection» (o rivalidare ogni hop).
- Schemi in allowlist (solo https qui).
- Difesa rete complementare: egress firewall/segregazione.

Pratica Rust: reqwest 0.13.5 `ClientBuilder::dns_resolver` (resolver custom che filtra IP), `redirect(Policy)` (default segue 10 redirect → va cambiato), `https_only`, `timeout`, `connect_timeout`. https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html (2026-10-21). Limite dimensione: contatore su stream body (reqwest senza limite body nativo — non verificato qui, da confermare in plan).
Alternativa: proxy egress Smokescreen (Stripe): HTTP CONNECT proxy, ACL hostname, risolve e blocca IP interni; «ACL is only applied to hostnames as they appear in the request». https://github.com/stripe/smokescreen (2026-10-21). Servizio Go in più nel Compose.

Prima fonte e formato (API/RSS/manifest) = discuss 12. Corpus spike usa Banca d'Italia (DECISIONS-INDEX 2026-10-21) — indizio, non decisione.

## I. Contract test e fake

Fonte: ROADMAP fase 3 criterio 1 (stessa suite su reale e fake); 02-CONTEXT D-17 (CI a ogni push: rust con PostgreSQL service container; contract con Compose avviato); CLAUDE.md (fake deterministico nei test unitari).
- Pratica: suite generica parametrizzata sull'oggetto porta, eseguita su ogni implementazione → `object_store::integration` (sopra). In Rust: funzioni `async fn suite(port: Arc<dyn Port>)` in modulo dietro feature, chiamate da un test per adapter.
- Cargo: «features should be additive»; dev-dependencies attivano feature solo per target che le usano (resolver 2). https://doc.rust-lang.org/cargo/reference/features.html (2026-10-21) → fake + suite dietro feature (es. `testkit`), mai nel binario di release.
- GitHub Actions service containers: container per job, hostname = label, richiede runner Ubuntu. https://docs.github.com/en/actions/tutorials/use-containerized-services/use-docker-service-containers (2026-10-21). GPU su runner standard: non documentata in quella pagina → assumere assente.
- TEI ha immagine CPU (`cpu-1.9`) → adapter Embedder reale testabile in CI su CPU con modello piccolo (es. `all-mpnet-base-v2`, 109M, citato dal README) o con Qwen3-0.6B (più lento, download più grande). Docling CPU esiste (spike `docling-serve-cpu:v1.37.0`), immagine pesante.
- Fake usati dalle fasi consumer (6, 7, 12) senza toccare file fase 3 → API del fake (seed fixture, iniezione guasti: errore N-esima chiamata, latenza, `OutcomeUnknown`) fa parte del contratto.

## J. Criticità

Proposta: **critica**. Motivo: goal include parsing di file non fidati (ingestion) e connettore che raggiunge la rete (SSRF), e `VectorIndex.search` è il punto dove si applica il predicato del retrieval autorizzato. Coincide con ROADMAP. Effetto: plan 3 → GPT `implementation-risk-audit` + `test-design` su AgentBridge.

**Modifica:** Claude (Anthropic) · 2026-10-21 · brief di ricerca del discuss fase 03
