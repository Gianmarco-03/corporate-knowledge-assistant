//! Trait locale allo spike `BlobSink` (WIRING W16) e i tre adapter.
//!
//! Chiave = sha256 esadecimale del contenuto, precomputata dal chiamante; il nome file
//! del client non entra mai nella chiave (T-08). Nessun tipo `object_store` o `sqlx`
//! compare nella firma del trait. `put` è idempotente per chiave: scrivere due volte lo
//! stesso contenuto lascia un solo oggetto fisico (D-02).
//!
//! Nessun I/O bloccante nel corpo async: `GarageSink` e `PgByteaSink` leggono il
//! tempfile con `tokio::fs::File` in streaming, mai con `std::fs`.

use std::ops::Range;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Context;
use bytes::Bytes;
use futures::StreamExt;
use futures::future::BoxFuture;
use object_store::aws::AmazonS3Builder;
use object_store::buffered::BufWriter;
use object_store::path::Path as ObjPath;
use object_store::{ObjectStore, ObjectStoreExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Dimensione di un chunk letto dal tempfile e di una riga di `blob_chunk` (1 MiB).
pub const CHUNK: usize = 1024 * 1024;

/// Prefisso delle chiavi nello storage a oggetti.
pub const PREFIX: &str = "sha256";

/// Storage degli originali per chiave = sha256 esadecimale del contenuto.
pub trait BlobSink: Send + Sync {
    fn put<'a>(&'a self, key: &'a str, path: &'a Path) -> BoxFuture<'a, anyhow::Result<()>>;
    fn get_range<'a>(
        &'a self,
        key: &'a str,
        range: Range<u64>,
    ) -> BoxFuture<'a, anyhow::Result<Bytes>>;
    fn delete<'a>(&'a self, key: &'a str) -> BoxFuture<'a, anyhow::Result<()>>;
    fn count(&self) -> BoxFuture<'_, anyhow::Result<u64>>;
}

/// La chiave deve essere un sha256 esadecimale minuscolo: niente percorsi (T-08).
fn check_key(key: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        key.len() == 64 && key.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
        "chiave non valida: attesi 64 caratteri esadecimali minuscoli"
    );
    Ok(())
}

/// Legge il prossimo blocco fino a `CHUNK` byte (meno solo a fine file).
async fn read_chunk(f: &mut tokio::fs::File, buf: &mut Vec<u8>) -> std::io::Result<usize> {
    buf.clear();
    buf.resize(CHUNK, 0);
    let mut n = 0;
    while n < CHUNK {
        let k = f.read(&mut buf[n..]).await?;
        if k == 0 {
            break;
        }
        n += k;
    }
    buf.truncate(n);
    Ok(n)
}

// ---------------------------------------------------------------------- NullSink

/// Adapter nullo (baseline): nessuna scrittura, nessuna lettura possibile.
#[derive(Default)]
pub struct NullSink;

impl BlobSink for NullSink {
    fn put<'a>(&'a self, key: &'a str, _path: &'a Path) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move { check_key(key) })
    }
    fn get_range<'a>(
        &'a self,
        _key: &'a str,
        _range: Range<u64>,
    ) -> BoxFuture<'a, anyhow::Result<Bytes>> {
        Box::pin(async { anyhow::bail!("NullSink non conserva oggetti") })
    }
    fn delete<'a>(&'a self, key: &'a str) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move { check_key(key) })
    }
    fn count(&self) -> BoxFuture<'_, anyhow::Result<u64>> {
        Box::pin(async { Ok(0) })
    }
}

// ---------------------------------------------------------------------- Garage

/// Garage via `object_store` S3, configurato da env.
pub struct GarageSink {
    store: Arc<dyn ObjectStore>,
    /// PUT completati (scritture tentate, anche sovrascritture idempotenti).
    pub puts: AtomicU64,
}

impl GarageSink {
    /// Legge `S3_ENDPOINT`, `S3_BUCKET`, `GARAGE_ACCESS_KEY`, `GARAGE_SECRET_KEY`.
    /// `with_allow_http(true)`: Garage è raggiungibile solo sulla rete compose locale
    /// dello spike (T-01-04-07, accettato); nel prodotto il TLS resta all'ADR.
    pub fn from_env() -> anyhow::Result<Self> {
        let store = AmazonS3Builder::new()
            .with_endpoint(common::env::required("S3_ENDPOINT")?)
            .with_allow_http(true)
            .with_region("garage")
            .with_bucket_name(common::env::required("S3_BUCKET")?)
            .with_access_key_id(common::env::required("GARAGE_ACCESS_KEY")?)
            .with_secret_access_key(common::env::required("GARAGE_SECRET_KEY")?)
            .build()
            .context("configurazione S3 di Garage")?;
        Ok(GarageSink {
            store: Arc::new(store),
            puts: AtomicU64::new(0),
        })
    }

    fn path(key: &str) -> ObjPath {
        ObjPath::from(format!("{PREFIX}/{key}"))
    }

    /// Elenca le chiavi sotto `sha256/` con `last_modified` in secondi Unix (per `probe gc`).
    pub async fn list(&self) -> anyhow::Result<Vec<(String, i64)>> {
        let prefix = ObjPath::from(PREFIX);
        let mut out = Vec::new();
        let mut s = self.store.list(Some(&prefix));
        while let Some(meta) = s.next().await {
            let meta = meta?;
            let key = meta.location.filename().unwrap_or_default().to_string();
            out.push((key, meta.last_modified.timestamp()));
        }
        Ok(out)
    }

    /// `head` di una chiave: `Some(epoch_s)` di `last_modified` se esiste.
    pub async fn head(&self, key: &str) -> anyhow::Result<Option<i64>> {
        check_key(key)?;
        match self.store.head(&Self::path(key)).await {
            Ok(m) => Ok(Some(m.last_modified.timestamp())),
            Err(object_store::Error::NotFound { .. }) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

impl BlobSink for GarageSink {
    fn put<'a>(&'a self, key: &'a str, path: &'a Path) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            check_key(key)?;
            let mut f = tokio::fs::File::open(path).await?;
            let mut w = BufWriter::new(self.store.clone(), Self::path(key));
            let mut buf = Vec::with_capacity(CHUNK);
            loop {
                let n = read_chunk(&mut f, &mut buf).await?;
                if n == 0 {
                    break;
                }
                w.put(Bytes::copy_from_slice(&buf)).await?;
            }
            // `shutdown` completa il PUT singolo o il multipart.
            w.shutdown().await?;
            self.puts.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
    }
    fn get_range<'a>(
        &'a self,
        key: &'a str,
        range: Range<u64>,
    ) -> BoxFuture<'a, anyhow::Result<Bytes>> {
        Box::pin(async move {
            check_key(key)?;
            Ok(self.store.get_range(&Self::path(key), range).await?)
        })
    }
    fn delete<'a>(&'a self, key: &'a str) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            check_key(key)?;
            match self.store.delete(&Self::path(key)).await {
                Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
                Err(e) => Err(e.into()),
            }
        })
    }
    fn count(&self) -> BoxFuture<'_, anyhow::Result<u64>> {
        Box::pin(async move {
            let prefix = ObjPath::from(PREFIX);
            let mut s = self.store.list(Some(&prefix));
            let mut n = 0;
            while let Some(meta) = s.next().await {
                meta?;
                n += 1;
            }
            Ok(n)
        })
    }
}

// ---------------------------------------------------------------------- PG bytea

/// Postgres `bytea` a chunk da 1 MiB (`blob_chunk`, `STORAGE EXTERNAL`), una
/// transazione per oggetto, `ON CONFLICT DO NOTHING` (idempotente per chiave).
pub struct PgByteaSink {
    db: sqlx::PgPool,
}

impl PgByteaSink {
    pub fn new(db: sqlx::PgPool) -> Self {
        PgByteaSink { db }
    }
}

impl BlobSink for PgByteaSink {
    fn put<'a>(&'a self, key: &'a str, path: &'a Path) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            check_key(key)?;
            // Lettura e scrittura in streaming: un chunk alla volta in memoria. La
            // transazione contiene solo INSERT sul DB, nessuna chiamata esterna.
            let mut f = tokio::fs::File::open(path).await?;
            let mut tx = self.db.begin().await?;
            let mut buf = Vec::with_capacity(CHUNK);
            let mut seq: i32 = 0;
            loop {
                let n = read_chunk(&mut f, &mut buf).await?;
                if n == 0 && seq > 0 {
                    break;
                }
                sqlx::query(
                    "INSERT INTO blob_chunk (object_key, seq, data) VALUES ($1, $2, $3) \
                     ON CONFLICT (object_key, seq) DO NOTHING",
                )
                .bind(key)
                .bind(seq)
                .bind(buf.as_slice())
                .execute(&mut *tx)
                .await?;
                seq += 1;
                if n < CHUNK {
                    break;
                }
            }
            tx.commit().await?;
            Ok(())
        })
    }
    fn get_range<'a>(
        &'a self,
        key: &'a str,
        range: Range<u64>,
    ) -> BoxFuture<'a, anyhow::Result<Bytes>> {
        Box::pin(async move {
            check_key(key)?;
            anyhow::ensure!(range.start < range.end, "range vuoto");
            let c = CHUNK as u64;
            let first = (range.start / c) as i32;
            let last = ((range.end - 1) / c) as i32;
            let rows: Vec<(i32, Vec<u8>)> = sqlx::query_as(
                "SELECT seq, data FROM blob_chunk WHERE object_key = $1 \
                 AND seq BETWEEN $2 AND $3 ORDER BY seq",
            )
            .bind(key)
            .bind(first)
            .bind(last)
            .fetch_all(&self.db)
            .await?;
            anyhow::ensure!(!rows.is_empty(), "oggetto {key} assente");
            let mut out = Vec::with_capacity((range.end - range.start) as usize);
            for (seq, data) in rows {
                let base = seq as u64 * c;
                let lo = range.start.saturating_sub(base).min(data.len() as u64) as usize;
                let hi = (range.end - base).min(data.len() as u64) as usize;
                if lo < hi {
                    out.extend_from_slice(&data[lo..hi]);
                }
            }
            anyhow::ensure!(!out.is_empty(), "range oltre la fine dell'oggetto {key}");
            Ok(Bytes::from(out))
        })
    }
    fn delete<'a>(&'a self, key: &'a str) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async move {
            check_key(key)?;
            sqlx::query("DELETE FROM blob_chunk WHERE object_key = $1")
                .bind(key)
                .execute(&self.db)
                .await?;
            Ok(())
        })
    }
    fn count(&self) -> BoxFuture<'_, anyhow::Result<u64>> {
        Box::pin(async move {
            let (n,): (i64,) = sqlx::query_as("SELECT count(DISTINCT object_key) FROM blob_chunk")
                .fetch_one(&self.db)
                .await?;
            Ok(n as u64)
        })
    }
}
