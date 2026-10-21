//! Strumenti di misura dello spike S-01, senza HTTP (il client di carico è `curl`).
//!
//! - `gen-pdf`: pool di fixture PDF di dimensione esatta (`pdfgen`);
//! - `reset --adapter`: truncate di `upload`, `outbox`, `blob`, `blob_chunk` e
//!   svuotamento del prefisso `sha256/` di Garage;
//! - `count --adapter`: righe `upload`, sha distinti, oggetti fisici;
//! - `aggregate`: percentili dai tempi di `curl`;
//! - `tsv2csv`: CSV a header fisso con `common::csv::Writer` (hw.json gemello);
//! - `dedup-ci`: regola pre-registrata del canale laterale (RISULTATI §Soglie);
//! - `gc`, `gc-bench`: GC degli orfani con N parametrico, `gc.csv`;
//! - `storage`: letture per range da 64 KiB, WAL per MiB e dimensione della tabella;
//! - `pgsize`: `pg_total_relation_size` di `blob_chunk`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use s01_upload::sink::{BlobSink, GarageSink, NullSink, PgByteaSink};
use s01_upload::{gc, pdfgen};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Adapter {
    Garage,
    Pgbytea,
    Null,
}

#[derive(Parser)]
#[command(about = "Strumenti di misura dello spike S-01")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Genera `count` PDF da `size` byte con semi `seed_from..seed_from+count`.
    GenPdf {
        #[arg(long)]
        size: u64,
        #[arg(long, default_value_t = 1)]
        seed_from: u64,
        #[arg(long, default_value_t = 1)]
        count: u64,
        #[arg(long)]
        dir: PathBuf,
        #[arg(long)]
        prefix: String,
    },
    /// Svuota metadati e storage dell'adapter.
    Reset {
        #[arg(long, value_enum)]
        adapter: Adapter,
    },
    /// Stampa `rows distinct_sha objects` su una riga.
    Count {
        #[arg(long, value_enum)]
        adapter: Adapter,
    },
    /// Legge righe `<http_code> <time_total_s> [...]` e stampa
    /// `req p50_ms p95_ms p99_ms status_2xx status_4xx status_5xx`.
    Aggregate {
        #[arg(long)]
        input: PathBuf,
    },
    /// Converte un TSV (senza header) in CSV a header fisso, con hw.json gemello.
    Tsv2csv {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Colonne separate da virgola.
        #[arg(long)]
        header: String,
    },
    /// Regola pre-registrata del canale laterale per `adapter` e `session`.
    DedupCi {
        #[arg(long)]
        csv: PathBuf,
        #[arg(long)]
        adapter: String,
        #[arg(long)]
        session: u32,
        #[arg(long, default_value_t = 2000)]
        resamples: u32,
    },
    /// Un giro di GC con N ore (anche frazionarie); stampa il report.
    Gc {
        #[arg(long, value_enum)]
        adapter: Adapter,
        #[arg(long)]
        n_hours: f64,
    },
    /// Misura del GC su `objects` oggetti di cui `orphans` orfani; scrive `out`.
    GcBench {
        #[arg(long, default_value_t = 1000)]
        objects: u64,
        #[arg(long, default_value_t = 100)]
        orphans: u64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Range da 64 KiB, WAL e dimensione dopo `docs` documenti; scrive `out`.
    Storage {
        #[arg(long)]
        fixtures: PathBuf,
        #[arg(long, default_value_t = 30)]
        docs: usize,
        #[arg(long, default_value_t = 200)]
        reads: usize,
        #[arg(long)]
        out: PathBuf,
    },
    /// `pg_total_relation_size('blob_chunk')` in byte.
    Pgsize,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(run(cli.cmd))
}

async fn db() -> anyhow::Result<sqlx::PgPool> {
    let url = common::env::required("DATABASE_URL")?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .context("connessione a Postgres")?;
    gc::apply_schema(&pool).await?;
    Ok(pool)
}

async fn sink(adapter: Adapter, pool: &sqlx::PgPool) -> anyhow::Result<Box<dyn BlobSink>> {
    Ok(match adapter {
        Adapter::Garage => Box::new(GarageSink::from_env()?),
        Adapter::Pgbytea => Box::new(PgByteaSink::new(pool.clone())),
        Adapter::Null => Box::new(NullSink),
    })
}

async fn run(cmd: Cmd) -> anyhow::Result<()> {
    match cmd {
        Cmd::GenPdf {
            size,
            seed_from,
            count,
            dir,
            prefix,
        } => {
            std::fs::create_dir_all(&dir)?;
            for seed in seed_from..seed_from + count {
                let p = dir.join(format!("{prefix}-{seed:03}.pdf"));
                pdfgen::generate(&p, size, seed)?;
            }
            Ok(())
        }
        Cmd::Reset { adapter } => {
            let pool = db().await?;
            sqlx::raw_sql("TRUNCATE upload, outbox, blob, blob_chunk")
                .execute(&pool)
                .await?;
            if adapter == Adapter::Garage {
                let g = GarageSink::from_env()?;
                for (key, _) in g.list().await? {
                    g.delete(&key).await?;
                }
            }
            let s = sink(adapter, &pool).await?;
            println!("RESET ok: oggetti residui {}", s.count().await?);
            Ok(())
        }
        Cmd::Count { adapter } => {
            let pool = db().await?;
            let (rows, distinct): (i64, i64) =
                sqlx::query_as("SELECT count(*), count(DISTINCT sha256) FROM upload")
                    .fetch_one(&pool)
                    .await?;
            let objects = sink(adapter, &pool).await?.count().await?;
            println!("{rows} {distinct} {objects}");
            Ok(())
        }
        Cmd::Aggregate { input } => aggregate(&input),
        Cmd::Tsv2csv { input, out, header } => tsv2csv(&input, &out, &header),
        Cmd::DedupCi {
            csv,
            adapter,
            session,
            resamples,
        } => dedup_ci(&csv, &adapter, session, resamples),
        Cmd::Gc { adapter, n_hours } => {
            let pool = db().await?;
            let s = sink(adapter, &pool).await?;
            let n = Duration::from_secs_f64(n_hours * 3600.0);
            let r = gc::run(&pool, s.as_ref(), n, &gc::NoopHooks).await?;
            println!(
                "GC marked={} deleted={} abandoned_reset={}",
                r.marked, r.deleted, r.abandoned_reset
            );
            Ok(())
        }
        Cmd::GcBench {
            objects,
            orphans,
            out,
        } => gc_bench(objects, orphans, &out).await,
        Cmd::Storage {
            fixtures,
            docs,
            reads,
            out,
        } => storage(&fixtures, docs, reads, &out).await,
        Cmd::Pgsize => {
            let pool = db().await?;
            let (n,): (i64,) = sqlx::query_as("SELECT pg_total_relation_size('blob_chunk')")
                .fetch_one(&pool)
                .await?;
            println!("{n}");
            Ok(())
        }
    }
}

// ------------------------------------------------------------------ aggregate

fn percentile_ms(sorted_us: &[u64], q: f64) -> f64 {
    if sorted_us.is_empty() {
        return f64::NAN;
    }
    // Nearest-rank, come hdrhistogram a 3 cifre ma senza quantizzazione.
    let rank = ((q * sorted_us.len() as f64).ceil() as usize).clamp(1, sorted_us.len());
    sorted_us[rank - 1] as f64 / 1000.0
}

fn aggregate(input: &Path) -> anyhow::Result<()> {
    let text = std::fs::read_to_string(input)
        .with_context(|| format!("lettura di {}", input.display()))?;
    let (mut s2, mut s4, mut s5) = (0u64, 0u64, 0u64);
    let mut ok_us = Vec::new();
    let mut req = 0u64;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let mut it = line.split_whitespace();
        let code: u16 = it.next().unwrap_or("0").parse().unwrap_or(0);
        let t: f64 = it.next().unwrap_or("0").parse().unwrap_or(0.0);
        req += 1;
        match code / 100 {
            2 => {
                s2 += 1;
                ok_us.push((t * 1e6).round() as u64);
            }
            4 => s4 += 1,
            _ => s5 += 1,
        }
    }
    ok_us.sort_unstable();
    println!(
        "{req} {:.3} {:.3} {:.3} {s2} {s4} {s5}",
        percentile_ms(&ok_us, 0.50),
        percentile_ms(&ok_us, 0.95),
        percentile_ms(&ok_us, 0.99)
    );
    Ok(())
}

fn tsv2csv(input: &Path, out: &Path, header: &str) -> anyhow::Result<()> {
    let cols: Vec<&str> = header.split(',').collect();
    let text = std::fs::read_to_string(input)
        .with_context(|| format!("lettura di {}", input.display()))?;
    let mut w = common::csv::Writer::create(out, &cols)?;
    let mut n = 0;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let fields: Vec<&str> = line.split('\t').collect();
        w.write_row(&fields)?;
        n += 1;
    }
    println!("TSV2CSV {}: {n} righe", out.display());
    Ok(())
}

// ------------------------------------------------------------------ dedup-ci

/// PRNG deterministico (splitmix64) per il bootstrap.
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_unstable_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

fn dedup_ci(csv: &Path, adapter: &str, session: u32, resamples: u32) -> anyhow::Result<()> {
    let text = std::fs::read_to_string(csv)?;
    let prefix = format!("s{session}-");
    let (mut new, mut dedup) = (Vec::new(), Vec::new());
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        if f.len() != 4 || f[0] != adapter || !f[2].starts_with(&prefix) {
            continue;
        }
        let ms: f64 = f[3].parse()?;
        if f[1] == "true" {
            dedup.push(ms);
        } else {
            new.push(ms);
        }
    }
    anyhow::ensure!(
        !new.is_empty() && !dedup.is_empty(),
        "nessun campione per {adapter} sessione {session}"
    );
    let p50_new = median(&mut new.clone());
    let p50_dedup = median(&mut dedup.clone());
    let d = p50_dedup - p50_new;
    let t = (0.10 * p50_new).max(20.0);
    let mut rng = SplitMix(u64::from(session));
    let mut ds = Vec::with_capacity(resamples as usize);
    let mut bn = vec![0.0; new.len()];
    let mut bd = vec![0.0; dedup.len()];
    for _ in 0..resamples {
        for x in bn.iter_mut() {
            *x = new[rng.below(new.len())];
        }
        for x in bd.iter_mut() {
            *x = dedup[rng.below(dedup.len())];
        }
        ds.push(median(&mut bd) - median(&mut bn));
    }
    ds.sort_unstable_by(|a, b| a.total_cmp(b));
    let lo = ds[((resamples as f64) * 0.025).floor() as usize];
    let hi = ds[(((resamples as f64) * 0.975).ceil() as usize).min(ds.len() - 1)];
    let pass = lo >= -t && hi <= t;
    println!(
        "adapter={adapter} session={session} n_new={} n_dedup={} p50_new_ms={p50_new:.3} \
         p50_dedup_ms={p50_dedup:.3} d_ms={d:.3} ci95_lo_ms={lo:.3} ci95_hi_ms={hi:.3} \
         t_ms={t:.3} pass={pass}",
        new.len(),
        dedup.len()
    );
    Ok(())
}

// ------------------------------------------------------------------ gc-bench

fn small_object(tag: &str, i: u64) -> (String, Vec<u8>) {
    let data = format!("s01 gc-bench {tag} {i}\n").repeat(64).into_bytes();
    (hex::encode(Sha256::digest(&data)), data)
}

async fn gc_bench(objects: u64, orphans: u64, out: &Path) -> anyhow::Result<()> {
    anyhow::ensure!(orphans <= objects, "orphans > objects");
    let pool = db().await?;
    let g = GarageSink::from_env()?;
    let dir = tempfile::tempdir_in(std::env::temp_dir())?;
    let mut csv = common::csv::Writer::create(
        out,
        &[
            "objects",
            "orphans",
            "n_hours",
            "list_ms",
            "head_ms_p95",
            "deleted",
        ],
    )?;
    // Due scenari: orfani vecchi di 48 h (raccolti con N = 24 h) e orfani di 1 h
    // (protetti da N = 24 h).
    for (scenario, age_h) in [("old", 48), ("young", 1)] {
        sqlx::raw_sql("TRUNCATE upload, outbox, blob, blob_chunk")
            .execute(&pool)
            .await?;
        for (key, _) in g.list().await? {
            g.delete(&key).await?;
        }
        for i in 0..objects {
            let (sha, data) = small_object(scenario, i);
            let p = dir.path().join("obj.bin");
            tokio::fs::write(&p, &data).await?;
            g.put(&sha, &p).await?;
            sqlx::query(
                "INSERT INTO blob (sha256, state, pending, updated_at) \
                 VALUES ($1, 'live', 0, now() - ($2::int * interval '1 hour'))",
            )
            .bind(&sha)
            .bind(age_h)
            .execute(&pool)
            .await?;
            if i >= orphans {
                sqlx::query(
                    "INSERT INTO upload (id, sha256, size_bytes, status, created_at) \
                     VALUES ($1, $2, $3, 'accepted', now())",
                )
                .bind(uuid::Uuid::new_v4())
                .bind(&sha)
                .bind(data.len() as i64)
                .execute(&pool)
                .await?;
            }
        }
        let sw = common::timer::Stopwatch::start();
        let listed = g.list().await?;
        let list_ms = sw.elapsed_ms_f64();
        anyhow::ensure!(listed.len() as u64 == objects, "list: {}", listed.len());
        let mut heads = Vec::with_capacity(listed.len());
        for (key, _) in &listed {
            let sw = common::timer::Stopwatch::start();
            g.head(key).await?;
            heads.push(sw.elapsed_us());
        }
        heads.sort_unstable();
        let head_p95 = percentile_ms(&heads, 0.95);
        let r = gc::run(&pool, &g, gc::DEFAULT_MIN_AGE, &gc::NoopHooks).await?;
        let left = g.count().await?;
        println!(
            "GC-BENCH {scenario}: list {list_ms:.1} ms, head p95 {head_p95:.2} ms, \
             deleted {}, oggetti rimasti {left}",
            r.deleted
        );
        csv.write_row(&[
            &objects.to_string(),
            &orphans.to_string(),
            "24",
            &format!("{list_ms:.3}"),
            &format!("{head_p95:.3}"),
            &r.deleted.to_string(),
        ])?;
    }
    Ok(())
}

// ------------------------------------------------------------------ storage

async fn storage(fixtures: &Path, docs: usize, reads: usize, out: &Path) -> anyhow::Result<()> {
    let pool = db().await?;
    let mut files: Vec<PathBuf> = std::fs::read_dir(fixtures)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "pdf"))
        .collect();
    files.sort();
    files.truncate(docs);
    anyhow::ensure!(
        files.len() == docs,
        "fixture insufficienti in {}",
        fixtures.display()
    );
    let mut csv = common::csv::Writer::create(out, &["adapter", "metric", "value", "unit"])?;
    for adapter in [Adapter::Garage, Adapter::Pgbytea] {
        let name = format!("{adapter:?}").to_lowercase();
        sqlx::raw_sql("TRUNCATE upload, outbox, blob, blob_chunk")
            .execute(&pool)
            .await?;
        let s = sink(adapter, &pool).await?;
        if adapter == Adapter::Garage {
            let g = GarageSink::from_env()?;
            for (key, _) in g.list().await? {
                g.delete(&key).await?;
            }
        }
        let (lsn0,): (String,) = sqlx::query_as("SELECT pg_current_wal_lsn()::text")
            .fetch_one(&pool)
            .await?;
        let mut keys = Vec::new();
        let mut total_bytes = 0u64;
        let sw = common::timer::Stopwatch::start();
        for f in &files {
            let data = tokio::fs::read(f).await?;
            total_bytes += data.len() as u64;
            let sha = hex::encode(Sha256::digest(&data));
            s.put(&sha, f).await?;
            keys.push((sha, data.len() as u64));
        }
        let put_ms = sw.elapsed_ms_f64();
        let (wal,): (i64,) =
            sqlx::query_as("SELECT pg_wal_lsn_diff(pg_current_wal_lsn(), $1::pg_lsn)::bigint")
                .bind(&lsn0)
                .fetch_one(&pool)
                .await?;
        let mib = total_bytes as f64 / (1024.0 * 1024.0);
        let mut rng = SplitMix(7);
        let mut lat = Vec::with_capacity(reads);
        for _ in 0..reads {
            let (sha, len) = &keys[rng.below(keys.len())];
            let span = 64 * 1024u64;
            let start = (rng.next() % (len - span)) & !0xfff;
            let sw = common::timer::Stopwatch::start();
            let b = s.get_range(sha, start..start + span).await?;
            lat.push(sw.elapsed_us());
            anyhow::ensure!(b.len() as u64 == span, "range corto: {}", b.len());
        }
        lat.sort_unstable();
        let rows: Vec<(String, String, &str)> = vec![
            ("documents".into(), docs.to_string(), "count"),
            ("bytes_written".into(), total_bytes.to_string(), "bytes"),
            ("put_total_ms".into(), format!("{put_ms:.1}"), "ms"),
            (
                "range_64k_p50_ms".into(),
                format!("{:.3}", percentile_ms(&lat, 0.50)),
                "ms",
            ),
            (
                "range_64k_p95_ms".into(),
                format!("{:.3}", percentile_ms(&lat, 0.95)),
                "ms",
            ),
            (
                "wal_bytes_per_mib".into(),
                format!("{:.0}", wal as f64 / mib),
                "bytes/MiB",
            ),
        ];
        for (metric, value, unit) in &rows {
            csv.write_row(&[&name, metric, value, unit])?;
        }
        if adapter == Adapter::Pgbytea {
            let (size,): (i64,) = sqlx::query_as("SELECT pg_total_relation_size('blob_chunk')")
                .fetch_one(&pool)
                .await?;
            csv.write_row(&[&name, "pg_total_relation_size", &size.to_string(), "bytes"])?;
        }
        println!("STORAGE {name}: {rows:?}");
    }
    Ok(())
}
