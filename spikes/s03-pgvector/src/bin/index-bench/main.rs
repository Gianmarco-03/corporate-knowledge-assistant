//! CLI di misura dello spike S-03/A-03, parte indice (piano 01-14): `synth`, `threshold`,
//! `hnsw`, `hybrid`, `leak`, `ingest-under-load`. Usa la libreria del piano 01-10
//! (`s03_pgvector::db`, `s03_pgvector::metrics`) senza modificarla.
//!
//! Si lancia da `spikes/` nel runner, dentro una sessione di `tools/session.sh`:
//! `cargo run -p s03-pgvector --release --bin index-bench -- <sottocomando>`. Postgres
//! (`DATABASE_URL`) e TEI (`TEI_EMBED_URL`, bge-m3) sono quelli della sessione.
//!
//! - `prepare`: prima parte di `sql/index.sql` (colonna `halfvec`, tabelle sintetiche);
//! - `synth --n <n>`: corpus sintetico della scala (`synth.rs`);
//! - `threshold --n <n>`: ricerca esatta autorizzata, 0 e 4 worker → `threshold.csv`,
//!   `latency.csv`, piani in `plans/`;
//! - `hnsw --n <n>`: costruzione degli indici HNSW, recall filtrata → `recall.csv`, latenza
//!   HNSW → `threshold.csv`;
//! - `drop-hnsw --n <n>`: rimozione degli indici HNSW della scala;
//! - `hybrid`: densa, lessicale e ibrida sul corpus reale → `hybrid.csv`, verifica metrica;
//! - `leak --n <n> | --real`: chunk negati per ruolo, parte e configurazione → `leak.csv`;
//! - `ingest-under-load --n <n>`: inserimento durante le query → `ingest_under_load.csv`.
//!
//! **Modifica:** Claude (test-write 01-14) · 2026-10-25 · scheletro e test rossi del design
//! **Modifica:** Claude (gsd-executor 01-14) · 2026-10-25 · CLI e sottocomandi di misura

mod hybrid;
mod ingest;
mod leak;
mod synth;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use common::hist::{Recorder, Summary};
use s03_pgvector::chunk::{Chunk, Lang};
use s03_pgvector::metrics::{self, OracleQuery};
use s03_pgvector::{Model, db, tei};
use sqlx::{Connection, PgConnection, PgPool, Row};
use uuid::Uuid;

use crate::hybrid::{Dense, HnswParams, IterativeScan, Strategy, Table};
use crate::synth::{SEED, SYNTH_ROLES, Scale};

/// DDL del piano: prima parte per `prepare`, seconda (dopo `-- HNSW`) per `hnsw`.
const INDEX_SQL: &str = include_str!("../../../sql/index.sql");

/// Numero di risultati delle misure di latenza e recall.
const K: i64 = 10;

#[derive(Parser)]
#[command(
    name = "index-bench",
    about = "Spike S-03/A-03: soglia esatto-HNSW, ibrida, leak test, ingestion concorrente"
)]
struct Cli {
    /// Cartella del corpus comune (manifest.json, acl.json, oracles/).
    #[arg(long, default_value = "data/corpus", global = true)]
    corpus: PathBuf,
    /// Cartella delle evidenze versionate.
    #[arg(long, default_value = "s03-pgvector/results/index", global = true)]
    out: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Applica sql/schema.sql e la prima parte di sql/index.sql (idempotente).
    Prepare,
    /// Genera la scala sintetica (sostituisce quella esistente).
    Synth {
        #[arg(long)]
        n: usize,
    },
    /// Ricerca esatta autorizzata per selettività e worker; latenza end-to-end.
    Threshold {
        #[arg(long)]
        n: usize,
        #[arg(long, default_value_t = 3)]
        runs: u32,
        #[arg(long, default_value_t = 200)]
        queries: usize,
        #[arg(long, default_value_t = 20)]
        warmup: usize,
    },
    /// Costruisce gli indici HNSW della scala e misura recall filtrata e latenza.
    Hnsw {
        #[arg(long)]
        n: usize,
        #[arg(long, default_value_t = 3)]
        runs: u32,
        #[arg(long, default_value_t = 200)]
        queries: usize,
        #[arg(long, default_value_t = 20)]
        warmup: usize,
    },
    /// Rimuove gli indici HNSW della scala.
    DropHnsw {
        #[arg(long)]
        n: usize,
    },
    /// Densa, lessicale e ibrida sul corpus reale; verifica della metrica.
    Hybrid {
        #[arg(long, default_value_t = 3)]
        passes: u32,
    },
    /// Leak test su una scala sintetica (`--n`) o sul corpus reale (`--real`).
    Leak {
        #[arg(long)]
        n: Option<usize>,
        #[arg(long)]
        real: bool,
    },
    /// Query in ciclo con ingestion concorrente a 20 chunk/s.
    IngestUnderLoad {
        #[arg(long, default_value_t = 100_000)]
        n: usize,
        #[arg(long, default_value_t = 300)]
        seconds: u64,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let url = common::env::required("DATABASE_URL")?;
    let (corpus, out) = (cli.corpus.as_path(), cli.out.as_path());
    match cli.cmd {
        Cmd::Prepare => prepare(&db::connect(&url).await?).await,
        Cmd::Synth { n } => synth_cmd(&url, corpus, Scale::from_n(n)?).await,
        Cmd::Threshold {
            n,
            runs,
            queries,
            warmup,
        } => {
            let qs = load_queries(corpus).await?;
            threshold(&url, out, Scale::from_n(n)?, &qs, runs, queries, warmup).await
        }
        Cmd::Hnsw {
            n,
            runs,
            queries,
            warmup,
        } => {
            let qs = load_queries(corpus).await?;
            hnsw(&url, out, Scale::from_n(n)?, &qs, runs, queries, warmup).await
        }
        Cmd::DropHnsw { n } => drop_hnsw(&db::connect(&url).await?, Scale::from_n(n)?).await,
        Cmd::Hybrid { passes } => {
            let qs = load_queries(corpus).await?;
            hybrid_cmd(&url, corpus, out, &qs, passes).await
        }
        Cmd::Leak { n, real } => {
            let scale = match (n, real) {
                (Some(n), false) => Some(Scale::from_n(n)?),
                (None, true) => None,
                _ => bail!("leak: indicare --n <scala> oppure --real"),
            };
            let qs = load_queries(corpus).await?;
            leak::run(&url, corpus, out, scale, &qs).await
        }
        Cmd::IngestUnderLoad { n, seconds } => {
            let qs = load_queries(corpus).await?;
            ingest::run(&url, corpus, out, Scale::from_n(n)?, seconds, &qs).await
        }
    }
}

// ---------------------------------------------------------------- funzioni pure

/// Recall filtrata dell'indice approssimato rispetto alla ricerca esatta: quota degli id
/// esatti presenti nei risultati approssimati. Esatto vuoto: 1,0 (non c'era nulla da
/// trovare), mai NaN.
pub fn recall_vs_exact(exact: &[Uuid], approx: &[Uuid]) -> f64 {
    if exact.is_empty() {
        return 1.0;
    }
    let found = exact.iter().filter(|id| approx.contains(id)).count();
    found as f64 / exact.len() as f64
}

/// Quota di query che restituiscono esattamente `k` risultati.
pub fn returns_k_rate(result_lens: &[usize], k: usize) -> f64 {
    if result_lens.is_empty() {
        return 0.0;
    }
    result_lens.iter().filter(|n| **n == k).count() as f64 / result_lens.len() as f64
}

/// Istruzioni di `sql` separate da `;` a fine riga (le righe di commento non chiudono
/// un'istruzione), come fette del testo statico: prima e dopo la riga `-- HNSW`.
pub fn index_statements(sql: &'static str) -> (Vec<&'static str>, Vec<&'static str>) {
    let (mut schema, mut hnsw) = (Vec::new(), Vec::new());
    let mut in_hnsw = false;
    let mut start: Option<usize> = None;
    let mut pos = 0usize;
    for line in sql.split_inclusive('\n') {
        let t = line.trim();
        let line_start = pos;
        pos += line.len();
        if t == "-- HNSW" {
            in_hnsw = true;
            continue;
        }
        if t.is_empty() || t.starts_with("--") {
            continue;
        }
        let s = *start.get_or_insert(line_start);
        if t.ends_with(';') {
            let stmt = sql[s..pos].trim();
            if in_hnsw {
                hnsw.push(stmt);
            } else {
                schema.push(stmt);
            }
            start = None;
        }
    }
    (schema, hnsw)
}

/// Istruzione di `index.sql` che costruisce l'indice HNSW della scala per tipo di vettore.
fn hnsw_statement(scale: Scale, dense: Dense) -> anyhow::Result<&'static str> {
    let name = hnsw_index_name(scale, dense);
    index_statements(INDEX_SQL)
        .1
        .into_iter()
        .find(|s| s.split_whitespace().any(|w| w == name))
        .with_context(|| format!("sql/index.sql: istruzione dell'indice {name} assente"))
}

/// Gruppo di lingua della query per la configurazione testuale: lingua del testo.
pub fn query_lang(lang: &str) -> anyhow::Result<Lang> {
    match lang {
        "it" | "cross-it-en" => Ok(Lang::Italian),
        "en" | "cross-en-it" => Ok(Lang::English),
        other => bail!("lingua di query non prevista: {other}"),
    }
}

fn ms(us: u64) -> String {
    format!("{:.3}", us as f64 / 1000.0)
}

fn f4(v: f64) -> String {
    format!("{v:.4}")
}

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

// ---------------------------------------------------------------- utilità condivise

pub(crate) fn read(path: &Path) -> anyhow::Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("{} illeggibile", path.display()))
}

/// Riscrive un CSV a header fisso togliendo le righe con la stessa chiave delle nuove
/// (prime `key_cols` colonne) e aggiungendo le nuove: rilanciare una misura la sostituisce.
/// Il writer di `common` rifiuta il file senza `hw.json` accanto.
pub(crate) fn csv_upsert(
    path: &Path,
    header: &[&str],
    key_cols: usize,
    new_rows: &[Vec<String>],
) -> anyhow::Result<()> {
    let mut kept: Vec<Vec<String>> = Vec::new();
    if path.exists() {
        let text = read(path)?;
        let mut lines = text.lines();
        if lines.next().unwrap_or("") != header.join(",") {
            bail!("{}: header diverso da quello atteso", path.display());
        }
        for line in lines.filter(|l| !l.trim().is_empty()) {
            let fields: Vec<String> = line.split(',').map(str::to_string).collect();
            let replaced = new_rows
                .iter()
                .any(|n| n[..key_cols] == fields[..key_cols.min(fields.len())]);
            if !replaced {
                kept.push(fields);
            }
        }
    }
    kept.extend(new_rows.iter().cloned());
    let mut w = common::csv::Writer::create(path, header)?;
    for r in &kept {
        let refs: Vec<&str> = r.iter().map(String::as_str).collect();
        w.write_row(&refs)?;
    }
    Ok(())
}

/// Query ratificata con il vettore di bge-m3 e la lingua del testo.
pub(crate) struct BenchQuery {
    pub oracle: OracleQuery,
    pub vec: Vec<f32>,
    pub lang: Lang,
}

/// Le query ratificate di `oracles/queries.json`, con il vettore calcolato da TEI.
async fn load_queries(corpus: &Path) -> anyhow::Result<Vec<BenchQuery>> {
    let rq = metrics::load_ratified_queries(&read(&corpus.join("oracles/queries.json"))?)?;
    let client = tei::Client::from_env(32)?;
    let mut out = Vec::with_capacity(rq.queries.len());
    for q in rq.queries {
        let vec = client.embed_query(&q.text, Model::BgeM3, true).await?;
        let lang = query_lang(&q.lang)?;
        out.push(BenchQuery {
            oracle: q,
            vec,
            lang,
        });
    }
    println!("query ratificate: {} (escluse {})", out.len(), rq.excluded);
    Ok(out)
}

/// Worker paralleli per gather: enum chiuso, ciascuno con la sua istruzione costante.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Workers {
    Zero,
    Four,
}

impl Workers {
    const ALL: [Workers; 2] = [Workers::Zero, Workers::Four];

    fn n(self) -> u32 {
        match self {
            Workers::Zero => 0,
            Workers::Four => 4,
        }
    }

    fn setup(self) -> &'static str {
        match self {
            Workers::Zero => "SET max_parallel_workers_per_gather = 0",
            Workers::Four => "SET max_parallel_workers_per_gather = 4",
        }
    }
}

/// Istruzione di sessione per 4 worker (default del compose), usata da `ingest`.
pub(crate) fn workers_setup(n: u32) -> &'static str {
    if n == 0 {
        Workers::Zero.setup()
    } else {
        Workers::Four.setup()
    }
}

/// Connessione dedicata con un'istruzione di sessione costante.
pub(crate) async fn conn_with(url: &str, setup: &'static str) -> anyhow::Result<PgConnection> {
    let mut conn = PgConnection::connect(url).await.context("connessione")?;
    sqlx::raw_sql(setup)
        .execute(&mut conn)
        .await
        .context("impostazione di sessione")?;
    Ok(conn)
}

/// Righe della tabella (la scala sintetica ha il suo numero fisso).
pub(crate) async fn count_rows(pool: &PgPool, table: Table) -> anyhow::Result<i64> {
    match table {
        Table::BgeM3 => sqlx::query_scalar("SELECT count(*) FROM chunk_bgem3")
            .fetch_one(pool)
            .await
            .context("count chunk_bgem3"),
        Table::Synth(s) => Ok(s.n() as i64),
    }
}

/// Ferma il comando se la scala ha un indice HNSW: la misura è della ricerca esatta.
async fn ensure_no_hnsw(pool: &PgPool, scale: Scale) -> anyhow::Result<()> {
    for d in Dense::ALL {
        let name = hnsw_index_name(scale, d);
        if leak::index_exists(pool, name).await? {
            bail!(
                "indice {name} presente: lanciare prima drop-hnsw --n {}",
                scale.n()
            );
        }
    }
    Ok(())
}

/// `warmup` query scartate e `n` misurate (le query in ciclo) sulla connessione data.
async fn time_queries(
    conn: &mut PgConnection,
    table: Table,
    strategy: Strategy,
    queries: &[BenchQuery],
    roles: &[String],
    warmup: usize,
    n: usize,
) -> anyhow::Result<Summary> {
    for i in 0..warmup {
        hybrid::sem_on(
            conn,
            table,
            strategy,
            &queries[i % queries.len()].vec,
            roles,
            K,
        )
        .await?;
    }
    let mut rec = Recorder::new();
    for i in 0..n {
        let q = &queries[(warmup + i) % queries.len()];
        let t = Instant::now();
        hybrid::sem_on(conn, table, strategy, &q.vec, roles, K).await?;
        rec.record_us(t.elapsed().as_micros() as u64);
    }
    Ok(rec.summary())
}

/// Piano `EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)` della ricerca densa, come JSON.
async fn explain(
    conn: &mut PgConnection,
    table: Table,
    strategy: Strategy,
    query_vec: &[f32],
    roles: &[String],
) -> anyhow::Result<serde_json::Value> {
    let sql = hybrid::sem_explain_sql(table, strategy)
        .with_context(|| format!("piano non previsto su {}", table.name()))?;
    let row = hybrid::bind_dense(sqlx::query(sql), strategy.dense(), query_vec)
        .bind(roles)
        .bind(K)
        .fetch_one(conn)
        .await
        .context("EXPLAIN")?;
    // La colonna di EXPLAIN (FORMAT JSON) è di tipo json: testo in entrambi i formati.
    let text: String = row.try_get_unchecked(0)?;
    serde_json::from_str(&text).context("piano JSON")
}

fn write_plan(out: &Path, file: &str, plan: &serde_json::Value) -> anyhow::Result<()> {
    let dir = out.join("plans");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(file), serde_json::to_string_pretty(plan)? + "\n")?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn threshold_row(
    scale: Scale,
    method: &str,
    w: Workers,
    share: f64,
    run: u32,
    n: usize,
    s: &Summary,
) -> Vec<String> {
    vec![
        scale.n().to_string(),
        method.to_string(),
        w.n().to_string(),
        format!("{share:.2}"),
        run.to_string(),
        n.to_string(),
        ms(s.p50_us),
        ms(s.p95_us),
        ms(s.p99_us),
    ]
}

const THRESHOLD_HEADER: [&str; 9] = [
    "chunks",
    "method",
    "parallel_workers",
    "selectivity",
    "run",
    "queries",
    "p50_ms",
    "p95_ms",
    "p99_ms",
];

// ---------------------------------------------------------------- prepare, synth

async fn prepare(pool: &PgPool) -> anyhow::Result<()> {
    db::apply_schema(pool).await?;
    let (schema, _) = index_statements(INDEX_SQL);
    for stmt in &schema {
        sqlx::raw_sql(*stmt)
            .execute(pool)
            .await
            .context("sql/index.sql")?;
    }
    println!(
        "prepare: {} istruzioni di sql/index.sql applicate",
        schema.len()
    );
    Ok(())
}

async fn synth_cmd(url: &str, corpus: &Path, scale: Scale) -> anyhow::Result<()> {
    let pool = db::connect(url).await?;
    prepare(&pool).await?;
    ensure_no_hnsw(&pool, scale).await?;
    let acl = read(&corpus.join("acl.json"))?;
    let t = Instant::now();
    synth::generate(&pool, scale, SEED, &acl).await?;
    let secs = t.elapsed().as_secs_f64();
    let expected = leak::full_expected(&pool, corpus).await?;
    db::check_doc_acl(&pool, &expected).await?;
    let table = Table::Synth(scale);
    let mut shares = Vec::new();
    for (role, target) in SYNTH_ROLES {
        let s = hybrid::role_share(&pool, table, role).await?;
        if (s - target).abs() > 0.005 {
            bail!("{role}: selettività {s:.4}, attesa {target}");
        }
        shares.push(format!("{role}={s:.4}"));
    }
    println!(
        "synth {}: {} righe, {} documenti in {secs:.1} s; doc_acl = matrice attesa; {}",
        scale.n(),
        scale.n(),
        scale.n_docs(),
        shares.join(" ")
    );
    Ok(())
}

// ---------------------------------------------------------------- threshold

#[allow(clippy::too_many_arguments)]
async fn threshold(
    url: &str,
    out: &Path,
    scale: Scale,
    queries: &[BenchQuery],
    runs: u32,
    n: usize,
    warmup: usize,
) -> anyhow::Result<()> {
    let pool = db::connect(url).await?;
    ensure_no_hnsw(&pool, scale).await?;
    let client = tei::Client::from_env(32)?;
    let table = Table::Synth(scale);
    let mut rows = Vec::new();
    let mut lat_rows = Vec::new();
    for run in 1..=runs {
        for (role, share) in SYNTH_ROLES {
            let roles = vec![role.to_string()];
            for w in Workers::ALL {
                let mut conn = conn_with(url, w.setup()).await?;
                let s = time_queries(
                    &mut conn,
                    table,
                    Strategy::Exact,
                    queries,
                    &roles,
                    warmup,
                    n,
                )
                .await?;
                rows.push(threshold_row(scale, "exact", w, share, run, n, &s));
                if run == 1 {
                    let plan =
                        explain(&mut conn, table, Strategy::Exact, &queries[0].vec, &roles).await?;
                    if plan.to_string().contains("hnsw") {
                        bail!("piano esatto con un indice HNSW: misura non valida");
                    }
                    write_plan(
                        out,
                        &format!("{}-exact-w{}-{role}.json", scale.n(), w.n()),
                        &plan,
                    )?;
                }
                conn.close().await.ok();
                println!(
                    "threshold {} run {run} {role} w{}: p95 {} ms",
                    scale.n(),
                    w.n(),
                    ms(s.p95_us)
                );
            }
        }
        // Latenza end-to-end: embedding della query, poi SQL esatta (sel100, 4 worker).
        let mut conn = conn_with(url, Workers::Four.setup()).await?;
        let roles = vec!["sel100".to_string()];
        let (mut emb, mut sql, mut e2e) = (Recorder::new(), Recorder::new(), Recorder::new());
        for i in 0..(warmup + n) {
            let q = &queries[i % queries.len()];
            let t0 = Instant::now();
            let v = client
                .embed_query(&q.oracle.text, Model::BgeM3, true)
                .await?;
            let t1 = Instant::now();
            hybrid::sem_on(&mut conn, table, Strategy::Exact, &v, &roles, K).await?;
            let t2 = Instant::now();
            if i >= warmup {
                emb.record_us((t1 - t0).as_micros() as u64);
                sql.record_us((t2 - t1).as_micros() as u64);
                e2e.record_us((t2 - t0).as_micros() as u64);
            }
        }
        conn.close().await.ok();
        for (stage, r) in [("embed_query", &emb), ("sql", &sql), ("end_to_end", &e2e)] {
            let s = r.summary();
            lat_rows.push(vec![
                scale.n().to_string(),
                stage.to_string(),
                run.to_string(),
                ms(s.p50_us),
                ms(s.p95_us),
                ms(s.p99_us),
            ]);
        }
    }
    csv_upsert(&out.join("threshold.csv"), &THRESHOLD_HEADER, 5, &rows)?;
    csv_upsert(
        &out.join("latency.csv"),
        &["chunks", "stage", "run", "p50_ms", "p95_ms", "p99_ms"],
        3,
        &lat_rows,
    )?;
    println!(
        "threshold {}: {} righe, latency {} righe",
        scale.n(),
        rows.len(),
        lat_rows.len()
    );
    Ok(())
}

// ---------------------------------------------------------------- hnsw

async fn drop_hnsw(pool: &PgPool, scale: Scale) -> anyhow::Result<()> {
    for d in Dense::ALL {
        sqlx::raw_sql(drop_hnsw_sql(scale, d))
            .execute(pool)
            .await
            .context("drop dell'indice HNSW")?;
    }
    println!("drop-hnsw {}: indici HNSW rimossi", scale.n());
    Ok(())
}

/// Costruisce l'indice HNSW della scala con l'istruzione di `index.sql`: secondi e MiB.
async fn build_index(
    url: &str,
    pool: &PgPool,
    scale: Scale,
    dense: Dense,
) -> anyhow::Result<(f64, f64)> {
    let stmt = hnsw_statement(scale, dense)?;
    let mut conn = PgConnection::connect(url).await?;
    for s in [
        // La build parallela tiene il grafo in memoria condivisa: `shm_size` del container è 1 GB.
        "SET maintenance_work_mem = '768MB'",
        "SET max_parallel_maintenance_workers = 4",
    ] {
        sqlx::raw_sql(s).execute(&mut conn).await?;
    }
    let t = Instant::now();
    sqlx::raw_sql(stmt)
        .execute(&mut conn)
        .await
        .with_context(|| format!("costruzione di {}", hnsw_index_name(scale, dense)))?;
    let secs = t.elapsed().as_secs_f64();
    conn.close().await.ok();
    let mb: f64 =
        sqlx::query_scalar("SELECT pg_relation_size(to_regclass($1))::float8 / 1048576.0")
            .bind(hnsw_index_name(scale, dense))
            .fetch_one(pool)
            .await
            .context("dimensione dell'indice")?;
    Ok((secs, mb))
}

#[allow(clippy::too_many_arguments)]
async fn hnsw(
    url: &str,
    out: &Path,
    scale: Scale,
    queries: &[BenchQuery],
    runs: u32,
    n: usize,
    warmup: usize,
) -> anyhow::Result<()> {
    let pool = db::connect(url).await?;
    drop_hnsw(&pool, scale).await?;
    let table = Table::Synth(scale);

    // Riferimento esatto, senza indici HNSW.
    let mut exact: BTreeMap<&str, Vec<Vec<Uuid>>> = BTreeMap::new();
    let mut conn = conn_with(url, Workers::Four.setup()).await?;
    for (role, _) in SYNTH_ROLES {
        let roles = vec![role.to_string()];
        let mut per_q = Vec::with_capacity(queries.len());
        for q in queries {
            let hits = hybrid::sem_on(&mut conn, table, Strategy::Exact, &q.vec, &roles, K).await?;
            per_q.push(hits.into_iter().map(|h| h.chunk_id).collect());
        }
        exact.insert(role, per_q);
    }
    conn.close().await.ok();

    let mut rec_rows = Vec::new();
    let mut thr_rows = Vec::new();
    for dense in Dense::ALL {
        let name = hnsw_index_name(scale, dense);
        let (build_s, mb) = build_index(url, &pool, scale, dense).await?;
        println!(
            "hnsw {} {}: costruito in {build_s:.1} s, {mb:.1} MiB",
            scale.n(),
            dense.name()
        );
        for (d, p) in leak::hnsw_grid() {
            if d != dense {
                continue;
            }
            for (role, share) in SYNTH_ROLES {
                let roles = vec![role.to_string()];
                let (mut recalls, mut lens) = (Vec::new(), Vec::new());
                for (qi, q) in queries.iter().enumerate() {
                    let hits = match dense {
                        Dense::Vector => {
                            hybrid::hnsw_search(&pool, table, &q.vec, &roles, K, p).await?
                        }
                        Dense::Half => {
                            hybrid::hnsw_search_dense(&pool, table, dense, &q.vec, &roles, K, p)
                                .await?
                        }
                    };
                    let ids: Vec<Uuid> = hits.iter().map(|h| h.chunk_id).collect();
                    recalls.push(recall_vs_exact(&exact[role][qi], &ids));
                    lens.push(ids.len());
                }
                rec_rows.push(vec![
                    scale.n().to_string(),
                    dense.name().to_string(),
                    p.ef_search.to_string(),
                    p.iterative_scan.setting().to_string(),
                    format!("{share:.2}"),
                    f4(mean(&recalls)),
                    f4(returns_k_rate(&lens, K as usize)),
                    format!("{build_s:.1}"),
                    format!("{mb:.1}"),
                ]);
            }
            // Il piano deve usare l'indice HNSW della combinazione.
            let mut conn = PgConnection::connect(url).await?;
            hybrid::set_hnsw(&mut conn, p, false).await?;
            let plan = explain(
                &mut conn,
                table,
                Strategy::Ann(dense),
                &queries[0].vec,
                &["sel100".to_string()],
            )
            .await?;
            conn.close().await.ok();
            if !plan.to_string().contains(name) {
                bail!("{name} non usato con {p:?}: misura non valida");
            }
        }
        // Latenza HNSW nella griglia di threshold (ef_search 100, relaxed_order).
        let p = HnswParams {
            ef_search: 100,
            iterative_scan: IterativeScan::RelaxedOrder,
        };
        let method = format!("hnsw_{}", dense.name());
        for run in 1..=runs {
            for (role, share) in SYNTH_ROLES {
                let roles = vec![role.to_string()];
                for w in Workers::ALL {
                    let mut conn = conn_with(url, w.setup()).await?;
                    hybrid::set_hnsw(&mut conn, p, false).await?;
                    let s = time_queries(
                        &mut conn,
                        table,
                        Strategy::Ann(dense),
                        queries,
                        &roles,
                        warmup,
                        n,
                    )
                    .await?;
                    thr_rows.push(threshold_row(scale, &method, w, share, run, n, &s));
                    if run == 1 {
                        let plan = explain(
                            &mut conn,
                            table,
                            Strategy::Ann(dense),
                            &queries[0].vec,
                            &roles,
                        )
                        .await?;
                        if !plan.to_string().contains(name) {
                            bail!("{name} non usato nella latenza: misura non valida");
                        }
                        write_plan(
                            out,
                            &format!("{}-{method}-w{}-{role}.json", scale.n(), w.n()),
                            &plan,
                        )?;
                    }
                    conn.close().await.ok();
                }
            }
        }
    }
    csv_upsert(
        &out.join("recall.csv"),
        &[
            "chunks",
            "index_type",
            "ef_search",
            "iterative_scan",
            "selectivity",
            "recall_vs_exact",
            "returns_k_rate",
            "build_s",
            "index_mb",
        ],
        5,
        &rec_rows,
    )?;
    csv_upsert(&out.join("threshold.csv"), &THRESHOLD_HEADER, 5, &thr_rows)?;
    println!(
        "hnsw {}: recall {} righe, latenza {} righe",
        scale.n(),
        rec_rows.len(),
        thr_rows.len()
    );
    Ok(())
}

// ---------------------------------------------------------------- hybrid

/// Una lista di risultati di una modalità per una query (passata 1).
struct ModeHits {
    ids: Vec<Uuid>,
    keys: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Mode {
    Dense,
    Lexical,
    Hybrid,
}

impl Mode {
    const ALL: [Mode; 3] = [Mode::Dense, Mode::Lexical, Mode::Hybrid];

    fn name(self) -> &'static str {
        match self {
            Mode::Dense => "dense",
            Mode::Lexical => "lexical",
            Mode::Hybrid => "hybrid",
        }
    }
}

async fn run_mode(
    pool: &PgPool,
    mode: Mode,
    q: &BenchQuery,
    roles: &[String],
) -> anyhow::Result<ModeHits> {
    let table = Table::BgeM3;
    let pairs: Vec<(Uuid, String)> = match mode {
        Mode::Dense => hybrid::semantic_search(pool, table, &q.vec, roles, K)
            .await?
            .into_iter()
            .map(|h| (h.chunk_id, h.doc_key))
            .collect(),
        Mode::Lexical => hybrid::lexical_search(pool, table, &q.oracle.text, q.lang, roles, K)
            .await?
            .into_iter()
            .map(|h| (h.chunk_id, h.doc_key))
            .collect(),
        Mode::Hybrid => {
            hybrid::hybrid_search(pool, table, &q.vec, &q.oracle.text, q.lang, roles, K)
                .await?
                .into_iter()
                .map(|h| (h.chunk_id, h.doc_key))
                .collect()
        }
    };
    Ok(ModeHits {
        ids: pairs.iter().map(|p| p.0).collect(),
        keys: pairs.into_iter().map(|p| p.1).collect(),
    })
}

async fn hybrid_cmd(
    url: &str,
    corpus: &Path,
    out: &Path,
    queries: &[BenchQuery],
    passes: u32,
) -> anyhow::Result<()> {
    let pool = db::connect(url).await?;
    let expected = leak::full_expected(&pool, corpus).await?;
    db::check_doc_acl(&pool, &expected).await?;
    let all = db::fetch_chunks(&pool, Model::BgeM3).await?;
    if all.is_empty() {
        bail!("chunk_bgem3 vuota: lanciare prima load del piano 01-10");
    }
    let by_id: std::collections::HashMap<Uuid, &Chunk> =
        all.iter().map(|(id, c)| (*id, c)).collect();
    let all_rows: Vec<Chunk> = all.iter().map(|(_, c)| c.clone()).collect();

    let mut first: BTreeMap<(Mode, usize), ModeHits> = BTreeMap::new();
    let mut lat: BTreeMap<(Mode, &str), Recorder> = BTreeMap::new();
    for round in 1..=passes {
        for (qi, q) in queries.iter().enumerate() {
            let roles = vec![q.oracle.role_profile.clone()];
            for mode in Mode::ALL {
                let t = Instant::now();
                let h = run_mode(&pool, mode, q, &roles).await?;
                let us = t.elapsed().as_micros() as u64;
                if round > 1 {
                    for g in [q.oracle.lang_group(), "all"] {
                        lat.entry((mode, g)).or_default().record_us(us);
                    }
                }
                if round == 1 {
                    first.insert((mode, qi), h);
                }
            }
        }
    }

    let mut rows = Vec::new();
    let mut leaked = 0usize;
    let mut summary = Vec::new();
    for mode in Mode::ALL {
        let mut per_group: BTreeMap<&str, (Vec<f64>, Vec<f64>)> = BTreeMap::new();
        for (qi, q) in queries.iter().enumerate() {
            let h = &first[&(mode, qi)];
            let keys: Vec<&str> = h.keys.iter().map(String::as_str).collect();
            leaked += leak::denied_in_topk(&keys, &q.oracle.role_profile, &expected);
            let total = metrics::relevant_chunks(&all_rows, &q.oracle.expected_passages)
                .into_iter()
                .filter(|r| *r)
                .count();
            if q.oracle.no_answer || total == 0 {
                continue;
            }
            let hit_rows: Vec<Chunk> = h
                .ids
                .iter()
                .map(|id| by_id.get(id).map(|c| (*c).clone()))
                .collect::<Option<_>>()
                .context("risultato assente da chunk_bgem3")?;
            let rel = metrics::relevant_chunks(&hit_rows, &q.oracle.expected_passages);
            let r10 = metrics::recall_at_k(&rel, total, 10);
            let n10 = metrics::ndcg_at_k(&rel, total, 10);
            for g in [q.oracle.lang_group(), "all"] {
                let e = per_group.entry(g).or_default();
                e.0.push(r10);
                e.1.push(n10);
            }
        }
        for g in ["it", "en", "cross", "all"] {
            let (r, nd) = per_group.get(g).cloned().unwrap_or_default();
            let p95 = lat
                .get(&(mode, g))
                .map(|r| ms(r.summary().p95_us))
                .unwrap_or_default();
            rows.push(vec![
                mode.name().to_string(),
                g.to_string(),
                r.len().to_string(),
                f4(mean(&r)),
                f4(mean(&nd)),
                p95.clone(),
            ]);
            if g == "all" {
                summary.push(format!(
                    "{} recall@10 {} nDCG@10 {} p95 {p95} ms",
                    mode.name(),
                    f4(mean(&r)),
                    f4(mean(&nd))
                ));
            }
        }
    }
    if leaked > 0 {
        bail!("{leaked} risultati non visibili al ruolo della query: misura rifiutata");
    }
    csv_upsert(
        &out.join("hybrid.csv"),
        &[
            "mode",
            "lang",
            "queries",
            "recall_at_10",
            "ndcg_at_10",
            "p95_ms",
        ],
        2,
        &rows,
    )?;

    // I-7: il prodotto interno non cambia il ranking dei top-10 (vettori normalizzati).
    let mut differ = Vec::new();
    for (qi, q) in queries.iter().enumerate() {
        let roles = vec![q.oracle.role_profile.clone()];
        let ip: Vec<Uuid> = sqlx::query_scalar(
            "SELECT c.id FROM chunk_bgem3 c \
             WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2)) \
             ORDER BY c.emb <#> $1, c.id LIMIT $3",
        )
        .bind(pgvector::Vector::from(q.vec.clone()))
        .bind(&roles)
        .bind(K)
        .fetch_all(&pool)
        .await
        .context("ranking per prodotto interno")?;
        if ip != first[&(Mode::Dense, qi)].ids {
            differ.push(q.oracle.oracle_id.clone());
        }
    }
    let norms = sqlx::query(
        "SELECT min(vector_norm(emb))::float8 AS lo, max(vector_norm(emb))::float8 AS hi \
         FROM chunk_bgem3",
    )
    .fetch_one(&pool)
    .await
    .context("norme")?;
    let (lo, hi): (f64, f64) = (norms.try_get("lo")?, norms.try_get("hi")?);
    let qn: Vec<f64> = queries
        .iter()
        .map(|q| {
            q.vec
                .iter()
                .map(|x| f64::from(*x).powi(2))
                .sum::<f64>()
                .sqrt()
        })
        .collect();
    let share = leak::restricted_only_selectivity(&pool, Table::BgeM3).await?;
    println!("hybrid: {}", summary.join("; "));
    println!(
        "metrica: top-10 coseno vs prodotto interno diversi in {} query su {} {:?}; norme dei documenti [{lo:.6}, {hi:.6}], delle query [{:.6}, {:.6}]",
        differ.len(),
        queries.len(),
        differ,
        qn.iter().cloned().fold(f64::INFINITY, f64::min),
        qn.iter().cloned().fold(0.0, f64::max)
    );
    println!("selettività restricted-only sul corpus reale: {share:.4}");
    Ok(())
}

// ---- Indici HNSW per scala: nomi e rimozione da enum chiuso ---------------------------------

/// Nome dell'indice HNSW della scala per tipo di vettore (istruzioni in coda a `sql/index.sql`).
pub fn hnsw_index_name(scale: Scale, dense: Dense) -> &'static str {
    match (scale, dense) {
        (Scale::N10k, Dense::Vector) => "chunk_synth_10000_emb_hnsw",
        (Scale::N10k, Dense::Half) => "chunk_synth_10000_embh_hnsw",
        (Scale::N50k, Dense::Vector) => "chunk_synth_50000_emb_hnsw",
        (Scale::N50k, Dense::Half) => "chunk_synth_50000_embh_hnsw",
        (Scale::N100k, Dense::Vector) => "chunk_synth_100000_emb_hnsw",
        (Scale::N100k, Dense::Half) => "chunk_synth_100000_embh_hnsw",
        (Scale::N300k, Dense::Vector) => "chunk_synth_300000_emb_hnsw",
        (Scale::N300k, Dense::Half) => "chunk_synth_300000_embh_hnsw",
    }
}

/// Rimozione dell'indice HNSW della scala per tipo di vettore.
fn drop_hnsw_sql(scale: Scale, dense: Dense) -> &'static str {
    match (scale, dense) {
        (Scale::N10k, Dense::Vector) => {
            "DROP INDEX CONCURRENTLY IF EXISTS chunk_synth_10000_emb_hnsw"
        }
        (Scale::N10k, Dense::Half) => {
            "DROP INDEX CONCURRENTLY IF EXISTS chunk_synth_10000_embh_hnsw"
        }
        (Scale::N50k, Dense::Vector) => {
            "DROP INDEX CONCURRENTLY IF EXISTS chunk_synth_50000_emb_hnsw"
        }
        (Scale::N50k, Dense::Half) => {
            "DROP INDEX CONCURRENTLY IF EXISTS chunk_synth_50000_embh_hnsw"
        }
        (Scale::N100k, Dense::Vector) => {
            "DROP INDEX CONCURRENTLY IF EXISTS chunk_synth_100000_emb_hnsw"
        }
        (Scale::N100k, Dense::Half) => {
            "DROP INDEX CONCURRENTLY IF EXISTS chunk_synth_100000_embh_hnsw"
        }
        (Scale::N300k, Dense::Vector) => {
            "DROP INDEX CONCURRENTLY IF EXISTS chunk_synth_300000_emb_hnsw"
        }
        (Scale::N300k, Dense::Half) => {
            "DROP INDEX CONCURRENTLY IF EXISTS chunk_synth_300000_embh_hnsw"
        }
    }
}

/// Supporto dei test d'integrazione `#[ignore]` (richiedono `DATABASE_URL` verso il Postgres
/// del compose). Lancio dal runner, dentro una sessione:
/// `cargo test -p s03-pgvector --bin index-bench -- --ignored --test-threads=1 <nome>`.
/// Ogni test lavora in uno schema temporaneo `test_<pid>_<uuid>` con le tabelle di
/// `sql/schema.sql` (piano 01-10) e, dove serve, di `sql/index.sql`, e lo elimina alla fine.
#[cfg(test)]
pub(crate) mod testdb {

    use std::path::{Path, PathBuf};

    use s03_pgvector::metrics::{self, Visibility};
    use sqlx::postgres::PgPoolOptions;
    use sqlx::{AssertSqlSafe, Executor, PgPool};
    use uuid::Uuid;

    pub const DIM: usize = 1024;

    pub fn manifest_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    pub fn acl_json() -> String {
        let p = manifest_dir().join("../data/corpus/acl.json");
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }

    /// Matrice attesa per un insieme di documenti `(doc_key, classification)`, da acl.json.
    pub fn visibility_for(docs: &[(&str, &str)]) -> Visibility {
        let manifest: Vec<serde_json::Value> = docs
            .iter()
            .map(|(k, c)| serde_json::json!({"doc_id": k, "classification": c}))
            .collect();
        metrics::expected_visibility(&acl_json(), &serde_json::to_string(&manifest).unwrap())
            .expect("matrice attesa da acl.json")
    }

    pub struct Scratch {
        pub pool: PgPool,
        admin: PgPool,
        schema: String,
    }

    impl Scratch {
        pub async fn new() -> Self {
            let url = std::env::var("DATABASE_URL").expect("DATABASE_URL richiesta dai test db");
            let admin = PgPoolOptions::new()
                .max_connections(1)
                .connect(&url)
                .await
                .expect("connessione");
            sqlx::raw_sql("CREATE EXTENSION IF NOT EXISTS vector SCHEMA public")
                .execute(&admin)
                .await
                .expect("estensione vector");
            // Nome generato dal test (pid + uuid), nessun valore in ingresso: DDL non parametrizzabile.
            let schema = format!("test_{}_{}", std::process::id(), Uuid::new_v4().simple());
            sqlx::raw_sql(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
                .execute(&admin)
                .await
                .expect("schema temporaneo");
            let sp: std::sync::Arc<str> = format!("SET search_path TO {schema}, public").into();
            let pool = PgPoolOptions::new()
                .max_connections(2)
                .after_connect(move |conn, _meta| {
                    let sp = sp.clone();
                    Box::pin(async move {
                        conn.execute(sqlx::raw_sql(AssertSqlSafe(sp))).await?;
                        Ok(())
                    })
                })
                .connect(&url)
                .await
                .expect("pool con search_path");
            let ddl = std::fs::read_to_string(manifest_dir().join("sql/schema.sql"))
                .expect("sql/schema.sql del piano 01-10");
            sqlx::raw_sql(AssertSqlSafe(ddl))
                .execute(&pool)
                .await
                .expect("schema.sql");
            Scratch {
                pool,
                admin,
                schema,
            }
        }

        /// Applica `sql/index.sql` istruzione per istruzione: `CREATE INDEX CONCURRENTLY` non
        /// può stare in un blocco transazionale né in una query multi-istruzione.
        pub async fn apply_index_sql(&self) {
            let p = manifest_dir().join("sql/index.sql");
            let ddl = std::fs::read_to_string(&p)
                .unwrap_or_else(|e| panic!("sql/index.sql del piano 01-14 ({}): {e}", p.display()));
            for stmt in split_sql(&ddl) {
                sqlx::raw_sql(AssertSqlSafe(stmt.clone()))
                    .execute(&self.pool)
                    .await
                    .unwrap_or_else(|e| panic!("index.sql: {stmt}: {e}"));
            }
        }

        pub async fn drop(self) {
            self.pool.close().await;
            let _ = sqlx::raw_sql(AssertSqlSafe(format!(
                "DROP SCHEMA {} CASCADE",
                self.schema
            )))
            .execute(&self.admin)
            .await;
        }

        /// Documento con le righe di `doc_acl` derivate da acl.json per la sua classificazione.
        pub async fn doc(&self, doc_key: &str, classification: &str) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO doc (id, doc_key, classification) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(doc_key)
                .bind(classification)
                .execute(&self.pool)
                .await
                .expect("insert doc");
            let vis = visibility_for(&[(doc_key, classification)]);
            for (role, docs) in &vis {
                if docs.contains(doc_key) {
                    sqlx::query("INSERT INTO doc_acl (doc_id, role) VALUES ($1, $2)")
                        .bind(id)
                        .bind(role)
                        .execute(&self.pool)
                        .await
                        .expect("insert doc_acl");
                }
            }
            id
        }

        /// Chunk nella tabella del modello scelto (bge-m3, `RISULTATI-embedding.md` §Esito).
        pub async fn chunk(&self, doc_id: Uuid, body: &str, emb: Vec<f32>) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO chunk_bgem3 (id, doc_id, page, section_path, char_start, char_end, lang, body, emb) \
                 VALUES ($1, $2, 1, NULL, 0, 4, 'italian'::regconfig, $3, $4)",
            )
            .bind(id)
            .bind(doc_id)
            .bind(body)
            .bind(pgvector::Vector::from(emb))
            .execute(&self.pool)
            .await
            .expect("insert chunk");
            id
        }

        /// Molti chunk dello stesso documento in una sola istruzione.
        pub async fn chunks_bulk(&self, doc_id: Uuid, body: &str, embs: Vec<Vec<f32>>) {
            let n = embs.len();
            let ids: Vec<Uuid> = (0..n).map(|_| Uuid::new_v4()).collect();
            let docs = vec![doc_id; n];
            let bodies = vec![body.to_string(); n];
            let vecs: Vec<pgvector::Vector> =
                embs.into_iter().map(pgvector::Vector::from).collect();
            sqlx::query(
                "INSERT INTO chunk_bgem3 (id, doc_id, page, section_path, char_start, char_end, lang, body, emb) \
                 SELECT u.id, u.doc_id, 1, NULL, 0, 4, 'italian'::regconfig, u.body, u.emb \
                 FROM UNNEST($1::uuid[], $2::uuid[], $3::text[], $4::vector[]) AS u(id, doc_id, body, emb)",
            )
            .bind(ids)
            .bind(docs)
            .bind(bodies)
            .bind(vecs)
            .execute(&self.pool)
            .await
            .expect("insert chunk a lotti");
        }
    }

    /// Istruzioni SQL separate da `;` a fine riga, commenti `--` di riga esclusi.
    pub fn split_sql(ddl: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        for line in ddl.lines() {
            let t = line.trim();
            if t.starts_with("--") || t.is_empty() {
                continue;
            }
            cur.push_str(line);
            cur.push('\n');
            if t.ends_with(';') {
                out.push(std::mem::take(&mut cur));
            }
        }
        if !cur.trim().is_empty() {
            out.push(cur);
        }
        out
    }

    /// Vettore unitario lungo l'asse 0 (il vettore di query).
    pub fn query_vec() -> Vec<f32> {
        axis(0)
    }

    pub fn axis(i: usize) -> Vec<f32> {
        let mut v = vec![0.0; DIM];
        v[i] = 1.0;
        v
    }

    /// Vettore lontano dalla query, distanza coseno crescente con `i`.
    pub fn far(i: usize) -> Vec<f32> {
        let mut v = vec![0.0; DIM];
        v[1] = 1.0;
        v[0] = -(i as f32) * 0.01;
        v
    }

    /// Vettore vicinissimo alla query, distinto per `i` (rumore deterministico).
    pub fn near(i: usize) -> Vec<f32> {
        let mut v = query_vec();
        v[2 + (i % (DIM - 2))] = 0.001 * (1 + i / (DIM - 2)) as f32;
        v
    }

    pub fn roles(r: &[&str]) -> Vec<String> {
        r.iter().map(|s| s.to_string()).collect()
    }

    pub fn union(a: Visibility, b: Visibility) -> Visibility {
        let mut out = a;
        for (role, docs) in b {
            out.entry(role).or_default().extend(docs);
        }
        out
    }

    pub fn repo_root() -> PathBuf {
        manifest_dir().join("../..")
    }

    pub fn read(p: &Path) -> String {
        std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::testdb::{manifest_dir, read, repo_root};
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use std::process::Command;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    // TEST-13 (RISK-7): recall filtrata contro l'esatto e tasso di k risultati, valori noti.
    #[test]
    fn test13_recall_vs_exact_known_values() {
        let (a, b, c, d, x, y) = (id(1), id(2), id(3), id(4), id(90), id(91));
        assert!((recall_vs_exact(&[a, b, c, d], &[a, c, x, y]) - 0.5).abs() < 1e-12);
        assert_eq!(recall_vs_exact(&[a, b, c, d], &[]), 0.0);
        let empty = recall_vs_exact(&[], &[a]);
        assert!(
            empty.is_finite() && (0.0..=1.0).contains(&empty),
            "esatto vuoto: valore documentato in [0, 1], mai NaN (got {empty})"
        );
        assert!((returns_k_rate(&[10, 10, 7, 10], 10) - 0.75).abs() < 1e-12);
        assert_eq!(returns_k_rate(&[10, 10], 10), 1.0);
    }

    /// Codice dei file di `src/bin/index-bench/`, senza il modulo di test.
    fn bench_sources() -> Vec<(String, String)> {
        let dir = manifest_dir().join("src/bin/index-bench");
        let mut out = Vec::new();
        for e in std::fs::read_dir(&dir).expect("src/bin/index-bench") {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "rs") {
                let src = read(&p);
                let code = src.split("#[cfg(test)]").next().unwrap().to_owned();
                out.push((p.file_name().unwrap().to_string_lossy().into_owned(), code));
            }
        }
        out
    }

    // TEST-9 (RISK-4): nessun SQL composto con format!/concat!, parametri con bind.
    #[test]
    fn test9_sql_only_bound_no_composed_strings() {
        // Parole chiave SQL in maiuscolo, come le scrivono le query del crate.
        const KW: [&str; 8] = [
            "SELECT ", "INSERT ", "UPDATE ", "DELETE ", "WITH ", "CREATE ", "SET ", " FROM ",
        ];
        let mut binds = 0;
        for (f, code) in bench_sources() {
            assert!(
                !code.contains("concat!"),
                "{f}: concat! vietato (tabelle da enum chiuso)"
            );
            for (n, line) in code.lines().enumerate() {
                let t = line.trim_start();
                if t.starts_with("//") {
                    continue;
                }
                if t.contains("format!") {
                    assert!(
                        !KW.iter().any(|k| t.contains(k)),
                        "{f}:{}: SQL composto con format!: {t}",
                        n + 1
                    );
                }
            }
            binds += code.matches(".bind(").count();
        }
        assert!(
            binds > 0,
            "le query di index-bench legano i parametri con bind"
        );
    }

    // TEST-9 (RISK-4): lo scanner di sicurezza esce 0 su src/bin e sql.
    #[test]
    #[ignore = "richiede python e lo scanner del repo (skills/security-scanner)"]
    fn test9_security_scanner_clean() {
        let root = repo_root();
        let st = Command::new("python")
            .arg(root.join("skills/security-scanner/security_scan.py"))
            .arg(manifest_dir().join("src/bin"))
            .arg(manifest_dir().join("sql"))
            .status()
            .expect("python");
        assert!(st.success(), "security_scan.py: {st}");
    }

    // ---- Evidenze del Task 2 (dopo le misure) ------------------------------------------------

    fn results() -> std::path::PathBuf {
        manifest_dir().join("results/index")
    }

    /// CSV come header + righe (campi separati da virgola, nessun campo quotato nei file).
    fn csv(name: &str) -> (Vec<String>, Vec<Vec<String>>) {
        let s = read(&results().join(name));
        let mut lines = s.lines().filter(|l| !l.trim().is_empty());
        let header = lines
            .next()
            .unwrap_or_else(|| panic!("{name} vuoto"))
            .split(',')
            .map(str::to_owned)
            .collect();
        let rows = lines
            .map(|l| l.split(',').map(str::to_owned).collect())
            .collect();
        (header, rows)
    }

    const SCALES: [&str; 4] = ["10000", "50000", "100000", "300000"];
    const LEAK_ROLES: [&str; 9] = [
        "admin",
        "manager",
        "employee",
        "guest",
        "restricted-only",
        "sel100",
        "sel25",
        "sel5",
        "sel1",
    ];

    // TEST-5 (RISK-2): leak.csv completo per scala × config × ruolo × parte, e 0 negati.
    #[test]
    #[ignore = "evidenza del Task 2: richiede results/index/leak.csv e recall.csv dopo le misure"]
    fn test5_leak_csv_complete_and_zero() {
        let (h, rows) = csv("leak.csv");
        assert_eq!(
            h,
            [
                "chunks",
                "config",
                "role",
                "part",
                "queries",
                "denied_in_topk"
            ]
        );
        let leaks: Vec<_> = rows.iter().filter(|r| r[5] != "0").collect();
        assert!(
            leaks.is_empty(),
            "chunk negati nei top-k (gate fallito, RISULTATI deve dichiararlo): {leaks:?}"
        );
        let configs: BTreeSet<&str> = rows.iter().map(|r| r[1].as_str()).collect();
        assert!(
            configs.iter().any(|c| c.contains("exact")),
            "manca la configurazione esatta: {configs:?}"
        );
        let (_, rec) = csv("recall.csv");
        let hnsw: BTreeSet<(&str, &str, &str)> = rec
            .iter()
            .map(|r| (r[1].as_str(), r[2].as_str(), r[3].as_str()))
            .collect();
        assert!(
            configs.len() > hnsw.len(),
            "attese la configurazione esatta più ogni combinazione HNSW di recall.csv ({}), trovate {configs:?}",
            hnsw.len()
        );
        let mut seen: BTreeMap<(&str, &str, &str, &str), u64> = BTreeMap::new();
        for r in &rows {
            let q: u64 = r[4].parse().expect("queries intero");
            *seen.entry((&r[0], &r[1], &r[2], &r[3])).or_default() += q;
        }
        let mut holes = Vec::new();
        for s in SCALES {
            for c in &configs {
                for role in LEAK_ROLES {
                    for part in ["sem", "lex"] {
                        if seen.get(&(s, c, role, part)).copied().unwrap_or(0) == 0 {
                            holes.push(format!("{s}/{c}/{role}/{part}"));
                        }
                    }
                }
            }
        }
        assert!(holes.is_empty(), "griglia del leak incompleta: {holes:?}");
    }

    // TEST-15 (RISK-9): ingestion sotto carico per 5 minuti, righe al secondo, errori dichiarati.
    #[test]
    #[ignore = "evidenza del Task 2: richiede results/index/ingest_under_load.csv"]
    fn test15_ingest_under_load_evidence() {
        let (h, rows) = csv("ingest_under_load.csv");
        assert_eq!(h, ["t", "query_p50", "query_p95", "ingest_rows", "errors"]);
        assert!(
            rows.len() >= 300,
            "attese ≥ 300 righe (5 minuti), trovate {}",
            rows.len()
        );
        let ingest: Vec<u64> = rows
            .iter()
            .map(|r| r[3].parse().expect("ingest_rows"))
            .collect();
        assert!(
            ingest.windows(2).all(|w| w[0] <= w[1]),
            "ingest_rows deve essere non decrescente"
        );
        let errors: u64 = rows
            .iter()
            .map(|r| r[4].parse::<u64>().expect("errors"))
            .sum();
        let ris = read(&manifest_dir().join("RISULTATI-indice.md"));
        assert!(
            ris.contains("ingest_under_load.csv") && ris.to_lowercase().contains("errori"),
            "RISULTATI-indice deve riportare gli errori dell'ingestion concorrente (somma = {errors})"
        );
    }

    // TEST-17 (RISK-11): header, scale, piani EXPLAIN, hw.json e link di RISULTATI e ADR-0006.
    #[test]
    #[ignore = "evidenza del Task 2: richiede results/index/, RISULTATI.md e ADR-0006"]
    fn test17_evidence_and_links() {
        let (h, thr) = csv("threshold.csv");
        assert_eq!(
            h,
            [
                "chunks",
                "method",
                "parallel_workers",
                "selectivity",
                "run",
                "queries",
                "p50_ms",
                "p95_ms",
                "p99_ms"
            ]
        );
        for s in SCALES {
            for w in ["0", "4"] {
                for run in ["2", "3"] {
                    assert!(
                        thr.iter().any(|r| r[0] == s && r[2] == w && r[4] == run),
                        "threshold.csv: manca chunks={s} parallel_workers={w} run={run}"
                    );
                }
            }
        }
        let combos: BTreeSet<(&str, &str, &str)> = thr
            .iter()
            .map(|r| (r[0].as_str(), r[1].as_str(), r[2].as_str()))
            .collect();
        let plans: Vec<_> = std::fs::read_dir(results().join("plans"))
            .expect("results/index/plans")
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        assert!(
            plans.len() >= combos.len(),
            "un piano EXPLAIN per configurazione di threshold: {} piani per {} configurazioni",
            plans.len(),
            combos.len()
        );
        for p in &plans {
            let v: serde_json::Value =
                serde_json::from_str(&read(p)).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            assert!(
                v.to_string().contains("\"Plan\""),
                "{}: manca \"Plan\"",
                p.display()
            );
        }
        let (lh, lat) = csv("latency.csv");
        assert_eq!(lh, ["chunks", "stage", "run", "p50_ms", "p95_ms", "p99_ms"]);
        for st in ["embed_query", "sql", "end_to_end"] {
            assert!(
                lat.iter().any(|r| r[1] == st),
                "latency.csv: manca lo stage {st}"
            );
        }
        let (rh, _) = csv("recall.csv");
        assert_eq!(
            rh,
            [
                "chunks",
                "index_type",
                "ef_search",
                "iterative_scan",
                "selectivity",
                "recall_vs_exact",
                "returns_k_rate",
                "build_s",
                "index_mb"
            ]
        );
        let (hh, _) = csv("hybrid.csv");
        assert_eq!(
            hh,
            [
                "mode",
                "lang",
                "queries",
                "recall_at_10",
                "ndcg_at_10",
                "p95_ms"
            ]
        );

        let hw: serde_json::Value =
            serde_json::from_str(&read(&results().join("hw.json"))).expect("hw.json valido");
        let images = hw["images"].as_object().expect("hw.json: images");
        assert!(
            images.keys().any(|k| k.contains("pgvector")),
            "hw.json: digest pgvector"
        );
        assert!(
            images
                .keys()
                .any(|k| k.contains("text-embeddings-inference")),
            "hw.json: digest TEI"
        );
        assert!(
            hw["tools"]["pgvector"].is_string(),
            "hw.json: tools.pgvector"
        );

        let ris = read(&manifest_dir().join("RISULTATI.md"));
        for l in [
            "RISULTATI-embedding.md",
            "RISULTATI-indice.md",
            "results/embedding/hw.json",
            "results/index/hw.json",
        ] {
            assert!(ris.contains(l), "RISULTATI.md di S-03 deve collegare {l}");
            assert!(manifest_dir().join(l).exists(), "{l} inesistente");
        }
        let adr =
            read(&repo_root().join("docs/adr/0006-indice-vettoriale-ricerca-esatta-ibrida.md"));
        assert!(
            adr.contains("s03-pgvector/RISULTATI.md"),
            "ADR-0006: link a RISULTATI.md"
        );
        for tok in adr.split(|c: char| c.is_whitespace() || "()[]<>`\"'".contains(c)) {
            if let Some(i) = tok.find("spikes/") {
                let p = tok[i..].trim_end_matches(['.', ',', ';', ':']);
                assert!(
                    repo_root().join(p).exists(),
                    "ADR-0006 cita un percorso inesistente: {p}"
                );
            }
        }
    }

    // ---- Controlli git ------------------------------------------------------------------------

    const BASE: &str = "4586ff305fd115a5b85ee83eaea5b3920ed3927f";
    const SOGLIE: &str = "## Soglie (scritte PRIMA della misura)";

    fn git(args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo_root())
            .args(args)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    fn soglie_section(md: &str) -> String {
        let start = md.find(SOGLIE).expect("sezione Soglie");
        let rest = &md[start + SOGLIE.len()..];
        let end = rest.find("\n## ").unwrap_or(rest.len());
        rest[..end].to_owned()
    }

    // TEST-10 (RISK-5): Soglie committate prima di ogni file in results/index/ e invariate.
    #[test]
    #[ignore = "verifica git del Task 2: richiede la storia del branch con results/index/"]
    fn test10_thresholds_committed_before_results() {
        let ris = "spikes/s03-pgvector/RISULTATI-indice.md";
        let intro = git(&["log", "--reverse", "--format=%H", "-S", SOGLIE, "--", ris]);
        let intro = intro
            .lines()
            .next()
            .expect("commit che introduce le Soglie")
            .to_owned();
        let first_res = git(&[
            "log",
            "--reverse",
            "--diff-filter=A",
            "--format=%H",
            "--",
            "spikes/s03-pgvector/results/index",
        ]);
        let first_res = first_res
            .lines()
            .next()
            .expect("primo commit di results/index")
            .to_owned();
        assert_ne!(intro, first_res, "Soglie e risultati nello stesso commit");
        let st = Command::new("git")
            .arg("-C")
            .arg(repo_root())
            .args(["merge-base", "--is-ancestor", &intro, &first_res])
            .status()
            .expect("git");
        assert!(
            st.success(),
            "le Soglie ({intro}) non precedono i risultati ({first_res})"
        );
        let at_intro = soglie_section(&git(&["show", &format!("{intro}:{ris}")]));
        let at_head = soglie_section(&read(&repo_root().join(ris)));
        assert_eq!(
            at_intro, at_head,
            "le Soglie sono state ritoccate dopo l'introduzione"
        );
        for needle in ["50 ms", "0,95", "100%", "0,03", "200 ms", "2 ×"] {
            assert!(at_head.contains(needle), "Soglie: manca «{needle}»");
        }
    }

    // TEST-14 (RISK-8): il piano non tocca i file del piano 01-10.
    #[test]
    #[ignore = "verifica git: richiede il repo sull'host (in container .git del worktree non risolve)"]
    fn test14_plan_0110_files_untouched() {
        let changed = git(&["diff", "--name-only", &format!("{BASE}..HEAD")]);
        let forbidden = [
            "spikes/s03-pgvector/src/lib.rs",
            "spikes/s03-pgvector/src/chunk.rs",
            "spikes/s03-pgvector/src/tei.rs",
            "spikes/s03-pgvector/src/db.rs",
            "spikes/s03-pgvector/src/metrics.rs",
            "spikes/s03-pgvector/src/main.rs",
            "spikes/s03-pgvector/sql/schema.sql",
            "spikes/s03-pgvector/RISULTATI-embedding.md",
        ];
        let touched: Vec<&str> = changed.lines().filter(|l| forbidden.contains(l)).collect();
        assert!(
            touched.is_empty(),
            "file del piano 01-10 modificati: {touched:?}"
        );
    }
}
