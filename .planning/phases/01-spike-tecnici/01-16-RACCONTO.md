# Racconto del piano 01-16: firme candidate delle porte e ADR-0012 (S-07)

## Cosa

Gli spike S-01..S-06 avevano misurato, uno per uno, come dovevano cambiare le porte descritte in `docs/architecture/interfaces-and-concurrency.md`: una chiave sha256 calcolata prima di salvare, un locator a due varianti, embedding separati per documenti e query, il rank come segnale a sé, una deadline esplicita, citazioni per claim. Ognuno lo aveva scritto nella sezione «Impatto su porte (S-07)» del proprio RISULTATI. Questo piano raccoglie quelle variazioni e le trasforma in **codice Rust che compila**: le firme candidate di tutte e 13 le porte, nel crate `spikes/s07-ports`. La Fondazione le trascriverà nei servizi senza importarle.

Non è solo un elenco di firme. Le firme portano **nei tipi** quattro garanzie chieste dall'audit dei rischi:

- **Solo evidenza autorizzata** verso reranker, JEV e LLM (RISK-001). Un passaggio arriva a `Reranker`, `DecisionEngine` o `AnswerGenerator` solo dentro `AuthorizedEvidence`, che si può costruire in un unico punto, dopo un ricontrollo.
- **Cache con scope** (RISK-002). `CacheKey` non si costruisce senza soggetto e versione ACL.
- **Mutazioni idempotenti** (RISK-003). Ogni scrittura riceve una chiave di idempotenza, e un esito sconosciuto è distinto da un rifiuto.
- **Spazio di embedding coerente** (RISK-004). Un vettore di un altro modello viene rifiutato prima della ricerca, anche a parità di dimensione.

In più `rank` esce da `DecisionEngine` e diventa il trait stretto `Reranker`. ADR-0012 (*Proposed*) registra lo stile async scelto, le regole delle firme e i vincoli verso la Fondazione. Resta *Proposed* finché non lo ratifichi nel consolidamento (01-17). Il piano porta PLAT-07, per la parte del threat model che riguarda T-13 e T-14.

## Come

### Quattro concetti prima del codice

- **Perché `async-trait`.** Una porta come `VectorIndex` fa I/O, quindi i suoi metodi sono `async`. La composizione però sceglie l'adapter a runtime (JEV o baseline, Sonnet o Haiku), quindi il caso d'uso riceve un `Arc<dyn VectorIndex>`: un puntatore a «qualcosa che implementa il trait», senza sapere cosa. Un esempio del problema: un `async fn search(...)` scritto direttamente nel trait restituisce un tipo di future diverso per ogni implementazione, con dimensioni diverse, e Rust non sa costruire la tabella dei metodi di un `dyn` con un tipo di ritorno che varia. Si dice che il trait non è *dyn-compatibile*. La soluzione è far restituire a ogni metodo lo stesso tipo, un future «in scatola»: `Pin<Box<dyn Future<Output = …> + Send + '_>>`. La macro `#[async_trait]` (per esempio a `spikes/s07-ports/src/lib.rs:1324`) fa esattamente questa riscrittura: tu scrivi `async fn`, lei genera la versione con `Box`. Il prezzo è un'allocazione per chiamata.
- **Campi privati come prova di autorizzazione.** In Rust i campi di una struct sono privati al modulo che la dichiara, salvo `pub`. `AuthorizedEvidence` (`lib.rs:914-919`) ha campi senza `pub` e nessuna funzione pubblica che la crei, tranne `authorize_response`, che sta nello stesso modulo e fa il ricontrollo. Quindi, se una funzione riceve un `&AuthorizedEvidence`, il compilatore garantisce che quel valore è passato dal ricontrollo: non c'è altro modo di averne uno. Il tipo stesso è la prova.
- **Doctest `compile_fail`.** Un esempio nei commenti `///` viene compilato da `cargo test` come test (*doctest*). Con l'attributo `compile_fail` il test passa solo se l'esempio **non** compila. È il modo per scrivere un test di un divieto. Per esempio a `lib.rs:891-899` si prova a costruire `AuthorizedEvidence { … }` da fuori: deve fallire. Il codice dopo la virgola (`E0451`, «campo privato») su Rust stabile è solo documentazione, perché rustdoc lo controlla davvero soltanto su nightly. Per questo il bridge ha ricompilato i cinque esempi a parte e ha controllato che fallissero proprio con E0451, E0277 ed E0061, e che una variante corretta compilasse. Senza questa controprova un doctest `compile_fail` può passare per il motivo sbagliato, per esempio un `use` scritto male.
- **`Debug` manuale.** `#[derive(Debug)]` stampa tutti i campi, testo compreso. Basta un `tracing::debug!("{evidence:?}")` per mandare il contenuto di un documento riservato nei log. I tipi che portano testo implementano quindi `fmt::Debug` a mano. Stampano identificativi e il numero di caratteri (`text_chars`), mai il testo: per esempio `AuthorizedPassage` a `lib.rs:877-885`. `finish_non_exhaustive()` aggiunge `..` all'output, per dire che alcuni campi sono stati omessi di proposito.

### Percorso di lettura consigliato

1. **Le soglie** (`spikes/s07-ports/RISULTATI.md:11-31`). Committate in `19a7304`, prima che esistesse una riga di `lib.rs`, e invariate fino a HEAD. P-6..P-9 descrivono il confine dell'evidenza, P-10 la redazione, P-11..P-13 cache, mutazioni ed embedding.

2. **La tabella delle variazioni** (`RISULTATI.md:50-83`). Ogni riga confronta la firma vecchia con quella nuova e cita lo spike d'origine con un link. Il paragrafo *Storia* (`:83`) spiega perché `query_text` è sparito da `VectorIndex::search`.

3. **I contesti** (`lib.rs:168-277`). `Deadline` (`:176-198`) è un istante assoluto. `TraceContext::parse` (`:209-246`) valida un `traceparent` W3C: 55 caratteri, esadecimale minuscolo, versione diversa da `ff`, trace id e parent id non tutti zero. La riga `let (Some(version), …, None) = (…) else { return … }` è un *let-else*: se il pattern non corrisponde, cioè se i campi separati da `-` non sono esattamente quattro, si esce subito. `CallContext` (`:255-260`) va a ogni lettura verso un provider, `MutationContext` (`:263-268`) a ogni scrittura, e `MutationOutcome` (`:272-276`) distingue `Applied` da `AlreadyApplied`.

4. **Lo spazio di embedding** (`lib.rs:519-574`). `EmbeddingSpace` deriva `Eq`, quindi due spazi sono uguali solo se coincidono modello, revisione, dimensione, normalizzazione, pooling e istruzioni. `Embedding::new` (`:555-563`) rifiuta un vettore della lunghezza sbagliata, e i campi privati impediscono di aggirarlo.

5. **Lato documento** (`lib.rs:601-772`), il futuro document-service. `retrieve_authorized` (`:736-771`) prende l'indice come `&dyn VectorIndex` e fa tre cose in ordine:
   - confronta lo spazio della query con quello dell'indice e, se diversi, esce prima di cercare (`:743-745`);
   - chiama `search` col predicato;
   - ricontrolla ogni candidato con `admits` (`:722-731`): deve essere pubblicato, con la **stessa** versione ACL del predicato, e ammesso per soggetto o per ruolo.

   Poi scarta i doppioni con un `HashSet` e tronca a `k` (`:751-756`). `seen.insert(x)` restituisce `false` se `x` c'era già, quindi dentro un `filter` tiene solo la prima occorrenza. L'unica uscita di `RetrievedEvidence` è `into_response` (`:689-705`), che la trasforma nel DTO.

6. **Il DTO** (`lib.rs:775-818`). `RetrievalSearchRequest` e `RetrievalSearchResponse` sono i corpi di `POST /internal/v1/retrieval/search`. Derivano `Serialize`/`Deserialize`: sono i soli tipi che attraversano la rete. Il commento li chiama «dato non fidato»: chi li riceve non deve credere a quello che contengono.

7. **Lato RAG** (`lib.rs:821-1040`), il futuro rag-service. La porta `EvidenceRetriever` (`:837-845`) restituisce il DTO; nel prodotto sarà un client HTTP. `authorize_response` (`:1005-1039`) è l'unica funzione che crea `AuthorizedEvidence`. Rifiuta la risposta se soggetto, versione ACL o spazio non coincidono con quelli attesi per la richiesta, o se ci sono `PassageRef` duplicati, e non restituisce mai un'evidenza parziale. `reorder` (`:948-983`) applica i segnali del reranker: può solo permutare o togliere passaggi, mai aggiungerne. `resolve` (`:986-988`) mappa una citazione a un passaggio presente, oppure dà `None`.

   In `reorder` c'è `b.0.total_cmp(&a.0)` (`:975`). Il problema: `f32` non ha un ordine totale, perché `NaN < 1.0` e `NaN > 1.0` sono entrambi falsi, e un ordinamento che usa `partial_cmp` dovrebbe decidere cosa fare. `total_cmp` dà un ordine totale secondo IEEE 754, in cui però un NaN positivo sta sopra ogni numero. Un reranker guasto che restituisce NaN finirebbe quindi **in cima** alla classifica. Per questo, prima dell'ordinamento, uno score non finito è rifiutato con `InvalidScore` (`:956`). `sort_by` è stabile, cioè a parità di score conserva l'ordine d'ingresso.

8. **I consumatori semantici** (`lib.rs:1348-1521`). `DecisionEngine` (`:1425-1449`) ha tre metodi e nessun `rank`. `Reranker` (`:1452-1461`) ha solo `rank`. `AnswerGenerator` (`:1511-1520`) ha solo `generate`. Tutti ricevono `&AuthorizedEvidence`. `Metered<T>` (`:1373-1376`) affianca al risultato la versione del modello davvero risolta e il consumo di token.

9. **Cache** (`lib.rs:1639-1784`). `CacheKey::new` (`:1697-1711`) vuole cinque argomenti, tutti obbligatori, e il primo è l'`AuthorizationScope`. Il doctest a `:1677` prova che con quattro argomenti non compila (E0061).

10. **Le altre porte**: `DocumentStore` (`:1083-1108`), `DocumentParser` e `Chunker` (`:1201-1224`, `Chunker` senza `async`), `VectorIndex` (`:1323-1344`), `AccessPolicy` (`:1579-1597`), `AuditSink` (`:1626-1635`), `DocumentSource` (`:1811-1827`).

### Flusso dei dati

`Principal` → `AccessPolicy::predicate_for` → `AuthorizationPredicate` → `retrieve_authorized(&dyn VectorIndex, …)` → `RetrievedEvidence` → `into_response` → JSON sul confine HTTP → `EvidenceRetriever::search` → `authorize_response(response, scope atteso, spazio atteso)` → `AuthorizedEvidence` → `Reranker::rank` → `reorder` → `DecisionEngine::assess_evidence` → `AnswerGenerator::generate` → `StructuredAnswer` → `validate_citations`. Il diagramma sta in `RISULTATI.md:87-97`. I due lati non si nominano mai a vicenda: il grep sulle righe dei due sottomoduli lo conferma.

## Perché

- **`async-trait` e non il boxing esplicito** (ADR-0012, opzioni 1-2). Il risultato a runtime è identico: la macro genera lo stesso `Pin<Box<…>>`. Scritto a mano sarebbero però lifetime espliciti e `Box::pin(async move { … })` in tredici trait e in ogni fake. Un'allocazione costa nanosecondi contro i millisecondi o centinaia di millisecondi di I/O misurati negli spike.
- **`Reranker` separato** (ISP). La baseline no-rerank decisa in ADR-0009 sa solo restituire l'ordine com'è. Se `rank` stesse in `DecisionEngine`, la baseline dovrebbe implementare anche `validate_query`, cosa che non sa fare. Ogni adapter implementa solo ciò che sa servire.
- **Prova nel tipo invece di contratto all'adapter** (ADR-0012, opzione 4). Con `&[Candidate]` e un commento «passare solo candidati autorizzati», una regressione si scopre solo se esiste un test d'integrazione che la cerca. Con `&AuthorizedEvidence` non compila.
- **Due lati invece di un solo tipo.** `VectorIndex` vive in document-service, i consumatori semantici in rag-service, e un tipo con costruttore privato non attraversa una chiamata HTTP. Di qui i tre sottomoduli e il ricontrollo sul lato RAG (re-gate R2 del piano, RISK-001 v2 di risk_audit_14). Se l'adapter HTTP deserializzasse direttamente `AuthorizedEvidence`, la garanzia sparirebbe. È impossibile perché `AuthorizedEvidence` non implementa `Deserialize`: lo provano i doctest a `lib.rs:903` e `:910`.
- **`acl_version` per uguaglianza.** Il test-write fissava il caso della versione vecchia. L'esecutore scarta anche una versione più **recente**: vuol dire che indice e politica non vedono lo stesso stato, e il deny by default (CLAUDE.md «Sicurezza») preferisce un passaggio in meno a uno di troppo.
- **`acl_version` che cambia anche coi ruoli, invece di un digest dei ruoli nella chiave** (ADR-0012, regola 6). L'autorizzazione dipende anche dalle ACL dei documenti, quindi una versione servirebbe comunque. Con una sola versione l'invalidazione resta `InvalidationScope::AclVersion`, e lo stesso valore viaggia nel DTO per il ricontrollo lato RAG. L'incremento a ogni cambio di ruolo diventa un vincolo per la fase identity/RBAC.
- **`query_text` tolto da `search`.** ADR-0006 non adotta la ricerca ibrida nell'MVP. Un parametro ignorato dall'adapter avrebbe portato il testo della domanda fino al database senza nessun uso: un dato in più da proteggere.
- **`Chunker` sincrono.** È calcolo puro, senza provider. Il chiamante lo esegue in `spawn_blocking` e applica la deadline da fuori (CLAUDE.md «Concorrenza»).
- **`Deadline` su `std::time::Instant`.** Mettere `tokio::time::Instant` in firma legherebbe ogni porta a tokio. L'adapter converte con `tokio::time::Instant::from_std`. ADR-0010 ammette «un contesto di query che la contiene», e `CallContext` lo è; resta comunque una differenza dalla lettera di quell'ADR (vedi i punti aperti).

## Test

Tutti da `spikes/`: `cargo test -p s07-ports --locked --offline` dà 45 test unitari e 5 doctest. Per lanciarne uno solo: `cargo test -p s07-ports --locked --offline t3_json_round_trip`. Per i soli doctest: `cargo test -p s07-ports --locked --offline --doc`.

I 40 test con prefisso `t1_`..`t9_` li ha scritti un agente distinto dall'esecutore (`6feb6a0`), prima dei corpi delle funzioni, ed erano rossi per `todo!`. Il bridge ha confrontato il modulo `tests` fra `6feb6a0` e HEAD: l'esecutore ha solo aggiunto i 5 test `x_*` in coda e non ha cambiato una riga dei test né dei fake esistenti.

- **TEST-1, dyn-compatibilità** (`lib.rs:2352-2494`). Per ognuna delle 13 porte, il fake `Failing` (`:2092`) restituisce un errore tipizzato attraverso un `Arc<dyn Trait + Send + Sync>`. Se un trait non fosse dyn-compatibile, il test non compilerebbe.
- **TEST-2, lato documento** (`:2499`, `:2560`, più `x_` a `:3287` e `:3324`). `FakeIndex` (`:2035-2087`) è un fake **positivo**: restituisce apposta anche candidati di un altro soggetto, con ACL vecchia o non pubblicati, e il test controlla che spariscano. Un fake che restituisse solo candidati buoni non proverebbe niente.
- **TEST-3, lato RAG** (`:2594-2746`). Tre rifiuti (soggetto, ACL, spazio), i doppioni, il caso coerente. `t3_json_round_trip_document_side_to_rag_side` (`:2676`) passa per `serde_json` e poi altera il JSON campo per campo, come avrebbe fatto un adapter o una rete ostile. È il test che risk_audit_14 chiedeva.
- **TEST-4, confine dei tipi**: i 5 doctest `compile_fail` a `:666`, `:891`, `:903`, `:910`, `:1677`.
- **TEST-5, redazione e riordino** (`:2754-2893`, più `x_` a `:3234` e `:3263`). Un testo sentinella (`SENTINELLA-7f3a-testo-riservato`, `:1882`) entra in ogni tipo e non deve comparire in nessun `{:?}`.
- **TEST-6, cache** (`:2894-3008`). Chiavi diverse per manager e guest, per `acl_version` incrementata, per versione del corpus e revisione dello spazio; la voce non è più servita dopo la revoca.
- **TEST-7, idempotenza** (`:3038-3110`). Il fake applica la scrittura e poi risponde `OutcomeUnknown`, come un timeout dopo il commit; il retry con la stessa chiave deve dare `AlreadyApplied` e lasciare un solo evento.
- **TEST-8, spazio di embedding** (`:3112-3206`). Qwen3 a 1024 dimensioni contro un indice bge-m3 a 1024: rifiutato, e il contatore delle ricerche resta a 0.
- **TEST-9, confini dei crate** (`:3368`, `:3387`). Le dipendenze di `Cargo.toml` stanno nell'elenco chiuso, e le radici dei `use` sono std, il crate stesso o le dipendenze ammesse. I nomi dei crate proibiti non sono scritti nei test, per non far scattare il grep del `<verify>`: lo copre quel grep, rilanciato dal bridge e vuoto.

## Deviazioni e punti aperti

Deviazioni di livello a, tutte dichiarate in `RISULTATI.md:149-157`:

- `EvidenceError::InvalidScore` non era nell'elenco del piano (motivo: il NaN in cima, spiegato sopra);
- `query_text` tolto dalle due firme dopo il Task 1, con la *Storia*;
- `spikes/Cargo.lock` rigenerato già dallo scheletro del test-write e poi non più toccato.

Rispetto alla lettera del piano cambiano anche la forma del `Locator` (a `spans`, come ADR-0005), il `ParseError::OutputLimit` in più, il `DocumentStore::delete` nuovo, `&Embedding` per riferimento in `search` e `Chunker` senza contesto. Tutto è motivato in RISULTATI o in ADR-0012.

Punti aperti, da decidere tu o nelle fasi successive:

- **Ratifica di ADR-0012** in 01-17, con l'aggiornamento di `interfaces-and-concurrency.md` e della riga `async-trait` di `DEPENDENCIES.md`.
- **`std::time::Instant` o `tokio::time::Instant`.** ADR-0010 (`:54`) scrive tokio, la firma usa std. La motivazione sta solo in `RISULTATI.md:156`, non in ADR-0012: in 01-17 va allineato uno dei due testi.
- **Dove si calcola l'embedding della query.** `RetrievalSearchRequest { query_text, k }` (`lib.rs:785-789`) presuppone che sia document-service a chiamare `Embedder::embed_query`. ADR-0012 però non colloca `Embedder` in nessun servizio. Se l'embedding della query va lato RAG, il DTO porta un `Embedding` invece del testo, e il testo della domanda non attraversa più il confine.
- **L'adapter HTTP reale di `EvidenceRetriever`.** È il rischio residuo di RISK-001 v2: la garanzia vale se l'adapter deserializza solo il DTO e il caso d'uso passa sempre da `authorize_response`. ADR-0012 (`:83`) chiede alla Fondazione di portare nel prodotto il test serde e quello della sentinella.
- **`Locator` nel `Debug`.** I passaggi lo omettono perché i titoli di sezione sono testo del documento, ma `Block` (`lib.rs:1171`), `IndexedChunk` e `IndexVersionDraft` (derive, `:1273`, `:1283`) lo stampano. Il piano non li elencava, quindi non è un criterio mancato, ma la regola andrebbe applicata in modo uniforme nella trascrizione.
- **Invalidazione per versione di documento.** `InvalidationScope::DocumentVersion` esiste, ma `CacheKey` non contiene versioni di documento, e il fake la implementa svuotando tutta la cache (`lib.rs:2967`). Un adapter reale avrà bisogno di un indice laterale, oppure di affidarsi alla versione del corpus.

**Modifica:** Claude (bridge 01-16) · 2026-10-26 · creazione (bridge di 01-16, passo 3-bis)
