//! GC degli orfani coordinato con l'adozione concorrente (RISK-004): protocollo a stati
//! per hash sulla tabella `blob` (`live` | `deleting`, contatore `pending`).

use std::time::Duration;

use futures::future::BoxFuture;

use crate::sink::BlobSink;

/// Età minima di un orfano prima della raccolta (N = 24 h, D-02).
pub const DEFAULT_MIN_AGE: Duration = Duration::from_secs(24 * 3600);

/// Punti di barriera iniettabili, solo per i test: dopo il passo (1) (riga portata a
/// `deleting`) e dopo il passo (2) (delete dell'oggetto). Default no-op.
pub trait GcHooks: Send + Sync {
    fn after_mark<'a>(&'a self, _sha: &'a str) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }
    fn after_delete<'a>(&'a self, _sha: &'a str) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }
}

/// Implementazione no-op di `GcHooks`.
pub struct NoopHooks;

impl GcHooks for NoopHooks {}

/// Esito di un giro di GC.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GcReport {
    pub marked: u64,
    pub deleted: u64,
    pub abandoned_reset: u64,
}

/// Applica `sql/schema.sql` (idempotente).
pub async fn apply_schema(_db: &sqlx::PgPool) -> anyhow::Result<()> {
    todo!()
}

/// Adozione A: `pending = pending + 1` (e `updated_at = now()`) solo se `state = 'live'`;
/// con `deleting`, retry con backoff (budget 5 tentativi), poi errore (503).
pub async fn adopt_begin(_db: &sqlx::PgPool, _sha: &str) -> anyhow::Result<()> {
    todo!()
}

/// Adozione B, dentro la transazione dell'upload: `pending = pending - 1`.
pub async fn adopt_commit(_conn: &mut sqlx::PgConnection, _sha: &str) -> anyhow::Result<()> {
    todo!()
}

/// Un giro di GC: riprende le righe `deleting`, azzera le adozioni abbandonate più
/// vecchie di `min_age`, raccoglie gli orfani più vecchi di `min_age`.
pub async fn run(
    _db: &sqlx::PgPool,
    _sink: &dyn BlobSink,
    _min_age: Duration,
    _hooks: &dyn GcHooks,
) -> anyhow::Result<GcReport> {
    todo!()
}
