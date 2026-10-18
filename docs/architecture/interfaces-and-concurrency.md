# Interfacce elementari e concorrenza

I nomi sono concettuali; le firme Rust definitive verranno validate con uno spike.

## Regole dei contratti

- UUID opachi, orari UTC RFC 3339 e API sotto `/api/v1`.
- Operazioni mutanti con idempotency key o identificatore stabile equivalente.
- Propagazione di `request_id` e trace context.
- Errori con `code`, `message`, `request_id` e dettagli non sensibili.
- Nessun tipo di database o SDK esterno attraversa una porta di dominio.

## Contratti HTTP minimi

### Identity & Access

| Metodo | Path | Scopo |
|---|---|---|
| POST | `/api/v1/auth/login` | Verifica credenziali ed emette token |
| POST | `/api/v1/auth/refresh` | Ruota il refresh token |
| POST | `/api/v1/auth/logout` | Revoca la sessione |
| GET | `/api/v1/auth/me` | Restituisce identità e ruoli correnti |
| POST | `/api/v1/users` | Crea un utente, solo admin |
| PATCH | `/api/v1/users/{id}/roles` | Aggiorna ruoli, solo admin |
| GET | `/.well-known/jwks.json` | Chiavi pubbliche di verifica |

### Document & Indexing

| Metodo | Path | Scopo |
|---|---|---|
| POST | `/api/v1/documents` | Upload streaming; restituisce `document_id` e `job_id` |
| GET | `/api/v1/documents` | Elenca soltanto documenti visibili |
| GET | `/api/v1/documents/{id}` | Metadati e versione corrente |
| GET | `/api/v1/documents/{id}/content` | Contenuto autorizzato per il viewer |
| DELETE | `/api/v1/documents/{id}` | Tombstone/cancellazione amministrativa |
| GET | `/api/v1/ingestion-jobs/{id}` | Stato e fasi dell'ingestion |
| POST | `/internal/v1/retrieval/search` | Candidati autorizzati per RAG Service |
| POST | `/internal/v1/documents/{id}/authorize` | Ricontrollo prima della risposta |

### RAG & Answer

| Metodo | Path | Scopo |
|---|---|---|
| POST | `/api/v1/queries` | Query con deadline esplicita |
| GET | `/api/v1/queries/{id}` | Risultato e citazioni |
| GET | `/api/v1/audit-events` | Audit secondo ruolo e filtri |
| POST | `/internal/v1/evaluations` | Avvia evaluation versionata |

## Porte Rust interne

```rust
trait DocumentSource {
    async fn discover(&self, cursor: Option<SourceCursor>) -> Result<DiscoveryPage>;
    async fn fetch(&self, candidate: &DocumentCandidate) -> Result<DocumentStream>;
}

trait DocumentStore {
    async fn put(&self, object: DocumentStream, metadata: ObjectMetadata) -> Result<ObjectRef>;
    async fn get(&self, object: &ObjectRef, range: Option<ByteRange>) -> Result<DocumentStream>;
}

trait DocumentParser {
    async fn parse(&self, object: &ObjectRef) -> Result<ParsedDocument>;
}

trait Chunker {
    fn chunk(&self, document: &ParsedDocument) -> Result<Vec<ChunkDraft>>;
}

trait Embedder {
    async fn embed_batch(&self, chunks: &[ChunkDraft]) -> Result<Vec<Embedding>>;
}

trait VectorIndex {
    async fn publish_version(&self, version: IndexedDocumentVersion) -> Result<()>;
    async fn search(&self, request: AuthorizedSearch) -> Result<Vec<CandidateChunk>>;
}

trait DecisionEngine {
    async fn validate_query(&self, query: &UserQuery) -> Result<QueryDecision>;
    async fn rank(&self, query: &UserQuery, candidates: &[CandidateChunk]) -> Result<Vec<RankedChunk>>;
    async fn assess_evidence(&self, query: &UserQuery, evidence: &[RankedChunk]) -> Result<EvidenceDecision>;
    async fn validate_citations(&self, claims: &[ClaimWithEvidence]) -> Result<Vec<CitationDecision>>;
}

trait AnswerGenerator {
    async fn generate(&self, request: GroundedAnswerRequest) -> Result<StructuredAnswer>;
}

trait AccessPolicy {
    async fn authorize(&self, subject: &Subject, action: Action, resource: &Resource) -> Result<AuthorizationDecision>;
}

trait AuditSink {
    async fn append(&self, event: AuditEvent) -> Result<()>;
}

trait Cache {
    async fn get(&self, key: &CacheKey) -> Result<Option<CacheValue>>;
    async fn put(&self, key: CacheKey, value: CacheValue, ttl: Duration) -> Result<()>;
    async fn invalidate(&self, scope: InvalidationScope) -> Result<()>;
}
```

`DecisionEngine` avrà adapter JEV, fake deterministico e baseline. Autorizzazione e provenance non appartengono a questa porta.

## Modello di concorrenza

Usare async per I/O e pool limitati per lavoro CPU-bound. Ogni fan-out ha deadline globale, limite, cancellazione, budget di retry e metriche di saturazione.

### Upload e ingestion

- Upload in streaming; mai l'intero file in memoria.
- Job restituito dopo persistenza sicura dell'originale e dell'outbox.
- Worker con lease e `FOR UPDATE SKIP LOCKED` o equivalente.
- Pool distinti per parsing, OCR, embedding e scrittura DB.
- Parsing/OCR CPU-bound tramite `spawn_blocking` o processi isolati.
- Embedding in batch bounded con backpressure.
- Lock logico su `(source, external_id, version)`.
- Checksum e unique constraint per retry idempotenti.

### Query RAG

1. Retrieval autorizzato con deadline interna.
2. Reranking JEV bounded; mai fan-out illimitato per chunk.
3. Cancellazione dei task residui dopo timeout.
4. Una sola generazione per tentativo; rigenerazione esplicita e limitata.
5. Citation validation concorrente con semaforo.
6. Ricontrollo autorizzativo prima di citazioni e link.

Baseline proposta: top-20 dal retrieval, massimo 8 decisioni JEV contemporanee e deadline complessiva configurabile. Sono parametri sperimentali da calibrare.

### Database ed eventi

- Pool separato per servizio e sotto il massimo del server.
- Transazioni brevi; nessuna chiamata esterna dentro una transazione.
- Version column per aggiornamenti concorrenti di metadati e ACL.
- Outbox nella stessa transazione del cambiamento di dominio.
- Consumer at-least-once, `event_id` univoco e handler idempotente.

## Race condition da testare

| Scenario | Comportamento richiesto |
|---|---|
| Due upload identici | Un solo contenuto fisico; job idempotenti o versioni esplicite |
| Cancellazione durante ingestion | Job cancellato e versione non pubblicata |
| Nuova versione durante query | Snapshot coerente e versione citata |
| Revoca ruolo durante query | Ricontrollo prima della risposta |
| Documento rimosso dopo risposta | Il viewer applica nuovamente l'autorizzazione |
| Retry evento audit | Nessun duplicato grazie a `event_id` |
| Timeout JEV | Fallback/rifiuto; mai bypass della sicurezza |

## Backpressure e degradazione

- `429` e `Retry-After` quando le code superano le soglie.
- Circuit breaker per JEV, embedding e LLM.
- Il guasto JEV non abilita azioni negate.
- Viewer e autenticazione restano indipendenti dal generative LLM.

## Spike necessari

- Streaming multipart e memoria in Axum.
- Parser/OCR in-process oppure worker isolato.
- `pgvector` sul corpus incrementale.
- JEV: più domande in una richiesta o chiamate concorrenti per candidato.
- Cancellazione verso provider HTTP.
- Invalidazione cache, se introdotta.
