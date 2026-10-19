//! Spike S-01 + A-02: upload multipart in streaming, admission control, validazione
//! prima dello storage, tre adapter di storage, GC degli orfani, generatore di PDF.
//!
//! Scheletro scritto dall'agente test-write del piano 01-04: solo le firme pubbliche
//! usate dai test, corpo `todo!()`. L'implementazione è dell'esecutore.

pub mod admission;
pub mod gc;
pub mod pdfgen;
pub mod sink;
pub mod upload;
pub mod validate;
