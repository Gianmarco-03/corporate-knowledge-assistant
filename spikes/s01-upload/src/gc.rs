//! GC degli orfani coordinato con l'adozione concorrente (RISK-004): protocollo a stati
//! per hash sulla tabella `blob` (`live` | `deleting`, contatore `pending`).
//!
//! Upload (adozione):
//! - A, prima del PUT: `pending = pending + 1` solo se `state = 'live'` (o riga nuova);
//!   con `deleting`, retry con backoff (5 tentativi), poi errore → 503.
//! - B, dentro la transazione dell'upload (riga `upload` + `outbox`): `pending - 1`.
//!
//! GC, per hash:
//! 1. transazione breve: lock della riga (`FOR UPDATE`), poi in uno statement nuovo
//!    (snapshot nuovo in READ COMMITTED, quindi vede gli upload committati mentre si
//!    aspettava il lock) verifica che non esistano righe `upload`; se `live`,
//!    `pending = 0` e più vecchia di N, la porta a `deleting`;
//! 2. delete dell'oggetto, fuori transazione;
//! 3. transazione che cancella la riga `deleting`.
//!
//! Un GC interrotto riprende le righe `deleting` al giro successivo; le righe con
//! `pending > 0` più vecchie di N sono adozioni abbandonate (crash fra PUT e
//! transazione) e tornano a 0. Nessuna chiamata esterna dentro una transazione.

use std::time::Duration;

use futures::future::BoxFuture;

use crate::sink::BlobSink;

/// Età minima di un orfano prima della raccolta (N = 24 h, D-02).
pub const DEFAULT_MIN_AGE: Duration = Duration::from_secs(24 * 3600);

/// Tentativi dell'adozione A su una riga `deleting` prima del 503.
pub const ADOPT_ATTEMPTS: u32 = 5;
const ADOPT_BACKOFF_BASE: Duration = Duration::from_millis(50);

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

/// Errore dell'adozione A con budget esaurito: il chiamante risponde 503.
#[derive(Debug)]
pub struct AdoptBusy;

impl std::fmt::Display for AdoptBusy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "hash in cancellazione: budget di adozione esaurito")
    }
}

impl std::error::Error for AdoptBusy {}

/// Applica `sql/schema.sql` (idempotente).
pub async fn apply_schema(db: &sqlx::PgPool) -> anyhow::Result<()> {
    sqlx::raw_sql(include_str!("../sql/schema.sql"))
        .execute(db)
        .await?;
    Ok(())
}

/// Adozione A: `pending = pending + 1` (e `updated_at = now()`) solo se `state = 'live'`;
/// con `deleting`, retry con backoff (budget 5 tentativi), poi errore (503).
pub async fn adopt_begin(db: &sqlx::PgPool, sha: &str) -> anyhow::Result<()> {
    for attempt in 0..ADOPT_ATTEMPTS {
        let row: Option<(i32,)> = sqlx::query_as(
            "INSERT INTO blob (sha256, state, pending, updated_at) VALUES ($1, 'live', 1, now()) \
             ON CONFLICT (sha256) DO UPDATE SET pending = blob.pending + 1, updated_at = now() \
             WHERE blob.state = 'live' \
             RETURNING pending",
        )
        .bind(sha)
        .fetch_optional(db)
        .await?;
        if row.is_some() {
            return Ok(());
        }
        if attempt + 1 < ADOPT_ATTEMPTS {
            tokio::time::sleep(ADOPT_BACKOFF_BASE * 2u32.pow(attempt)).await;
        }
    }
    Err(AdoptBusy.into())
}

/// Adozione B, dentro la transazione dell'upload: `pending = pending - 1`.
pub async fn adopt_commit(conn: &mut sqlx::PgConnection, sha: &str) -> anyhow::Result<()> {
    let res = sqlx::query(
        "UPDATE blob SET pending = GREATEST(pending - 1, 0), updated_at = now() \
         WHERE sha256 = $1 AND state = 'live'",
    )
    .bind(sha)
    .execute(&mut *conn)
    .await?;
    anyhow::ensure!(
        res.rows_affected() == 1,
        "adozione B senza riga live per {sha}: protocollo violato"
    );
    Ok(())
}

fn secs(d: Duration) -> f64 {
    d.as_secs_f64()
}

/// Passi (2) e (3) per un hash già `deleting`.
async fn finish_delete(
    db: &sqlx::PgPool,
    sink: &dyn BlobSink,
    sha: &str,
    hooks: &dyn GcHooks,
) -> anyhow::Result<()> {
    sink.delete(sha).await?;
    hooks.after_delete(sha).await;
    sqlx::query("DELETE FROM blob WHERE sha256 = $1 AND state = 'deleting'")
        .bind(sha)
        .execute(db)
        .await?;
    Ok(())
}

/// Passo (1) per un candidato: `true` se la riga è stata portata a `deleting`.
async fn mark(db: &sqlx::PgPool, sha: &str, min_age: Duration) -> anyhow::Result<bool> {
    let mut tx = db.begin().await?;
    let row: Option<(String, i32, bool)> = sqlx::query_as(
        "SELECT state, pending, updated_at <= now() - make_interval(secs => $2) \
         FROM blob WHERE sha256 = $1 FOR UPDATE",
    )
    .bind(sha)
    .bind(secs(min_age))
    .fetch_optional(&mut *tx)
    .await?;
    let Some((state, pending, old)) = row else {
        tx.rollback().await?;
        return Ok(false);
    };
    if state != "live" || pending != 0 || !old {
        tx.rollback().await?;
        return Ok(false);
    }
    // Statement nuovo dopo il lock: vede gli upload committati nel frattempo.
    let (adopted,): (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM upload WHERE sha256 = $1)")
            .bind(sha)
            .fetch_one(&mut *tx)
            .await?;
    if adopted {
        tx.rollback().await?;
        return Ok(false);
    }
    sqlx::query("UPDATE blob SET state = 'deleting', updated_at = now() WHERE sha256 = $1")
        .bind(sha)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}

/// Un giro di GC: riprende le righe `deleting`, azzera le adozioni abbandonate più
/// vecchie di `min_age`, raccoglie gli orfani più vecchi di `min_age`.
pub async fn run(
    db: &sqlx::PgPool,
    sink: &dyn BlobSink,
    min_age: Duration,
    hooks: &dyn GcHooks,
) -> anyhow::Result<GcReport> {
    let mut report = GcReport::default();

    // Ripresa di un GC interrotto fra (1) e (3).
    let resumed: Vec<(String,)> =
        sqlx::query_as("SELECT sha256 FROM blob WHERE state = 'deleting'")
            .fetch_all(db)
            .await?;
    for (sha,) in &resumed {
        finish_delete(db, sink, sha, hooks).await?;
        report.deleted += 1;
    }

    // Adozioni abbandonate: `updated_at` resta quello dell'ultima adozione, quindi la
    // riga è raccoglibile già in questo giro se nessun upload la usa.
    let reset = sqlx::query(
        "UPDATE blob SET pending = 0 WHERE state = 'live' AND pending > 0 \
         AND updated_at <= now() - make_interval(secs => $1)",
    )
    .bind(secs(min_age))
    .execute(db)
    .await?;
    report.abandoned_reset = reset.rows_affected();

    let candidates: Vec<(String,)> = sqlx::query_as(
        "SELECT b.sha256 FROM blob b WHERE b.state = 'live' AND b.pending = 0 \
         AND b.updated_at <= now() - make_interval(secs => $1) \
         AND NOT EXISTS (SELECT 1 FROM upload u WHERE u.sha256 = b.sha256) \
         ORDER BY b.sha256",
    )
    .bind(secs(min_age))
    .fetch_all(db)
    .await?;
    for (sha,) in &candidates {
        if !mark(db, sha, min_age).await? {
            continue;
        }
        report.marked += 1;
        hooks.after_mark(sha).await;
        finish_delete(db, sink, sha, hooks).await?;
        report.deleted += 1;
    }
    Ok(report)
}
