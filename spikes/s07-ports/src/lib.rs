//! Spike S-07: firme Rust candidate delle porte di dominio di CKA.
//!
//! Il crate fissa la forma dei tipi e dei trait-porta raccolti in
//! `spikes/s07-ports/RISULTATI.md` e la prova con fake di contratto nei test.
//! Nessun adapter reale: le firme si trascrivono nella Fondazione, non si importano.
//!
//! Stato: test di contratto scritti dall'agente test-write del piano 01-16, corpi
//! implementati dall'esecutore dello stesso piano (Task 2). Decisione in ADR-0012.

/// Identificativi opachi, versioni e testi con `Debug` redatto.
pub mod ids {
    use std::fmt;

    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    macro_rules! opaque_id {
        ($($(#[$meta:meta])* $name:ident),+ $(,)?) => {
            $(
                $(#[$meta])*
                #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
                #[serde(transparent)]
                pub struct $name(Uuid);

                impl $name {
                    /// Avvolge un UUID già generato altrove.
                    pub fn new(id: Uuid) -> Self {
                        Self(id)
                    }

                    /// UUID v4 casuale (identificativo opaco, convenzioni API).
                    pub fn random() -> Self {
                        Self(Uuid::new_v4())
                    }

                    /// UUID sottostante.
                    pub fn as_uuid(&self) -> Uuid {
                        self.0
                    }
                }
            )+
        };
    }

    opaque_id!(
        /// Documento logico (tutte le versioni).
        DocumentId,
        /// Versione pubblicata di un documento.
        DocumentVersionId,
        /// Riferimento opaco a un passaggio indicizzato (D-20).
        PassageRef,
        /// Soggetto autenticato (utente).
        SubjectId,
        /// Attore di una mutazione (utente o worker).
        ActorId,
        /// Identificativo della richiesta, propagato fra servizi.
        RequestId,
        /// Chiave di idempotenza delle operazioni mutanti (RISK-003).
        IdempotencyKey,
    );

    /// Versione monotona dello stato autorizzativo del soggetto: cambia anche a
    /// ogni cambio dei suoi ruoli (RISK-002, contratto 4 del piano 01-16).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct AclVersion(u64);

    impl AclVersion {
        /// Versione esplicita.
        pub fn new(value: u64) -> Self {
            Self(value)
        }

        /// Valore numerico.
        pub fn value(&self) -> u64 {
            self.0
        }
    }

    /// Versione del knowledge corpus usata dalla chiave di cache (RISK-002).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct CorpusVersion(u64);

    impl CorpusVersion {
        /// Versione esplicita.
        pub fn new(value: u64) -> Self {
            Self(value)
        }
    }

    /// sha256 del contenuto di un originale, chiave `sha256/<hex>` (S-01, D-02).
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct ContentHash([u8; 32]);

    impl ContentHash {
        /// Digest già calcolato in streaming dal chiamante.
        pub fn new(digest: [u8; 32]) -> Self {
            Self(digest)
        }

        /// Byte del digest.
        pub fn as_bytes(&self) -> &[u8; 32] {
            &self.0
        }
    }

    impl fmt::Debug for ContentHash {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "ContentHash(")?;
            for byte in &self.0[..4] {
                write!(f, "{byte:02x}")?;
            }
            write!(f, "…)")
        }
    }

    /// Testo della domanda dell'utente; `Debug` ne stampa solo la lunghezza.
    #[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct QueryText(String);

    impl QueryText {
        /// Avvolge il testo della domanda.
        pub fn new(text: impl Into<String>) -> Self {
            Self(text.into())
        }

        /// Testo della domanda.
        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    impl fmt::Debug for QueryText {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "QueryText(<{} caratteri>)", self.0.chars().count())
        }
    }

    /// Domanda in ingresso al flusso RAG (stesso valore di [`QueryText`]).
    pub type UserQuery = QueryText;

    /// Testo di un chunk; `Debug` ne stampa solo la lunghezza.
    #[derive(Clone, PartialEq, Eq)]
    pub struct ChunkText(String);

    impl ChunkText {
        /// Avvolge il testo del chunk.
        pub fn new(text: impl Into<String>) -> Self {
            Self(text.into())
        }

        /// Testo del chunk.
        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    impl fmt::Debug for ChunkText {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "ChunkText(<{} caratteri>)", self.0.chars().count())
        }
    }
}

/// Contesti di chiamata: deadline, request_id e trace (S-05, D-22, RISK-003).
pub mod context {
    use std::time::{Duration, Instant};

    use crate::errors::TraceContextError;
    use crate::ids::{ActorId, IdempotencyKey, RequestId};

    /// Istante oltre il quale la chiamata è cancellata (drop del future, ADR-0010).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Deadline {
        at: Instant,
    }

    impl Deadline {
        /// Deadline assoluta.
        pub fn at(at: Instant) -> Self {
            Self { at }
        }

        /// Deadline a `budget` da adesso.
        pub fn after(budget: Duration) -> Self {
            Self {
                at: Instant::now() + budget,
            }
        }

        /// Istante della deadline.
        pub fn instant(&self) -> Instant {
            self.at
        }
    }

    /// `traceparent` W3C validato, valore puro senza tipi di telemetria in firma.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct TraceContext {
        traceparent: String,
    }

    impl TraceContext {
        /// Valida `version-traceid-parentid-flags` (55 caratteri, esadecimale minuscolo,
        /// trace id e parent id non tutti zero).
        pub fn parse(traceparent: &str) -> Result<Self, TraceContextError> {
            fn lower_hex(field: &str, len: usize) -> bool {
                field.len() == len
                    && field
                        .bytes()
                        .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            }
            fn all_zero(field: &str) -> bool {
                field.bytes().all(|b| b == b'0')
            }

            if traceparent.len() != 55 {
                return Err(TraceContextError::Malformed);
            }
            let mut fields = traceparent.split('-');
            let (Some(version), Some(trace_id), Some(parent_id), Some(flags), None) = (
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
                fields.next(),
            ) else {
                return Err(TraceContextError::Malformed);
            };
            let well_formed = lower_hex(version, 2)
                && version != "ff"
                && lower_hex(trace_id, 32)
                && !all_zero(trace_id)
                && lower_hex(parent_id, 16)
                && !all_zero(parent_id)
                && lower_hex(flags, 2);
            if !well_formed {
                return Err(TraceContextError::Malformed);
            }
            Ok(Self {
                traceparent: traceparent.to_owned(),
            })
        }

        /// Stringa `traceparent`.
        pub fn as_str(&self) -> &str {
            &self.traceparent
        }
    }

    /// Contesto di ogni lettura verso un provider (S-05, D-22).
    #[derive(Clone, Debug)]
    pub struct CallContext {
        pub request_id: RequestId,
        pub trace: TraceContext,
        pub deadline: Deadline,
    }

    /// Contesto di ogni operazione mutante (RISK-003).
    #[derive(Clone, Debug)]
    pub struct MutationContext {
        pub call: CallContext,
        pub idempotency_key: IdempotencyKey,
        pub actor: ActorId,
    }

    /// Esito di una mutazione riuscita: un retry con la stessa chiave dà
    /// `AlreadyApplied` (RISK-003).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum MutationOutcome {
        Applied,
        AlreadyApplied,
    }
}

/// Locator di citazione a due varianti (S-02, ADR-0005, D-10).
pub mod locator {
    use serde::{Deserialize, Serialize};

    use crate::ids::ContentHash;

    /// Posizione verificabile di un passaggio nell'originale.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub enum Locator {
        /// PDF: pagina e rettangoli in punti PDF, al più due span.
        Pdf { spans: Vec<PdfSpan> },
        /// DOCX/MD: proiezione testuale canonica, al più due span.
        Text {
            projection: TextProjection,
            doc_sha256: ContentHash,
            spans: Vec<TextSpan>,
        },
    }

    /// Span PDF di una pagina.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct PdfSpan {
        pub page: u32,
        pub rects: Vec<Rect>,
    }

    /// Rettangolo in punti PDF, origine in basso a sinistra.
    #[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Rect {
        pub x0: f32,
        pub y0: f32,
        pub x1: f32,
        pub y1: f32,
    }

    /// Proiezione testuale canonica.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub enum TextProjection {
        CkaTextV1,
    }

    /// Span testuale dentro una sezione.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct TextSpan {
        pub section_path: Vec<SectionStep>,
        pub char_range: CharRange,
        pub section_text_sha256: ContentHash,
    }

    /// Passo del percorso di sezione, con ordinale fra sorelle omonime.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct SectionStep {
        pub title: String,
        pub ordinal: u32,
    }

    /// Intervallo semiaperto in Unicode scalar sul testo canonico.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct CharRange {
        pub start: u32,
        pub end: u32,
    }
}

/// Errori tipizzati, un enum per porta (niente `anyhow`, nessun errore di SDK).
pub mod errors {
    /// `traceparent` non valido.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum TraceContextError {
        Malformed,
    }

    /// Errori di risoluzione del locator (ADR-0005).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum LocatorError {
        ProjectionMismatch,
        VersionMismatch,
        SectionHashMismatch,
        SectionNotFound,
        RangeOutOfBounds,
    }

    /// `Embedding::new` con dimensione diversa da quella dello spazio (RISK-004).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum EmbeddingError {
        DimensionMismatch { expected: u32, actual: usize },
    }

    /// `IndexVersionDraft::new` con un chunk di spazio diverso (RISK-004).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum DraftError {
        SpaceMismatch { chunk_index: usize },
    }

    /// Errori di `DocumentStore` (S-01).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum StoreError {
        NotFound,
        NotApplied { reason: String },
        OutcomeUnknown,
        Timeout,
        Unavailable,
    }

    /// Errori di `DocumentParser`, uno per esito ostile di S-02 (D-08).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum ParseError {
        Encrypted,
        DecompressionLimit,
        Timeout,
        ResourceLimit,
        OutputLimit,
        Crashed { signal: i32 },
        Corrupt,
        Unsupported,
    }

    /// Errori di `Chunker`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum ChunkError {
        EmptyDocument,
        InconsistentLocator,
    }

    /// Errori di `Embedder` (S-03).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum EmbedError {
        Timeout,
        Unavailable,
        InvalidResponse,
        DimensionMismatch { expected: u32, actual: usize },
    }

    /// Errori di `VectorIndex` (S-03, RISK-003, RISK-004).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum IndexError {
        SpaceMismatch,
        NotApplied { reason: String },
        OutcomeUnknown,
        Timeout,
        Unavailable,
    }

    /// Errori dei due lati di `evidence` (RISK-001, re-gate R2).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum EvidenceError {
        SpaceMismatch,
        SubjectMismatch,
        AclVersionMismatch,
        DuplicatePassageRef,
        UnknownPassageRef,
        InvalidScore,
        Index(IndexError),
    }

    /// Errori di `EvidenceRetriever` (client HTTP interno, adapter futuro).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum RetrieverError {
        Timeout,
        Unauthorized,
        Http { status: u16 },
        InvalidResponse,
        Unavailable,
    }

    /// Errori di `DecisionEngine` (S-04, S-06): mai una decisione di default.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum DecisionError {
        Timeout,
        RateLimited,
        BudgetExceeded,
        Http { status: u16 },
        InvalidResponse,
        Unavailable,
    }

    /// Errori di `Reranker` (S-04, ADR-0008).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum RerankError {
        Unauthorized,
        Timeout,
        RateLimited,
        BudgetExceeded,
        Http { status: u16 },
        InvalidResponse,
    }

    /// Errori di `AnswerGenerator` (S-06).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum AnswerError {
        Timeout,
        RateLimited,
        Http { status: u16 },
        InvalidResponse,
        Unavailable,
    }

    /// Errori di `AccessPolicy`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum PolicyError {
        UnknownSubject,
        Timeout,
        Unavailable,
    }

    /// Errori di `AuditSink` (RISK-003).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum AuditError {
        NotApplied { reason: String },
        OutcomeUnknown,
        EventIdMismatch,
        Timeout,
    }

    /// Errori di `Cache` (RISK-002).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum CacheError {
        NotApplied { reason: String },
        OutcomeUnknown,
        Unavailable,
    }

    /// Errori di `DocumentSource` (S-05).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum SourceError {
        Timeout,
        Unreachable,
        InvalidResponse,
    }
}

/// Spazio di embedding, vettori ed `Embedder` (S-03, D-14, D-38, RISK-004).
pub mod embedding {
    use async_trait::async_trait;
    use serde::{Deserialize, Serialize};

    use crate::context::CallContext;
    use crate::errors::{EmbedError, EmbeddingError};
    use crate::ids::{ChunkText, QueryText};

    /// Identità completa dello spazio: la dimensione da sola non basta (RISK-004).
    #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct EmbeddingSpace {
        pub model_id: String,
        pub revision: String,
        pub dimension: u32,
        pub normalization: Normalization,
        pub pooling: Pooling,
        pub query_instruction: Option<String>,
        pub document_instruction: Option<String>,
    }

    /// Normalizzazione dei vettori.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum Normalization {
        L2,
        None,
    }

    /// Pooling del modello.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum Pooling {
        Cls,
        Mean,
        LastToken,
    }

    /// Vettore con il suo spazio; costruibile solo a dimensione coerente.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Embedding {
        space: EmbeddingSpace,
        values: Vec<f32>,
    }

    impl Embedding {
        /// Rifiuta `values.len() != space.dimension` (S-03, RISK-004).
        pub fn new(space: EmbeddingSpace, values: Vec<f32>) -> Result<Self, EmbeddingError> {
            if values.len() != space.dimension as usize {
                return Err(EmbeddingError::DimensionMismatch {
                    expected: space.dimension,
                    actual: values.len(),
                });
            }
            Ok(Self { space, values })
        }

        /// Spazio del vettore.
        pub fn space(&self) -> &EmbeddingSpace {
            &self.space
        }

        /// Componenti del vettore.
        pub fn values(&self) -> &[f32] {
            &self.values
        }
    }

    /// Porta degli embedding: documenti e query separati (S-03, D-38).
    #[async_trait]
    pub trait Embedder: Send + Sync {
        /// Embedding dei chunk; l'istruzione di documento la applica l'adapter se lo spazio la dichiara (D-38).
        async fn embed_documents(
            &self,
            ctx: CallContext,
            texts: &[ChunkText],
        ) -> Result<Vec<Embedding>, EmbedError>;

        /// Embedding della query con l'istruzione di query dello spazio (S-03, D-38).
        async fn embed_query(
            &self,
            ctx: CallContext,
            query: &QueryText,
        ) -> Result<Embedding, EmbedError>;

        /// Spazio prodotto da questo embedder (RISK-004).
        fn space(&self) -> &EmbeddingSpace;
    }
}

/// I due lati del confine `POST /internal/v1/retrieval/search` (RISK-001, re-gate R2).
pub mod evidence {
    /// Lato documento (futuro document-service): predicato, ricontrollo e `RetrievedEvidence`.
    pub mod document_side {
        use std::collections::HashSet;
        use std::fmt;

        use crate::context::CallContext;
        use crate::embedding::{Embedding, EmbeddingSpace};
        use crate::errors::EvidenceError;
        use crate::evidence::wire::{RetrievalSearchResponse, RetrievedPassageDto};
        use crate::ids::{AclVersion, DocumentVersionId, PassageRef, SubjectId};
        use crate::index::{Candidate, VectorIndex};
        use crate::locator::Locator;
        use crate::policy::Role;

        /// Predicato autorizzativo applicato dall'indice dentro la query del ranking (S-03, D-12).
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct AuthorizationPredicate {
            pub subject_id: SubjectId,
            pub roles: Vec<Role>,
            pub acl_version: AclVersion,
        }

        /// Passaggio ricontrollato lato documento; `Debug` senza testo.
        pub struct RetrievedPassage {
            passage_ref: PassageRef,
            document_version_id: DocumentVersionId,
            locator: Locator,
            text: String,
        }

        impl RetrievedPassage {
            /// Riferimento del passaggio.
            pub fn passage_ref(&self) -> PassageRef {
                self.passage_ref
            }

            /// Versione di documento del passaggio.
            pub fn document_version_id(&self) -> DocumentVersionId {
                self.document_version_id
            }

            /// Locator del passaggio.
            pub fn locator(&self) -> &Locator {
                &self.locator
            }

            /// Testo del passaggio.
            pub fn text(&self) -> &str {
                &self.text
            }
        }

        impl fmt::Debug for RetrievedPassage {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("RetrievedPassage")
                    .field("passage_ref", &self.passage_ref)
                    .field("document_version_id", &self.document_version_id)
                    .field("text_chars", &self.text.chars().count())
                    .finish_non_exhaustive()
            }
        }

        /// Evidenza uscita da `retrieve_authorized`: campi e costruttore privati.
        ///
        /// Costruirla fuori da `evidence::document_side` non compila (P-7):
        ///
        /// ```compile_fail,E0451
        /// use s07_ports::embedding::EmbeddingSpace;
        /// use s07_ports::evidence::document_side::RetrievedEvidence;
        /// use s07_ports::ids::{AclVersion, SubjectId};
        ///
        /// fn forge(subject_id: SubjectId, acl_version: AclVersion, space: EmbeddingSpace) -> RetrievedEvidence {
        ///     RetrievedEvidence { subject_id, acl_version, space, passages: Vec::new() }
        /// }
        /// ```
        pub struct RetrievedEvidence {
            subject_id: SubjectId,
            acl_version: AclVersion,
            space: EmbeddingSpace,
            passages: Vec<RetrievedPassage>,
        }

        impl RetrievedEvidence {
            /// Passaggi ricontrollati, nell'ordine del ranking.
            pub fn passages(&self) -> &[RetrievedPassage] {
                &self.passages
            }

            /// Unica uscita: DTO del confine con `subject_id` e `acl_version` del predicato usato.
            pub fn into_response(self) -> RetrievalSearchResponse {
                RetrievalSearchResponse {
                    subject_id: self.subject_id,
                    acl_version: self.acl_version,
                    space: self.space,
                    passages: self
                        .passages
                        .into_iter()
                        .map(|p| RetrievedPassageDto {
                            passage_ref: p.passage_ref,
                            document_version_id: p.document_version_id,
                            locator: p.locator,
                            text: p.text,
                        })
                        .collect(),
                }
            }
        }

        impl fmt::Debug for RetrievedEvidence {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("RetrievedEvidence")
                    .field("subject_id", &self.subject_id)
                    .field("acl_version", &self.acl_version)
                    .field("space", &self.space.model_id)
                    .field("passages", &self.passages)
                    .finish()
            }
        }

        /// Ricontrollo deterministico di un candidato contro il predicato usato per la
        /// ricerca: versione pubblicata, stessa versione ACL (deny by default su una
        /// versione diversa, anche più recente) e soggetto o ruolo ammesso.
        fn admits(predicate: &AuthorizationPredicate, candidate: &Candidate) -> bool {
            let acl = &candidate.acl;
            acl.published
                && acl.acl_version == predicate.acl_version
                && (acl.allowed_subjects.contains(&predicate.subject_id)
                    || acl
                        .allowed_roles
                        .iter()
                        .any(|role| predicate.roles.contains(role)))
        }

        /// (a) rifiuta il mismatch di spazio prima di `search`, (b) cerca col predicato,
        /// (c) ricontrolla ogni candidato (soggetto o ruolo, versione ACL, pubblicazione)
        /// e scarta i non conformi (S-03, RISK-001, RISK-004).
        pub async fn retrieve_authorized(
            index: &dyn VectorIndex,
            ctx: CallContext,
            query: Embedding,
            predicate: &AuthorizationPredicate,
            k: usize,
        ) -> Result<RetrievedEvidence, EvidenceError> {
            if query.space() != index.space() {
                return Err(EvidenceError::SpaceMismatch);
            }
            let candidates = index
                .search(ctx, &query, predicate, k)
                .await
                .map_err(EvidenceError::Index)?;

            let mut seen = HashSet::new();
            let passages = candidates
                .into_iter()
                .filter(|candidate| admits(predicate, candidate))
                .filter(|candidate| seen.insert(candidate.passage_ref))
                .take(k)
                .map(|candidate| RetrievedPassage {
                    passage_ref: candidate.passage_ref,
                    document_version_id: candidate.document_version_id,
                    locator: candidate.locator,
                    text: candidate.text,
                })
                .collect();

            Ok(RetrievedEvidence {
                subject_id: predicate.subject_id,
                acl_version: predicate.acl_version,
                space: index.space().clone(),
                passages,
            })
        }
    }

    /// DTO del confine, destinati a `crates/contracts`: dato non fidato per chi li riceve.
    pub mod wire {
        use std::fmt;

        use serde::{Deserialize, Serialize};

        use crate::embedding::EmbeddingSpace;
        use crate::ids::{AclVersion, DocumentVersionId, PassageRef, QueryText, SubjectId};
        use crate::locator::Locator;

        /// Corpo della richiesta: il principal viaggia nel token verificato, non qui.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct RetrievalSearchRequest {
            pub query_text: QueryText,
            pub k: u32,
        }

        /// Corpo della risposta, ricontrollato lato RAG da `authorize_response`.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct RetrievalSearchResponse {
            pub subject_id: SubjectId,
            pub acl_version: AclVersion,
            pub space: EmbeddingSpace,
            pub passages: Vec<RetrievedPassageDto>,
        }

        /// Passaggio sul filo; `Debug` manuale che omette `text`.
        #[derive(Clone, PartialEq, Serialize, Deserialize)]
        pub struct RetrievedPassageDto {
            pub passage_ref: PassageRef,
            pub document_version_id: DocumentVersionId,
            pub locator: Locator,
            pub text: String,
        }

        impl fmt::Debug for RetrievedPassageDto {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("RetrievedPassageDto")
                    .field("passage_ref", &self.passage_ref)
                    .field("document_version_id", &self.document_version_id)
                    .field("text_chars", &self.text.chars().count())
                    .finish_non_exhaustive()
            }
        }
    }

    /// Lato RAG (futuro rag-service): `EvidenceRetriever`, `authorize_response`, `AuthorizedEvidence`.
    pub mod rag_side {
        use std::collections::{HashMap, HashSet};
        use std::fmt;

        use async_trait::async_trait;

        use crate::context::CallContext;
        use crate::decision::RankScore;
        use crate::embedding::EmbeddingSpace;
        use crate::errors::{EvidenceError, RetrieverError};
        use crate::evidence::wire::{RetrievalSearchRequest, RetrievalSearchResponse};
        use crate::ids::{AclVersion, DocumentVersionId, PassageRef, SubjectId};
        use crate::locator::Locator;
        use crate::policy::AuthorizationScope;

        /// Porta lato RAG del confine di retrieval; adapter futuro: client HTTP interno.
        #[async_trait]
        pub trait EvidenceRetriever: Send + Sync {
            /// Restituisce il DTO, dato non fidato da passare ad `authorize_response` (re-gate R2).
            async fn search(
                &self,
                ctx: CallContext,
                request: RetrievalSearchRequest,
            ) -> Result<RetrievalSearchResponse, RetrieverError>;
        }

        /// Passaggio autorizzato; nessun `Serialize`/`Deserialize`, `Debug` senza testo.
        pub struct AuthorizedPassage {
            passage_ref: PassageRef,
            document_version_id: DocumentVersionId,
            locator: Locator,
            text: String,
        }

        impl AuthorizedPassage {
            /// Riferimento del passaggio.
            pub fn passage_ref(&self) -> PassageRef {
                self.passage_ref
            }

            /// Versione di documento del passaggio.
            pub fn document_version_id(&self) -> DocumentVersionId {
                self.document_version_id
            }

            /// Locator del passaggio.
            pub fn locator(&self) -> &Locator {
                &self.locator
            }

            /// Testo del passaggio.
            pub fn text(&self) -> &str {
                &self.text
            }
        }

        impl fmt::Debug for AuthorizedPassage {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("AuthorizedPassage")
                    .field("passage_ref", &self.passage_ref)
                    .field("document_version_id", &self.document_version_id)
                    .field("text_chars", &self.text.chars().count())
                    .finish_non_exhaustive()
            }
        }

        /// Unico input dei consumatori semantici (RISK-001, T-14).
        ///
        /// Costruirla fuori da `evidence::rag_side` non compila (P-6):
        ///
        /// ```compile_fail,E0451
        /// use s07_ports::embedding::EmbeddingSpace;
        /// use s07_ports::evidence::rag_side::AuthorizedEvidence;
        /// use s07_ports::ids::{AclVersion, SubjectId};
        ///
        /// fn forge(subject_id: SubjectId, acl_version: AclVersion, space: EmbeddingSpace) -> AuthorizedEvidence {
        ///     AuthorizedEvidence { subject_id, acl_version, space, passages: Vec::new() }
        /// }
        /// ```
        ///
        /// Non è serializzabile (P-10):
        ///
        /// ```compile_fail,E0277
        /// fn requires_serialize<T: serde::Serialize>() {}
        /// requires_serialize::<s07_ports::evidence::rag_side::AuthorizedEvidence>();
        /// ```
        ///
        /// Né deserializzabile: un adapter non può saltare `authorize_response` (P-8):
        ///
        /// ```compile_fail,E0277
        /// fn requires_deserialize<T: serde::de::DeserializeOwned>() {}
        /// requires_deserialize::<s07_ports::evidence::rag_side::AuthorizedEvidence>();
        /// ```
        pub struct AuthorizedEvidence {
            subject_id: SubjectId,
            acl_version: AclVersion,
            space: EmbeddingSpace,
            passages: Vec<AuthorizedPassage>,
        }

        impl AuthorizedEvidence {
            /// Soggetto per cui l'evidenza è autorizzata.
            pub fn subject_id(&self) -> SubjectId {
                self.subject_id
            }

            /// Versione ACL ricontrollata.
            pub fn acl_version(&self) -> AclVersion {
                self.acl_version
            }

            /// Spazio di embedding ricontrollato.
            pub fn space(&self) -> &EmbeddingSpace {
                &self.space
            }

            /// Passaggi autorizzati, nell'ordine corrente.
            pub fn passages(&self) -> &[AuthorizedPassage] {
                &self.passages
            }

            /// Riordina per `score` decrescente (a parità resta l'ordine corrente) e toglie i
            /// passaggi senza score: può solo permutare o togliere. Un `PassageRef` assente
            /// dall'evidenza è `UnknownPassageRef`, uno ripetuto `DuplicatePassageRef` (S-04, D-21).
            ///
            /// Uno score non finito (NaN o infinito) è `InvalidScore`: un segnale guasto non
            /// decide l'ordine.
            pub fn reorder(self, scores: &[RankScore]) -> Result<Self, EvidenceError> {
                let known: HashSet<PassageRef> =
                    self.passages.iter().map(|p| p.passage_ref).collect();
                let mut by_ref: HashMap<PassageRef, f32> = HashMap::with_capacity(scores.len());
                for score in scores {
                    if !known.contains(&score.passage_ref) {
                        return Err(EvidenceError::UnknownPassageRef);
                    }
                    if !score.score.is_finite() {
                        return Err(EvidenceError::InvalidScore);
                    }
                    if by_ref.insert(score.passage_ref, score.score).is_some() {
                        return Err(EvidenceError::DuplicatePassageRef);
                    }
                }

                let Self {
                    subject_id,
                    acl_version,
                    space,
                    passages,
                } = self;
                let mut scored: Vec<(f32, AuthorizedPassage)> = passages
                    .into_iter()
                    .filter_map(|p| by_ref.get(&p.passage_ref).map(|s| (*s, p)))
                    .collect();
                // `sort_by` è stabile: a parità di score resta l'ordine corrente.
                scored.sort_by(|a, b| b.0.total_cmp(&a.0));

                Ok(Self {
                    subject_id,
                    acl_version,
                    space,
                    passages: scored.into_iter().map(|(_, p)| p).collect(),
                })
            }

            /// Mapping deterministico di una citazione al passaggio; `None` se ignoto (D-20).
            pub fn resolve(&self, passage_ref: &PassageRef) -> Option<&AuthorizedPassage> {
                self.passages.iter().find(|p| p.passage_ref == *passage_ref)
            }
        }

        impl fmt::Debug for AuthorizedEvidence {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("AuthorizedEvidence")
                    .field("subject_id", &self.subject_id)
                    .field("acl_version", &self.acl_version)
                    .field("space", &self.space.model_id)
                    .field("passages", &self.passages)
                    .finish()
            }
        }

        /// Unica conversione DTO -> `AuthorizedEvidence`: rifiuta soggetto, versione ACL o
        /// spazio diversi da quelli attesi e i `PassageRef` duplicati, senza evidenza parziale
        /// (RISK-001 v2, re-gate R2).
        pub fn authorize_response(
            response: RetrievalSearchResponse,
            expected: &AuthorizationScope,
            expected_space: &EmbeddingSpace,
        ) -> Result<AuthorizedEvidence, EvidenceError> {
            if response.subject_id != expected.subject_id {
                return Err(EvidenceError::SubjectMismatch);
            }
            if response.acl_version != expected.acl_version {
                return Err(EvidenceError::AclVersionMismatch);
            }
            if response.space != *expected_space {
                return Err(EvidenceError::SpaceMismatch);
            }
            let mut seen = HashSet::with_capacity(response.passages.len());
            if !response.passages.iter().all(|p| seen.insert(p.passage_ref)) {
                return Err(EvidenceError::DuplicatePassageRef);
            }

            Ok(AuthorizedEvidence {
                subject_id: response.subject_id,
                acl_version: response.acl_version,
                space: response.space,
                passages: response
                    .passages
                    .into_iter()
                    .map(|p| AuthorizedPassage {
                        passage_ref: p.passage_ref,
                        document_version_id: p.document_version_id,
                        locator: p.locator,
                        text: p.text,
                    })
                    .collect(),
            })
        }
    }
}

/// Originali per hash di contenuto (S-01, ADR-0003, D-02).
pub mod store {
    use std::ops::Range;
    use std::path::{Path, PathBuf};

    use async_trait::async_trait;
    use bytes::Bytes;
    use futures::stream::BoxStream;

    use crate::context::{CallContext, MutationContext, MutationOutcome};
    use crate::errors::StoreError;
    use crate::ids::ContentHash;

    /// Flusso di byte in streaming, senza tipi di SDK.
    pub type ByteStream<E> = BoxStream<'static, Result<Bytes, E>>;

    /// File già validato (tempfile + dimensione); lo costruisce solo la validazione.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ValidatedFile {
        path: PathBuf,
        size: u64,
    }

    impl ValidatedFile {
        /// Esito della validazione dell'upload o della quarantena (S-01).
        pub fn from_validation(path: PathBuf, size: u64) -> Self {
            Self { path, size }
        }

        /// Percorso del tempfile.
        pub fn path(&self) -> &Path {
            &self.path
        }

        /// Dimensione in byte.
        pub fn size(&self) -> u64 {
            self.size
        }
    }

    /// Porta degli originali.
    #[async_trait]
    pub trait DocumentStore: Send + Sync {
        /// Idempotente per chiave anche sotto PUT concorrenti (S-01, D-02, RISK-003).
        async fn put(
            &self,
            ctx: MutationContext,
            hash: ContentHash,
            source: ValidatedFile,
        ) -> Result<MutationOutcome, StoreError>;

        /// Lettura per range per il viewer (S-01).
        async fn get(
            &self,
            ctx: CallContext,
            key: &ContentHash,
            range: Option<Range<u64>>,
        ) -> Result<ByteStream<StoreError>, StoreError>;

        /// Cancellazione degli orfani, usata solo dal GC (S-01, D-02).
        async fn delete(
            &self,
            ctx: MutationContext,
            key: &ContentHash,
        ) -> Result<MutationOutcome, StoreError>;
    }
}

/// Parsing isolato e chunking (S-02, ADR-0004, D-05, D-08, D-10).
pub mod parser {
    use std::fmt;

    use async_trait::async_trait;

    use crate::context::CallContext;
    use crate::errors::{ChunkError, ParseError};
    use crate::ids::{ChunkText, ContentHash};
    use crate::locator::Locator;

    /// Tipo di file dopo la validazione.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum FileKind {
        Pdf,
        Docx,
        Markdown,
    }

    /// Ingresso del parser.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ParseInput {
        pub key: ContentHash,
        pub kind: FileKind,
    }

    /// Esito: un PDF attivo esce come `Quarantined`, mai come `Parsed` (D-05).
    #[derive(Debug)]
    pub enum ParseOutcome {
        Parsed(ParsedDocument),
        Quarantined { reasons: Vec<QuarantineReason> },
    }

    /// Motivi fissi di quarantena.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum QuarantineReason {
        JavaScript,
        LaunchAction,
        EmbeddedFile,
        OpenAction,
    }

    /// Documento parsato in blocchi con locator.
    #[derive(Debug, Default)]
    pub struct ParsedDocument {
        pub blocks: Vec<Block>,
    }

    /// Blocco di testo, tabella o parole OCR.
    pub struct Block {
        pub kind: BlockKind,
        pub text: String,
        pub locator: Locator,
    }

    impl fmt::Debug for Block {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("Block")
                .field("kind", &self.kind)
                .field("text_chars", &self.text.chars().count())
                .field("locator", &self.locator)
                .finish()
        }
    }

    /// Tipo di blocco.
    #[derive(Debug)]
    pub enum BlockKind {
        Text,
        Table { cells: Vec<Cell> },
        OcrWords,
    }

    /// Cella di tabella; `Debug` senza testo.
    pub struct Cell {
        pub row: u32,
        pub col: u32,
        pub text: String,
    }

    impl fmt::Debug for Cell {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("Cell")
                .field("row", &self.row)
                .field("col", &self.col)
                .field("text_chars", &self.text.chars().count())
                .finish()
        }
    }

    /// Porta del parser: fuori processo, con budget del worker (S-02, ADR-0004).
    #[async_trait]
    pub trait DocumentParser: Send + Sync {
        /// Blocchi con `Locator` o quarantena; errori tipizzati per esito ostile (D-08).
        async fn parse(
            &self,
            ctx: CallContext,
            input: ParseInput,
        ) -> Result<ParseOutcome, ParseError>;
    }

    /// Bozza di chunk con un solo locator coerente.
    #[derive(Clone, Debug, PartialEq)]
    pub struct ChunkDraft {
        pub text: ChunkText,
        pub locator: Locator,
        pub ordinal: u32,
    }

    /// Porta del chunking: sincrona e pura, il chiamante la esegue in `spawn_blocking`.
    pub trait Chunker: Send + Sync {
        /// Un chunk non attraversa mai due pagine o due sezioni (S-03 «Setup»).
        fn chunk(&self, document: &ParsedDocument) -> Result<Vec<ChunkDraft>, ChunkError>;
    }
}

/// Indice vettoriale e bozza di versione (S-03, ADR-0006, D-12, D-15).
pub mod index {
    use std::fmt;

    use async_trait::async_trait;

    use crate::context::{CallContext, MutationContext, MutationOutcome};
    use crate::embedding::{Embedding, EmbeddingSpace};
    use crate::errors::{DraftError, IndexError};
    use crate::evidence::document_side::AuthorizationPredicate;
    use crate::ids::{AclVersion, ChunkText, DocumentVersionId, PassageRef, SubjectId};
    use crate::locator::Locator;
    use crate::policy::Role;

    /// ACL e stato del candidato, per il ricontrollo di `retrieve_authorized`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CandidateAcl {
        pub allowed_subjects: Vec<SubjectId>,
        pub allowed_roles: Vec<Role>,
        pub acl_version: AclVersion,
        pub published: bool,
    }

    /// Candidato restituito dall'indice; `Debug` senza testo.
    pub struct Candidate {
        pub passage_ref: PassageRef,
        pub document_version_id: DocumentVersionId,
        pub acl: CandidateAcl,
        pub locator: Locator,
        pub distance: f32,
        pub text: String,
    }

    impl fmt::Debug for Candidate {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("Candidate")
                .field("passage_ref", &self.passage_ref)
                .field("document_version_id", &self.document_version_id)
                .field("acl", &self.acl)
                .field("distance", &self.distance)
                .field("text_chars", &self.text.chars().count())
                .finish_non_exhaustive()
        }
    }

    /// Chunk indicizzato di una bozza di versione.
    #[derive(Clone, Debug, PartialEq)]
    pub struct IndexedChunk {
        pub passage_ref: PassageRef,
        pub embedding: Embedding,
        pub locator: Locator,
        pub text: ChunkText,
        pub acl: CandidateAcl,
    }

    /// Bozza di versione pubblicata atomicamente; tutti i chunk nello stesso spazio.
    #[derive(Clone, Debug, PartialEq)]
    pub struct IndexVersionDraft {
        space: EmbeddingSpace,
        document_version_id: DocumentVersionId,
        chunks: Vec<IndexedChunk>,
    }

    impl IndexVersionDraft {
        /// Rifiuta un chunk il cui embedding ha spazio diverso da `space` (RISK-004).
        pub fn new(
            space: EmbeddingSpace,
            document_version_id: DocumentVersionId,
            chunks: Vec<IndexedChunk>,
        ) -> Result<Self, DraftError> {
            if let Some(chunk_index) = chunks.iter().position(|c| c.embedding.space() != &space) {
                return Err(DraftError::SpaceMismatch { chunk_index });
            }
            Ok(Self {
                space,
                document_version_id,
                chunks,
            })
        }

        /// Spazio della bozza.
        pub fn space(&self) -> &EmbeddingSpace {
            &self.space
        }

        /// Versione di documento.
        pub fn document_version_id(&self) -> DocumentVersionId {
            self.document_version_id
        }

        /// Chunk della bozza.
        pub fn chunks(&self) -> &[IndexedChunk] {
            &self.chunks
        }
    }

    /// Porta dell'indice vettoriale.
    #[async_trait]
    pub trait VectorIndex: Send + Sync {
        /// Predicato applicato dentro la query del ranking, nessun post-filtro (S-03, D-12, RISK-001).
        async fn search(
            &self,
            ctx: CallContext,
            query: &Embedding,
            predicate: &AuthorizationPredicate,
            k: usize,
        ) -> Result<Vec<Candidate>, IndexError>;

        /// Pubblicazione atomica e idempotente sulla chiave (S-03, RISK-003).
        async fn publish_version(
            &self,
            ctx: MutationContext,
            draft: IndexVersionDraft,
        ) -> Result<MutationOutcome, IndexError>;

        /// Spazio con cui l'indice è costruito (RISK-004).
        fn space(&self) -> &EmbeddingSpace;
    }
}

/// `DecisionEngine` e il trait stretto `Reranker` (S-04, S-06, ADR-0008, ADR-0009).
pub mod decision {
    use async_trait::async_trait;

    use crate::answer::StructuredAnswer;
    use crate::context::CallContext;
    use crate::errors::{DecisionError, RerankError};
    use crate::evidence::rag_side::AuthorizedEvidence;
    use crate::ids::{PassageRef, UserQuery};

    /// Metadati di una chiamata a un modello: versione risolta e consumo (S-04, S-06).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CallMeta {
        pub resolved_version: String,
        pub usage: Usage,
    }

    /// Consumo dichiarato dal provider, se presente.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Usage {
        pub input_tokens: Option<u32>,
        pub output_tokens: Option<u32>,
    }

    /// Valore con i metadati della chiamata.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Metered<T> {
        pub value: T,
        pub meta: CallMeta,
    }

    /// Complessità per il routing del modello (D-18).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Complexity {
        Simple,
        Complex,
    }

    /// Esito di `validate_query`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct QueryDecision {
        pub admissible: bool,
        pub reasons: Vec<String>,
        pub complexity: Complexity,
    }

    /// Esito di `assess_evidence`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct EvidenceDecision {
        pub sufficient: bool,
        pub contradictory: bool,
        pub instructs_model: bool,
    }

    /// Verdetto per coppia claim–citazione.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CitationVerdict {
        Supported,
        Unsupported,
        Unavailable,
    }

    /// Decisione su una citazione di un claim.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct CitationDecision {
        pub claim_index: usize,
        pub citation: PassageRef,
        pub verdict: CitationVerdict,
    }

    /// Segnale di rank per passaggio; il riordino lo applica `AuthorizedEvidence::reorder`.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct RankScore {
        pub passage_ref: PassageRef,
        pub score: f32,
    }

    /// Decisioni semantiche JEV; `rank` sta in `Reranker` (ISP, ADR-0012).
    #[async_trait]
    pub trait DecisionEngine: Send + Sync {
        /// Validazione e complessità della query; errore tipizzato, mai complessità di default (S-06, D-18).
        async fn validate_query(
            &self,
            ctx: CallContext,
            query: &UserQuery,
        ) -> Result<Metered<QueryDecision>, DecisionError>;

        /// Sufficienza e contraddizione, solo su evidenza autorizzata (S-04, D-17, RISK-001).
        async fn assess_evidence(
            &self,
            ctx: CallContext,
            query: &UserQuery,
            evidence: &AuthorizedEvidence,
        ) -> Result<Metered<EvidenceDecision>, DecisionError>;

        /// Validazione claim–citazione con testo risolto da `AuthorizedEvidence::resolve` (S-06, D-20).
        async fn validate_citations(
            &self,
            ctx: CallContext,
            evidence: &AuthorizedEvidence,
            answer: &StructuredAnswer,
        ) -> Result<Metered<Vec<CitationDecision>>, DecisionError>;
    }

    /// Trait stretto del solo rank: JEV, baseline identità, eventuale TEI (S-04, ADR-0009).
    #[async_trait]
    pub trait Reranker: Send + Sync {
        /// Segnali per passaggio, ordine stabile a parità (S-04, D-21).
        async fn rank(
            &self,
            ctx: CallContext,
            query: &UserQuery,
            evidence: &AuthorizedEvidence,
        ) -> Result<Metered<Vec<RankScore>>, RerankError>;
    }
}

/// Generazione strutturata con citazioni per claim (S-06, ADR-0011, D-20).
pub mod answer {
    use std::fmt;

    use async_trait::async_trait;

    use crate::context::CallContext;
    use crate::decision::Metered;
    use crate::errors::AnswerError;
    use crate::evidence::rag_side::AuthorizedEvidence;
    use crate::ids::{PassageRef, UserQuery};

    /// Claim con le sue citazioni; `Debug` senza testo.
    #[derive(Clone, PartialEq, Eq)]
    pub struct Claim {
        pub text: String,
        pub citations: Vec<PassageRef>,
    }

    impl fmt::Debug for Claim {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("Claim")
                .field("text_chars", &self.text.chars().count())
                .field("citations", &self.citations)
                .finish()
        }
    }

    /// Risposta strutturata; `Debug` senza testi.
    #[derive(Clone, PartialEq, Eq)]
    pub struct StructuredAnswer {
        pub answer: String,
        pub claims: Vec<Claim>,
        pub refusal: Option<String>,
    }

    impl fmt::Debug for StructuredAnswer {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("StructuredAnswer")
                .field("answer_chars", &self.answer.chars().count())
                .field("claims", &self.claims)
                .field("refused", &self.refusal.is_some())
                .finish()
        }
    }

    /// Porta della generazione; modello e fallback sono composizione (S-06, D-18).
    #[async_trait]
    pub trait AnswerGenerator: Send + Sync {
        /// Structured output con citazioni per claim, solo da evidenza autorizzata (S-06, D-20).
        async fn generate(
            &self,
            ctx: CallContext,
            query: &UserQuery,
            evidence: &AuthorizedEvidence,
        ) -> Result<Metered<StructuredAnswer>, AnswerError>;
    }
}

/// Politica di accesso, principal e scope autorizzativo (RISK-001, RISK-002).
pub mod policy {
    use async_trait::async_trait;
    use serde::{Deserialize, Serialize};

    use crate::context::CallContext;
    use crate::errors::PolicyError;
    use crate::evidence::document_side::AuthorizationPredicate;
    use crate::ids::{AclVersion, DocumentId, DocumentVersionId, SubjectId};

    /// Ruoli dell'MVP.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum Role {
        Admin,
        Manager,
        Employee,
        Guest,
    }

    /// Principal verificato della richiesta (derivazione dal JWT alla fase identity).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Principal {
        pub subject_id: SubjectId,
        pub roles: Vec<Role>,
        pub acl_version: AclVersion,
    }

    /// Scope atteso: soggetto e versione ACL (RISK-001 lato RAG, RISK-002 cache).
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub struct AuthorizationScope {
        pub subject_id: SubjectId,
        pub acl_version: AclVersion,
    }

    /// Azione da autorizzare.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Action {
        Read,
        Cite,
        ViewSource,
    }

    /// Risorsa da autorizzare.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Resource {
        pub document_id: DocumentId,
        pub document_version_id: Option<DocumentVersionId>,
    }

    /// Esito deny by default.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum AuthorizationDecision {
        Allow,
        Deny,
    }

    /// Porta della politica di accesso.
    #[async_trait]
    pub trait AccessPolicy: Send + Sync {
        /// Predicato per l'indice dai ruoli e dalla versione ACL del soggetto (S-03, RISK-001).
        async fn predicate_for(
            &self,
            ctx: CallContext,
            principal: &Principal,
        ) -> Result<AuthorizationPredicate, PolicyError>;

        /// Ricontrollo puntuale per citazioni, link e viewer; deny by default (CLAUDE.md «Sicurezza»).
        async fn authorize(
            &self,
            ctx: CallContext,
            principal: &Principal,
            action: Action,
            resource: &Resource,
        ) -> Result<AuthorizationDecision, PolicyError>;
    }
}

/// Audit append-only con `event_id` uguale all'idempotency key (RISK-003).
pub mod audit {
    use async_trait::async_trait;

    use crate::context::{MutationContext, MutationOutcome};
    use crate::errors::AuditError;
    use crate::ids::{ActorId, IdempotencyKey, RequestId};

    /// Tipo di evento d'audit.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum AuditEventKind {
        QueryAnswered,
        QueryRefused,
        DocumentPublished,
        AccessDenied,
    }

    /// Evento d'audit, senza testi di query né di passaggi.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct AuditEvent {
        pub event_id: IdempotencyKey,
        pub kind: AuditEventKind,
        pub actor: ActorId,
        pub request_id: RequestId,
    }

    /// Porta dell'audit.
    #[async_trait]
    pub trait AuditSink: Send + Sync {
        /// Un retry dopo `OutcomeUnknown` con la stessa chiave non registra due eventi (RISK-003).
        async fn append(
            &self,
            ctx: MutationContext,
            event: AuditEvent,
        ) -> Result<MutationOutcome, AuditError>;
    }
}

/// Cache con scope autorizzativo obbligatorio (RISK-002).
pub mod cache {
    use std::time::Duration;

    use async_trait::async_trait;
    use bytes::Bytes;

    use crate::context::{CallContext, MutationContext, MutationOutcome};
    use crate::embedding::EmbeddingSpace;
    use crate::errors::CacheError;
    use crate::ids::{AclVersion, CorpusVersion, DocumentVersionId, SubjectId};
    use crate::policy::AuthorizationScope;

    /// Versione della pipeline di query.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PipelineVersion(u32);

    impl PipelineVersion {
        /// Versione esplicita.
        pub fn new(value: u32) -> Self {
            Self(value)
        }
    }

    /// Digest della query normalizzata, calcolato dal chiamante.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct QueryDigest([u8; 32]);

    impl QueryDigest {
        /// Digest già calcolato.
        pub fn new(digest: [u8; 32]) -> Self {
            Self(digest)
        }
    }

    /// Chiave di cache: scope, versioni e spazio tutti obbligatori, nessun `Default`.
    ///
    /// Senza scope autorizzativo non si costruisce (P-11):
    ///
    /// ```compile_fail,E0061
    /// use s07_ports::cache::{CacheKey, PipelineVersion, QueryDigest};
    /// use s07_ports::embedding::EmbeddingSpace;
    /// use s07_ports::ids::CorpusVersion;
    ///
    /// fn unscoped(space: EmbeddingSpace) -> CacheKey {
    ///     CacheKey::new(CorpusVersion::new(3), space, PipelineVersion::new(1), QueryDigest::new([0; 32]))
    /// }
    /// ```
    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    pub struct CacheKey {
        scope: AuthorizationScope,
        corpus_version: CorpusVersion,
        space: EmbeddingSpace,
        pipeline_version: PipelineVersion,
        query_digest: QueryDigest,
    }

    impl CacheKey {
        /// Chiave completa (RISK-002, RISK-004).
        pub fn new(
            scope: AuthorizationScope,
            corpus_version: CorpusVersion,
            space: EmbeddingSpace,
            pipeline_version: PipelineVersion,
            query_digest: QueryDigest,
        ) -> Self {
            Self {
                scope,
                corpus_version,
                space,
                pipeline_version,
                query_digest,
            }
        }

        /// Scope autorizzativo della voce.
        pub fn scope(&self) -> &AuthorizationScope {
            &self.scope
        }

        /// Versione del corpus.
        pub fn corpus_version(&self) -> CorpusVersion {
            self.corpus_version
        }

        /// Spazio di embedding.
        pub fn space(&self) -> &EmbeddingSpace {
            &self.space
        }

        /// Versione della pipeline.
        pub fn pipeline_version(&self) -> PipelineVersion {
            self.pipeline_version
        }

        /// Digest della query.
        pub fn query_digest(&self) -> QueryDigest {
            self.query_digest
        }
    }

    /// Valore in cache, opaco.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CacheValue {
        pub bytes: Bytes,
    }

    /// Ambito di invalidazione per revoche e cambi di versione.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum InvalidationScope {
        Subject(SubjectId),
        AclVersion {
            subject_id: SubjectId,
            acl_version: AclVersion,
        },
        DocumentVersion(DocumentVersionId),
        EmbeddingSpace(EmbeddingSpace),
    }

    /// Porta della cache. Una voce letta si ricontrolla contro l'autorizzazione corrente
    /// prima del consumo; il primo adapter resta `NoOpCache` (RISK-002).
    #[async_trait]
    pub trait Cache: Send + Sync {
        /// Lettura per chiave con scope (RISK-002).
        async fn get(
            &self,
            ctx: CallContext,
            key: &CacheKey,
        ) -> Result<Option<CacheValue>, CacheError>;

        /// Sovrascrivere la stessa chiave è idempotente per costruzione: basta `CallContext` (RISK-002).
        async fn put(
            &self,
            ctx: CallContext,
            key: CacheKey,
            value: CacheValue,
            ttl: Duration,
        ) -> Result<(), CacheError>;

        /// Revoche e cambi di versione rendono inutilizzabili le voci precedenti (RISK-002, RISK-003).
        async fn invalidate(
            &self,
            ctx: MutationContext,
            scope: InvalidationScope,
        ) -> Result<MutationOutcome, CacheError>;
    }
}

/// Fonti online, prima della quarantena (S-05, ADR-0010, D-22).
pub mod source {
    use async_trait::async_trait;

    use crate::context::CallContext;
    use crate::errors::SourceError;
    use crate::store::ByteStream;

    /// Cursore opaco della discovery.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SourceCursor(pub String);

    /// Documento candidato scoperto da una fonte.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct DocumentCandidate {
        pub uri: String,
    }

    /// Pagina di discovery.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct DiscoveryPage {
        pub candidates: Vec<DocumentCandidate>,
        pub next: Option<SourceCursor>,
    }

    /// Porta delle fonti online; il contenuto passa da validazione e quarantena.
    #[async_trait]
    pub trait DocumentSource: Send + Sync {
        /// Discovery paginata con deadline (S-05, D-22).
        async fn discover(
            &self,
            ctx: CallContext,
            cursor: Option<SourceCursor>,
        ) -> Result<DiscoveryPage, SourceError>;

        /// Download in streaming (S-01, S-05).
        async fn fetch(
            &self,
            ctx: CallContext,
            candidate: &DocumentCandidate,
        ) -> Result<ByteStream<SourceError>, SourceError>;
    }
}

#[cfg(test)]
mod tests {
    //! Test di contratto del design 01-16 (TEST-1..9). Fake solo qui, mai adapter.

    use std::collections::HashMap;
    use std::ops::Range;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use async_trait::async_trait;
    use bytes::Bytes;
    use futures::executor::block_on;

    use crate::answer::{AnswerGenerator, Claim, StructuredAnswer};
    use crate::audit::{AuditEvent, AuditEventKind, AuditSink};
    use crate::cache::{
        Cache, CacheKey, CacheValue, InvalidationScope, PipelineVersion, QueryDigest,
    };
    use crate::context::{CallContext, Deadline, MutationContext, MutationOutcome, TraceContext};
    use crate::decision::{
        CitationDecision, DecisionEngine, EvidenceDecision, Metered, QueryDecision, RankScore,
        Reranker,
    };
    use crate::embedding::{Embedder, Embedding, EmbeddingSpace, Normalization, Pooling};
    use crate::errors::{
        AnswerError, AuditError, CacheError, ChunkError, DecisionError, DraftError, EmbedError,
        EmbeddingError, EvidenceError, IndexError, ParseError, PolicyError, RerankError,
        RetrieverError, SourceError, StoreError,
    };
    use crate::evidence::document_side::{AuthorizationPredicate, retrieve_authorized};
    use crate::evidence::rag_side::{AuthorizedEvidence, EvidenceRetriever, authorize_response};
    use crate::evidence::wire::{
        RetrievalSearchRequest, RetrievalSearchResponse, RetrievedPassageDto,
    };
    use crate::ids::{
        AclVersion, ActorId, ChunkText, ContentHash, CorpusVersion, DocumentId, DocumentVersionId,
        IdempotencyKey, PassageRef, QueryText, RequestId, SubjectId, UserQuery,
    };
    use crate::index::{Candidate, CandidateAcl, IndexVersionDraft, IndexedChunk, VectorIndex};
    use crate::locator::{CharRange, Locator, SectionStep, TextProjection, TextSpan};
    use crate::parser::{
        Chunker, DocumentParser, FileKind, ParseInput, ParseOutcome, ParsedDocument,
    };
    use crate::policy::{
        AccessPolicy, Action, AuthorizationDecision, AuthorizationScope, Principal, Resource, Role,
    };
    use crate::source::{DiscoveryPage, DocumentCandidate, DocumentSource, SourceCursor};
    use crate::store::{ByteStream, DocumentStore, ValidatedFile};

    // ---------------------------------------------------------------- fixture

    const SENTINEL: &str = "SENTINELLA-7f3a-testo-riservato";
    const DIMENSION: u32 = 1024;

    fn ctx() -> CallContext {
        CallContext {
            request_id: RequestId::random(),
            trace: TraceContext::parse("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
                .expect("traceparent W3C valido"),
            deadline: Deadline::after(Duration::from_secs(5)),
        }
    }

    fn mctx(key: IdempotencyKey) -> MutationContext {
        MutationContext {
            call: ctx(),
            idempotency_key: key,
            actor: ActorId::random(),
        }
    }

    fn space_of(model_id: &str, revision: &str) -> EmbeddingSpace {
        EmbeddingSpace {
            model_id: model_id.to_owned(),
            revision: revision.to_owned(),
            dimension: DIMENSION,
            normalization: Normalization::L2,
            pooling: Pooling::Cls,
            query_instruction: None,
            document_instruction: None,
        }
    }

    fn bge() -> EmbeddingSpace {
        space_of("BAAI/bge-m3", "5617a9f6")
    }

    fn embedding_in(space: EmbeddingSpace) -> Embedding {
        Embedding::new(space, vec![0.0; DIMENSION as usize]).expect("dimensione coerente")
    }

    fn text_locator() -> Locator {
        Locator::Text {
            projection: TextProjection::CkaTextV1,
            doc_sha256: ContentHash::new([7; 32]),
            spans: vec![TextSpan {
                section_path: vec![SectionStep {
                    title: "Bilancio".to_owned(),
                    ordinal: 0,
                }],
                char_range: CharRange { start: 0, end: 40 },
                section_text_sha256: ContentHash::new([9; 32]),
            }],
        }
    }

    fn acl(
        subjects: Vec<SubjectId>,
        roles: Vec<Role>,
        version: u64,
        published: bool,
    ) -> CandidateAcl {
        CandidateAcl {
            allowed_subjects: subjects,
            allowed_roles: roles,
            acl_version: AclVersion::new(version),
            published,
        }
    }

    fn candidate(passage_ref: PassageRef, acl: CandidateAcl, text: &str) -> Candidate {
        Candidate {
            passage_ref,
            document_version_id: DocumentVersionId::random(),
            acl,
            locator: text_locator(),
            distance: 0.1,
            text: text.to_owned(),
        }
    }

    fn dto(passage_ref: PassageRef, text: &str) -> RetrievedPassageDto {
        RetrievedPassageDto {
            passage_ref,
            document_version_id: DocumentVersionId::random(),
            locator: text_locator(),
            text: text.to_owned(),
        }
    }

    fn response(
        subject_id: SubjectId,
        acl_version: u64,
        space: EmbeddingSpace,
        passages: Vec<RetrievedPassageDto>,
    ) -> RetrievalSearchResponse {
        RetrievalSearchResponse {
            subject_id,
            acl_version: AclVersion::new(acl_version),
            space,
            passages,
        }
    }

    fn scope(subject_id: SubjectId, acl_version: u64) -> AuthorizationScope {
        AuthorizationScope {
            subject_id,
            acl_version: AclVersion::new(acl_version),
        }
    }

    fn query() -> UserQuery {
        QueryText::new("Qual è l'utile netto 2025?")
    }

    /// Evidenza autorizzata coerente, ottenuta solo da `authorize_response`.
    fn authorized(refs: &[PassageRef], text: &str) -> AuthorizedEvidence {
        let subject = SubjectId::random();
        let passages = refs.iter().map(|r| dto(*r, text)).collect();
        authorize_response(
            response(subject, 3, bge(), passages),
            &scope(subject, 3),
            &bge(),
        )
        .expect("risposta coerente")
    }

    /// Fake di `EvidenceRetriever` che restituisce una risposta fissata.
    struct FakeRetriever {
        response: RetrievalSearchResponse,
    }

    #[async_trait]
    impl EvidenceRetriever for FakeRetriever {
        async fn search(
            &self,
            _ctx: CallContext,
            _request: RetrievalSearchRequest,
        ) -> Result<RetrievalSearchResponse, RetrieverError> {
            Ok(self.response.clone())
        }
    }

    fn fetch_via_port(response: RetrievalSearchResponse) -> RetrievalSearchResponse {
        let port: Arc<dyn EvidenceRetriever + Send + Sync> = Arc::new(FakeRetriever { response });
        let request = RetrievalSearchRequest {
            query_text: query(),
            k: 8,
        };
        block_on(port.search(ctx(), request)).expect("fake sempre Ok")
    }

    /// Fake **positivo** di `VectorIndex`: restituisce tutti i candidati, anche i non
    /// autorizzati, conta le ricerche e applica `publish_version` una volta per chiave.
    struct FakeIndex {
        space: EmbeddingSpace,
        candidates: Mutex<Vec<Candidate>>,
        search_calls: AtomicUsize,
        published: Mutex<HashMap<IdempotencyKey, DocumentVersionId>>,
        fail_after_apply_once: AtomicBool,
    }

    impl FakeIndex {
        fn new(space: EmbeddingSpace, candidates: Vec<Candidate>) -> Self {
            Self {
                space,
                candidates: Mutex::new(candidates),
                search_calls: AtomicUsize::new(0),
                published: Mutex::new(HashMap::new()),
                fail_after_apply_once: AtomicBool::new(false),
            }
        }
    }

    #[async_trait]
    impl VectorIndex for FakeIndex {
        async fn search(
            &self,
            _ctx: CallContext,
            _query: &Embedding,
            _predicate: &AuthorizationPredicate,
            _k: usize,
        ) -> Result<Vec<Candidate>, IndexError> {
            self.search_calls.fetch_add(1, Ordering::SeqCst);
            Ok(std::mem::take(&mut *self.candidates.lock().expect("mutex")))
        }

        async fn publish_version(
            &self,
            ctx: MutationContext,
            draft: IndexVersionDraft,
        ) -> Result<MutationOutcome, IndexError> {
            let mut published = self.published.lock().expect("mutex");
            if published.contains_key(&ctx.idempotency_key) {
                return Ok(MutationOutcome::AlreadyApplied);
            }
            published.insert(ctx.idempotency_key, draft.document_version_id());
            if self.fail_after_apply_once.swap(false, Ordering::SeqCst) {
                return Err(IndexError::OutcomeUnknown);
            }
            Ok(MutationOutcome::Applied)
        }

        fn space(&self) -> &EmbeddingSpace {
            &self.space
        }
    }

    // ------------------------------------------------- TEST-1 dyn-compatibilità

    /// Implementazione minima di ogni porta: restituisce sempre un errore tipizzato.
    struct Failing {
        space: EmbeddingSpace,
    }

    fn failing() -> Failing {
        Failing { space: bge() }
    }

    #[async_trait]
    impl DocumentSource for Failing {
        async fn discover(
            &self,
            _ctx: CallContext,
            _cursor: Option<SourceCursor>,
        ) -> Result<DiscoveryPage, SourceError> {
            Err(SourceError::Unreachable)
        }

        async fn fetch(
            &self,
            _ctx: CallContext,
            _candidate: &DocumentCandidate,
        ) -> Result<ByteStream<SourceError>, SourceError> {
            Err(SourceError::Unreachable)
        }
    }

    #[async_trait]
    impl DocumentStore for Failing {
        async fn put(
            &self,
            _ctx: MutationContext,
            _hash: ContentHash,
            _source: ValidatedFile,
        ) -> Result<MutationOutcome, StoreError> {
            Err(StoreError::Unavailable)
        }

        async fn get(
            &self,
            _ctx: CallContext,
            _key: &ContentHash,
            _range: Option<Range<u64>>,
        ) -> Result<ByteStream<StoreError>, StoreError> {
            Err(StoreError::NotFound)
        }

        async fn delete(
            &self,
            _ctx: MutationContext,
            _key: &ContentHash,
        ) -> Result<MutationOutcome, StoreError> {
            Err(StoreError::Unavailable)
        }
    }

    #[async_trait]
    impl DocumentParser for Failing {
        async fn parse(
            &self,
            _ctx: CallContext,
            _input: ParseInput,
        ) -> Result<ParseOutcome, ParseError> {
            Err(ParseError::Unsupported)
        }
    }

    impl Chunker for Failing {
        fn chunk(
            &self,
            _document: &ParsedDocument,
        ) -> Result<Vec<crate::parser::ChunkDraft>, ChunkError> {
            Err(ChunkError::EmptyDocument)
        }
    }

    #[async_trait]
    impl Embedder for Failing {
        async fn embed_documents(
            &self,
            _ctx: CallContext,
            _texts: &[ChunkText],
        ) -> Result<Vec<Embedding>, EmbedError> {
            Err(EmbedError::Unavailable)
        }

        async fn embed_query(
            &self,
            _ctx: CallContext,
            _query: &QueryText,
        ) -> Result<Embedding, EmbedError> {
            Err(EmbedError::Unavailable)
        }

        fn space(&self) -> &EmbeddingSpace {
            &self.space
        }
    }

    #[async_trait]
    impl VectorIndex for Failing {
        async fn search(
            &self,
            _ctx: CallContext,
            _query: &Embedding,
            _predicate: &AuthorizationPredicate,
            _k: usize,
        ) -> Result<Vec<Candidate>, IndexError> {
            Err(IndexError::Unavailable)
        }

        async fn publish_version(
            &self,
            _ctx: MutationContext,
            _draft: IndexVersionDraft,
        ) -> Result<MutationOutcome, IndexError> {
            Err(IndexError::NotApplied {
                reason: "indice non disponibile".to_owned(),
            })
        }

        fn space(&self) -> &EmbeddingSpace {
            &self.space
        }
    }

    #[async_trait]
    impl DecisionEngine for Failing {
        async fn validate_query(
            &self,
            _ctx: CallContext,
            _query: &UserQuery,
        ) -> Result<Metered<QueryDecision>, DecisionError> {
            Err(DecisionError::Unavailable)
        }

        async fn assess_evidence(
            &self,
            _ctx: CallContext,
            _query: &UserQuery,
            _evidence: &AuthorizedEvidence,
        ) -> Result<Metered<EvidenceDecision>, DecisionError> {
            Err(DecisionError::Unavailable)
        }

        async fn validate_citations(
            &self,
            _ctx: CallContext,
            _evidence: &AuthorizedEvidence,
            _answer: &StructuredAnswer,
        ) -> Result<Metered<Vec<CitationDecision>>, DecisionError> {
            Err(DecisionError::Unavailable)
        }
    }

    #[async_trait]
    impl Reranker for Failing {
        async fn rank(
            &self,
            _ctx: CallContext,
            _query: &UserQuery,
            _evidence: &AuthorizedEvidence,
        ) -> Result<Metered<Vec<RankScore>>, RerankError> {
            Err(RerankError::Timeout)
        }
    }

    #[async_trait]
    impl AnswerGenerator for Failing {
        async fn generate(
            &self,
            _ctx: CallContext,
            _query: &UserQuery,
            _evidence: &AuthorizedEvidence,
        ) -> Result<Metered<StructuredAnswer>, AnswerError> {
            Err(AnswerError::Unavailable)
        }
    }

    #[async_trait]
    impl AccessPolicy for Failing {
        async fn predicate_for(
            &self,
            _ctx: CallContext,
            _principal: &Principal,
        ) -> Result<AuthorizationPredicate, PolicyError> {
            Err(PolicyError::Unavailable)
        }

        async fn authorize(
            &self,
            _ctx: CallContext,
            _principal: &Principal,
            _action: Action,
            _resource: &Resource,
        ) -> Result<AuthorizationDecision, PolicyError> {
            Err(PolicyError::Unavailable)
        }
    }

    #[async_trait]
    impl AuditSink for Failing {
        async fn append(
            &self,
            _ctx: MutationContext,
            _event: AuditEvent,
        ) -> Result<MutationOutcome, AuditError> {
            Err(AuditError::Timeout)
        }
    }

    #[async_trait]
    impl Cache for Failing {
        async fn get(
            &self,
            _ctx: CallContext,
            _key: &CacheKey,
        ) -> Result<Option<CacheValue>, CacheError> {
            Err(CacheError::Unavailable)
        }

        async fn put(
            &self,
            _ctx: CallContext,
            _key: CacheKey,
            _value: CacheValue,
            _ttl: Duration,
        ) -> Result<(), CacheError> {
            Err(CacheError::Unavailable)
        }

        async fn invalidate(
            &self,
            _ctx: MutationContext,
            _scope: InvalidationScope,
        ) -> Result<MutationOutcome, CacheError> {
            Err(CacheError::OutcomeUnknown)
        }
    }

    #[async_trait]
    impl EvidenceRetriever for Failing {
        async fn search(
            &self,
            _ctx: CallContext,
            _request: RetrievalSearchRequest,
        ) -> Result<RetrievalSearchResponse, RetrieverError> {
            Err(RetrieverError::Unavailable)
        }
    }

    fn principal(roles: Vec<Role>, acl_version: u64) -> Principal {
        Principal {
            subject_id: SubjectId::random(),
            roles,
            acl_version: AclVersion::new(acl_version),
        }
    }

    #[test]
    fn t1_dyn_document_source() {
        let port: Arc<dyn DocumentSource + Send + Sync> = Arc::new(failing());
        assert_eq!(
            block_on(port.discover(ctx(), None)).err(),
            Some(SourceError::Unreachable)
        );
    }

    #[test]
    fn t1_dyn_document_store() {
        let port: Arc<dyn DocumentStore + Send + Sync> = Arc::new(failing());
        let file = ValidatedFile::from_validation("upload.tmp".into(), 10);
        let outcome = block_on(port.put(
            mctx(IdempotencyKey::random()),
            ContentHash::new([1; 32]),
            file,
        ));
        assert_eq!(outcome, Err(StoreError::Unavailable));
    }

    #[test]
    fn t1_dyn_document_parser() {
        let port: Arc<dyn DocumentParser + Send + Sync> = Arc::new(failing());
        let input = ParseInput {
            key: ContentHash::new([2; 32]),
            kind: FileKind::Pdf,
        };
        assert!(matches!(
            block_on(port.parse(ctx(), input)),
            Err(ParseError::Unsupported)
        ));
    }

    #[test]
    fn t1_dyn_chunker() {
        let port: Arc<dyn Chunker + Send + Sync> = Arc::new(failing());
        assert_eq!(
            port.chunk(&ParsedDocument::default()),
            Err(ChunkError::EmptyDocument)
        );
    }

    #[test]
    fn t1_dyn_embedder() {
        let port: Arc<dyn Embedder + Send + Sync> = Arc::new(failing());
        assert_eq!(
            block_on(port.embed_query(ctx(), &query())),
            Err(EmbedError::Unavailable)
        );
        assert_eq!(port.space(), &bge());
    }

    #[test]
    fn t1_dyn_vector_index() {
        let port: Arc<dyn VectorIndex + Send + Sync> = Arc::new(failing());
        let predicate = AuthorizationPredicate {
            subject_id: SubjectId::random(),
            roles: vec![Role::Employee],
            acl_version: AclVersion::new(1),
        };
        let found = block_on(port.search(ctx(), &embedding_in(bge()), &predicate, 8));
        assert_eq!(found.err(), Some(IndexError::Unavailable));
    }

    #[test]
    fn t1_dyn_decision_engine() {
        let port: Arc<dyn DecisionEngine + Send + Sync> = Arc::new(failing());
        assert_eq!(
            block_on(port.validate_query(ctx(), &query())),
            Err(DecisionError::Unavailable)
        );
        let evidence = authorized(&[PassageRef::random()], "testo");
        let assessed = block_on(port.assess_evidence(ctx(), &query(), &evidence));
        assert_eq!(assessed, Err(DecisionError::Unavailable));
    }

    #[test]
    fn t1_dyn_reranker() {
        let port: Arc<dyn Reranker + Send + Sync> = Arc::new(failing());
        let evidence = authorized(&[PassageRef::random()], "testo");
        assert_eq!(
            block_on(port.rank(ctx(), &query(), &evidence)),
            Err(RerankError::Timeout)
        );
    }

    #[test]
    fn t1_dyn_answer_generator() {
        let port: Arc<dyn AnswerGenerator + Send + Sync> = Arc::new(failing());
        let evidence = authorized(&[PassageRef::random()], "testo");
        let generated = block_on(port.generate(ctx(), &query(), &evidence));
        assert_eq!(generated.err(), Some(AnswerError::Unavailable));
    }

    #[test]
    fn t1_dyn_access_policy() {
        let port: Arc<dyn AccessPolicy + Send + Sync> = Arc::new(failing());
        let who = principal(vec![Role::Guest], 1);
        assert_eq!(
            block_on(port.predicate_for(ctx(), &who)),
            Err(PolicyError::Unavailable)
        );
        let resource = Resource {
            document_id: DocumentId::random(),
            document_version_id: None,
        };
        let decision = block_on(port.authorize(ctx(), &who, Action::Read, &resource));
        assert_eq!(decision, Err(PolicyError::Unavailable));
    }

    #[test]
    fn t1_dyn_audit_sink() {
        let port: Arc<dyn AuditSink + Send + Sync> = Arc::new(failing());
        let key = IdempotencyKey::random();
        let event = audit_event(key);
        assert_eq!(
            block_on(port.append(mctx(key), event)),
            Err(AuditError::Timeout)
        );
    }

    #[test]
    fn t1_dyn_cache() {
        let port: Arc<dyn Cache + Send + Sync> = Arc::new(failing());
        let key = cache_key(scope(SubjectId::random(), 1), 1, bge());
        assert_eq!(
            block_on(port.get(ctx(), &key)),
            Err(CacheError::Unavailable)
        );
    }

    #[test]
    fn t1_dyn_evidence_retriever() {
        let port: Arc<dyn EvidenceRetriever + Send + Sync> = Arc::new(failing());
        let request = RetrievalSearchRequest {
            query_text: query(),
            k: 8,
        };
        assert_eq!(
            block_on(port.search(ctx(), request)),
            Err(RetrieverError::Unavailable)
        );
    }

    // ------------------------------- TEST-2 retrieve_authorized, lato documento

    #[test]
    fn t2_retrieve_authorized_drops_other_subject_stale_acl_and_unpublished() {
        let me = SubjectId::random();
        let other = SubjectId::random();
        let by_subject = PassageRef::random();
        let by_role = PassageRef::random();
        let candidates = vec![
            candidate(by_subject, acl(vec![me], vec![], 7, true), "ok soggetto"),
            candidate(
                PassageRef::random(),
                acl(vec![other], vec![Role::Admin], 7, true),
                "altro soggetto",
            ),
            candidate(
                by_role,
                acl(vec![], vec![Role::Employee], 7, true),
                "ok ruolo",
            ),
            candidate(
                PassageRef::random(),
                acl(vec![me], vec![], 6, true),
                "ACL vecchia",
            ),
            candidate(
                PassageRef::random(),
                acl(vec![me], vec![], 7, false),
                "non pubblicata",
            ),
        ];
        let index = FakeIndex::new(bge(), candidates);
        let predicate = AuthorizationPredicate {
            subject_id: me,
            roles: vec![Role::Employee],
            acl_version: AclVersion::new(7),
        };

        let evidence = block_on(retrieve_authorized(
            &index,
            ctx(),
            embedding_in(bge()),
            &predicate,
            8,
        ))
        .expect("spazio coerente");

        assert_eq!(index.search_calls.load(Ordering::SeqCst), 1);
        let kept: Vec<PassageRef> = evidence
            .passages()
            .iter()
            .map(|p| p.passage_ref())
            .collect();
        assert_eq!(kept, vec![by_subject, by_role]);

        let wire = evidence.into_response();
        assert_eq!(wire.subject_id, me);
        assert_eq!(wire.acl_version, AclVersion::new(7));
        assert_eq!(wire.space, bge());
        let sent: Vec<PassageRef> = wire.passages.iter().map(|p| p.passage_ref).collect();
        assert_eq!(sent, vec![by_subject, by_role]);
    }

    #[test]
    fn t2_retrieve_authorized_keeps_order_and_text_of_authorized_candidates() {
        let me = SubjectId::random();
        let first = PassageRef::random();
        let second = PassageRef::random();
        let index = FakeIndex::new(
            bge(),
            vec![
                candidate(first, acl(vec![me], vec![], 2, true), "primo"),
                candidate(second, acl(vec![me], vec![], 2, true), "secondo"),
            ],
        );
        let predicate = AuthorizationPredicate {
            subject_id: me,
            roles: vec![],
            acl_version: AclVersion::new(2),
        };

        let wire = block_on(retrieve_authorized(
            &index,
            ctx(),
            embedding_in(bge()),
            &predicate,
            8,
        ))
        .expect("spazio coerente")
        .into_response();

        let texts: Vec<&str> = wire.passages.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(texts, vec!["primo", "secondo"]);
    }

    // ------------------------------------ TEST-3 authorize_response, lato RAG

    #[test]
    fn t3_authorize_response_rejects_other_subject() {
        let expected = SubjectId::random();
        let wire = fetch_via_port(response(
            SubjectId::random(),
            3,
            bge(),
            vec![dto(PassageRef::random(), "x")],
        ));
        let result = authorize_response(wire, &scope(expected, 3), &bge());
        assert_eq!(result.err(), Some(EvidenceError::SubjectMismatch));
    }

    #[test]
    fn t3_authorize_response_rejects_other_acl_version() {
        let subject = SubjectId::random();
        let wire = fetch_via_port(response(
            subject,
            2,
            bge(),
            vec![dto(PassageRef::random(), "x")],
        ));
        let result = authorize_response(wire, &scope(subject, 3), &bge());
        assert_eq!(result.err(), Some(EvidenceError::AclVersionMismatch));
    }

    #[test]
    fn t3_authorize_response_rejects_other_space() {
        let subject = SubjectId::random();
        let other_space = space_of("Qwen/Qwen3-Embedding-0.6B", "c54f2e6e");
        let wire = fetch_via_port(response(
            subject,
            3,
            other_space,
            vec![dto(PassageRef::random(), "x")],
        ));
        let result = authorize_response(wire, &scope(subject, 3), &bge());
        assert_eq!(result.err(), Some(EvidenceError::SpaceMismatch));
    }

    #[test]
    fn t3_authorize_response_rejects_duplicate_passage_ref() {
        let subject = SubjectId::random();
        let twice = PassageRef::random();
        let wire = fetch_via_port(response(
            subject,
            3,
            bge(),
            vec![dto(twice, "a"), dto(twice, "b")],
        ));
        let result = authorize_response(wire, &scope(subject, 3), &bge());
        assert_eq!(result.err(), Some(EvidenceError::DuplicatePassageRef));
    }

    #[test]
    fn t3_authorize_response_accepts_consistent_response() {
        let subject = SubjectId::random();
        let a = PassageRef::random();
        let b = PassageRef::random();
        let wire = fetch_via_port(response(
            subject,
            3,
            bge(),
            vec![dto(a, "alfa"), dto(b, "beta")],
        ));

        let evidence =
            authorize_response(wire, &scope(subject, 3), &bge()).expect("risposta coerente");

        assert_eq!(evidence.subject_id(), subject);
        assert_eq!(evidence.acl_version(), AclVersion::new(3));
        assert_eq!(evidence.space(), &bge());
        let refs: Vec<PassageRef> = evidence
            .passages()
            .iter()
            .map(|p| p.passage_ref())
            .collect();
        assert_eq!(refs, vec![a, b]);
        let texts: Vec<&str> = evidence.passages().iter().map(|p| p.text()).collect();
        assert_eq!(texts, vec!["alfa", "beta"]);
    }

    #[test]
    fn t3_json_round_trip_document_side_to_rag_side() {
        let me = SubjectId::random();
        let kept = PassageRef::random();
        let index = FakeIndex::new(
            bge(),
            vec![
                candidate(kept, acl(vec![me], vec![], 5, true), "autorizzato"),
                candidate(
                    PassageRef::random(),
                    acl(vec![SubjectId::random()], vec![], 5, true),
                    "estraneo",
                ),
            ],
        );
        let predicate = AuthorizationPredicate {
            subject_id: me,
            roles: vec![Role::Employee],
            acl_version: AclVersion::new(5),
        };
        let json = serde_json::to_string(
            &block_on(retrieve_authorized(
                &index,
                ctx(),
                embedding_in(bge()),
                &predicate,
                8,
            ))
            .expect("spazio coerente")
            .into_response(),
        )
        .expect("DTO serializzabile");

        let received: RetrievalSearchResponse =
            serde_json::from_str(&json).expect("DTO deserializzabile");
        let evidence = authorize_response(fetch_via_port(received), &scope(me, 5), &bge())
            .expect("giro coerente accettato");
        let refs: Vec<PassageRef> = evidence
            .passages()
            .iter()
            .map(|p| p.passage_ref())
            .collect();
        assert_eq!(refs, vec![kept]);

        let tampered = |field: &str, value: serde_json::Value| -> RetrievalSearchResponse {
            let mut doc: serde_json::Value = serde_json::from_str(&json).expect("JSON valido");
            match field {
                "revision" => doc["space"]["revision"] = value,
                _ => doc[field] = value,
            }
            serde_json::from_value(doc).expect("DTO ancora ben formato")
        };
        let other_subject = serde_json::Value::String(SubjectId::random().as_uuid().to_string());
        let cases = [
            (
                tampered("subject_id", other_subject),
                EvidenceError::SubjectMismatch,
            ),
            (
                tampered("acl_version", serde_json::json!(4)),
                EvidenceError::AclVersionMismatch,
            ),
            (
                tampered("revision", serde_json::json!("deadbeef")),
                EvidenceError::SpaceMismatch,
            ),
        ];
        for (wire, expected) in cases {
            let result = authorize_response(fetch_via_port(wire), &scope(me, 5), &bge());
            assert_eq!(result.err(), Some(expected));
        }
    }

    // TEST-4: i doctest `compile_fail` su `RetrievedEvidence`, `AuthorizedEvidence`
    // (letterale, `Serialize`, `Deserialize`) e `CacheKey` coprono il confine dei tipi.

    // ---------------------------------------------- TEST-5 redazione del Debug

    #[test]
    fn t5_debug_redacts_sentinel_text() {
        let passage_ref = PassageRef::random();
        let evidence = authorized(&[passage_ref], SENTINEL);
        let claim = Claim {
            text: SENTINEL.to_owned(),
            citations: vec![passage_ref],
        };
        let answer = StructuredAnswer {
            answer: SENTINEL.to_owned(),
            claims: vec![claim.clone()],
            refusal: Some(SENTINEL.to_owned()),
        };
        let found = candidate(
            passage_ref,
            acl(vec![], vec![Role::Guest], 1, true),
            SENTINEL,
        );
        let wire = response(
            SubjectId::random(),
            1,
            bge(),
            vec![dto(passage_ref, SENTINEL)],
        );

        let rendered = [
            ("AuthorizedEvidence", format!("{evidence:?}")),
            ("AuthorizedPassage", format!("{:?}", evidence.passages()[0])),
            ("Candidate", format!("{found:?}")),
            ("Claim", format!("{claim:?}")),
            ("StructuredAnswer", format!("{answer:?}")),
            ("RetrievalSearchResponse", format!("{wire:?}")),
            ("RetrievedPassageDto", format!("{:?}", wire.passages[0])),
        ];
        for (name, text) in &rendered {
            assert!(!text.contains(SENTINEL), "{name} espone il testo: {text}");
        }
        assert!(
            rendered[1].1.contains(&passage_ref.as_uuid().to_string()),
            "AuthorizedPassage deve stampare l'identificativo"
        );
    }

    #[test]
    fn t5_debug_redacts_sentinel_on_document_side() {
        let me = SubjectId::random();
        let index = FakeIndex::new(
            bge(),
            vec![candidate(
                PassageRef::random(),
                acl(vec![me], vec![], 1, true),
                SENTINEL,
            )],
        );
        let predicate = AuthorizationPredicate {
            subject_id: me,
            roles: vec![],
            acl_version: AclVersion::new(1),
        };
        let evidence = block_on(retrieve_authorized(
            &index,
            ctx(),
            embedding_in(bge()),
            &predicate,
            8,
        ))
        .expect("spazio coerente");

        assert!(!format!("{evidence:?}").contains(SENTINEL));
        assert!(!format!("{:?}", evidence.passages()[0]).contains(SENTINEL));
    }

    #[test]
    fn t5_reorder_rejects_unknown_passage_ref() {
        let known = PassageRef::random();
        let evidence = authorized(&[known], "testo");
        let scores = [
            RankScore {
                passage_ref: known,
                score: 0.9,
            },
            RankScore {
                passage_ref: PassageRef::random(),
                score: 0.5,
            },
        ];
        assert_eq!(
            evidence.reorder(&scores).err(),
            Some(EvidenceError::UnknownPassageRef)
        );
    }

    #[test]
    fn t5_reorder_only_permutes_or_drops() {
        let a = PassageRef::random();
        let b = PassageRef::random();
        let c = PassageRef::random();
        let evidence = authorized(&[a, b, c], "testo");
        let scores = [
            RankScore {
                passage_ref: a,
                score: 0.1,
            },
            RankScore {
                passage_ref: c,
                score: 0.8,
            },
        ];
        let reordered = evidence.reorder(&scores).expect("riferimenti noti");
        let refs: Vec<PassageRef> = reordered
            .passages()
            .iter()
            .map(|p| p.passage_ref())
            .collect();
        assert_eq!(refs, vec![c, a]);
    }

    #[test]
    fn t5_resolve_maps_only_known_refs() {
        let known = PassageRef::random();
        let evidence = authorized(&[known], "testo citato");
        assert_eq!(
            evidence.resolve(&known).map(|p| p.text()),
            Some("testo citato")
        );
        assert!(evidence.resolve(&PassageRef::random()).is_none());
    }

    // ----------------------------------------- TEST-6 chiave e revoca della cache

    fn cache_key(scope: AuthorizationScope, corpus: u64, space: EmbeddingSpace) -> CacheKey {
        CacheKey::new(
            scope,
            CorpusVersion::new(corpus),
            space,
            PipelineVersion::new(1),
            QueryDigest::new([42; 32]),
        )
    }

    #[test]
    fn t6_cache_key_differs_for_manager_and_guest() {
        let manager = principal(vec![Role::Manager], 4);
        let guest = principal(vec![Role::Guest], 4);
        let manager_key = cache_key(scope(manager.subject_id, 4), 1, bge());
        let guest_key = cache_key(scope(guest.subject_id, 4), 1, bge());
        assert_ne!(manager_key, guest_key);
    }

    #[test]
    fn t6_cache_key_differs_when_acl_version_increments_after_role_change() {
        let subject = SubjectId::random();
        let before = cache_key(scope(subject, 4), 1, bge());
        let after_role_change = cache_key(scope(subject, 5), 1, bge());
        assert_ne!(before, after_role_change);
        assert_eq!(before.scope().acl_version, AclVersion::new(4));
    }

    #[test]
    fn t6_cache_key_differs_by_corpus_version_and_space() {
        let subject = SubjectId::random();
        let base = cache_key(scope(subject, 1), 1, bge());
        assert_ne!(base, cache_key(scope(subject, 1), 2, bge()));
        assert_ne!(
            base,
            cache_key(scope(subject, 1), 1, space_of("BAAI/bge-m3", "altra-rev"))
        );
        assert_eq!(base, cache_key(scope(subject, 1), 1, bge()));
    }

    /// Cache fake in memoria: l'invalidazione toglie le voci dello scope revocato.
    #[derive(Default)]
    struct FakeCache {
        entries: Mutex<HashMap<CacheKey, CacheValue>>,
    }

    #[async_trait]
    impl Cache for FakeCache {
        async fn get(
            &self,
            _ctx: CallContext,
            key: &CacheKey,
        ) -> Result<Option<CacheValue>, CacheError> {
            Ok(self.entries.lock().expect("mutex").get(key).cloned())
        }

        async fn put(
            &self,
            _ctx: CallContext,
            key: CacheKey,
            value: CacheValue,
            _ttl: Duration,
        ) -> Result<(), CacheError> {
            self.entries.lock().expect("mutex").insert(key, value);
            Ok(())
        }

        async fn invalidate(
            &self,
            _ctx: MutationContext,
            scope: InvalidationScope,
        ) -> Result<MutationOutcome, CacheError> {
            self.entries
                .lock()
                .expect("mutex")
                .retain(|key, _| match &scope {
                    InvalidationScope::Subject(subject) => key.scope().subject_id != *subject,
                    InvalidationScope::AclVersion {
                        subject_id,
                        acl_version,
                    } => {
                        !(key.scope().subject_id == *subject_id
                            && key.scope().acl_version == *acl_version)
                    }
                    InvalidationScope::DocumentVersion(_) => false,
                    InvalidationScope::EmbeddingSpace(space) => key.space() != space,
                });
            Ok(MutationOutcome::Applied)
        }
    }

    #[test]
    fn t6_cache_fake_does_not_serve_entry_after_revocation() {
        let cache: Arc<dyn Cache + Send + Sync> = Arc::new(FakeCache::default());
        let subject = SubjectId::random();
        let key = cache_key(scope(subject, 4), 1, bge());
        let value = CacheValue {
            bytes: Bytes::from_static(b"risposta"),
        };
        block_on(cache.put(ctx(), key.clone(), value.clone(), Duration::from_secs(60)))
            .expect("put");
        assert_eq!(block_on(cache.get(ctx(), &key)), Ok(Some(value)));

        let after_role_change = cache_key(scope(subject, 5), 1, bge());
        assert_eq!(block_on(cache.get(ctx(), &after_role_change)), Ok(None));

        let revoke = InvalidationScope::AclVersion {
            subject_id: subject,
            acl_version: AclVersion::new(4),
        };
        block_on(cache.invalidate(mctx(IdempotencyKey::random()), revoke)).expect("invalidate");
        assert_eq!(block_on(cache.get(ctx(), &key)), Ok(None));
    }

    // ------------------------------------- TEST-7 retry dopo OutcomeUnknown

    fn audit_event(key: IdempotencyKey) -> AuditEvent {
        AuditEvent {
            event_id: key,
            kind: AuditEventKind::QueryAnswered,
            actor: ActorId::random(),
            request_id: RequestId::random(),
        }
    }

    /// Audit fake: applica, poi (una volta) risponde `OutcomeUnknown`; deduplica per chiave.
    #[derive(Default)]
    struct FakeAuditSink {
        events: Mutex<HashMap<IdempotencyKey, AuditEvent>>,
        fail_after_apply_once: AtomicBool,
    }

    #[async_trait]
    impl AuditSink for FakeAuditSink {
        async fn append(
            &self,
            ctx: MutationContext,
            event: AuditEvent,
        ) -> Result<MutationOutcome, AuditError> {
            if event.event_id != ctx.idempotency_key {
                return Err(AuditError::EventIdMismatch);
            }
            let mut events = self.events.lock().expect("mutex");
            if events.contains_key(&ctx.idempotency_key) {
                return Ok(MutationOutcome::AlreadyApplied);
            }
            events.insert(ctx.idempotency_key, event);
            if self.fail_after_apply_once.swap(false, Ordering::SeqCst) {
                return Err(AuditError::OutcomeUnknown);
            }
            Ok(MutationOutcome::Applied)
        }
    }

    #[test]
    fn t7_audit_retry_after_outcome_unknown_applies_once() {
        let fake = Arc::new(FakeAuditSink::default());
        fake.fail_after_apply_once.store(true, Ordering::SeqCst);
        let sink: Arc<dyn AuditSink + Send + Sync> = fake.clone();
        let key = IdempotencyKey::random();

        assert_eq!(
            block_on(sink.append(mctx(key), audit_event(key))),
            Err(AuditError::OutcomeUnknown)
        );
        assert_eq!(
            block_on(sink.append(mctx(key), audit_event(key))),
            Ok(MutationOutcome::AlreadyApplied)
        );
        assert_eq!(fake.events.lock().expect("mutex").len(), 1);

        let other = IdempotencyKey::random();
        assert_eq!(
            block_on(sink.append(mctx(other), audit_event(other))),
            Ok(MutationOutcome::Applied)
        );
        assert_eq!(fake.events.lock().expect("mutex").len(), 2);
    }

    #[test]
    fn t7_audit_rejects_event_id_different_from_idempotency_key() {
        let sink: Arc<dyn AuditSink + Send + Sync> = Arc::new(FakeAuditSink::default());
        let outcome = block_on(sink.append(
            mctx(IdempotencyKey::random()),
            audit_event(IdempotencyKey::random()),
        ));
        assert_eq!(outcome, Err(AuditError::EventIdMismatch));
    }

    fn draft() -> IndexVersionDraft {
        let chunk = IndexedChunk {
            passage_ref: PassageRef::random(),
            embedding: embedding_in(bge()),
            locator: text_locator(),
            text: ChunkText::new("chunk"),
            acl: acl(vec![], vec![Role::Employee], 1, true),
        };
        IndexVersionDraft::new(bge(), DocumentVersionId::random(), vec![chunk])
            .expect("stesso spazio")
    }

    #[test]
    fn t7_publish_version_retry_after_outcome_unknown_publishes_once() {
        let fake = Arc::new(FakeIndex::new(bge(), Vec::new()));
        fake.fail_after_apply_once.store(true, Ordering::SeqCst);
        let index: Arc<dyn VectorIndex + Send + Sync> = fake.clone();
        let key = IdempotencyKey::random();

        assert_eq!(
            block_on(index.publish_version(mctx(key), draft())),
            Err(IndexError::OutcomeUnknown)
        );
        assert_eq!(
            block_on(index.publish_version(mctx(key), draft())),
            Ok(MutationOutcome::AlreadyApplied)
        );
        assert_eq!(fake.published.lock().expect("mutex").len(), 1);

        let other = IdempotencyKey::random();
        assert_eq!(
            block_on(index.publish_version(mctx(other), draft())),
            Ok(MutationOutcome::Applied)
        );
        assert_eq!(fake.published.lock().expect("mutex").len(), 2);
    }

    // ------------------------------------ TEST-8 spazio di embedding coerente

    #[test]
    fn t8_retrieve_authorized_rejects_other_model_same_dimension_before_search() {
        let index = FakeIndex::new(
            bge(),
            vec![candidate(
                PassageRef::random(),
                acl(vec![], vec![Role::Guest], 1, true),
                "x",
            )],
        );
        let predicate = AuthorizationPredicate {
            subject_id: SubjectId::random(),
            roles: vec![Role::Guest],
            acl_version: AclVersion::new(1),
        };
        let other = embedding_in(space_of("Qwen/Qwen3-Embedding-0.6B", "c54f2e6e"));
        assert_eq!(other.values().len(), DIMENSION as usize);

        let result = block_on(retrieve_authorized(&index, ctx(), other, &predicate, 8));

        assert_eq!(result.err(), Some(EvidenceError::SpaceMismatch));
        assert_eq!(index.search_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn t8_retrieve_authorized_rejects_other_revision_before_search() {
        let index = FakeIndex::new(bge(), Vec::new());
        let predicate = AuthorizationPredicate {
            subject_id: SubjectId::random(),
            roles: vec![Role::Guest],
            acl_version: AclVersion::new(1),
        };
        let other_revision = embedding_in(space_of("BAAI/bge-m3", "altra-rev"));

        let result = block_on(retrieve_authorized(
            &index,
            ctx(),
            other_revision,
            &predicate,
            8,
        ));

        assert_eq!(result.err(), Some(EvidenceError::SpaceMismatch));
        assert_eq!(index.search_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn t8_retrieve_authorized_accepts_same_space() {
        let index = FakeIndex::new(bge(), Vec::new());
        let predicate = AuthorizationPredicate {
            subject_id: SubjectId::random(),
            roles: vec![Role::Guest],
            acl_version: AclVersion::new(1),
        };
        let result = block_on(retrieve_authorized(
            &index,
            ctx(),
            embedding_in(bge()),
            &predicate,
            8,
        ));
        assert!(result.is_ok());
        assert_eq!(index.search_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn t8_embedding_new_rejects_wrong_dimension() {
        let result = Embedding::new(bge(), vec![0.0; 768]);
        assert_eq!(
            result.err(),
            Some(EmbeddingError::DimensionMismatch {
                expected: DIMENSION,
                actual: 768
            })
        );
        assert!(Embedding::new(bge(), vec![0.0; DIMENSION as usize]).is_ok());
    }

    #[test]
    fn t8_index_version_draft_rejects_chunk_of_other_space() {
        let foreign = IndexedChunk {
            passage_ref: PassageRef::random(),
            embedding: embedding_in(space_of("Qwen/Qwen3-Embedding-0.6B", "c54f2e6e")),
            locator: text_locator(),
            text: ChunkText::new("chunk"),
            acl: acl(vec![], vec![Role::Employee], 1, true),
        };
        let result = IndexVersionDraft::new(bge(), DocumentVersionId::random(), vec![foreign]);
        assert_eq!(
            result.err(),
            Some(DraftError::SpaceMismatch { chunk_index: 0 })
        );
    }

    // ------------------- test dell'esecutore: comportamenti oltre lo scheletro

    #[test]
    fn x_trace_context_rejects_malformed_traceparent() {
        let valid = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
        assert_eq!(
            TraceContext::parse(valid).map(|t| t.as_str().to_owned()),
            Ok(valid.to_owned())
        );
        let malformed = [
            "",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7",
            "00-4BF92F3577B34DA6A3CE929D0E0E4736-00f067aa0ba902b7-01",
            "00-00000000000000000000000000000000-00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-0000000000000000-01",
            "ff-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
            "00-4bf92f3577b34da6a3ce929d0e0e473g-00f067aa0ba902b7-01",
            "00_4bf92f3577b34da6a3ce929d0e0e4736_00f067aa0ba902b7_01",
        ];
        for value in malformed {
            assert_eq!(
                TraceContext::parse(value),
                Err(crate::errors::TraceContextError::Malformed),
                "accettato: {value:?}"
            );
        }
    }

    #[test]
    fn x_reorder_rejects_duplicate_and_non_finite_scores() {
        let a = PassageRef::random();
        let duplicate = [
            RankScore {
                passage_ref: a,
                score: 0.4,
            },
            RankScore {
                passage_ref: a,
                score: 0.6,
            },
        ];
        assert_eq!(
            authorized(&[a], "t").reorder(&duplicate).err(),
            Some(EvidenceError::DuplicatePassageRef)
        );
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let scores = [RankScore {
                passage_ref: a,
                score: bad,
            }];
            assert_eq!(
                authorized(&[a], "t").reorder(&scores).err(),
                Some(EvidenceError::InvalidScore)
            );
        }
    }

    #[test]
    fn x_reorder_is_stable_on_equal_scores() {
        let a = PassageRef::random();
        let b = PassageRef::random();
        let scores = [
            RankScore {
                passage_ref: b,
                score: 0.5,
            },
            RankScore {
                passage_ref: a,
                score: 0.5,
            },
        ];
        let refs: Vec<PassageRef> = authorized(&[a, b], "t")
            .reorder(&scores)
            .expect("riferimenti noti")
            .passages()
            .iter()
            .map(|p| p.passage_ref())
            .collect();
        assert_eq!(refs, vec![a, b]);
    }

    #[test]
    fn x_retrieve_authorized_drops_duplicate_candidates_and_caps_at_k() {
        let me = SubjectId::random();
        let twice = PassageRef::random();
        let third = PassageRef::random();
        let index = FakeIndex::new(
            bge(),
            vec![
                candidate(twice, acl(vec![me], vec![], 1, true), "primo"),
                candidate(twice, acl(vec![me], vec![], 1, true), "doppione"),
                candidate(third, acl(vec![me], vec![], 1, true), "terzo"),
                candidate(
                    PassageRef::random(),
                    acl(vec![me], vec![], 1, true),
                    "oltre k",
                ),
            ],
        );
        let predicate = AuthorizationPredicate {
            subject_id: me,
            roles: vec![],
            acl_version: AclVersion::new(1),
        };
        let wire = block_on(retrieve_authorized(
            &index,
            ctx(),
            embedding_in(bge()),
            &predicate,
            2,
        ))
        .expect("spazio coerente")
        .into_response();
        let refs: Vec<PassageRef> = wire.passages.iter().map(|p| p.passage_ref).collect();
        assert_eq!(refs, vec![twice, third]);
        assert!(authorize_response(wire, &scope(me, 1), &bge()).is_ok());
    }

    #[test]
    fn x_retrieve_authorized_propagates_index_error() {
        let failing_index: Arc<dyn VectorIndex + Send + Sync> = Arc::new(failing());
        let predicate = AuthorizationPredicate {
            subject_id: SubjectId::random(),
            roles: vec![Role::Guest],
            acl_version: AclVersion::new(1),
        };
        let result = block_on(retrieve_authorized(
            failing_index.as_ref(),
            ctx(),
            embedding_in(bge()),
            &predicate,
            8,
        ));
        assert_eq!(
            result.err(),
            Some(EvidenceError::Index(IndexError::Unavailable))
        );
    }

    // ------------------------------- TEST-9 nessun crate provider nelle firme

    const ALLOWED_DEPENDENCIES: [&str; 5] = ["async-trait", "futures", "bytes", "uuid", "serde"];
    const ALLOWED_DEV_DEPENDENCIES: [&str; 1] = ["serde_json"];

    fn section_names(manifest: &str, header: &str) -> Vec<String> {
        manifest
            .lines()
            .skip_while(|line| line.trim() != header)
            .skip(1)
            .take_while(|line| !line.trim_start().starts_with('['))
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    return None;
                }
                line.split(['=', '.'])
                    .next()
                    .map(|name| name.trim().to_owned())
            })
            .collect()
    }

    #[test]
    fn t9_manifest_dependencies_are_in_closed_list() {
        let manifest = include_str!("../Cargo.toml");
        for name in section_names(manifest, "[dependencies]") {
            assert!(
                ALLOWED_DEPENDENCIES.contains(&name.as_str()),
                "dipendenza fuori elenco: {name}"
            );
        }
        for name in section_names(manifest, "[dev-dependencies]") {
            assert!(
                ALLOWED_DEV_DEPENDENCIES.contains(&name.as_str()),
                "dev-dipendenza fuori elenco: {name}"
            );
        }
    }

    /// I nomi proibiti non compaiono qui (li controlla il grep del `<verify>`): il test
    /// verifica in positivo che ogni `use` del codice parta da un crate ammesso.
    #[test]
    fn t9_use_roots_are_std_crate_or_allowed_dependencies() {
        const ROOTS: [&str; 10] = [
            "std",
            "core",
            "crate",
            "super",
            "self",
            "async_trait",
            "futures",
            "bytes",
            "uuid",
            "serde",
        ];
        const TEST_ROOTS: [&str; 1] = ["serde_json"];
        let source = include_str!("lib.rs");
        for line in source.lines() {
            let line = line.trim_start();
            let Some(path) = line
                .strip_prefix("use ")
                .or_else(|| line.strip_prefix("pub use "))
            else {
                continue;
            };
            let root = path
                .split("::")
                .next()
                .unwrap_or_default()
                .trim_end_matches(';');
            assert!(
                ROOTS.contains(&root) || TEST_ROOTS.contains(&root),
                "use da crate non ammesso: {line}"
            );
        }
    }
}
