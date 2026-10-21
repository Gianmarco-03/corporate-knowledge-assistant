//! Job JSON tipizzato e allowlist operazione → bin (contratto W17, punto 1).
// stub del test-write: da implementare in 01-07
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

/// Codice di `job_error` del contratto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobErrorCode {
    UnknownOp,
    InvalidParams,
    PathNotAllowed,
    ChildMissing,
    BudgetExceeded,
}

/// Job non valido: nessun processo lanciato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobError {
    pub code: JobErrorCode,
    pub detail: String,
}

impl fmt::Display for JobError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // stub del test-write: da implementare in 01-07
        todo!()
    }
}

impl std::error::Error for JobError {}

/// Job decodificato (forma serde a scelta dell'esecutore).
#[derive(Debug, Clone)]
pub struct Job {
    // stub del test-write: da implementare in 01-07
    _stub: (),
}

/// Decodifica un job JSON (al più 64 KiB, `deny_unknown_fields`).
pub fn parse(_bytes: &[u8]) -> Result<Job, JobError> {
    // stub del test-write: da implementare in 01-07
    todo!()
}

/// Figlio risolto dall'allowlist: bin fisso, argv ed env costruiti dal padre.
#[derive(Debug, Clone)]
pub struct ChildSpec {
    pub job_id: String,
    pub bin: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(OsString, OsString)>,
    /// Discendenti ammessi dall'op, oltre al figlio diretto.
    pub descendants: u32,
}

/// Applica l'allowlist e valida parametri e percorsi.
pub fn resolve(_job: &Job) -> Result<ChildSpec, JobError> {
    // stub del test-write: da implementare in 01-07
    todo!()
}
