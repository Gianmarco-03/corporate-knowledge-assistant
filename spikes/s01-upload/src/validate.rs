//! Validazione prima di storage e DB: magic bytes, limiti DOCX, prefiltro byte-level PDF.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

/// Esito di un file ammesso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Accepted,
    Quarantined,
}

/// File rifiutato: stato HTTP e codice d'errore (es. `validation_timeout`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection {
    pub http_status: u16,
    pub code: String,
}

/// Controlla il file in `path` entro `deadline`, interrompibile con `cancel`.
pub fn check(_path: &Path, _deadline: Instant, _cancel: &AtomicBool) -> Result<Verdict, Rejection> {
    todo!()
}
