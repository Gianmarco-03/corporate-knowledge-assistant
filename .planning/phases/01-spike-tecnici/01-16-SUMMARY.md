---
phase: 01-spike-tecnici
plan: 16
subsystem: spike-firme-porte-s07-adr-0012
tags: [s07-ports, porte, trait, async-trait, dyn-compatibilita, arc-dyn, isp, reranker, decision-engine, evidence-retriever, authorized-evidence, retrieved-evidence, retrieve-authorized, authorize-response, dto, serde, compile-fail, doctest, debug-redatto, cache-key, acl-version, mutation-context, outcome-unknown, idempotency, embedding-space, traceparent, adr-0012]

# Dependency graph
requires:
  - phase: 01-04
    provides: "S-01 «Impatto su porte»: chiave sha256 precomputata, put idempotente, delete per il GC (ADR-0003, D-02)"
  - phase: 01-05
    provides: "S-02 «Impatto su porte»: blocchi con Locator a due varianti (ADR-0005), errori tipizzati degli esiti ostili, quarantena (ADR-0004, D-05, D-08, D-10)"
  - phase: 01-12
    provides: "S-06 «Impatto sulle porte»: complessità della query per il routing (D-18), risposta strutturata con citazioni per claim (D-20), ADR-0011"
  - phase: 01-13
    provides: "S-03 «Impatto su porte»: predicato dentro la query del ranking, spazio di embedding completo, embedding query/documenti separati (ADR-0006, ADR-0007, D-12, D-14, D-38)"
  - phase: 01-14
    provides: "S-03/S-05: ricerca esatta, deadline esplicita e cancellazione per drop (ADR-0010, D-22)"
  - phase: 01-15
    provides: "S-04: Reranker stretto con adapter JEV forma A e baseline identità, versione risolta nei metadati (ADR-0008, ADR-0009)"
provides:
  - "Crate spikes/s07-ports (src/lib.rs, 3.421 righe): firme Rust candidate di 13 trait-porta (DocumentSource, DocumentStore, DocumentParser, Chunker, Embedder, VectorIndex, DecisionEngine, Reranker, AnswerGenerator, AccessPolicy, AuditSink, Cache, EvidenceRetriever), tutti dyn-compatibili via async-trait salvo Chunker sincrono"
  - "Evidenza autorizzata sui due lati del confine POST /internal/v1/retrieval/search: evidence::document_side (retrieve_authorized → RetrievedEvidence), evidence::wire (RetrievalSearchRequest/RetrievalSearchResponse), evidence::rag_side (EvidenceRetriever, authorize_response → AuthorizedEvidence)"
  - "Contesti CallContext/MutationContext, MutationOutcome con OutcomeUnknown distinto da NotApplied, TraceContext W3C validato, CacheKey con AuthorizationScope obbligatorio, EmbeddingSpace completo"
  - "45 test unitari (40 del test-write + 5 dell'esecutore) e 5 doctest compile_fail (E0451 ×2, E0277 ×2, E0061)"
  - "spikes/s07-ports/RISULTATI.md: soglie P-1..P-15 (19a7304, prima di lib.rs), variazioni per porta con fonte, risultati, anomalie, esito"
  - "docs/adr/0012-firme-porte-candidate.md (Proposed): stile async-trait, trait per client, prova di autorizzazione nel tipo, una sezione «Vincoli verso la Fondazione»"
affects: [01-17, ADR-0012, ADR-0010, ADR-0006, docs/architecture/interfaces-and-concurrency.md, DEPENDENCIES.md riga async-trait, Fondazione (fase 2), crates/contracts, document-service, rag-service, fase identity/RBAC, threat model T-13 T-14]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Prova di autorizzazione nel tipo: struct con campi privati e unico costruttore dentro il modulo che fa il ricontrollo; i consumatori semantici accettano solo quel tipo"
    - "Confine HTTP fra servizi modellato in tre sottomoduli (lato produttore, DTO, lato consumatore) che non si nominano a vicenda se non attraverso il DTO"
    - "DTO di confine trattato come dato non fidato: una sola funzione deterministica lo converte nel tipo autorizzato, ricontrollando soggetto, versione ACL e spazio"
    - "Debug manuale che stampa identificativi e conteggi (text_chars) e mai testi, verificato con un testo sentinella"
    - "Vincolo di forma provato con doctest compile_fail, accompagnato da una variante positiva che compila"

key-files:
  created:
    - spikes/s07-ports/RISULTATI.md
    - docs/adr/0012-firme-porte-candidate.md
  modified:
    - spikes/s07-ports/src/lib.rs
    - spikes/s07-ports/Cargo.toml
    - spikes/Cargo.lock

key-decisions:
  - "Stile async: crate async-trait invece del boxing esplicito dei future (leggibilità di 13 trait e dei fake; un'allocazione per chiamata è trascurabile rispetto all'I/O); Chunker resta sincrono e senza contesto (ADR-0012 «Decisione»)"
  - "rank esce da DecisionEngine nel trait stretto Reranker (ISP/LSP): JEV implementa entrambi, baseline identità ed eventuale TEI solo Reranker"
  - "AuthorizedEvidence unico input di Reranker, DecisionEngine::{assess_evidence, validate_citations}, AnswerGenerator; si costruisce solo in authorize_response dopo il ricontrollo di subject_id, acl_version e space della RetrievalSearchResponse"
  - "acl_version del candidato accettata solo per uguaglianza col predicato (deny by default anche su una versione più recente); doppioni di PassageRef scartati e tetto k lato documento"
  - "acl_version cambia anche al cambio dei ruoli del soggetto, invece di un digest dei ruoli nella CacheKey (vincolo alla fase identity/RBAC)"
  - "query_text tolto da VectorIndex::search e retrieve_authorized (decisione del coordinatore, ADR-0006 non adotta l'ibrida), con paragrafo Storia in RISULTATI; resta nel DTO RetrievalSearchRequest"
  - "Deadline su std::time::Instant (non tokio::time::Instant come scrive ADR-0010): tokio fuori dalle firme, l'adapter converte con from_std"

patterns-established:
  - "Firme di porta in uno spike: forma provata con cargo check, Arc<dyn Trait + Send + Sync> per ogni trait, fake positivi nei test e doctest compile_fail per i divieti; nessun adapter reale"
  - "Le firme si trascrivono nella Fondazione, non si importano da spikes/"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "S-07 produce le firme Rust candidate di tutte le porte e il crate compila (criterio 4); ogni trait è usabile come Arc<dyn Trait>"
    verification:
      - {test: TEST-1, risks: [RISK-001], ref: "spikes/s07-ports/src/lib.rs:2352#t1_dyn_document_source ; :2361#t1_dyn_document_store ; :2373#t1_dyn_document_parser ; :2386#t1_dyn_chunker ; :2395#t1_dyn_embedder ; :2405#t1_dyn_vector_index ; :2417#t1_dyn_decision_engine ; :2429#t1_dyn_reranker ; :2439#t1_dyn_answer_generator ; :2447#t1_dyn_access_policy ; :2463#t1_dyn_audit_sink ; :2474#t1_dyn_cache ; :2484#t1_dyn_evidence_retriever", status: pass, evidence: "rieseguiti dal bridge sul main fuso 0cd4c42 (cargo test -p s07-ports --locked --offline da spikes/: 45 passati, 0 falliti; 5 doctest passati); ogni test costruisce un Arc<dyn Trait + Send + Sync> dal fake Failing (lib.rs:2092) che restituisce un errore tipizzato, nessun unimplemented!/todo! nel file (grep del <verify> vuoto); grep -c \"Arc<dyn\" = 19 ≥ 13"}
      - {test: "blocco <verify> del Task 2", risks: [RISK-001], ref: ".planning/phases/01-spike-tecnici/01-16-PLAN.md:165", status: pass, evidence: "rilanciato dal bridge da spikes/ tale e quale: exit 0 (cargo check --workspace, cargo test -p s07-ports, grep dei crate proibiti vuoto, [dependencies] nell'elenco chiuso, Debug manuali presenti, nessun derive Debug/Serialize vietato, compile_fail = 6 occorrenze, git diff --quiet -- Cargo.toml, link a RISULTATI in ADR-0012)"}
      - {test: "conteggio metodi per trait (P-5)", risks: [], ref: "spikes/s07-ports/src/lib.rs:578 ; :838 ; :1085 ; :1203 ; :1221 ; :1325 ; :1426 ; :1453 ; :1512 ; :1581 ; :1628 ; :1760 ; :1813", status: pass, evidence: "contati dal bridge: DocumentSource 2, DocumentStore 3, DocumentParser 1, Chunker 1, Embedder 3, VectorIndex 3, DecisionEngine 3, Reranker 1, AnswerGenerator 1, AccessPolicy 2, AuditSink 1, Cache 3, EvidenceRetriever 1; massimo 3, soglia 6; coincide col rapporto"}
  - must_have: "rank estratto da DecisionEngine nel trait stretto Reranker; DecisionEngine conserva validate_query, assess_evidence, validate_citations"
    verification:
      - {test: "controllo a vista", risks: [RISK-001], ref: "spikes/s07-ports/src/lib.rs:1424-1461", status: pass, evidence: "letto dal bridge: Reranker (1452-1461) ha il solo rank; DecisionEngine (1425-1449) non ha fn rank. Parametri dei consumatori semantici: Reranker::rank (ctx, &UserQuery, &AuthorizedEvidence), assess_evidence (ctx, &UserQuery, &AuthorizedEvidence), validate_citations (ctx, &AuthorizedEvidence, &StructuredAnswer), AnswerGenerator::generate (ctx, &UserQuery, &AuthorizedEvidence) a lib.rs:1514-1519: nessun Candidate, PassageRef sciolto o testo di passaggio. UserQuery è il testo della domanda dell'utente (QueryText, Debug redatto a lib.rs:136-140), non un passaggio"}
  - must_have: "Le firme incorporano le variazioni misurate dagli spike (embed_documents/embed_query, complessità D-18, locator a due varianti D-10, citazioni per claim D-20, chiave sha256 D-02, errori tipizzati del parser D-08, deadline D-22)"
    verification:
      - {test: "controllo a vista contro RISULTATI «Variazioni per porta»", risks: [], ref: "spikes/s07-ports/RISULTATI.md:50-83 ; spikes/s07-ports/src/lib.rs:287-296 ; :385-394 ; :580-594 ; :1087-1107 ; :1378-1391 ; :1477-1498", status: pass, evidence: "letto dal bridge: ogni riga della tabella ha la sua firma in lib.rs; 26+ link relativi di RISULTATI e i link di ADR-0012 risolti tutti su file esistenti (script del bridge, nessun mancante); Locator a spans (al più due) invece di { page, rects } del piano, secondo ADR-0005; ParseError con OutputLimit in più, dichiarato in RISULTATI:56"}
  - must_have: "Evidenza autorizzata sui due lati del confine: retrieve_authorized lato documento, authorize_response lato RAG; un mismatch non produce AuthorizedEvidence"
    verification:
      - {test: TEST-2, risks: [RISK-001], ref: "spikes/s07-ports/src/lib.rs:2499#t2_retrieve_authorized_drops_other_subject_stale_acl_and_unpublished ; :2560#t2_retrieve_authorized_keeps_order_and_text_of_authorized_candidates ; :3287#x_retrieve_authorized_drops_duplicate_candidates_and_caps_at_k ; :3324#x_retrieve_authorized_propagates_index_error", status: pass, evidence: "verdi alla riesecuzione del bridge; FakeIndex positivo (lib.rs:2035-2087) restituisce anche candidati di altro soggetto, ACL 6 su 7 e non pubblicati: restano solo by_subject e by_role, anche nel DTO derivato; ricontrollo in admits (lib.rs:722-731), dedupe e take(k) a lib.rs:751-763"}
      - {test: TEST-3, risks: [RISK-001], ref: "spikes/s07-ports/src/lib.rs:2594#t3_authorize_response_rejects_other_subject ; :2607#t3_authorize_response_rejects_other_acl_version ; :2620#t3_authorize_response_rejects_other_space ; :2634#t3_authorize_response_rejects_duplicate_passage_ref ; :2648#t3_authorize_response_accepts_consistent_response ; :2676#t3_json_round_trip_document_side_to_rag_side", status: pass, evidence: "verdi alla riesecuzione del bridge; il giro document_side → serde_json → rag_side passa la risposta coerente e rifiuta subject_id, acl_version e space.revision alterati nel JSON (test_hint di risk_audit_14); authorize_response a lib.rs:1005-1039 controlla nell'ordine soggetto, ACL, spazio, doppioni, senza evidenza parziale"}
      - {test: "separazione dei lati (P-9), controllo a vista", risks: [RISK-001], ref: "spikes/s07-ports/src/lib.rs:601-772 ; :821-1040", status: pass, evidence: "grep del bridge sulle righe dei due sottomoduli: document_side non nomina AuthorizedEvidence, Reranker, DecisionEngine, AnswerGenerator né rag_side; rag_side non nomina VectorIndex, AuthorizationPredicate né document_side (entrambi vuoti); decision e answer importano l'evidenza solo da evidence::rag_side (lib.rs:1354, :1473)"}
      - {test: "RISK-001 v2 residuo: adapter HTTP reale di EvidenceRetriever", risks: [RISK-001], ref: "docs/adr/0012-firme-porte-candidate.md:83 ; .planning/phases/01-spike-tecnici/01-16-RISK-AUDIT-v2.md", status: human_judgment, human_judgment: true, rationale: "il rischio dell'audit riguarda un adapter futuro che deserializzi o costruisca direttamente un tipo semantico: nello spike l'adapter non esiste e la garanzia è provata solo con un fake e col giro serde_json. ADR-0012 lo registra come vincolo verso la Fondazione (deserializzare solo il DTO, authorize_response unica conversione, test serde e Debug da portare nel prodotto); la verifica spetta alla fase che scrive l'adapter"}
  - must_have: "Costruire AuthorizedEvidence fuori da evidence::rag_side non compila; Debug manuale redatto su AuthorizedEvidence, AuthorizedPassage, Candidate, Claim, StructuredAnswer; nessun Serialize su AuthorizedEvidence/AuthorizedPassage"
    verification:
      - {test: TEST-4, risks: [RISK-001], ref: "spikes/s07-ports/src/lib.rs:666#doctest_compile_fail_E0451_RetrievedEvidence ; :891#doctest_compile_fail_E0451_AuthorizedEvidence ; :903#doctest_compile_fail_E0277_Serialize ; :910#doctest_compile_fail_E0277_DeserializeOwned ; :1677#doctest_compile_fail_E0061_CacheKey", status: pass, evidence: "5 doctest passati alla riesecuzione; i codici d'errore di compile_fail rustdoc li controlla solo su nightly, quindi il bridge ha compilato i cinque snippet in un crate esterno (path dependency, target in spikes/target/cfprobe, poi rimosso): falliscono con E0451 (campi privati di RetrievedEvidence e di AuthorizedEvidence), E0277 (Serialize, DeserializeOwned non implementati), E0061 (CacheKey::new con 4 argomenti su 5); le varianti positive compilano (RetrievedEvidence::passages, AuthorizedEvidence::{subject_id, acl_version, passages}, Serialize e DeserializeOwned di RetrievalSearchResponse, CacheKey::new con AuthorizationScope). I doctest sono identici fra 6feb6a0 e HEAD"}
      - {test: "costruttori pubblici e Default", risks: [RISK-001], ref: "spikes/s07-ports/src/lib.rs:675-706 ; :914-989", status: pass, evidence: "letto dal bridge: RetrievedEvidence e AuthorizedEvidence hanno campi privati, nessun fn new o From pubblico, nessun impl/derive Default (gli unici Default del file sono ParsedDocument, Usage e due fake di test); l'unico letterale AuthorizedEvidence { fuori dai doctest è in authorize_response e in reorder, che ricostruisce da un'evidenza già autorizzata"}
      - {test: TEST-5, risks: [RISK-001], ref: "spikes/s07-ports/src/lib.rs:2754#t5_debug_redacts_sentinel_text ; :2797#t5_debug_redacts_sentinel_on_document_side ; :2826#t5_reorder_rejects_unknown_passage_ref ; :2846#t5_reorder_only_permutes_or_drops ; :2871#t5_resolve_maps_only_known_refs ; :3234#x_reorder_rejects_duplicate_and_non_finite_scores ; :3263#x_reorder_is_stable_on_equal_scores", status: pass, evidence: "verdi alla riesecuzione; la sentinella non compare nel Debug di AuthorizedEvidence, AuthorizedPassage, Candidate, Claim, StructuredAnswer, RetrievalSearchResponse, RetrievedPassageDto, RetrievedEvidence, RetrievedPassage; Debug manuali a lib.rs:652, :708, :809, :877, :991, :1260, :1483, :1500; reorder a lib.rs:948-983 (InvalidScore a :956)"}
      - {test: "Locator nel Debug di tipi fuori elenco", risks: [RISK-001], ref: "spikes/s07-ports/src/lib.rs:1166-1174 ; :1273-1280 ; :1283-1288", status: human_judgment, human_judgment: true, rationale: "il Debug manuale di Block stampa il locator (lib.rs:1171) e IndexedChunk e IndexVersionDraft derivano Debug con il Locator: la variante Text contiene i titoli di sezione, che RISULTATI «Anomalie» chiama testo del documento per giustificare l'omissione del locator dai passaggi. Questi tipi non sono nell'elenco del piano né in P-10, quindi nessun criterio è violato, ma la regola non è applicata in modo uniforme; da decidere nella trascrizione della Fondazione"}
  - must_have: "CacheKey con scope autorizzativo, versioni e spazio; MutationContext con NotApplied distinto da OutcomeUnknown; EmbeddingSpace completo e mismatch rifiutato prima della ricerca"
    verification:
      - {test: TEST-6, risks: [RISK-001], ref: "spikes/s07-ports/src/lib.rs:2894#t6_cache_key_differs_for_manager_and_guest ; :2903#t6_cache_key_differs_when_acl_version_increments_after_role_change ; :2912#t6_cache_key_differs_by_corpus_version_and_space ; :2975#t6_cache_fake_does_not_serve_entry_after_revocation", status: pass, evidence: "verdi alla riesecuzione; copre anche RISK-002 di risk_audit_7 (01-16-RISK-AUDIT.md); CacheKey::new a lib.rs:1697-1711, tutti i campi obbligatori"}
      - {test: TEST-7, risks: [RISK-001], ref: "spikes/s07-ports/src/lib.rs:3038#t7_audit_retry_after_outcome_unknown_applies_once ; :3063#t7_audit_rejects_event_id_different_from_idempotency_key ; :3085#t7_publish_version_retry_after_outcome_unknown_publishes_once", status: pass, evidence: "verdi alla riesecuzione; copre RISK-003 di risk_audit_7; i fake applicano e poi restituiscono OutcomeUnknown, il retry con la stessa chiave dà AlreadyApplied con un solo effetto, una chiave diversa due"}
      - {test: TEST-8, risks: [RISK-001], ref: "spikes/s07-ports/src/lib.rs:3112#t8_retrieve_authorized_rejects_other_model_same_dimension_before_search ; :3136#t8_retrieve_authorized_rejects_other_revision_before_search ; :3158#t8_retrieve_authorized_accepts_same_space ; :3177#t8_embedding_new_rejects_wrong_dimension ; :3190#t8_index_version_draft_rejects_chunk_of_other_space", status: pass, evidence: "verdi alla riesecuzione; copre RISK-004 di risk_audit_7; il confronto di spazio è a lib.rs:743-745, prima di search (contatore del fake a 0)"}
      - {test: "traceparent W3C", risks: [], ref: "spikes/s07-ports/src/lib.rs:3208#x_trace_context_rejects_malformed_traceparent", status: pass, evidence: "verde; TraceContext::parse a lib.rs:209-246"}
  - must_have: "Nessun tipo di provider, SDK o database attraversa una firma"
    verification:
      - {test: TEST-9, risks: [RISK-001], ref: "spikes/s07-ports/src/lib.rs:3368#t9_manifest_dependencies_are_in_closed_list ; :3387#t9_use_roots_are_std_crate_or_allowed_dependencies", status: pass, evidence: "verdi alla riesecuzione; i nomi proibiti non compaiono nei test per non far scattare il grep (scelta del test-write), quindi la copertura del nome proibito è il grep -w del <verify>, rilanciato dal bridge: vuoto. [dependencies] = async-trait, futures, bytes, uuid, serde; [dev-dependencies] = serde_json; spikes/Cargo.toml invariato fra a7341dc e HEAD"}
  - must_have: "Scelta fra async-trait e boxing esplicito decisa e registrata in ADR-0012"
    verification:
      - {test: "ADR-0012", risks: [], ref: "docs/adr/0012-firme-porte-candidate.md:25-44", status: pass, evidence: "letto dal bridge: Opzioni 1-4 con vantaggi e svantaggi, Decisione async-trait con motivo e Chunker sincrono; Stato Proposed; una sola sezione «Vincoli verso la Fondazione» (grep -c = 1); link tutti esistenti"}
      - {test: "ratifica di ADR-0012", risks: [], ref: "docs/adr/0012-firme-porte-candidate.md:3-5", status: human_judgment, human_judgment: true, rationale: "l'ADR è Proposed; la decisione sullo stile async e sulle regole delle firme è dell'utente nel consolidamento (01-17)"}
      - {test: "Deadline std::time::Instant contro ADR-0010", risks: [], ref: "spikes/s07-ports/src/lib.rs:170-198 ; docs/adr/0010-deadline-cancellazione-http.md:54 ; spikes/s07-ports/RISULTATI.md:156", status: human_judgment, human_judgment: true, rationale: "ADR-0010 scrive tokio::time::Instant «o un contesto di query che la contiene»; la firma usa std::time::Instant dentro CallContext per tenere tokio fuori dalle porte. La motivazione sta solo in RISULTATI «Anomalie», non in ADR-0012; serve una decisione (allineare il testo di ADR-0010 o la firma) in 01-17"}
      - {test: "query_text in RetrievalSearchRequest", risks: [RISK-001], ref: "spikes/s07-ports/src/lib.rs:785-789 ; spikes/s07-ports/RISULTATI.md:83", status: human_judgment, human_judgment: true, rationale: "il DTO porta il testo della domanda verso document-service perché l'embedding della query lo calcola il lato documento; ADR-0012 non colloca Embedder in nessun servizio. Se l'embedding della query va lato RAG, il DTO porta un Embedding invece del testo: punto aperto per 01-17 o per la Fondazione"}
  - must_have: "Gate di piano e di sicurezza"
    verification:
      - {test: "perimetro e ordine dei commit", risks: [], ref: "git diff --stat 19a7304^ 5fe295b", status: pass, evidence: "5 file, esattamente i files_modified; 0cd4c42^2 = 5fe295b; 19a7304 (Soglie, 11:31) antenato di 6feb6a0 (test rossi, 11:43) e la sezione Soglie è identica fra 19a7304 e HEAD; dopo il Task 1 RISULTATI perde solo le due righe con query_text e tre segnaposto «Da compilare al Task 2»; gate-proprieta-file conforme"}
      - {test: "integrità dei test del test-write", risks: [RISK-001], ref: "git diff 6feb6a0 HEAD -- spikes/s07-ports/src/lib.rs", status: pass, evidence: "confrontato il modulo tests fra 6feb6a0 e HEAD: diff solo additivo (le 5 funzioni x_* in coda), nessuna riga dei 40 test né dei fake cambiata o tolta; i 5 doctest compile_fail identici"}
      - {test: "fmt, clippy, scanner", risks: [], ref: "nota del bridge", status: pass, evidence: "rilanciati dal bridge: cargo fmt --all -- --check exit 0; cargo clippy -p s07-ports --all-targets --locked --offline -- -D warnings verde (il rapporto dichiara anche --workspace, non rilanciato); security_scan.py su spikes/s07-ports e ADR-0012: OK, 0 problemi su 4 file"}

# Metrics
duration: "circa 22 min di esecuzione: Soglie 19a7304 11:31:12 → test rossi 6feb6a0 11:43:15 → corpi 7697ff7 11:48:03 → RISULTATI c6c2a00 11:49:58 → ADR 5fe295b 11:51:10 → merge 0cd4c42 11:52:48"
completed: 2026-10-26
---

# Fase 1 Piano 16: S-07, firme candidate delle porte e ADR-0012 · Summary

**Le 13 porte di CKA hanno ora firme Rust che compilano, sono tutte usabili come `Arc<dyn Trait>` (async-trait, `Chunker` sincrono) e portano nei tipi le garanzie chieste dall'audit: `Reranker`, `DecisionEngine::{assess_evidence, validate_citations}` e `AnswerGenerator` accettano solo `AuthorizedEvidence`, che nasce unicamente da `authorize_response` dopo il ricontrollo di soggetto, versione ACL e spazio della `RetrievalSearchResponse` prodotta lato documento da `retrieve_authorized`; costruirla altrove, serializzarla o deserializzarla non compila. `rank` esce da `DecisionEngine` nel trait stretto `Reranker`; `CacheKey` non si costruisce senza scope; le mutazioni distinguono `OutcomeUnknown` da `NotApplied`; un embedding di altro spazio a parità di dimensione è rifiutato prima della ricerca. 45 test e 5 doctest `compile_fail` verdi alla riesecuzione del bridge. ADR-0012 (Proposed) registra stile async, regole delle firme e vincoli verso la Fondazione**

## Performance

- **Duration:** circa 22 min, da `19a7304` (11:31:12) al merge `0cd4c42` (11:52:48) del 2026-10-26. Fra il Task 1 e i test rossi è passato sul main il bridge di 01-15 (`3e07dd3`, 11:34:48), estraneo a questo piano.
- **Started:** 2026-10-26T11:31:12+01:00
- **Completed:** 2026-10-26T11:52:48+01:00
- **Tasks:** 2 di 2
- **Files modified:** 5 su 5 dei `files_modified` (`git diff --stat 19a7304^ 5fe295b`): `spikes/s07-ports/RISULTATI.md`, `spikes/s07-ports/Cargo.toml`, `spikes/s07-ports/src/lib.rs`, `spikes/Cargo.lock`, `docs/adr/0012-firme-porte-candidate.md`. `spikes/Cargo.toml` invariato.

## Accomplishments

- **Soglie prima del codice** (`spikes/s07-ports/RISULTATI.md:11-31`, commit `19a7304`): P-1..P-15, invariate fino a HEAD.
- **Contesti e identificativi** (`spikes/s07-ports/src/lib.rs:11-277`): UUID opachi con una macro, `AclVersion`, `ContentHash` e testi con `Debug` redatto; `Deadline`, `TraceContext::parse` (W3C, `:209-246`), `CallContext`, `MutationContext`, `MutationOutcome`.
- **Errori tipizzati** per porta (`:344-508`), compreso `EvidenceError::InvalidScore`.
- **Embedding** (`:511-596`): `EmbeddingSpace` completo con `Eq`, `Embedding::new` che rifiuta la dimensione sbagliata, `Embedder` con `embed_documents`, `embed_query`, `space`.
- **Evidenza sui due lati** (`:599-1041`): `retrieve_authorized` e `RetrievedEvidence` (`:601-772`), DTO `RetrievalSearchRequest`/`RetrievalSearchResponse` (`:775-818`), `EvidenceRetriever`, `AuthorizedEvidence` con `reorder` e `resolve`, `authorize_response` (`:821-1040`).
- **Le altre porte**: `DocumentStore` (`:1044-1109`), `DocumentParser` e `Chunker` (`:1112-1225`), `VectorIndex` e `IndexVersionDraft` (`:1228-1345`), `DecisionEngine` e `Reranker` (`:1348-1462`), `AnswerGenerator` (`:1465-1521`), `AccessPolicy` (`:1524-1598`), `AuditSink` (`:1601-1636`), `Cache` e `CacheKey` (`:1639-1784`), `DocumentSource` (`:1787-1828`).
- **Test** (`:1830-3421`): 40 del test-write, 5 dell'esecutore, fake solo nel modulo `tests`.
- **RISULTATI** (Risultati, Anomalie, Esito) e **ADR-0012** Proposed.

## Task Commits

1. **Task 1, soglie e variazioni:** `19a7304` (docs), direttamente sul main prima del branch.
- **Test rossi (agente test-write, distinto dall'esecutore):** `6feb6a0` (test), RED_EXPECTED: 3 passati e 37 rossi per `todo!`, 5 doctest `compile_fail` già verdi (`.planning/tmp/test-write-01-16.yaml`). Il test-write ha creato anche lo scheletro delle firme e rigenerato `spikes/Cargo.lock`.
2. **Task 2, corpi, RISULTATI e ADR:** `7697ff7` (feat), `c6c2a00` (docs), `5fe295b` (docs).

**Merge:** `0cd4c42` (`--no-ff` del branch `task/PLAN-010-16`, secondo genitore `5fe295b`). **Plan metadata:** questo SUMMARY e `01-16-RACCONTO.md`, scritti dal bridge senza commit.

## Files Created/Modified

- `spikes/s07-ports/src/lib.rs`: firme, tipi, fake e test (3.421 righe).
- `spikes/s07-ports/Cargo.toml`: `[dependencies]` async-trait, futures, bytes, uuid, serde (`.workspace = true`); `[dev-dependencies]` serde_json.
- `spikes/Cargo.lock`: le sei dipendenze del pacchetto `s07-ports`, rigenerato da cargo in `6feb6a0`.
- `spikes/s07-ports/RISULTATI.md`: soglie, variazioni, tipi dei contratti condivisi, risultati, anomalie, esito.
- `docs/adr/0012-firme-porte-candidate.md`: decisione Proposed.

## Controlli a vista richiesti dalle acceptance

- **`Reranker`** ha il solo `rank` (`lib.rs:1452-1461`); **`DecisionEngine`** non ha `fn rank` (`:1425-1449`).
- **Parametri dei consumatori semantici:** `rank(ctx, &UserQuery, &AuthorizedEvidence)`, `assess_evidence(ctx, &UserQuery, &AuthorizedEvidence)`, `validate_citations(ctx, &AuthorizedEvidence, &StructuredAnswer)`, `generate(ctx, &UserQuery, &AuthorizedEvidence)`. Nessun `Candidate`, `PassageRef` sciolto o testo di passaggio; `UserQuery` è la domanda dell'utente.
- **Lati del confine:** `evidence::rag_side` (`:821-1040`) non nomina `VectorIndex` né `AuthorizationPredicate`; `evidence::document_side` (`:601-772`) non nomina `AuthorizedEvidence`, `Reranker`, `DecisionEngine`, `AnswerGenerator`. Grep del bridge sui due intervalli: vuoti.
- **Metodi per trait:** DocumentSource 2, DocumentStore 3, DocumentParser 1, Chunker 1, Embedder 3, VectorIndex 3, DecisionEngine 3, Reranker 1, AnswerGenerator 1, AccessPolicy 2, AuditSink 1, Cache 3, EvidenceRetriever 1 (≤ 6).
- **`AuthorizedEvidence` e `RetrievedEvidence`:** campi privati, nessun costruttore pubblico né `Default`.
- **Doctest `compile_fail`:** i 5 snippet falliscono per il motivo atteso (E0451 ×2, E0277 ×2, E0061) e le varianti positive compilano, provato dal bridge in un crate esterno, perché rustdoc stabile non verifica il codice d'errore.

## Decisions Made

### Decisioni già prese (`01-CONTEXT.md`, ADR, coordinatore)

- D-02, D-08, D-10, D-12, D-14, D-18, D-20, D-22, D-38 come variazioni misurate dagli spike; ADR-0003..0011 come fonti.
- **Coordinatore del piano:** niente `query_text` in `search`/`retrieve_authorized`; `Reranker::rank(&AuthorizedEvidence)`; `Metered<T>`; `Chunker` sincrono; `async-trait`.

### Scelte dell'esecutore

- **`acl_version` per uguaglianza** (`lib.rs:725`): una versione diversa, anche più recente, è scartata.
- **Doppioni e tetto `k` lato documento** (`:751-756`): tiene il primo `PassageRef`, tronca a `k`, così lato RAG non scatta `DuplicatePassageRef` su tutta la risposta.
- **`InvalidScore`** (`:956`): uno score NaN o infinito è errore; con `total_cmp` un NaN positivo andrebbe in cima.
- **`Debug` dei passaggi senza `Locator`**: la variante `Text` contiene titoli di sezione.
- **`Deadline` su `std::time::Instant`** (`:170-198`): tokio fuori dalle firme.

## Deviations from Plan

### Livello a

1. **`EvidenceError::InvalidScore`** aggiunto all'elenco di errori del piano (`7697ff7`), con test `x_reorder_rejects_duplicate_and_non_finite_scores`.
2. **`query_text` tolto** da `VectorIndex::search` e `retrieve_authorized` rispetto alla stesura del Task 1, con paragrafo *Storia* (`RISULTATI.md:83`, `c6c2a00`). La firma finale coincide con quella del piano.
3. **`spikes/Cargo.lock`** rigenerato nello scheletro del test-write (`6feb6a0`) e non più toccato nel Task 2.

### Scostamenti dalla lettera del piano, dichiarati in RISULTATI

- `Locator` a `spans` (al più due) invece di `Pdf { page, rects }` / `Text { section_path, char_range }`, secondo ADR-0005 (`RISULTATI.md:57`).
- `ParseError::OutputLimit` in più (`RISULTATI.md:56`); `DocumentStore::delete` nuovo per il GC (`RISULTATI.md:54`).
- `VectorIndex::search` riceve `&Embedding` invece di `Embedding`; `Reranker::rank` riceve `&AuthorizedEvidence` (decisione del coordinatore).
- `Chunker` sincrono e senza contesto: il piano chiedeva gli aggiustamenti comuni (contesto con deadline) anche per `Chunker`; motivato in ADR-0012.

**Total deviations:** 3 di livello a; nessuna di livello b o c.
**Impact on plan:** nessuna estensione di perimetro (5 file su 5).

## Discrepanze fra rapporto e prove

- **Nessuna sostanziale.** Commit, conteggi (45 + 5, `Arc<dyn` = 19, metodi per trait), gate e coverage del rapporto coincidono con le riesecuzioni del bridge.
- **Clippy:** il rapporto dichiara `cargo clippy --workspace`; il bridge ha rilanciato solo `-p s07-ports` (verde).
- **Hash abbreviati:** il rapporto riporta `7697ff7`, `c6c2a00`, `5fe295b` senza hash completo; esistono tutti nel log.
- **«40 test del test-write»:** il test-write dichiara 40 test unitari (13 + 2 + 6 + 5 + 4 + 3 + 5 + 2) più 5 doctest; tornano.

## Incoerenze negli artefatti (non corrette dal bridge)

- **ADR-0012 non registra la scelta `std::time::Instant`**, che contraddice la lettera di ADR-0010:54; la motivazione sta solo in `RISULTATI.md:156`.
- **Regola di redazione non uniforme:** `RISULTATI.md:155` omette il `Locator` dal `Debug` dei passaggi perché contiene titoli di sezione, ma `Block` (`lib.rs:1171`), `IndexedChunk` e `IndexVersionDraft` (derive, `:1273`, `:1283`) lo stampano. Nessun criterio del piano li copre.
- **`InvalidationScope::DocumentVersion`** non ha un campo corrispondente in `CacheKey` (che porta `corpus_version`, non le versioni di documento): il fake di test lo implementa svuotando tutta la cache (`lib.rs:2967`). Un adapter reale non può invalidare per documento senza un indice laterale.
- **ADR-0012 non colloca `Embedder`** in nessun servizio, mentre `RISULTATI.md:83` presuppone l'embedding della query lato documento.
- **PLAT-07** (threat model compilato) è il requisito del piano, copiato in `requirements-completed` come vuole il bridge; questo piano contribuisce solo all'impatto su T-13 e T-14, non chiude il threat model da solo.

## Threat Flags

- **T-01-16-01 (passaggi non autorizzati verso JEV, LLM, reranker):** mitigato nei tipi su entrambi i lati; doctest e TEST-2/3 verdi. Residuo RISK-001 v2 sull'adapter HTTP futuro, vincolo in ADR-0012.
- **T-01-16-08 (Debug):** redazione su 8 tipi, sentinella assente; nota sopra su `Block` e `IndexedChunk`.
- **T-01-16-02 (citazioni):** `reorder` e `resolve` non accettano riferimenti assenti.
- **T-01-16-03 (import del codice spike):** nessun file fuori da `spikes/` e `docs/adr/`.
- **T-01-16-04..07:** `CallContext` con deadline, `CacheKey` con scope, `MutationContext`, `EmbeddingSpace`: test verdi.

## Known Stubs

Nessuno nel codice (nessun `todo!`/`unimplemented!`); nessun adapter reale, per perimetro.

## User Setup Required

Nessuno.

## Punti aperti

- **Ratifica di ADR-0012** nel consolidamento (01-17), con aggiornamento di `docs/architecture/interfaces-and-concurrency.md` e della riga `async-trait` di `DEPENDENCIES.md`.
- **`std::time::Instant` contro `tokio::time::Instant`** di ADR-0010: allineare il testo di ADR-0010 o la firma.
- **`RetrievalSearchRequest { query_text, k }`:** se l'embedding della query va lato RAG, il DTO porta un `Embedding`; decidere dove sta `Embedder` (01-17 o Fondazione).
- **Adapter HTTP di `EvidenceRetriever`** e test serde/Debug da portare nel prodotto (RISK-001 v2).
- **Invalidazione per `DocumentVersion`** senza campo nella chiave.
- **`Locator` nel `Debug`** di `Block`, `IndexedChunk`, `IndexVersionDraft`.

## Next Phase Readiness

- **01-17 (consolidamento):** ratifica di ADR-0012, divergenze da `interfaces-and-concurrency.md`, ADR-0010 e `DEPENDENCIES.md`.
- **Fondazione:** trascrizione dei tipi della regola 8 in `crates/contracts` e delle porte nei servizi, con i vincoli di ADR-0012.

## Self-Check: PASSED

- 5 file su 5 dei `files_modified` in `19a7304^..5fe295b`; `spikes/Cargo.toml` invariato; `gate-proprieta-file` conforme.
- Rieseguiti dal bridge sul main fuso `0cd4c42`: `cargo test -p s07-ports --locked --offline` (45 passati, 5 doctest), il blocco `<verify>` del Task 2 (exit 0), `cargo fmt --check`, `cargo clippy -p s07-ports -D warnings`, `security_scan.py` (0 problemi).
- I 5 doctest di non compilazione provati per codice d'errore con varianti positive; test del test-write invariati fra `6feb6a0` e HEAD; Soglie invariate da `19a7304`; link di ADR-0012 e RISULTATI esistenti.

**Modifica:** Claude (bridge 01-16) · 2026-10-26 · creazione (bridge di 01-16, passi 1-3)
