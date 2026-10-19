//! Trait locale allo spike `BlobSink` (WIRING W16) e i tre adapter.

use std::ops::Range;
use std::path::Path;

use bytes::Bytes;
use futures::future::BoxFuture;

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

/// Adapter nullo (baseline).
pub struct NullSink;

/// Garage via `object_store` S3, configurato da env.
pub struct GarageSink {
    _skeleton: (),
}

impl GarageSink {
    pub fn from_env() -> anyhow::Result<Self> {
        todo!()
    }
}

/// Postgres `bytea` a chunk da 1 MiB (`blob_chunk`).
pub struct PgByteaSink {
    _skeleton: (),
}

impl PgByteaSink {
    pub fn new(_db: sqlx::PgPool) -> Self {
        todo!()
    }
}

impl BlobSink for NullSink {
    fn put<'a>(&'a self, _key: &'a str, _path: &'a Path) -> BoxFuture<'a, anyhow::Result<()>> {
        todo!()
    }
    fn get_range<'a>(
        &'a self,
        _key: &'a str,
        _range: Range<u64>,
    ) -> BoxFuture<'a, anyhow::Result<Bytes>> {
        todo!()
    }
    fn delete<'a>(&'a self, _key: &'a str) -> BoxFuture<'a, anyhow::Result<()>> {
        todo!()
    }
    fn count(&self) -> BoxFuture<'_, anyhow::Result<u64>> {
        todo!()
    }
}

impl BlobSink for GarageSink {
    fn put<'a>(&'a self, _key: &'a str, _path: &'a Path) -> BoxFuture<'a, anyhow::Result<()>> {
        todo!()
    }
    fn get_range<'a>(
        &'a self,
        _key: &'a str,
        _range: Range<u64>,
    ) -> BoxFuture<'a, anyhow::Result<Bytes>> {
        todo!()
    }
    fn delete<'a>(&'a self, _key: &'a str) -> BoxFuture<'a, anyhow::Result<()>> {
        todo!()
    }
    fn count(&self) -> BoxFuture<'_, anyhow::Result<u64>> {
        todo!()
    }
}

impl BlobSink for PgByteaSink {
    fn put<'a>(&'a self, _key: &'a str, _path: &'a Path) -> BoxFuture<'a, anyhow::Result<()>> {
        todo!()
    }
    fn get_range<'a>(
        &'a self,
        _key: &'a str,
        _range: Range<u64>,
    ) -> BoxFuture<'a, anyhow::Result<Bytes>> {
        todo!()
    }
    fn delete<'a>(&'a self, _key: &'a str) -> BoxFuture<'a, anyhow::Result<()>> {
        todo!()
    }
    fn count(&self) -> BoxFuture<'_, anyhow::Result<u64>> {
        todo!()
    }
}
