//! s03-pgvector: libreria di misura dello spike S-03/A-03 (piano 01-10), riusata dai piani 01-14 e 01-15.
//!
//! - [`chunk`]: chunker di misura per pagina (PDF) e per sezione (Markdown);
//! - [`tei`]: client TEI con embedding dei documenti e della query separati;
//! - [`db`]: schema, inserimento e ricerca esatta con il predicato ACL nella stessa query;
//! - [`metrics`]: regola di rilevanza, recall/MRR/nDCG, matrice di visibilità attesa e leak.
//!
//! STUB test-write 01-10: firme minime per far compilare i test rossi; il corpo lo scrive
//! l'esecutore del piano.
pub mod chunk;
pub mod db;
pub mod metrics;
pub mod tei;

/// I tre modelli candidati: enum chiuso, da cui derivano tabella e istruzione di query.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Model {
    Qwen3,
    BgeM3,
    E5,
}
