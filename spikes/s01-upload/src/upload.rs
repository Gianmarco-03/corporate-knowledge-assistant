//! Handler `POST /upload` in streaming (RESEARCH §3), `GET /healthz`, `GET /bench/stats`.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::admission::AdmissionConfig;
use crate::sink::BlobSink;

/// Dimensione massima del contenuto del campo `file` (25 MiB, D-04).
pub const MAX_FILE: u64 = 26_214_400;

/// Risposta di un upload riuscito: stessa forma con o senza deduplica (D-02, T-11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResponse {
    pub upload_id: Uuid,
    pub sha256: String,
    pub size_bytes: u64,
    /// `accepted` | `quarantined`.
    pub status: String,
}

/// Configurazione del server di upload.
#[derive(Debug, Clone)]
pub struct UploadConfig {
    /// Cartella dei tempfile (`/work/tmp` nel runner, D-03).
    pub tmp_dir: PathBuf,
    /// `false` = `--validate off` (solo con `NullSink`, solo puro streaming).
    pub validate: bool,
    /// `true` = `--bench` (cronometri per fase).
    pub bench: bool,
    pub admission: AdmissionConfig,
}

/// Router con `POST /upload`, `GET /healthz`, `GET /bench/stats` e i limiti di corpo.
/// `db = None`: nessun metadato né GC (puro streaming e test in-process).
pub fn router(
    _cfg: UploadConfig,
    _sink: Arc<dyn BlobSink>,
    _db: Option<sqlx::PgPool>,
) -> anyhow::Result<axum::Router> {
    todo!()
}
