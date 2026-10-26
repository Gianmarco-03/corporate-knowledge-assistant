# ADR-0012 — Firme candidate delle porte di dominio

- Stato: Proposed
- Data: 2026-10-26
- Decisori: l'utente (ratifica nel consolidamento, piano 01-17); la proposta è di Claude (gsd-executor 01-16), dallo spike S-07
- Sostituisce: nessuno

**Modifica:** Claude (gsd-executor 01-16) · 2026-10-26 · ADR proposto con le firme dello spike S-07

## Contesto

[ADR-0001](0001-system-boundaries-and-stack.md) fissa tre servizi di dominio e chiede che i casi d'uso dipendano da trait-porta interni, senza tipi di provider, SDK o database in firma. [`docs/architecture/interfaces-and-concurrency.md`](../architecture/interfaces-and-concurrency.md) elenca undici porte in pseudo-Rust, con `Result<T>` generico e `rank` dentro `DecisionEngine`. Gli spike S-01..S-06 hanno misurato variazioni a quelle firme, ciascuna scritta nella sezione «Impatto su porte (S-07)» del proprio RISULTATI:

- S-01: chiave sha256 precomputata, `put` idempotente, `delete` per il GC ([ADR-0003](0003-storage-originali-garage.md));
- S-02: blocchi con `Locator` a due varianti ([ADR-0005](0005-locator-citazione.md)), errori tipizzati per gli esiti ostili, quarantena ([ADR-0004](0004-parser-ocr-isolamento.md));
- S-03: embedding di documenti e query separati, spazio di embedding completo, predicato autorizzativo dentro la query del ranking ([ADR-0006](0006-indice-vettoriale-ricerca-esatta-ibrida.md), [ADR-0007](0007-embedding-tei-gpu.md));
- S-04: rank come segnale separato, baseline no-rerank, versione risolta nei metadati ([ADR-0008](0008-jev-forma-rank-concorrenza-pin.md), [ADR-0009](0009-reranker-baseline.md));
- S-05: deadline esplicita passata a ogni porta che chiama un provider ([ADR-0010](0010-deadline-cancellazione-http.md));
- S-06: complessità della query per il routing, risposta strutturata con citazioni per claim ([ADR-0011](0011-llm-generativo-routing-structured-output.md)).

L'audit dei rischi del piano ([risk_audit_7](../../.planning/phases/01-spike-tecnici/01-16-RISK-AUDIT.md), poi [risk_audit_14](../../.planning/phases/01-spike-tecnici/01-16-RISK-AUDIT-v2.md) sulla versione 2) ha chiesto quattro garanzie da esprimere nei tipi: solo evidenza autorizzata verso reranker, JEV e LLM (RISK-001); cache con scope autorizzativo (RISK-002); mutazioni idempotenti con esito sconosciuto distinto (RISK-003); spazio di embedding coerente fra query e indice (RISK-004). Il re-gate ha aggiunto che `VectorIndex` e i consumatori semantici stanno in servizi diversi, separati dal confine HTTP `POST /internal/v1/retrieval/search`, e che i testi non devono comparire nel `Debug`.

Le porte devono essere usabili come `Arc<dyn Trait>`, perché la composizione sceglie l'adapter a runtime (JEV o baseline, Sonnet o Haiku). Un `async fn` nativo in un trait non è dyn-compatibile.

## Opzioni considerate

1. **Stile async: crate `async-trait`.**
   - Vantaggi: le firme si leggono come funzioni `async` normali; la macro genera `Pin<Box<dyn Future + Send + '_>>` e il vincolo `Send`, quindi ogni trait è dyn-compatibile senza scriverlo a mano; è lo stile più diffuso, e gli errori di lifetime sui riferimenti in argomento li risolve la macro.
   - Svantaggi: un'allocazione per chiamata, una dipendenza (già dichiarata nel workspace degli spike) e messaggi d'errore del compilatore talvolta meno chiari.
2. **Stile async: boxing esplicito**, con `fn search<'a>(&'a self, …) -> Pin<Box<dyn Future<Output = …> + Send + 'a>>`.
   - Vantaggi: nessuna macro, il tipo di ritorno è visibile.
   - Svantaggi: tredici trait con lifetime espliciti su ogni metodo che prende riferimenti, `Box::pin(async move { … })` in ogni implementazione, fake compresi; stessa allocazione dell'opzione 1. Il guadagno è nullo a runtime e il costo in leggibilità è alto.
3. **Trait larghi** (un trait per servizio, o `rank` dentro `DecisionEngine` come in `interfaces-and-concurrency.md`) **contro trait per client.**
   - Larghi: meno tipi, ma la baseline no-rerank e un eventuale adapter TEI dovrebbero implementare `validate_query` e `assess_evidence`, che non sanno servire (violazione di ISP e LSP).
   - Per client: un trait per ciò che il caso d'uso chiede, e ogni adapter implementa solo ciò che sa fare.
4. **Prova di autorizzazione nel tipo contro contratto all'adapter.**
   - Contratto all'adapter: le porte semantiche ricevono `&[Candidate]` e la documentazione chiede di passare solo candidati autorizzati. Una regressione si scopre solo con un test d'integrazione, se c'è.
   - Prova nel tipo: le porte ricevono `&AuthorizedEvidence`, che si costruisce solo dopo il ricontrollo; passare altro non compila.

## Decisione

**Stile async: `async-trait`** (opzione 1). Un'allocazione per chiamata è trascurabile rispetto all'I/O misurato dagli spike (decine o centinaia di millisecondi verso JEV, LLM e TEI, millisecondi verso PostgreSQL e Garage), e la leggibilità di tredici trait e dei loro fake conta di più. Se un giorno la toolchain stabile rende dyn-compatibile `async fn` nei trait, la migrazione è meccanica e non cambia il contratto. `Chunker` resta sincrono e senza contesto: è CPU-bound puro, non chiama provider, e la deadline la applica il chiamante che lo esegue in `spawn_blocking`.

**Trait per client** (opzione 3): `rank` esce da `DecisionEngine` e diventa il trait stretto `Reranker`. L'adapter JEV implementa entrambi i trait; la baseline no-rerank e un eventuale adapter TEI bge-reranker implementano solo `Reranker`. Ogni trait ha al più 6 metodi; nello spike il massimo è 3.

**Prova di autorizzazione nel tipo** (opzione 4), sui due lati del confine di retrieval.

Regole delle firme, valide per tutte le porte (forma in [`spikes/s07-ports/src/lib.rs`](../../spikes/s07-ports/src/lib.rs), variazioni una per una in [`spikes/s07-ports/RISULTATI.md`](../../spikes/s07-ports/RISULTATI.md)):

1. **Nessun tipo di provider, SDK o database in firma.** Byte come `Bytes` e stream boxed di `futures`, identificativi come UUID opachi, nessun tipo di `object_store`, `sqlx`, `reqwest`, `pgvector`, `pdfium-render`, `axum` od `opentelemetry`.
2. **Errori tipizzati**, un enum per porta, niente `anyhow`. Un errore non diventa mai un segnale, un ordine o una decisione di default.
3. **Contesti espliciti.** Ogni lettura verso un provider riceve `CallContext { request_id, trace, deadline }`, con `TraceContext` come valore puro (`traceparent` W3C validato). La `deadline` è uno `std::time::Instant`: ADR-0010 ammette «un contesto di query che la contiene», e `CallContext` è quel contesto; l'istante resta `std` per non mettere `tokio` in firma, e l'adapter lo converte con `tokio::time::Instant::from_std` (motivazione in `spikes/s07-ports/RISULTATI.md`, Anomalie). Ogni mutazione (`DocumentStore::put` e `delete`, `VectorIndex::publish_version`, `AuditSink::append`, `Cache::invalidate`) riceve `MutationContext { call, idempotency_key, actor }` e restituisce `MutationOutcome::{Applied, AlreadyApplied}`; l'errore distingue `NotApplied { reason }` da `OutcomeUnknown`, perché un timeout non implica rollback e il chiamante ritenta con la stessa chiave. `Cache::put` riceve solo `CallContext`: sovrascrivere la stessa chiave con lo stesso valore è idempotente per costruzione, e una voce persa costa solo un ricalcolo.
4. **`AuthorizedEvidence` è l'unico input dei consumatori semantici** (`Reranker::rank`, `DecisionEngine::{assess_evidence, validate_citations}`, `AnswerGenerator::generate`). La catena è una sola:
   - lato documento, `retrieve_authorized(&dyn VectorIndex, ctx, query, &AuthorizationPredicate, k)` rifiuta il mismatch di spazio prima di `search`, cerca col predicato, ricontrolla ogni candidato (pubblicato, stessa versione ACL, soggetto o ruolo ammesso), scarta doppioni e oltre-k, e produce `RetrievedEvidence`, la cui unica uscita è il DTO `RetrievalSearchResponse { subject_id, acl_version, space, passages }`;
   - lato RAG, `EvidenceRetriever::search` restituisce quel DTO come dato non fidato, e `authorize_response(response, &AuthorizationScope, &EmbeddingSpace)` è l'**unica** conversione verso `AuthorizedEvidence`: rifiuta soggetto, versione ACL o spazio diversi da quelli del principal della richiesta e i `PassageRef` duplicati, senza evidenza parziale;
   - `AuthorizedEvidence` e `RetrievedEvidence` hanno campi e costruttore privati e nessun `Default`; `AuthorizedEvidence` e `AuthorizedPassage` non implementano `Serialize` né `Deserialize`, così nessun adapter può deserializzare direttamente un tipo semantico; `reorder` può solo permutare o togliere, `resolve` mappa una citazione solo a un passaggio presente.
5. **Redazione.** Evidenze, passaggi, candidati, claim e risposta strutturata hanno `Debug` manuale che stampa identificativi e conteggi e mai testi.
6. **`CacheKey` con scope.** `CacheKey::new(AuthorizationScope { subject_id, acl_version }, corpus_version, space, pipeline_version, query_digest)`, tutti obbligatori. **`acl_version` è la versione monotona dello stato autorizzativo del soggetto e cambia anche a ogni cambio dei suoi ruoli**, invece di aggiungere un digest dei ruoli alla chiave. I ruoli sono solo uno degli ingressi dell'autorizzazione (anche le ACL dei documenti la cambiano), quindi un digest dei ruoli non basterebbe e servirebbe comunque una versione; con una sola versione l'invalidazione resta `InvalidationScope::AclVersion`, la chiave non espone la composizione dei ruoli e lo stesso valore viaggia in `RetrievalSearchResponse` per il ricontrollo lato RAG. Una voce letta si ricontrolla contro l'autorizzazione corrente prima del consumo; il primo adapter resta `NoOpCache`.
7. **`EmbeddingSpace` completo** (`model_id`, `revision`, `dimension`, `normalization`, `pooling`, istruzioni di query e di documento) su vettori, bozze di versione e indice; la dimensione da sola non identifica lo spazio (bge-m3 e Qwen3 hanno entrambi 1024 dimensioni).
8. **Tipi destinati a `crates/contracts`:** `Locator`, `ContentHash`, identificativi opachi, `RequestId`, `IdempotencyKey`, `TraceContext`, `EmbeddingSpace`, `AclVersion`, `RetrievalSearchRequest`, `RetrievalSearchResponse`. I trait, gli errori e i tipi di evidenza restano interni ai servizi.

## Conseguenze

- Positive:
  - un passaggio non autorizzato non raggiunge reranker, JEV o LLM senza che il compilatore lo segnali (T-13, T-14); i doctest `compile_fail` e i test con fake positivi lo provano su entrambi i lati;
  - cache, mutazioni e spazio di embedding hanno le garanzie dell'audit nei tipi, non nella documentazione;
  - ogni adapter implementa solo ciò che sa servire, e il ripiego JEV → baseline è composizione di due `dyn Reranker`.
- Negative:
  - **divergenza da [`interfaces-and-concurrency.md`](../architecture/interfaces-and-concurrency.md)**: `rank` è in `Reranker`, compaiono `EvidenceRetriever` e la coppia `retrieve_authorized`/`authorize_response`, cambiano contesti ed errori. Il documento si aggiorna nel piano 01-17 o nella Fondazione, non qui;
  - un'allocazione per chiamata di porta (`async-trait`);
  - più tipi da trascrivere (contesti, scope, evidenze sui due lati).
- Follow-up:
  - ratifica in 01-17; aggiornamento di `interfaces-and-concurrency.md`;
  - la riga `async-trait` di `DEPENDENCIES.md` la aggiorna 01-17.

## Vincoli verso la Fondazione

Dal WIRING di fase 1 ([`01-WIRING.md`](../../.planning/phases/01-spike-tecnici/01-WIRING.md) §2) e dall'audit:

- `Cargo.toml` radice con `[workspace] exclude = ["spikes"]`; nessun workflow CI su `spikes/**`; nessuna path dependency verso `spikes/` da `services/`, `crates/`, `apps/` (`grep -r "spikes" --include=Cargo.toml services crates apps` vuoto).
- Le firme si **trascrivono**, non si importano: `spikes/s07-ports` resta codice usa-e-getta.
- Collocazione per servizio: document-service ospita `VectorIndex`, `AccessPolicy`, `retrieve_authorized` e `RetrievedEvidence`; `crates/contracts` ospita `RetrievalSearchRequest` e `RetrievalSearchResponse` (con gli altri tipi della regola 8); rag-service ospita `EvidenceRetriever` (adapter: client HTTP interno), `authorize_response`, `AuthorizedEvidence`, `Reranker`, `DecisionEngine` e `AnswerGenerator`. Nessuno dei due servizi importa i tipi interni dell'altro.
- **Confine DTO → tipo autorizzato (RISK-001 v2, MEDIUM, [risk_audit_14](../../.planning/phases/01-spike-tecnici/01-16-RISK-AUDIT-v2.md)).** L'adapter HTTP di `EvidenceRetriever` deserializza solo `RetrievalSearchResponse` e lo restituisce; `authorize_response` resta l'unica conversione verso `AuthorizedEvidence`, che non riceve mai `Deserialize`, un costruttore pubblico o un `Default`. Il caso d'uso chiama `authorize_response` con lo scope del principal verificato della richiesta, prima di qualunque `Reranker` o `AnswerGenerator`. La Fondazione porta nel prodotto un test di contratto che attraversa serde (JSON con soggetto, versione ACL o spazio alterati rifiutato prima dei consumatori semantici) e la verifica che il testo dei passaggi non compaia nel `Debug`, come `t3_json_round_trip_document_side_to_rag_side` e `t5_debug_redacts_sentinel_text` nello spike.
- Persistenza dell'idempotenza, outbox, ricontrollo delle voci di cache, invalidazione reale e derivazione dell'`AuthorizationPredicate` e dell'`AuthorizationScope` da un `Principal` verificato (JWT) restano alle fasi proprietarie (Fondazione, provider, identity/RBAC).
- L'incremento di `acl_version` a ogni cambio dei ruoli di un soggetto (oltre che a ogni cambio delle ACL che lo riguardano) passa alla fase identity/RBAC.

## Evidenze

- [`spikes/s07-ports/RISULTATI.md`](../../spikes/s07-ports/RISULTATI.md): soglie P-1..P-15, variazioni per porta con la fonte, risultati con il nome dei test, anomalie ed esito.
- [`spikes/s07-ports/src/lib.rs`](../../spikes/s07-ports/src/lib.rs): firme, fake di contratto, 45 test unitari e 5 doctest `compile_fail`.
- Comandi da `spikes/`: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`, `cargo test -p s07-ports --locked --offline`, tutti verdi.
- Hardware non rilevante: è una verifica di compilazione e di test unitari, senza misure, quindi non c'è un `hw.json`.

**Modifica:** Claude (Anthropic) · 2026-10-26 · registrata la scelta di `std::time::Instant` per la deadline, segnalata dal bridge di 01-16
