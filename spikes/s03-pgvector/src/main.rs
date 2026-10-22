//! CLI di misura dello spike S-03/A-03, parte embedding (piano 01-10).
//!
//! Si lancia da `spikes/` nel runner (`session.sh oneshot … cargo run -p s03-pgvector
//! --release -- <sottocomando>`): `pdftotext`, Postgres (`DATABASE_URL`) e TEI
//! (`TEI_EMBED_URL`) sono quelli della sessione.
//!
//! - `load`: chunking, embedding dei documenti e inserimento nella tabella del modello;
//! - `query-bench`: recall@5/@10, MRR@10, nDCG@10 per lingua e leak, con e senza istruzione;
//! - `throughput`: embedding del documento da 25 pagine e latenza di 200 query;
//! - `export-candidates`: top-k autorizzati per query, per il reranker (piano 01-15).
//!
//! Prima di `query-bench` ed `export-candidates` `doc_acl` si confronta con la matrice
//! attesa da `acl.json` + manifest: una differenza ferma il comando prima di misurare.
//!
//! **Modifica:** Claude (gsd-executor 01-10) · 2026-10-22 · CLI di misura
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand, ValueEnum};
use common::hist::Recorder;
use common::timer::Stopwatch;
use s03_pgvector::chunk::{self, Chunk, CorpusDoc};
use s03_pgvector::db::{self, CandidateQuery, ChunkRow};
use s03_pgvector::metrics::{self, OracleQuery, Visibility};
use s03_pgvector::{Model, tei};
use serde_json::json;
use uuid::Uuid;

/// Documento del throughput: l'estratto TTQ da 25 pagine.
const TTQ_DOC: &str = "der-it-01-relazione-annuale-2025-p1-25";

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ModelArg {
    Qwen3,
    Bgem3,
    E5,
}

impl ModelArg {
    fn model(self) -> Model {
        match self {
            ModelArg::Qwen3 => Model::Qwen3,
            ModelArg::Bgem3 => Model::BgeM3,
            ModelArg::E5 => Model::E5,
        }
    }

    fn name(self) -> &'static str {
        match self {
            ModelArg::Qwen3 => "qwen3",
            ModelArg::Bgem3 => "bgem3",
            ModelArg::E5 => "e5",
        }
    }
}

#[derive(Parser)]
#[command(
    name = "s03-pgvector",
    about = "Spike S-03/A-03: embedding e ricerca esatta autorizzata"
)]
struct Cli {
    /// Cartella del corpus comune (manifest.json, acl.json, raw/, synthetic/, oracles/).
    #[arg(long, default_value = "data/corpus", global = true)]
    corpus: PathBuf,
    /// Cartella delle evidenze versionate.
    #[arg(long, default_value = "s03-pgvector/results/embedding", global = true)]
    out: PathBuf,
    /// Cartella dei grezzi ignorati da git.
    #[arg(long, default_value = "s03-pgvector/results/raw", global = true)]
    raw: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Chunking + embedding + insert nella tabella del modello; scrive chunks_manifest.json.
    Load {
        #[arg(long, value_enum)]
        model: ModelArg,
        #[arg(long, default_value_t = 32)]
        batch: usize,
    },
    /// Metriche per query → models.csv e query_instruction.csv.
    QueryBench {
        #[arg(long, value_enum)]
        model: ModelArg,
        #[arg(long)]
        no_instruction: bool,
    },
    /// Embedding del documento da 25 pagine e latenza di 200 query → throughput.csv.
    Throughput {
        #[arg(long, value_enum)]
        model: ModelArg,
        #[arg(long)]
        batch: usize,
        /// Valore di --max-batch-tokens con cui è stato avviato TEI (solo registrato).
        #[arg(long)]
        max_batch_tokens: u32,
        /// Numero della ripetizione (la prima si scarta in analisi).
        #[arg(long)]
        run: u32,
        #[arg(long, default_value = "gpu")]
        device: String,
        #[arg(long, default_value_t = 200)]
        queries: usize,
        /// Log di `nvidia-smi --query-gpu=memory.used --format=csv,noheader -lms 500`.
        #[arg(long)]
        vram_log: Option<PathBuf>,
        /// VRAM occupata a riposo prima dell'avvio di TEI (MiB), sottratta al picco.
        #[arg(long, default_value_t = 0)]
        vram_baseline_mib: u32,
    },
    /// Top-k autorizzati per query (raw/candidates_top20.jsonl) + candidates_manifest.json.
    ExportCandidates {
        #[arg(long, value_enum)]
        model: ModelArg,
        #[arg(long, default_value_t = 20)]
        k: i64,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Load { model, batch } => load(&cli.corpus, &cli.out, model, batch).await,
        Cmd::QueryBench {
            model,
            no_instruction,
        } => query_bench(&cli.corpus, &cli.out, &cli.raw, model, !no_instruction).await,
        Cmd::Throughput {
            model,
            batch,
            max_batch_tokens,
            run,
            device,
            queries,
            vram_log,
            vram_baseline_mib,
        } => {
            let t = ThroughputArgs {
                model,
                batch,
                max_batch_tokens,
                run,
                device,
                queries,
                vram_log,
                vram_baseline_mib,
            };
            throughput(&cli.corpus, &cli.out, &t).await
        }
        Cmd::ExportCandidates { model, k } => {
            export(&cli.corpus, &cli.out, &cli.raw, model, k).await
        }
    }
}

// ---------------------------------------------------------------- utilità

fn read(path: &Path) -> anyhow::Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("{} illeggibile", path.display()))
}

/// sha256 di un file con `sha256sum` (presente nel runner e in Git Bash).
fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let out = Command::new("sha256sum")
        .arg(path)
        .output()
        .context("sha256sum non eseguibile")?;
    if !out.status.success() {
        bail!("sha256sum su {} uscito con {}", path.display(), out.status);
    }
    let s = String::from_utf8(out.stdout).context("output di sha256sum")?;
    let hex = s.split_whitespace().next().unwrap_or("").to_string();
    if hex.len() != 64 {
        bail!("sha256sum su {}: output inatteso", path.display());
    }
    Ok(hex)
}

fn expected_visibility(corpus: &Path) -> anyhow::Result<Visibility> {
    metrics::expected_visibility(
        &read(&corpus.join("acl.json"))?,
        &read(&corpus.join("manifest.json"))?,
    )
}

fn ratified(corpus: &Path) -> anyhow::Result<metrics::RatifiedQueries> {
    metrics::load_ratified_queries(&read(&corpus.join("oracles/queries.json"))?)
}

/// Riscrive un CSV a header fisso togliendo le righe con la stessa chiave delle nuove
/// (prime `key_cols` colonne) e aggiungendo le nuove: rilanciare una misura la sostituisce.
fn upsert_csv(
    path: &Path,
    header: &[&str],
    key_cols: usize,
    new_rows: &[Vec<String>],
) -> anyhow::Result<()> {
    let mut kept: Vec<Vec<String>> = Vec::new();
    if path.exists() {
        let text = read(path)?;
        let mut lines = text.lines();
        let head = lines.next().unwrap_or("");
        if head != header.join(",") {
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

// ---------------------------------------------------------------- load

async fn load(corpus: &Path, out: &Path, model: ModelArg, batch: usize) -> anyhow::Result<()> {
    let docs = chunk::read_manifest(corpus)?;
    let expected = expected_visibility(corpus)?;
    let client = tei::Client::from_env(batch)?;
    let pool = db::connect_from_env().await?;
    db::apply_schema(&pool).await?;

    let pairs: Vec<(String, String)> = docs
        .iter()
        .map(|d| (d.doc_id.clone(), d.classification.clone()))
        .collect();
    let doc_ids = db::upsert_docs(&pool, &pairs).await?;
    db::insert_doc_acl(&pool, &expected, &doc_ids).await?;
    db::check_doc_acl(&pool, &expected).await?;

    let mut all: Vec<Chunk> = Vec::new();
    let mut per_doc = Vec::new();
    for d in &docs {
        per_doc.push(doc_entry(corpus, d, &mut all)?);
    }

    let texts: Vec<&str> = all.iter().map(|c| c.body.as_str()).collect();
    let sw = Stopwatch::start();
    let embs = client.embed_documents(&texts).await?;
    let embed_s = sw.elapsed_ms_f64() / 1000.0;

    let m = model.model();
    db::truncate_chunks(&pool, m).await?;
    let mut rows = Vec::with_capacity(all.len());
    for (c, e) in all.iter().zip(embs) {
        let doc_id = *doc_ids
            .get(&c.doc_key)
            .with_context(|| format!("doc {} senza id", c.doc_key))?;
        rows.push(ChunkRow {
            id: c.stable_id(),
            doc_id,
            chunk: c,
            emb: e,
        });
    }
    let sw = Stopwatch::start();
    db::insert_chunks(&pool, m, &rows).await?;
    let insert_s = sw.elapsed_ms_f64() / 1000.0;

    let info = client.info().await.unwrap_or(serde_json::Value::Null);
    write_chunks_manifest(out, &per_doc, &all, model, &info, embed_s, insert_s)?;
    println!(
        "load {}: {} frammenti, embedding {embed_s:.1} s, insert {insert_s:.1} s",
        model.name(),
        all.len()
    );
    Ok(())
}

fn doc_entry(
    corpus: &Path,
    d: &CorpusDoc,
    all: &mut Vec<Chunk>,
) -> anyhow::Result<serde_json::Value> {
    let Some(src) = chunk::source_path(corpus, d) else {
        return Ok(json!({
            "doc_id": d.doc_id, "format": d.format, "classification": d.classification,
            "indexed": false, "reason": "DOCX: duplicato del Markdown (ORACOLI.md), estrazione materia di S-02",
        }));
    };
    let sha = sha256_file(&src)?;
    if sha != d.sha256 {
        bail!("{}: sha256 diverso dal manifest", src.display());
    }
    let chunks = chunk::chunk_document(corpus, d)?.unwrap_or_default();
    let n = chunks.len();
    let chars: usize = chunks.iter().map(|c| c.body.chars().count()).sum();
    all.extend(chunks);
    Ok(json!({
        "doc_id": d.doc_id, "format": d.format, "classification": d.classification,
        "indexed": true, "sha256": sha, "chunks": n, "chars": chars,
    }))
}

fn write_chunks_manifest(
    out: &Path,
    per_doc: &[serde_json::Value],
    all: &[Chunk],
    model: ModelArg,
    info: &serde_json::Value,
    embed_s: f64,
    insert_s: f64,
) -> anyhow::Result<()> {
    let path = out.join("chunks_manifest.json");
    // Impronta dell'insieme dei chunk: id stabili e offset, in ordine.
    let mut listing = String::new();
    for c in all {
        let _ = writeln!(listing, "{} {} {}", c.stable_id(), c.char_start, c.char_end);
    }
    let tmp = out.join(".chunk_listing.tmp");
    std::fs::write(&tmp, &listing)?;
    let listing_sha = sha256_file(&tmp);
    let _ = std::fs::remove_file(&tmp);
    let listing_sha = listing_sha?;

    let mut loads = serde_json::Map::new();
    if path.exists()
        && let Ok(prev) = serde_json::from_str::<serde_json::Value>(&read(&path)?)
        && let Some(prev_loads) = prev.get("loads").and_then(|v| v.as_object())
    {
        if prev.get("chunk_set_sha256").and_then(|v| v.as_str()) != Some(listing_sha.as_str()) {
            bail!(
                "insieme dei frammenti diverso da quello dei load precedenti: chunker o corpus cambiati"
            );
        }
        loads.extend(prev_loads.clone());
    }
    loads.insert(
        model.name().to_string(),
        json!({
            "rows": all.len(),
            "embed_seconds": f4(embed_s),
            "insert_seconds": f4(insert_s),
            "tei_model_id": info.get("model_id"),
            "tei_model_sha": info.get("model_sha"),
            "tei_version": info.get("version"),
        }),
    );
    let doc = json!({
        "chunker": {
            "chunk_chars": chunk::CHUNK_CHARS,
            "overlap_chars": chunk::OVERLAP_CHARS,
            "pdf": "pdftotext -layout -enc UTF-8, una pagina per form feed, offset in caratteri sulla pagina",
            "markdown": "una sezione per intestazione ATX, front matter escluso, H1 fuori dal percorso, offset in caratteri sul documento",
            "docx": "non indicizzati",
            "id": "UUID v8 da FNV-1a 128 di doc_key, pagina, sezione, offset",
        },
        "total_rows": all.len(),
        "chunk_set_sha256": listing_sha,
        "docs": per_doc,
        "loads": loads,
    });
    std::fs::write(&path, serde_json::to_string_pretty(&doc)? + "\n")?;
    Ok(())
}

// ---------------------------------------------------------------- query-bench

struct QueryScore {
    group: &'static str,
    evaluable: bool,
    recall5: f64,
    recall10: f64,
    mrr10: f64,
    ndcg10: f64,
    leaked5: usize,
    leaked10: usize,
}

async fn query_bench(
    corpus: &Path,
    out: &Path,
    raw: &Path,
    model: ModelArg,
    with_instruction: bool,
) -> anyhow::Result<()> {
    let expected = expected_visibility(corpus)?;
    let rq = ratified(corpus)?;
    let pool = db::connect_from_env().await?;
    db::check_doc_acl(&pool, &expected).await?;
    let client = tei::Client::from_env(32)?;
    let m = model.model();

    let all = db::fetch_chunks(&pool, m).await?;
    if all.is_empty() {
        bail!("tabella {} vuota: lanciare prima load", db::table(m));
    }
    let by_id: HashMap<Uuid, &Chunk> = all.iter().map(|(id, c)| (*id, c)).collect();
    let all_chunks: Vec<Chunk> = all.iter().map(|(_, c)| c.clone()).collect();

    let mut scores = Vec::new();
    let mut per_query = String::from(
        "oracle_id,lang,role_profile,no_answer,total_relevant,first_rank,recall_at_5,recall_at_10,mrr_at_10,ndcg_at_10,leaked_at_10\n",
    );
    for q in &rq.queries {
        let total = metrics::relevant_chunks(&all_chunks, &q.expected_passages)
            .into_iter()
            .filter(|r| *r)
            .count();
        let qv = client.embed_query(&q.text, m, with_instruction).await?;
        let hits =
            db::exact_search(&pool, m, &qv, std::slice::from_ref(&q.role_profile), 10).await?;
        let hit_chunks: Vec<Chunk> = hits
            .iter()
            .map(|h| by_id.get(&h.chunk_id).map(|c| (*c).clone()))
            .collect::<Option<_>>()
            .context("frammento restituito ma assente dalla tabella")?;
        let rel = metrics::relevant_chunks(&hit_chunks, &q.expected_passages);
        let keys: Vec<&str> = hits.iter().map(|h| h.doc_key.as_str()).collect();
        let s = QueryScore {
            group: q.lang_group(),
            evaluable: !q.no_answer && total > 0,
            recall5: metrics::recall_at_k(&rel, total, 5),
            recall10: metrics::recall_at_k(&rel, total, 10),
            mrr10: metrics::mrr_at_k(&rel, 10),
            ndcg10: metrics::ndcg_at_k(&rel, total, 10),
            leaked5: metrics::leaked_chunks(&keys[..keys.len().min(5)], &q.role_profile, &expected),
            leaked10: metrics::leaked_chunks(&keys, &q.role_profile, &expected),
        };
        let first = rel
            .iter()
            .position(|r| *r)
            .map_or(String::new(), |i| (i + 1).to_string());
        let _ = writeln!(
            per_query,
            "{},{},{},{},{},{},{},{},{},{},{}",
            q.oracle_id,
            q.lang,
            q.role_profile,
            q.no_answer,
            total,
            first,
            f4(s.recall5),
            f4(s.recall10),
            f4(s.mrr10),
            f4(s.ndcg10),
            s.leaked10
        );
        scores.push(s);
    }
    std::fs::create_dir_all(raw)?;
    let instr = if with_instruction { "on" } else { "off" };
    std::fs::write(
        raw.join(format!("per_query_{}_{instr}.csv", model.name())),
        &per_query,
    )?;

    let unreachable = rq
        .queries
        .iter()
        .zip(&scores)
        .filter(|(q, s)| !q.no_answer && !s.evaluable)
        .map(|(q, _)| q.oracle_id.as_str())
        .collect::<Vec<_>>();

    let groups = ["it", "en", "cross", "all"];
    let in_group = |g: &str, s: &QueryScore| g == "all" || s.group == g;
    let mut model_rows = Vec::new();
    let mut instr_rows = Vec::new();
    for g in groups {
        let ev: Vec<&QueryScore> = scores
            .iter()
            .filter(|s| in_group(g, s) && s.evaluable)
            .collect();
        let n = ev.len().to_string();
        let mrr = f4(mean(&ev.iter().map(|s| s.mrr10).collect::<Vec<_>>()));
        let ndcg = f4(mean(&ev.iter().map(|s| s.ndcg10).collect::<Vec<_>>()));
        let r5 = f4(mean(&ev.iter().map(|s| s.recall5).collect::<Vec<_>>()));
        let r10 = f4(mean(&ev.iter().map(|s| s.recall10).collect::<Vec<_>>()));
        let l5: usize = scores
            .iter()
            .filter(|s| in_group(g, s))
            .map(|s| s.leaked5)
            .sum();
        let l10: usize = scores
            .iter()
            .filter(|s| in_group(g, s))
            .map(|s| s.leaked10)
            .sum();
        let name = model.name().to_string();
        model_rows.push(vec![
            name.clone(),
            g.into(),
            "5".into(),
            n.clone(),
            r5,
            mrr.clone(),
            ndcg.clone(),
            l5.to_string(),
        ]);
        model_rows.push(vec![
            name.clone(),
            g.into(),
            "10".into(),
            n,
            r10.clone(),
            mrr,
            ndcg.clone(),
            l10.to_string(),
        ]);
        instr_rows.push(vec![name, instr.into(), g.into(), ndcg, r10]);
    }
    if with_instruction {
        upsert_csv(
            &out.join("models.csv"),
            &[
                "model",
                "lang",
                "k",
                "queries",
                "recall_at_k",
                "mrr_at_10",
                "ndcg_at_10",
                "leaked_chunks",
            ],
            3,
            &model_rows,
        )?;
    }
    upsert_csv(
        &out.join("query_instruction.csv"),
        &["model", "instruction", "lang", "ndcg_at_10", "recall_at_10"],
        3,
        &instr_rows,
    )?;
    let leaked: usize = scores.iter().map(|s| s.leaked10).sum();
    println!(
        "query-bench {} istruzione={instr}: {} query ratificate ({} escluse), {} valutabili, irraggiungibili {:?}, leak top-10 {leaked}",
        model.name(),
        rq.queries.len(),
        rq.excluded,
        scores.iter().filter(|s| s.evaluable).count(),
        unreachable
    );
    for r in &model_rows {
        println!("  {}", r.join(","));
    }
    Ok(())
}

// ---------------------------------------------------------------- throughput

struct ThroughputArgs {
    model: ModelArg,
    batch: usize,
    max_batch_tokens: u32,
    run: u32,
    device: String,
    queries: usize,
    vram_log: Option<PathBuf>,
    vram_baseline_mib: u32,
}

/// Picco di `memory.used` (MiB) nel log di `nvidia-smi`, meno il valore a riposo.
fn vram_peak(log: &Path, baseline: u32) -> anyhow::Result<u32> {
    let text = read(log)?;
    let peak = text
        .lines()
        .filter_map(|l| l.split_whitespace().next()?.trim().parse::<u32>().ok())
        .max()
        .with_context(|| format!("{}: nessun campione", log.display()))?;
    Ok(peak.saturating_sub(baseline))
}

async fn throughput(corpus: &Path, out: &Path, t: &ThroughputArgs) -> anyhow::Result<()> {
    let docs = chunk::read_manifest(corpus)?;
    let doc = docs
        .iter()
        .find(|d| d.doc_id == TTQ_DOC)
        .context("documento TTQ assente dal manifest")?;
    let chunks = chunk::chunk_document(corpus, doc)?.unwrap_or_default();
    let texts: Vec<&str> = chunks.iter().map(|c| c.body.as_str()).collect();
    let rq = ratified(corpus)?;
    let qtexts: Vec<&OracleQuery> = rq.queries.iter().collect();
    if qtexts.is_empty() || texts.is_empty() {
        bail!("niente da misurare");
    }
    let client = tei::Client::from_env(t.batch)?;
    let m = t.model.model();

    let sw = Stopwatch::start();
    let embs = client.embed_documents(&texts).await?;
    let secs = sw.elapsed_ms_f64() / 1000.0;
    if embs.len() != texts.len() {
        bail!("vettori mancanti");
    }

    let mut hist = Recorder::new();
    for i in 0..t.queries {
        let q = qtexts[i % qtexts.len()];
        let sw = Stopwatch::start();
        client.embed_query(&q.text, m, true).await?;
        hist.record_us(sw.elapsed_us());
    }
    let s = hist.summary();
    let vram = match &t.vram_log {
        Some(p) => vram_peak(p, t.vram_baseline_mib)?.to_string(),
        None => String::new(),
    };
    let row = vec![
        t.model.name().to_string(),
        t.device.clone(),
        t.batch.to_string(),
        t.max_batch_tokens.to_string(),
        t.run.to_string(),
        texts.len().to_string(),
        f4(secs),
        format!("{:.2}", texts.len() as f64 / secs),
        format!("{:.2}", s.p50_us as f64 / 1000.0),
        format!("{:.2}", s.p95_us as f64 / 1000.0),
        vram,
    ];
    println!("throughput {}", row.join(","));
    upsert_csv(
        &out.join("throughput.csv"),
        &[
            "model",
            "device",
            "batch",
            "max_batch_tokens",
            "run",
            "chunks",
            "seconds",
            "chunks_per_s",
            "query_p50_ms",
            "query_p95_ms",
            "vram_peak_mib",
        ],
        5,
        &[row],
    )
}

// ---------------------------------------------------------------- export-candidates

async fn export(
    corpus: &Path,
    out: &Path,
    raw: &Path,
    model: ModelArg,
    k: i64,
) -> anyhow::Result<()> {
    let expected = expected_visibility(corpus)?;
    let rq = ratified(corpus)?;
    let pool = db::connect_from_env().await?;
    db::check_doc_acl(&pool, &expected).await?;
    let client = tei::Client::from_env(32)?;
    let m = model.model();

    let mut qs = Vec::with_capacity(rq.queries.len());
    for q in &rq.queries {
        qs.push(CandidateQuery {
            oracle_id: q.oracle_id.clone(),
            roles: vec![q.role_profile.clone()],
            query_vec: client.embed_query(&q.text, m, true).await?,
        });
    }
    let bytes = db::export_candidates(&pool, m, &qs, k).await?;

    // Controllo del leak sui candidati, contro la matrice attesa.
    let mut leaked = 0usize;
    let mut total = 0usize;
    let roles: BTreeMap<&str, &str> = qs
        .iter()
        .map(|q| (q.oracle_id.as_str(), q.roles[0].as_str()))
        .collect();
    for line in bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
        let v: serde_json::Value = serde_json::from_slice(line)?;
        let id = v["oracle_id"].as_str().unwrap_or("");
        let keys: Vec<&str> = v["candidates"]
            .as_array()
            .map(|a| a.iter().filter_map(|c| c["doc_key"].as_str()).collect())
            .unwrap_or_default();
        total += keys.len();
        leaked += metrics::leaked_chunks(&keys, roles.get(id).copied().unwrap_or(""), &expected);
    }
    if leaked > 0 {
        bail!("{leaked} candidati non visibili al ruolo: esportazione rifiutata");
    }

    std::fs::create_dir_all(raw)?;
    let file = raw.join("candidates_top20.jsonl");
    std::fs::write(&file, &bytes)?;
    let sha = sha256_file(&file)?;
    let info = client.info().await.unwrap_or(serde_json::Value::Null);
    let manifest = json!({
        "model": model.name(),
        "tei_model_id": info.get("model_id"),
        "tei_model_sha": info.get("model_sha"),
        "k": k,
        "queries": qs.len(),
        "excluded_oracles": rq.excluded,
        "candidates_total": total,
        "leaked_candidates": leaked,
        "file": "spikes/s03-pgvector/results/raw/candidates_top20.jsonl (ignorato da git)",
        "sha256": sha,
        "ordering": "query in ordine di queries.json; candidati per distanza coseno poi id (ORDER BY nella SQL)",
        "query_instruction": "on",
        "fields": ["oracle_id", "roles", "candidates[rank, chunk_id, doc_key, page, section_path, char_start, char_end, distance, body]"],
    });
    std::fs::write(
        out.join("candidates_manifest.json"),
        serde_json::to_string_pretty(&manifest)? + "\n",
    )?;
    println!(
        "export-candidates {}: {} query, {total} candidati, sha256 {sha}",
        model.name(),
        qs.len()
    );
    Ok(())
}
