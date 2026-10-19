# 01 — CONTRATTO della fase «Spike tecnici»

Questo documento elenca le promesse della fase 1 su cui possono pianificare le fasi successive: la Fondazione (2), i Provider (3 e 4) e, dopo di loro, le fasi consumatrici (5-15). La fase 1 è di spike: il codice vive in `spikes/`, fuori dal workspace di prodotto e dalla CI, e nessun file di prodotto lo importa (ROADMAP, criterio 5). Le sue uscite verso le altre fasi sono **solo documenti**: ADR 0003-0013, `DEPENDENCIES.md`, `docs/security/threat-model.md` e le firme candidate di S-07 (`spikes/s07-ports/src/lib.rs` con ADR-0012). Le firme si **trascrivono** nel prodotto, non si importano.

Fonti: i 18 piani `01-01`…`01-18-PLAN.md`, `01-CONTEXT.md` (D-01..D-30), `01-WIRING.md` §2, `01-ONDE.md`, `.planning/WIRING-MAP.md`, ROADMAP fase 1.

Ogni ADR nasce con Stato `Proposed`. Diventa `Accepted` solo con la ratifica dell'utente nel piano 01-17. Finché manca quella ratifica, una decisione attesa qui sotto resta un default su cui pianificare, non un fatto compiuto.

## 1. Rotte e DTO (`/api/v1`, `/internal/v1`)

**Nessuna rotta di prodotto.** La fase non crea servizi né router: le rotte degli spike (l'upload di S-01, i mock h1/h2 di S-05) restano in `spikes/` e non sono un contratto.

**DTO candidati, da ratificare nella Fondazione (non sono promesse).** Il piano 01-16 propone, per il confine `POST /internal/v1/retrieval/search` (da document-service a rag-service), due DTO destinati a `crates/contracts`:

- `RetrievalSearchRequest { query_text, k }`. Il principal non viaggia nel corpo ma nel token verificato.
- `RetrievalSearchResponse { subject_id, acl_version, space: EmbeddingSpace, passages: Vec<RetrievedPassageDto { passage_ref, document_version_id, locator, text }> }`, con `Serialize`/`Deserialize` e un `Debug` manuale che omette `text`. Per chi la riceve è un dato non fidato.

La forma definitiva della rotta, gli errori (`code`, `message`, `request_id`) e la versione OpenAPI li decide la Fondazione, partendo da ADR-0012.

## 2. Tabelle e migrazioni per servizio

**Nessuna.** La fase non ha schemi né migrazioni di prodotto. Gli SQL degli spike (`spikes/s03-pgvector/sql/index.sql`, la baseline `bytea` di S-01) sono materiale di misura. ADR-0006 e ADR-0003 ne riportano le conclusioni: ricerca esatta con il predicato ACL nella stessa query, ricerca ibrida `tsvector` + RRF, chiave sha256 per gli originali. Le migrazioni reali nascono nelle fasi proprietarie, ciascuna con la propria numerazione per servizio secondo la WIRING-MAP.

## 3. Tipi in `crates/contracts` e trait-porta

La fase **non pubblica** tipi in `crates/contracts` e non crea trait di prodotto. Promette però il documento S-07 (01-16): un `spikes/s07-ports/src/lib.rs` che compila, con fake di contratto nei test, insieme ad ADR-0012 (`docs/adr/0012-firme-porte-candidate.md`). La Fondazione e i Provider lo trascrivono. I contenuti promessi sono questi.

**Porte candidate (13), ciascuna usabile come `Arc<dyn Trait + Send + Sync>` e con al massimo 6 metodi.** Sono le 11 porte di `interfaces-and-concurrency.md` (`DocumentSource`, `DocumentStore`, `DocumentParser`, `Chunker`, `Embedder`, `VectorIndex`, `DecisionEngine`, `AnswerGenerator`, `AccessPolicy`, `AuditSink`, `Cache`), più `Reranker` ed `EvidenceRetriever`. Nessun tipo di provider, SDK o database compare in firma (`object_store`, `sqlx`, `reqwest`, `pgvector`, `pdfium-render`, `axum`, `opentelemetry`). Gli errori sono enum tipizzati per porta. La scelta fra `async-trait` e il boxing esplicito dei future viene decisa e registrata in ADR-0012.

**Variazioni rispetto a `interfaces-and-concurrency.md`:**

- **Split `Reranker`/`DecisionEngine` (ISP).** Il trait stretto `Reranker` contiene il solo metodo `rank(ctx, query, AuthorizedEvidence) → Vec<RankScore>`. Lo implementano l'adapter JEV e la baseline TEI bge-reranker. `DecisionEngine` conserva `validate_query → QueryDecision { complexity: Simple|Complex, … }` (D-18), `assess_evidence(&AuthorizedEvidence)` e `validate_citations(&AuthorizedEvidence, &StructuredAnswer)`, e registra nei metadati la versione JEV risolta (D-17).
- **`AuthorizedEvidence` su due lati del confine.**
  - *Lato documento* (futuro document-service): `retrieve_authorized(index: &dyn VectorIndex, ctx, query: Embedding, predicate: &AuthorizationPredicate, k)` lavora in tre passi. Rifiuta il mismatch di `EmbeddingSpace` prima della ricerca. Chiama `VectorIndex::search` col predicato. Ricontrolla ogni candidato. Il risultato è una `RetrievedEvidence` con costruttore privato, la cui unica uscita è `into_response() → RetrievalSearchResponse`.
  - *Lato RAG* (futuro rag-service): la porta `EvidenceRetriever::search(ctx: CallContext, RetrievalSearchRequest) → RetrievalSearchResponse` ha come adapter futuro un client HTTP interno. `authorize_response(response, expected: &AuthorizationScope, expected_space)` è l'unico costruttore di `AuthorizedEvidence`. Rifiuta `SubjectMismatch`, `AclVersionMismatch`, `SpaceMismatch` e i `PassageRef` duplicati, senza evidenza parziale.
  - `AuthorizedEvidence` non ha `Default` né `Serialize`/`Deserialize`. Offre solo `reorder` (permuta o toglie) e `resolve(&PassageRef)`. `Reranker`, `DecisionEngine` e `AnswerGenerator` ricevono passaggi solo dentro di essa.
  - `Debug` manuale e redatto su `RetrievedEvidence`, `RetrievedPassage`, `RetrievedPassageDto`, `AuthorizedEvidence`, `AuthorizedPassage`, `Candidate`, `Claim` e `StructuredAnswer`.
- **`CacheKey` con scope.** Il costruttore è `CacheKey::new(AuthorizationScope { subject_id, acl_version }, corpus_version, space: EmbeddingSpace, pipeline_version, query_digest)`, senza `Default`. `Cache::invalidate(ctx, InvalidationScope::{Subject, AclVersion, DocumentVersion, EmbeddingSpace})`. Una voce letta si ricontrolla contro l'autorizzazione corrente prima di usarla. `acl_version` è monotona e cambia anche quando cambiano i ruoli del soggetto: questa scelta è preferita a un digest dei ruoli. Il primo adapter resta `NoOpCache`.
- **`MutationContext` e `CallContext`.** `CallContext { request_id, trace: TraceContext, deadline }` vale per le letture verso i provider; `TraceContext` è una stringa W3C validata, senza tipi di tracing. `MutationContext { call, idempotency_key, actor }` vale per `DocumentStore::put`, `VectorIndex::publish_version`, `AuditSink::append` (con `event_id` uguale all'idempotency key) e `Cache::invalidate`. Le mutazioni restituiscono `MutationOutcome::{Applied, AlreadyApplied}` e distinguono `NotApplied { reason }` da `OutcomeUnknown`. `Cache::put` riceve `CallContext` ed è idempotente per costruzione.
- **`EmbeddingSpace`.** Contiene `model_id`, `revision`, `dimension`, `normalization`, `pooling`, `query_instruction` e `document_instruction`, con `Eq`. `Embedding::new` e `IndexVersionDraft::new` rifiutano dimensione o spazio incoerenti. Un mismatch viene rifiutato anche a parità di dimensione.
- **Altre firme:**
  - `DocumentStore::put(MutationContext, ContentHash, ValidatedFile)` e `get(CallContext, key, range)`, con la chiave sha256 calcolata prima dello storage (D-02).
  - `DocumentParser::parse` con blocchi e `Locator` a due varianti, `Pdf { page, rects }` e `Text { section_path, char_range }` (D-10, D-24). Errori `Encrypted`, `DecompressionLimit`, `Timeout`, `ResourceLimit`, `OutputLimit`, `Crashed { signal }`, `Corrupt`, `Unsupported`, più l'esito `Quarantined { reasons }`; il parsing gira fuori processo (D-08).
  - `Embedder::embed_documents` ed `embed_query` separati (D-14).
  - `VectorIndex::search(ctx, Embedding, &AuthorizationPredicate, k)`, `publish_version(MutationContext, IndexVersionDraft)` e `space()`.
  - `AnswerGenerator::generate(ctx, query, &AuthorizedEvidence) → StructuredAnswer { answer, claims: Vec<Claim { text, citations: Vec<PassageRef> }>, refusal }` (D-20).
  - `AccessPolicy::predicate_for(ctx, &Principal) → AuthorizationPredicate`.
- **Tipi destinati a `crates/contracts`, da ratificare in Fondazione:** `Locator`, `ContentHash`, gli identificativi opachi, `RequestId`, `IdempotencyKey`, `TraceContext`, `EmbeddingSpace`, `RetrievalSearchRequest` e `RetrievalSearchResponse`. `RetrievedEvidence` resta interno a document-service e `AuthorizedEvidence` interno a rag-service: non si condividono.

## 4. Route e componenti di `apps/web`

**Nessuno.** La fase non tocca il frontend. Per il viewer (fase 9) conta soltanto la decisione documentale di ADR-0005: il locator a due varianti, gli offset in Unicode scalar su testo NFC convertiti in UTF-16 nel viewer, e la proiezione `cka-text-v1` (D-24).

## 5. Seam della WIRING-MAP

**Nessuno, né creato né riempito.** `.planning/WIRING-MAP.md` non contiene righe e il suo registro delle variazioni lo conferma per la fase 01. `01-WIRING.md` §2 esclude ogni seam inter-fase: il codice della fase 1 è usa-e-getta e nessun goal delle fasi 2-15 estende codice costruito qui. Gli innesti di prodotto li crea la Fondazione, dalla prima riga `WM-001`. I seam intra-fase (W1-W20 di `01-WIRING.md` §3-§5, per esempio i membri di `spikes/Cargo.toml`, il compose chiuso e il lanciatore W17) valgono solo dentro `spikes/` e non entrano nella mappa.

## 6. Esiti promessi che sbloccano le fasi successive

### ADR prodotti

| Piano | ADR | Decisione attesa (default da CONTEXT, confermata o smentita dalla misura) | Fasi consumatrici |
|---|---|---|---|
| 01-04 | 0003 — storage degli originali (`0003-storage-originali-garage.md`) | Garage S3 dietro `DocumentStore`, misurato contro PG `bytea` e un adapter nullo. Chiave sha256, PUT prima della transazione, GC degli orfani con protocollo di adozione, risposta identica con o senza deduplica. Upload in streaming su tempfile, validazione prima dello storage, limite rigido di 25 MB su tre livelli, admission control (D-01..D-06) | 2, 3, 6 |
| 01-13 (con le evidenze di 01-07, 01-08, 01-09, 01-18) | 0004 — parser, OCR, isolamento | PDFium per testo, bbox e pagine; Docling solo sulle pagine con tabelle (CPU o GPU secondo i numeri); Tesseract 5.5 confermato salvo vittoria netta di PP-OCRv6. Processo figlio per documento con limiti calibrati, budget aggregato, tetti di decompressione e limite di pagine. Docling come sidecar su rete interna (D-07..D-09, D-28) | 3, 6, 14 |
| 01-09 | 0005 — locator di citazione | PDF = pagina + rettangoli normalizzati; DOCX/MD = `section_path` con ordinale + intervallo di caratteri su `cka-text-v1`; conversione in UTF-16 nel viewer (D-10, D-24) | 2 (`Locator` in contracts), 3, 8, 9 |
| 01-14 | 0006 — indice vettoriale | Ricerca esatta nell'MVP con predicato ACL nella stessa query; soglia misurata per passare a HNSW con `iterative_scan`; ibrida `tsvector` + RRF k=60 con ACL in entrambe le parti; pgvector ≥ 0.8.4 (D-12, D-15) | 3, 7, 14 |
| 01-10 | 0007 — embedding TEI su GPU | Modello scelto fra Qwen3-Embedding-0.6B (default), bge-m3 e multilingual-e5-large-instruct; serving TEI su CUDA; istruzione della query decisa; documenti e query separati (D-13, D-14) | 3, 6, 7 |
| 01-11 | 0008 — JEV: forma di `rank`, concorrenza, pin | Forma della chiamata (per candidato o in batch), concorrenza e retry sui 429, risoluzione degli alias, pin `jev-1.13.0` (D-17, D-22) | 4, 7, 8 |
| 01-15 | 0009 — reranker baseline | Scelta fra no-rerank, bge-reranker-v2-m3 su TEI GPU e JEV `rank`; budget VRAM della RTX 3050 per embedding, reranker e Docling (D-21) | 4, 7, 14 |
| 01-05 | 0010 — deadline e cancellazione HTTP | Deadline globale (`timeout_at` + `JoinSet`/`Semaphore`); nessun leak su 10.000 cicli h1/h2; verifica se JEV addebita il lavoro cancellato (D-22) | 2, 4 (adapter JEV, LLM, Embedder) |
| 01-12 | 0011 — LLM generativo, routing, structured output | Haiku 5.5 per le query semplici, Sonnet 5.5 per le complesse, con la complessità decisa da JEV in `validate_query`; fallback su Sonnet senza JEV, con risposta degradata o rifiutata secondo policy; JSON con citazioni per claim; tenuta alla prompt injection indiretta misurata (D-18..D-20) | 4, 8, 10 |
| 01-16 | 0012 — firme delle porte candidate | Le firme della §3, lo stile async e la sezione «Vincoli verso la Fondazione» | 2, 3, 4 |
| 01-17 | 0013 — rinvio di antivirus e CDR | Rinvio alla fase di hardening, con rischio residuo nel threat model (D-05) | 14 |

Il piano 01-17 promette anche:

- `DEPENDENCIES.md` aggiornato da Proposta/Aperta a Decisa per Garage, pgvector con ricerca esatta, embedding, parser/OCR, LLM, reranker, JEV `jev-1.13.0` e reqwest con Rustls; ogni regola superata resta con un paragrafo *Storia*;
- `docs/security/threat-model.md` compilato (PLAT-07), con le minacce T-01..T-28 più le nuove da T-29, i requisiti e i file di evidenza degli spike;
- la contabilità dei numeri ADR 0003-0013: un numero non emesso si dichiara, non si riassegna.

Gli oracoli di misura di 01-06 (`spikes/data/corpus/oracles/`) non sono voci dell'evaluation corpus. La fase 13 non può darli per acquisiti come golden.

### Limiti noti da documentare (D-25)

I PDF con stream `LZWDecode`/`RunLengthDecode` o con filtri ignoti, e quelli con object stream oltre il limite, si rifiutano con `Unsupported` o `DecompressionLimit`. Il limite lo dichiara 01-17 (Task 3): sezione `## Limiti noti` in ADR-0004, unica eccezione alla regola «solo Stato e Modifica» sugli ADR delle slice, e sezione `## Limiti noti` in `README.md` con il link all'ADR. Per lo stesso motivo un PDF benigno rifiutato in calibrazione è un'anomalia da portare all'utente, non un motivo per ritoccare la soglia (01-07).

### Dimensionamento estrapolato (D-28)

Il worker resta a 2 GiB. 01-13 riporta il `max_admitted` misurato per ogni concorrenza richiesta (1, 4, 8, 16), e ADR-0004 dichiara il dimensionamento della concorrenza OCR e parse per la produzione come **estrapolato** da quel dato, con formula e ipotesi esplicite: non è una misura. Le fasi 6 e 14 lo pianificano come stima da verificare.

### Vincoli verso la Fondazione (ADR-0012, `01-WIRING.md` §2)

- Il `Cargo.toml` alla radice ha `[workspace] exclude = ["spikes"]`. Nessun workflow CI copre `spikes/**`. Nessuna path dependency punta a `spikes/` da `services/`, `crates/` o `apps/` (`grep -r "spikes" --include=Cargo.toml services crates apps` deve restare vuoto).
- Le firme si trascrivono, non si importano.
- Collocazione per servizio:
  - document-service: `VectorIndex`, `AccessPolicy`, `retrieve_authorized`, `RetrievedEvidence`;
  - `crates/contracts`: `RetrievalSearchRequest`, `RetrievalSearchResponse`;
  - rag-service: `EvidenceRetriever` (con adapter client HTTP interno), `authorize_response`, `AuthorizedEvidence`, `Reranker`, `DecisionEngine`, `AnswerGenerator`.

  Nessuno dei due servizi importa i tipi interni dell'altro.
- Restano alle fasi proprietarie: la persistenza dell'idempotenza, l'outbox, il ricontrollo e l'invalidazione reale della cache, Redis, la verifica dei JWT e la derivazione dell'`AuthorizationScope`/`AuthorizationPredicate` da un `Principal` verificato.
- La fase identity/RBAC (5, 10) deve far crescere `acl_version` a ogni cambio di ruolo del soggetto.
- `docs/architecture/interfaces-and-concurrency.md` va riallineato alla divergenza dello split `Reranker`/`DecisionEngine` e alle nuove porte `EvidenceRetriever`, `CallContext` e `MutationContext`. ADR-0012 attribuisce l'aggiornamento «a 01-17 o alla Fondazione», ma 01-17 non elenca il file: in pratica spetta alla Fondazione.

## 7. Rischi residui da portare nel brief di execute

Ogni piano auditato ha l'ultimo audit GPT `APPROVED_WITH_WARNINGS`, con un solo rischio MEDIUM e nessun HIGH o CRITICAL.

| Piano | Rischio MEDIUM residuo | Cosa deve fare l'esecutore | Riferimento |
|---|---|---|---|
| 01-02 | Il manifest dei pesi Docling non vincola un inventario atteso: un preload parziale può passare | Fissare repository, revisione e lista dei file obbligatori; una cache vuota o con un artefatto mancante deve fallire prima della misura | `01-02-RISK-AUDIT-v2.md` |
| 01-04 | La soglia sul canale laterale della deduplica è sensibile al rumore | Fissare in anticipo numero di run, ordine alternato new/dedup, criterio di esclusione e intervallo di confidenza | `01-04-RISK-AUDIT-v2.md` |
| 01-07 | L'evidenza runtime dell'isolamento è demandata a 01-18 | 01-18 resta il gate di validazione di 01-07; una difformità va in gap-closure, non si corregge nell'onda | `01-07-RISK-AUDIT-v3.md` |
| 01-08 | Il riavvio del container Docling per documento non prova l'assenza di stato esterno (cache, volumi, GPU) | Enumerare volumi e processi per run, cache in sola lettura, test con un marker fra job | `01-08-RISK-AUDIT-v2.md` |
| 01-09 | La proiezione `cka-text-v1` può cambiare senza invalidare le fixture | Rendere la versione obbligatoria nel locator e aggiungere un test di migrazione negativa | `01-09-RISK-AUDIT-v2.md` |
| 01-13 | Il controllo di famiglia PP-OCRv6 può non identificare i pesi effettivamente caricati | Normalizzare la configurazione risolta e verificare gli hash dei file aperti dal loader, includendo lingua, tier e famiglie det/rec | `01-13-RISK-AUDIT-v3.md` |
| 01-16 | Il confine fra DTO e `AuthorizedEvidence` deve restare non aggirabile dal futuro adapter HTTP | `authorize_response` resta l'unica conversione; serve un test di contratto attraverso serde con i mismatch. Il vincolo si porta anche alla Fondazione | `01-16-RISK-AUDIT-v2.md` |
| 01-18 | M10 può approvare una concorrenza sotto pressione sostenuta sul cgroup (reclaim senza OOM) | Fissare prima delle misure una soglia su `memory.events:max` e `margin_kb`, oppure dichiarare che M10 prova solo l'assenza di OOM; non pubblicare una concorrenza come «sicura» sotto pressione | `01-18-RISK-AUDIT-v2.md` |

I piani 01-01, 01-03, 01-05, 01-06, 01-10, 01-11, 01-12, 01-14, 01-15 e 01-17 non hanno un audit GPT di rischio. I loro rischi stanno nei `0X-YY-TEST-DESIGN.md`.

**Modifica:** Claude (Anthropic) · 2026-10-19 · contratto della fase 1
