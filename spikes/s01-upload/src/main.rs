//! Server di upload dello spike S-01 su `0.0.0.0:8080` nella rete compose (nessuna
//! porta pubblicata: lo raggiunge il client `curl` del runner per nome di container).
//!
//! `--adapter garage|pgbytea|null`, `--validate on|off` (`off` solo con `null`, per il
//! puro streaming), `--bench` (`GET /bench/stats` e `GET /bench/phases`),
//! `--fault none|after_put` (solo con `--bench`), parametri di admission.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, ValueEnum};
use s01_upload::admission::{self, AdmissionConfig};
use s01_upload::gc;
use s01_upload::sink::{BlobSink, GarageSink, NullSink, PgByteaSink};
use s01_upload::upload::{self, ServerOptions, UploadConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Adapter {
    Garage,
    Pgbytea,
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OnOff {
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Fault {
    None,
    #[value(name = "after_put")]
    AfterPut,
}

#[derive(Parser, Debug)]
#[command(about = "Server di upload dello spike S-01")]
struct Args {
    #[arg(long, value_enum)]
    adapter: Adapter,
    #[arg(long, value_enum, default_value = "on")]
    validate: OnOff,
    /// Metadati, outbox e protocollo GC su Postgres (`DATABASE_URL`).
    #[arg(long, value_enum, default_value = "on")]
    db: OnOff,
    #[arg(long)]
    bench: bool,
    #[arg(long, value_enum, default_value = "none")]
    fault: Fault,
    #[arg(long, default_value = "0.0.0.0:8080")]
    listen: String,
    #[arg(long, default_value = "/work/tmp")]
    tmp_dir: PathBuf,
    #[arg(long, default_value_t = 32)]
    max_inflight: usize,
    #[arg(long, default_value_t = 8)]
    max_validations: usize,
    #[arg(long, default_value_t = 10)]
    validation_wait_s: u64,
    #[arg(long, default_value_t = 120)]
    receive_deadline_s: u64,
    #[arg(long, default_value_t = 15)]
    idle_timeout_s: u64,
    #[arg(long, default_value_t = 10)]
    validation_deadline_s: u64,
    /// Thread del pool blocking (validazioni, tempfile).
    #[arg(long, default_value_t = 16)]
    max_blocking_threads: usize,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(args.max_blocking_threads)
        .build()?;
    rt.block_on(serve(args))
}

async fn serve(args: Args) -> anyhow::Result<()> {
    anyhow::ensure!(
        args.validate == OnOff::On || args.adapter == Adapter::Null,
        "--validate off è ammesso solo con --adapter null (puro streaming)"
    );
    anyhow::ensure!(
        args.fault == Fault::None || args.bench,
        "--fault after_put è ammesso solo con --bench"
    );
    anyhow::ensure!(
        args.adapter != Adapter::Pgbytea || args.db == OnOff::On,
        "--adapter pgbytea richiede --db on"
    );

    let adm = AdmissionConfig {
        max_inflight: args.max_inflight,
        max_validations: args.max_validations,
        validation_wait: upload::secs(args.validation_wait_s),
        receive_deadline: upload::secs(args.receive_deadline_s),
        idle_timeout: upload::secs(args.idle_timeout_s),
        validation_deadline: upload::secs(args.validation_deadline_s),
    };
    // Budget di byte temporanei verificato all'avvio: rifiuto di partire se lo spazio
    // libero di tmp_dir non basta o non è determinabile (RISK-003).
    let budget = adm.tmp_budget_bytes();
    let free = admission::free_bytes(&args.tmp_dir).with_context(|| {
        format!(
            "spazio libero di {} non determinabile",
            args.tmp_dir.display()
        )
    })?;
    anyhow::ensure!(
        free >= budget,
        "spazio libero {free} B in {} sotto il budget {budget} B (max_inflight × 25 MiB)",
        args.tmp_dir.display()
    );

    let db = if args.db == OnOff::On {
        let url = common::env::required("DATABASE_URL")?;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections((args.max_inflight as u32).clamp(4, 64))
            .connect(&url)
            .await
            .context("connessione a Postgres")?;
        gc::apply_schema(&pool).await?;
        Some(pool)
    } else {
        None
    };
    let sink: Arc<dyn BlobSink> = match args.adapter {
        Adapter::Garage => Arc::new(GarageSink::from_env()?),
        Adapter::Pgbytea => Arc::new(PgByteaSink::new(db.clone().expect("db on"))),
        Adapter::Null => Arc::new(NullSink),
    };
    let cfg = UploadConfig {
        tmp_dir: args.tmp_dir.clone(),
        validate: args.validate == OnOff::On,
        bench: args.bench,
        admission: adm,
    };
    let opts = ServerOptions {
        fault_after_put: args.fault == Fault::AfterPut,
    };
    let app = upload::router_with(cfg, sink, db, opts)?;
    let listener = tokio::net::TcpListener::bind(&args.listen).await?;
    eprintln!(
        "s01-upload in ascolto su {} (adapter {:?}, validate {:?}, bench {}, fault {:?})",
        args.listen, args.adapter, args.validate, args.bench, args.fault
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// SIGTERM (`session.sh stop`) o Ctrl-C: arresto ordinato. Il server gira come PID 1
/// del container, che senza un gestore ignorerebbe SIGTERM fino al SIGKILL.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(_) => return std::future::pending().await,
        };
        tokio::select! {
            _ = term.recv() => {}
            _ = tokio::signal::ctrl_c() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
