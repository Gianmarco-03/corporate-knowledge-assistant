//! Padre del lanciatore: `run`, `batch`, `calibrate` e i driver `hostile`, `budget`, `overhead` (piano 01-07).
//!
//! Ogni sottocomando lancia figli solo tramite job JSON (`job::parse` → `job::resolve` →
//! `isolate::run_child`): nessun sottocomando accetta un nome di bin o argomenti liberi.
//! All'avvio, prima di leggere o lanciare qualsiasi job, il padre si dichiara
//! `PR_SET_CHILD_SUBREAPER` (i nipoti orfani tornano a lui e si raccolgono); se non può, stampa
//! `subreaper_unavailable errno=<n>` ed esce 2.
//!
//! Codici d'uscita: 0 riuscito; 2 `job_error`, argomenti o manifest non validi, subreaper
//! assente; 3 misura non isolata (`oom_adj_set=false` in `budget`/`overhead`); 1 altri errori.
//! I driver `hostile`, `budget`, `overhead` li esegue il piano 01-18 (che sceglie ripetizioni,
//! concorrenze e input) e non tocca questo codice.
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use s02_parser::isolate::{Budget, Limits, RunReport, run_child};
use s02_parser::job::{self, ChildSpec, JobError, MAX_JOB_BYTES};
use s02_parser::protocol::{ChildOutcome, Kind};
use serde_json::{Value, json};

const MIB: u64 = 1024 * 1024;

/// File dei limiti di default (scritto da `calibrate --op parse`, Task 4).
const DEFAULT_LIMITS: &str = "/results/isolation/limits.json";
/// Riserva del padre nel budget aggregato (Soglie del lanciatore).
const PARENT_RESERVE_BYTES: u64 = 256 * MIB;
/// Tetto fisso del timeout di parete, sopra qualunque profilo.
const MAX_WALL_TIMEOUT: Duration = Duration::from_secs(120);

/// Limiti che la formula delle Soglie non calibra (fissati qui prima della calibrazione,
/// uguali per ogni op): CPU, parete, file aperti; `file_size_bytes` = quota (≥ 1 MiB).
const FIXED_CPU_SECONDS: u64 = 60;
const FIXED_WALL_TIMEOUT: Duration = Duration::from_secs(120);
const FIXED_MAX_OPEN_FILES: u64 = 64;
const MIN_FILE_SIZE_BYTES: u64 = MIB;
/// Formula delle Soglie: `RLIMIT_AS` = 3 × max `VmPeak`, quota = 2 × max spazio scritto.
const AS_FACTOR: u64 = 3;
const QUOTA_FACTOR: u64 = 2;
/// Quota dei job di calibrazione; l'AS è il resto del budget.
const CALIBRATION_QUOTA_BYTES: u64 = 256 * MIB;

/// Documenti per ripetizione del driver `overhead`.
const OVERHEAD_DOCS_PER_RUN: usize = 16;

const HOSTILE_HEADER: &str = "case_id,run,expected,outcome,kind,extra_reasons,signal,peak_rss_kb,ms_to_fail,parent_alive,oom_kill_delta,leftover_procs,oom_adj_set,pass";
const BUDGET_HEADER: &str = "requested_concurrency,jobs,max_admitted,completed,oom_kill_delta,parent_vm_hwm_kb,parent_alive";
const OVERHEAD_HEADER: &str = "mode,concurrency,run,docs,wall_ms,spawn_init_ms,docs_per_s";
const CALIBRATION_HEADER: &str = "doc_id,pages,vm_peak_kb,out_bytes,max_flate_ratio,max_stream_bytes,total_decompressed_bytes,objstm_count,rejected_filter_streams";

#[derive(Parser)]
#[command(
    name = "parse-parent",
    about = "Lanciatore isolato di s02-parser (job JSON)"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Un job JSON da file o da stdin (`-`).
    Run {
        #[arg(long)]
        job: String,
        #[arg(long, default_value = DEFAULT_LIMITS)]
        limits: PathBuf,
    },
    /// Un job per riga, ammessi col budget aggregato.
    Batch {
        #[arg(long)]
        jobs: PathBuf,
        #[arg(long)]
        concurrency: usize,
        #[arg(long, default_value = DEFAULT_LIMITS)]
        limits: PathBuf,
    },
    /// Calibra i limiti di un'op sugli input benigni con la formula delle Soglie.
    Calibrate {
        #[arg(long)]
        op: String,
        #[arg(long)]
        inputs: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// CSV della calibrazione (default: `calibration.csv` per `parse`,
        /// `calibration-<op>.csv` per le altre op, accanto a `--out`).
        #[arg(long)]
        csv: Option<PathBuf>,
        /// `params` del job (JSON), uguale per ogni input.
        #[arg(long, default_value = "{}")]
        params: String,
        #[arg(long)]
        run_tag: Option<String>,
    },
    /// Driver dei casi ostili (piano 01-18).
    Hostile {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        cases_dir: PathBuf,
        #[arg(long)]
        runs: u32,
        #[arg(long)]
        run_tag: String,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = DEFAULT_LIMITS)]
        limits: PathBuf,
    },
    /// Driver del budget aggregato (piano 01-18).
    Budget {
        #[arg(long)]
        concurrency: String,
        #[arg(long)]
        jobs: u32,
        #[arg(long)]
        reserve_fraction: f64,
        #[arg(long)]
        run_tag: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Driver dell'overhead figlio-per-documento contro in-process (piano 01-18).
    Overhead {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        runs: u32,
        #[arg(long)]
        concurrency: String,
        #[arg(long)]
        run_tag: String,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = DEFAULT_LIMITS)]
        limits: PathBuf,
    },
}

/// Uscita del programma: codice e, se serve, il motivo su stderr.
struct Exit(u8);

fn main() -> ExitCode {
    if let Err(errno) = become_subreaper() {
        eprintln!("subreaper_unavailable errno={errno}");
        return ExitCode::from(2);
    }
    let cli = Cli::parse();
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("parse-parent: runtime tokio: {e}");
            return ExitCode::from(1);
        }
    };
    match rt.block_on(dispatch(cli.cmd)) {
        Ok(Exit(code)) => ExitCode::from(code),
        Err(e) => {
            eprintln!("parse-parent: {e:#}");
            ExitCode::from(1)
        }
    }
}

#[cfg(target_os = "linux")]
fn become_subreaper() -> Result<(), i32> {
    // PR_SET_CHILD_SUBREAPER: i nipoti orfani dei job si riattaccano a questo processo.
    nix::sys::prctl::set_child_subreaper(true).map_err(|e| e as i32)
}

#[cfg(not(target_os = "linux"))]
fn become_subreaper() -> Result<(), i32> {
    // PR_SET_CHILD_SUBREAPER esiste solo su Linux: fuori dal worker il padre non parte.
    Err(0)
}

async fn dispatch(cmd: Cmd) -> anyhow::Result<Exit> {
    match cmd {
        Cmd::Run { job, limits } => cmd_run(&job, &limits).await,
        Cmd::Batch {
            jobs,
            concurrency,
            limits,
        } => cmd_batch(&jobs, concurrency, &limits).await,
        Cmd::Calibrate {
            op,
            inputs,
            out,
            csv,
            params,
            run_tag,
        } => cmd_calibrate(&op, &inputs, &out, csv, &params, run_tag).await,
        Cmd::Hostile {
            manifest,
            cases_dir,
            runs,
            run_tag,
            out,
            limits,
        } => cmd_hostile(&manifest, &cases_dir, runs, &run_tag, &out, &limits).await,
        Cmd::Budget {
            concurrency,
            jobs,
            reserve_fraction,
            run_tag,
            out,
        } => cmd_budget(&concurrency, jobs, reserve_fraction, &run_tag, &out).await,
        Cmd::Overhead {
            input,
            runs,
            concurrency,
            run_tag,
            out,
            limits,
        } => cmd_overhead(&input, runs, &concurrency, &run_tag, &out, &limits).await,
    }
}

// ---- job, limiti, budget ----------------------------------------------------------------

/// Legge al più `MAX_JOB_BYTES + 1` byte (oltre, `job::parse` rifiuta).
fn read_bounded(r: impl Read) -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    std::io::copy(&mut r.take(MAX_JOB_BYTES as u64 + 1), &mut buf)?;
    Ok(buf)
}

fn read_job(src: &str) -> anyhow::Result<Vec<u8>> {
    if src == "-" {
        read_bounded(std::io::stdin().lock())
    } else {
        let f = std::fs::File::open(src).with_context(|| format!("job {src}"))?;
        read_bounded(f)
    }
}

/// `job::parse` + `job::resolve`, con il `job_id` del job anche quando non è valido.
fn prepare_job(bytes: &[u8]) -> Result<ChildSpec, (Option<String>, JobError)> {
    let hint = job::job_id_hint(bytes);
    let j = job::parse(bytes).map_err(|e| (hint.clone(), e))?;
    job::resolve(&j).map_err(|e| (Some(j.job_id().to_owned()), e))
}

fn print_line(line: &str) {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let _ = out.write_all(line.as_bytes());
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

fn print_job_error(job_id: Option<&str>, e: &JobError) {
    print_line(&e.to_line(job_id));
}

fn report_line(r: &RunReport) -> String {
    serde_json::to_string(r).unwrap_or_else(|e| {
        json!({"job_id": r.job_id, "status": "report_error", "detail": e.to_string()}).to_string()
    })
}

/// Profili di `limits.json`: `default` più eventuali profili per op.
struct LimitsFile {
    profiles: serde_json::Map<String, Value>,
}

impl LimitsFile {
    fn load(path: &Path) -> anyhow::Result<LimitsFile> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("limiti {}", path.display()))?;
        let v: Value = serde_json::from_str(&text).context("limits.json non è JSON")?;
        let profiles = v
            .as_object()
            .cloned()
            .context("limits.json non è un oggetto")?;
        if !profiles.contains_key("default") {
            bail!("limits.json senza profilo default");
        }
        Ok(LimitsFile { profiles })
    }

    fn for_op(&self, op: &str, budget: &Budget) -> anyhow::Result<Limits> {
        let v = self
            .profiles
            .get(op)
            .or_else(|| self.profiles.get("default"))
            .context("profilo assente")?;
        let l: Limits = serde_json::from_value(v.clone())
            .with_context(|| format!("profilo {op} di limits.json"))?;
        Ok(clamp(l, budget))
    }
}

/// Tetti fissi del codice sopra ogni profilo: AS entro il budget, parete entro 120 s.
fn clamp(mut l: Limits, budget: &Budget) -> Limits {
    l.address_space_bytes = l.address_space_bytes.min(budget.capacity_bytes());
    l.wall_timeout = l.wall_timeout.min(MAX_WALL_TIMEOUT);
    l
}

fn budget() -> anyhow::Result<Budget> {
    Budget::from_cgroup(PARENT_RESERVE_BYTES)
}

/// Esito di `run_child` come riga: `RunReport` o `job_error`; `None` su errore di lancio.
fn job_error_of(e: &anyhow::Error) -> Option<&JobError> {
    e.downcast_ref::<JobError>()
}

// ---- run e batch ------------------------------------------------------------------------

async fn cmd_run(src: &str, limits_path: &Path) -> anyhow::Result<Exit> {
    let bytes = read_job(src)?;
    let spec = match prepare_job(&bytes) {
        Ok(s) => s,
        Err((id, e)) => {
            print_job_error(id.as_deref(), &e);
            return Ok(Exit(2));
        }
    };
    let budget = budget()?;
    let op = job::parse(&bytes).map(|j| j.op()).unwrap_or("default");
    let limits = LimitsFile::load(limits_path)?.for_op(op, &budget)?;
    match run_child(&spec, &limits, &budget).await {
        Ok(r) => {
            print_line(&report_line(&r));
            Ok(Exit(0))
        }
        Err(e) => match job_error_of(&e) {
            Some(je) => {
                print_job_error(Some(&spec.job_id), je);
                Ok(Exit(2))
            }
            None => Err(e),
        },
    }
}

async fn cmd_batch(jobs: &Path, concurrency: usize, limits_path: &Path) -> anyhow::Result<Exit> {
    if concurrency == 0 {
        bail!("--concurrency deve essere ≥ 1");
    }
    let text = std::fs::read_to_string(jobs).with_context(|| format!("jobs {}", jobs.display()))?;
    let budget = Arc::new(budget()?);
    let file = LimitsFile::load(limits_path)?;
    let mut ready: Vec<(ChildSpec, Limits)> = Vec::new();
    let mut job_errors = 0u32;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        match prepare_job(line.as_bytes()) {
            Ok(spec) => {
                let op = job::parse(line.as_bytes())
                    .map(|j| j.op())
                    .unwrap_or("default");
                ready.push((spec, file.for_op(op, &budget)?));
            }
            Err((id, e)) => {
                job_errors += 1;
                print_job_error(id.as_deref(), &e);
            }
        }
    }
    let total = ready.len();
    let mut set = tokio::task::JoinSet::new();
    let mut queue = ready.into_iter();
    let mut launch_errors = 0u32;
    loop {
        while set.len() < concurrency {
            let Some((spec, limits)) = queue.next() else {
                break;
            };
            let b = budget.clone();
            set.spawn(async move {
                let r = run_child(&spec, &limits, &b).await;
                (spec.job_id, r)
            });
        }
        let Some(done) = set.join_next().await else {
            break;
        };
        let (job_id, r) = done.context("task del batch")?;
        match r {
            Ok(rep) => print_line(&report_line(&rep)),
            Err(e) => match job_error_of(&e) {
                Some(je) => {
                    job_errors += 1;
                    print_job_error(Some(&job_id), je);
                }
                None => {
                    launch_errors += 1;
                    eprintln!("launch_error job_id={job_id}: {e:#}");
                }
            },
        }
    }
    print_line(
        &json!({
            "status": "batch_summary",
            "jobs": total,
            "concurrency": concurrency,
            "max_admitted": budget.max_admitted(),
            "job_errors": job_errors,
            "launch_errors": launch_errors,
        })
        .to_string(),
    );
    Ok(Exit(if job_errors > 0 { 2 } else { 0 }))
}

// ---- calibrazione -----------------------------------------------------------------------

/// `[a-z0-9-]` dal nome del file, al più 32 caratteri.
fn doc_id(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let mut id: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                '-'
            }
        })
        .collect();
    id.truncate(32);
    let id = id.trim_matches('-').to_owned();
    if id.is_empty() { "doc".to_owned() } else { id }
}

fn default_run_tag() -> String {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:08x}", (n as u64) ^ ((n >> 64) as u64))
        .chars()
        .rev()
        .take(8)
        .collect()
}

fn valid_run_tag(t: &str) -> bool {
    t.len() == 8
        && t.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// Area dell'`out_dir` per op: `isolation` per `parse` (Task 4), `cal-<op>` per le altre.
fn calibration_area(op: &str) -> String {
    if op == "parse" {
        "isolation".to_owned()
    } else {
        format!("cal-{}", op.replace('_', "-"))
    }
}

fn op_has_output(op: &str) -> bool {
    matches!(op, "parse" | "extract_pdf" | "ocr_render")
}

fn job_bytes(job_id: &str, op: &str, input: &Path, area: &str, params: &Value) -> Vec<u8> {
    let out_dir = op_has_output(op).then(|| format!("/results/raw/{area}/{job_id}"));
    serde_json::to_vec(&json!({
        "job_id": job_id,
        "op": op,
        "input": input.to_string_lossy(),
        "out_dir": out_dir,
        "params": params,
    }))
    .unwrap_or_default()
}

/// Byte di una cartella (nessun symlink seguito).
fn dir_bytes(dir: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            if let Ok(m) = std::fs::symlink_metadata(e.path()) {
                if m.is_dir() {
                    stack.push(e.path());
                } else {
                    total = total.saturating_add(m.len());
                }
            }
        }
    }
    total
}

/// Numero `key=N` in un detail del prescan (statistiche dei documenti rifiutati).
fn detail_number(detail: &str, key: &str) -> Option<u64> {
    detail
        .split_whitespace()
        .find_map(|t| t.strip_prefix(key)?.strip_prefix('=')?.parse().ok())
}

fn is_filter_rejection(kind: Kind, detail: &str) -> bool {
    (kind == Kind::Unsupported && (detail.starts_with("filter") || detail.starts_with("image")))
        || (kind == Kind::DecompressionLimit
            && (detail.starts_with("filter_chain") || detail.starts_with("image")))
}

async fn cmd_calibrate(
    op: &str,
    inputs: &Path,
    out: &Path,
    csv: Option<PathBuf>,
    params: &str,
    run_tag: Option<String>,
) -> anyhow::Result<Exit> {
    if !job::OPS.contains(&op) {
        eprintln!("calibrate: op fuori allowlist: {op}");
        return Ok(Exit(2));
    }
    let params: Value = serde_json::from_str(params).context("--params non è JSON")?;
    let tag = run_tag.unwrap_or_else(default_run_tag);
    if !valid_run_tag(&tag) {
        eprintln!("calibrate: --run-tag fuori da [a-z0-9]{{8}}");
        return Ok(Exit(2));
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(inputs)
        .with_context(|| format!("inputs {}", inputs.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| {
            op != "parse"
                || p.extension()
                    .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("pdf"))
        })
        .collect();
    files.sort();
    if files.is_empty() {
        bail!("nessun input in {}", inputs.display());
    }

    let budget = budget()?;
    let cal = Limits {
        address_space_bytes: budget
            .capacity_bytes()
            .saturating_sub(CALIBRATION_QUOTA_BYTES),
        cpu_seconds: FIXED_CPU_SECONDS,
        wall_timeout: FIXED_WALL_TIMEOUT,
        file_size_bytes: CALIBRATION_QUOTA_BYTES,
        max_open_files: FIXED_MAX_OPEN_FILES,
        out_quota_bytes: CALIBRATION_QUOTA_BYTES,
    };
    let csv_path = csv.unwrap_or_else(|| {
        let name = if op == "parse" {
            "calibration.csv".to_owned()
        } else {
            format!("calibration-{op}.csv")
        };
        out.parent().unwrap_or(Path::new(".")).join(name)
    });
    let header: Vec<&str> = CALIBRATION_HEADER.split(',').collect();
    let mut w = common::csv::Writer::create(&csv_path, &header)?;

    let area = calibration_area(op);
    let mut seen = BTreeSet::new();
    let (mut max_peak_kb, mut max_out, mut max_objstm, mut ok_docs) = (0u64, 0u64, 0u64, 0u32);
    let mut anomalies: Vec<String> = Vec::new();
    for f in &files {
        let mut id = doc_id(f);
        while !seen.insert(id.clone()) {
            id.push('x');
        }
        let job_id = format!("cal-{id}-{tag}");
        let bytes = job_bytes(&job_id, op, f, &area, &params);
        let spec = match prepare_job(&bytes) {
            Ok(s) => s,
            Err((jid, e)) => {
                print_job_error(jid.as_deref(), &e);
                return Ok(Exit(2));
            }
        };
        let r = match run_child(&spec, &cal, &budget).await {
            Ok(r) => r,
            Err(e) => {
                if let Some(je) = job_error_of(&e) {
                    print_job_error(Some(&job_id), je);
                    return Ok(Exit(2));
                }
                return Err(e);
            }
        };
        print_line(&report_line(&r));
        let out_bytes = spec.out_dir().map(dir_bytes).unwrap_or(0);
        let cell = |v: Option<String>| v.unwrap_or_default();
        let (pages, peak, ratio, stream, total, objstm, rejected) = match &r.outcome {
            ChildOutcome::Ok { pages, data, .. } => {
                let d = data.clone().unwrap_or(Value::Null);
                let p = &d["prescan"];
                let peak = d["vm_peak_kb"].as_u64();
                ok_docs += 1;
                max_peak_kb = max_peak_kb.max(peak.unwrap_or(0));
                max_out = max_out.max(out_bytes);
                max_objstm = max_objstm.max(p["objstm_count"].as_u64().unwrap_or(0));
                (
                    pages.map(|v| v.to_string()),
                    peak.map(|v| v.to_string()),
                    p["max_flate_ratio"].as_f64().map(|v| format!("{v:.2}")),
                    p["max_stream_bytes"].as_u64().map(|v| v.to_string()),
                    p["total_decompressed_bytes"]
                        .as_u64()
                        .map(|v| v.to_string()),
                    p["objstm_count"].as_u64().map(|v| v.to_string()),
                    "0".to_owned(),
                )
            }
            ChildOutcome::Failed { kind, detail } => {
                anomalies.push(format!("{id}: failed {} {detail}", kind.as_str()));
                let objstm = detail_number(detail, "objstm_count");
                if let Some(n) = objstm {
                    max_objstm = max_objstm.max(n);
                }
                (
                    None,
                    None,
                    None,
                    None,
                    None,
                    objstm.map(|v| v.to_string()),
                    u8::from(is_filter_rejection(*kind, detail)).to_string(),
                )
            }
            ChildOutcome::Quarantined { reasons } => {
                anomalies.push(format!("{id}: quarantined {}", reasons.join(";")));
                (None, None, None, None, None, None, "0".to_owned())
            }
        };
        let row = [
            id.clone(),
            cell(pages),
            cell(peak),
            out_bytes.to_string(),
            cell(ratio),
            cell(stream),
            cell(total),
            cell(objstm),
            rejected,
        ];
        let refs: Vec<&str> = row.iter().map(String::as_str).collect();
        w.write_row(&refs)?;
    }
    if ok_docs == 0 || max_peak_kb == 0 {
        bail!("calibrazione senza documenti riusciti con VmPeak: limiti non calcolabili");
    }

    let quota = max_out.saturating_mul(QUOTA_FACTOR);
    let limits = Limits {
        address_space_bytes: max_peak_kb.saturating_mul(1024).saturating_mul(AS_FACTOR),
        cpu_seconds: FIXED_CPU_SECONDS,
        wall_timeout: FIXED_WALL_TIMEOUT,
        file_size_bytes: quota.max(MIN_FILE_SIZE_BYTES),
        max_open_files: FIXED_MAX_OPEN_FILES,
        out_quota_bytes: quota,
    };
    let mut doc = match std::fs::read_to_string(out) {
        Ok(t) => serde_json::from_str::<Value>(&t)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default(),
        Err(_) => serde_json::Map::new(),
    };
    let lv = serde_json::to_value(&limits)?;
    if op == "parse" || !doc.contains_key("default") {
        doc.insert("default".into(), lv.clone());
    }
    doc.insert(op.to_owned(), lv);
    doc.insert(
        format!("_meta_{op}"),
        json!({
            "formula": "address_space_bytes = 3 × max VmPeak; out_quota_bytes = 2 × max byte scritti nell'out_dir; file_size_bytes = max(quota, 1 MiB); cpu_seconds 60, wall_timeout 120 s, max_open_files 64 fissi",
            "run_tag": tag,
            "inputs": inputs.to_string_lossy(),
            "docs": files.len(),
            "ok_docs": ok_docs,
            "max_vm_peak_kb": max_peak_kb,
            "max_out_bytes": max_out,
            "max_objstm_count": max_objstm,
            "object_streams_cap_formula": max_objstm.saturating_mul(2),
            "anomalies": anomalies,
        }),
    );
    std::fs::write(
        out,
        serde_json::to_string_pretty(&Value::Object(doc))? + "\n",
    )
    .with_context(|| format!("scrittura di {}", out.display()))?;
    print_line(
        &json!({
            "status": "calibration_summary",
            "op": op,
            "docs": files.len(),
            "ok_docs": ok_docs,
            "anomalies": anomalies.len(),
            "limits": serde_json::to_value(&limits)?,
        })
        .to_string(),
    );
    Ok(Exit(0))
}

// ---- driver hostile ---------------------------------------------------------------------

/// Regola di confronto del driver `hostile`: `(pass, extra_reasons)`.
///
/// `pass` solo se: `oom_adj_set`; `status` uguale all'atteso; per `failed` il `kind` è fra le
/// alternative dell'array; per `quarantined` i motivi attesi sono inclusi negli ottenuti (i
/// motivi in più tornano in `extra_reasons`); per `ok` l'array è vuoto.
fn hostile_verdict(
    expected_status: &str,
    expected: &[String],
    outcome: &ChildOutcome,
    oom_adj_set: bool,
) -> (bool, Vec<String>) {
    let extra: Vec<String> = match outcome {
        ChildOutcome::Quarantined { reasons } => reasons
            .iter()
            .filter(|r| !expected.contains(r))
            .cloned()
            .collect(),
        _ => Vec::new(),
    };
    if !oom_adj_set || outcome.status() != expected_status {
        return (false, extra);
    }
    let pass = match outcome {
        ChildOutcome::Ok { .. } => expected.is_empty(),
        ChildOutcome::Failed { kind, .. } => expected.iter().any(|e| e == kind.as_str()),
        ChildOutcome::Quarantined { reasons } => {
            !expected.is_empty() && expected.iter().all(|e| reasons.contains(e))
        }
    };
    (pass, extra)
}

struct Case {
    case_id: String,
    file: String,
    expected_status: String,
    expected: Vec<String>,
}

/// Legge del manifest solo i quattro campi del contratto; un caso non valido → `Err`.
fn read_manifest(path: &Path) -> anyhow::Result<Vec<Case>> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let arr = v.as_array().context("manifest: atteso un array di casi")?;
    let mut out = Vec::new();
    for (i, c) in arr.iter().enumerate() {
        let s = |k: &str| {
            c.get(k)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .with_context(|| format!("caso {i}: {k} mancante o non stringa"))
        };
        let (case_id, file, status) = (s("case_id")?, s("file")?, s("expected_status")?);
        if !matches!(status.as_str(), "ok" | "failed" | "quarantined") {
            bail!("caso {case_id}: expected_status {status:?} non ammesso");
        }
        let arr = c
            .get("expected_kind_or_reasons")
            .and_then(Value::as_array)
            .with_context(|| format!("caso {case_id}: expected_kind_or_reasons non è un array"))?;
        let expected = arr
            .iter()
            .map(|x| x.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
            .with_context(|| format!("caso {case_id}: elementi non stringa"))?;
        if status != "ok" && expected.is_empty() {
            bail!("caso {case_id}: array vuoto per {status}");
        }
        out.push(Case {
            case_id,
            file,
            expected_status: status,
            expected,
        });
    }
    Ok(out)
}

/// `job_id` della run: tutti validi e unici, altrimenti `Err` prima di qualsiasi job.
fn check_job_ids<'a>(ids: impl IntoIterator<Item = &'a String>) -> anyhow::Result<()> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !job::valid_job_id(id) {
            bail!("job_id non valido: {id}");
        }
        if !seen.insert(id.as_str()) {
            bail!("job_id duplicato: {id}");
        }
    }
    Ok(())
}

fn csv_writer(out: &Path, header: &str) -> anyhow::Result<common::csv::Writer> {
    let cols: Vec<&str> = header.split(',').collect();
    common::csv::Writer::create(out, &cols)
}

fn opt<T: ToString>(v: Option<T>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

async fn cmd_hostile(
    manifest: &Path,
    cases_dir: &Path,
    runs: u32,
    run_tag: &str,
    out: &Path,
    limits_path: &Path,
) -> anyhow::Result<Exit> {
    if !valid_run_tag(run_tag) {
        eprintln!("hostile: --run-tag fuori da [a-z0-9]{{8}}");
        return Ok(Exit(2));
    }
    let cases = match read_manifest(manifest) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("hostile: manifest non valido: {e:#}");
            return Ok(Exit(2));
        }
    };
    let ids: Vec<String> = cases
        .iter()
        .flat_map(|c| (1..=runs).map(move |r| format!("hst-{}-r{r}-{run_tag}", c.case_id)))
        .collect();
    if let Err(e) = check_job_ids(&ids) {
        eprintln!("hostile: {e:#}");
        return Ok(Exit(2));
    }
    let budget = budget()?;
    let limits = LimitsFile::load(limits_path)?.for_op("parse", &budget)?;
    let mut w = csv_writer(out, HOSTILE_HEADER)?;
    for c in &cases {
        let expected_col = format!("{}/{}", c.expected_status, c.expected.join("|"));
        for run in 1..=runs {
            let job_id = format!("hst-{}-r{run}-{run_tag}", c.case_id);
            let input = cases_dir.join(&c.file);
            let bytes = job_bytes(&job_id, "parse", &input, "isolation", &json!({}));
            let res = match prepare_job(&bytes) {
                Ok(spec) => run_child(&spec, &limits, &budget).await,
                Err((_, e)) => Err(e.into()),
            };
            let row: Vec<String> = match res {
                Ok(r) => {
                    let (pass, extra) =
                        hostile_verdict(&c.expected_status, &c.expected, &r.outcome, r.oom_adj_set);
                    let kind = match &r.outcome {
                        ChildOutcome::Failed { kind, .. } => kind.as_str().to_owned(),
                        ChildOutcome::Quarantined { reasons } => reasons.join(";"),
                        ChildOutcome::Ok { .. } => String::new(),
                    };
                    let failed = !matches!(r.outcome, ChildOutcome::Ok { .. });
                    vec![
                        c.case_id.clone(),
                        run.to_string(),
                        expected_col.clone(),
                        r.outcome.status().to_owned(),
                        kind,
                        extra.join(";"),
                        opt(r.exit_signal),
                        opt(r.peak_rss_kb),
                        if failed {
                            r.wall_ms.to_string()
                        } else {
                            String::new()
                        },
                        "true".to_owned(),
                        r.oom_kill_delta.to_string(),
                        r.leftover_procs.to_string(),
                        r.oom_adj_set.to_string(),
                        pass.to_string(),
                    ]
                }
                Err(e) => {
                    let (outcome, kind) = match job_error_of(&e) {
                        Some(je) => ("job_error", je.code.as_str().to_owned()),
                        None => {
                            eprintln!("hostile: launch_error job_id={job_id}: {e:#}");
                            ("launch_error", String::new())
                        }
                    };
                    vec![
                        c.case_id.clone(),
                        run.to_string(),
                        expected_col.clone(),
                        outcome.to_owned(),
                        kind,
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                        "true".to_owned(),
                        String::new(),
                        String::new(),
                        "false".to_owned(),
                        "false".to_owned(),
                    ]
                }
            };
            let refs: Vec<&str> = row.iter().map(String::as_str).collect();
            w.write_row(&refs)?;
        }
    }
    Ok(Exit(0))
}

// ---- driver budget ----------------------------------------------------------------------

fn parse_list(s: &str) -> anyhow::Result<Vec<usize>> {
    let v = s
        .split(',')
        .map(|x| x.trim().parse::<usize>())
        .collect::<Result<Vec<_>, _>>()
        .context("lista di concorrenze non valida")?;
    if v.is_empty() || v.contains(&0) {
        bail!("concorrenze ≥ 1 richieste");
    }
    Ok(v)
}

fn read_oom_kills() -> Option<u64> {
    std::fs::read_to_string("/sys/fs/cgroup/memory.events")
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("oom_kill "))
        .and_then(|v| v.trim().parse().ok())
}

/// File d'ingresso dei job `selftest` dei driver, sotto la radice dei job.
fn selftest_input(run_tag: &str) -> anyhow::Result<PathBuf> {
    let root = std::env::var_os("S02_WORK_ROOT")
        .filter(|v| !v.is_empty())
        .map_or_else(|| PathBuf::from("/work"), PathBuf::from);
    let p = root.join(format!("selftest-input-{run_tag}"));
    std::fs::write(&p, b"selftest\n").with_context(|| format!("ingresso {}", p.display()))?;
    Ok(p)
}

/// Esegue i job con al più `concurrency` in corsa; il budget ammette per conto suo.
async fn run_all(
    specs: Vec<ChildSpec>,
    limits: Limits,
    budget: Arc<Budget>,
    concurrency: usize,
) -> Vec<(String, anyhow::Result<RunReport>)> {
    let mut set = tokio::task::JoinSet::new();
    let mut queue = specs.into_iter();
    let mut out = Vec::new();
    loop {
        while set.len() < concurrency {
            let Some(spec) = queue.next() else { break };
            let (l, b) = (limits.clone(), budget.clone());
            set.spawn(async move {
                let r = run_child(&spec, &l, &b).await;
                (spec.job_id, r)
            });
        }
        match set.join_next().await {
            Some(Ok(v)) => out.push(v),
            Some(Err(e)) => out.push((String::new(), Err(anyhow::anyhow!("task: {e}")))),
            None => return out,
        }
    }
}

async fn cmd_budget(
    concurrency: &str,
    jobs: u32,
    reserve_fraction: f64,
    run_tag: &str,
    out: &Path,
) -> anyhow::Result<Exit> {
    let levels = match parse_list(concurrency) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("budget: {e:#}");
            return Ok(Exit(2));
        }
    };
    if !(valid_run_tag(run_tag) && reserve_fraction > 0.0 && reserve_fraction <= 1.0) {
        eprintln!("budget: --run-tag [a-z0-9]{{8}} e --reserve-fraction in (0, 1] richiesti");
        return Ok(Exit(2));
    }
    let ids: Vec<String> = levels
        .iter()
        .flat_map(|n| (0..jobs).map(move |k| format!("bud-c{n}-j{k}-{run_tag}")))
        .collect();
    if let Err(e) = check_job_ids(&ids) {
        eprintln!("budget: {e:#}");
        return Ok(Exit(2));
    }
    let input = selftest_input(run_tag)?;
    let mut w = csv_writer(out, BUDGET_HEADER)?;
    let mut not_isolated: Vec<String> = Vec::new();
    for &n in &levels {
        let budget = Arc::new(budget()?);
        let as_mib = ((budget.capacity_bytes() as f64 * reserve_fraction) as u64 / MIB).max(2);
        let limits = Limits {
            address_space_bytes: as_mib * MIB,
            cpu_seconds: FIXED_CPU_SECONDS,
            wall_timeout: FIXED_WALL_TIMEOUT,
            file_size_bytes: MIN_FILE_SIZE_BYTES,
            max_open_files: FIXED_MAX_OPEN_FILES,
            out_quota_bytes: 0,
        };
        let alloc = (as_mib / 2).max(1).to_string();
        let mut specs = Vec::new();
        for k in 0..jobs {
            let id = format!("bud-c{n}-j{k}-{run_tag}");
            let bytes = job_bytes(
                &id,
                "selftest",
                &input,
                "",
                &json!({"mode": "alloc", "arg": alloc}),
            );
            match prepare_job(&bytes) {
                Ok(s) => specs.push(s),
                Err((jid, e)) => {
                    print_job_error(jid.as_deref(), &e);
                    return Ok(Exit(2));
                }
            }
        }
        let before = read_oom_kills();
        let results = run_all(specs, limits, budget.clone(), n).await;
        let after = read_oom_kills();
        let mut completed = 0u32;
        for (id, r) in &results {
            match r {
                Ok(rep) => {
                    if matches!(rep.outcome, ChildOutcome::Ok { .. }) {
                        completed += 1;
                    }
                    if !rep.oom_adj_set {
                        not_isolated.push(id.clone());
                    }
                }
                Err(e) => eprintln!("budget: job_id={id}: {e:#}"),
            }
        }
        let delta = match (before, after) {
            (Some(b), Some(a)) => a.saturating_sub(b),
            _ => 0,
        };
        let row = [
            n.to_string(),
            jobs.to_string(),
            budget.max_admitted().to_string(),
            completed.to_string(),
            delta.to_string(),
            opt(s02_parser::protocol::vm_hwm_kb()),
            "true".to_owned(),
        ];
        let refs: Vec<&str> = row.iter().map(String::as_str).collect();
        w.write_row(&refs)?;
    }
    let _ = std::fs::remove_file(&input);
    if !not_isolated.is_empty() {
        eprintln!("oom_adj_set=false: {}", not_isolated.join(" "));
        return Ok(Exit(3));
    }
    Ok(Exit(0))
}

// ---- driver overhead --------------------------------------------------------------------

/// Modo `in-process-mutex`: PDFium nel processo padre, dietro il mutex globale della
/// feature `thread_safe`, solo per il confronto M9 (input fidato scelto dal piano 01-18).
fn in_process_run(
    pdfium: &pdfium_render::prelude::Pdfium,
    bytes: &[u8],
    docs: usize,
    n: usize,
) -> anyhow::Result<()> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| -> anyhow::Result<()> {
        let handles: Vec<_> = (0..n)
            .map(|_| {
                s.spawn(|| -> anyhow::Result<()> {
                    while next.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < docs {
                        let doc = pdfium
                            .load_pdf_from_byte_slice(bytes, None)
                            .map_err(|e| anyhow::anyhow!("pdfium: {e}"))?;
                        for page in doc.pages().iter() {
                            let _ = page.text().map(|t| t.all().len());
                        }
                    }
                    Ok(())
                })
            })
            .collect();
        for h in handles {
            h.join()
                .map_err(|_| anyhow::anyhow!("thread in-process"))??;
        }
        Ok(())
    })
}

async fn cmd_overhead(
    input: &Path,
    runs: u32,
    concurrency: &str,
    run_tag: &str,
    out: &Path,
    limits_path: &Path,
) -> anyhow::Result<Exit> {
    let levels = match parse_list(concurrency) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("overhead: {e:#}");
            return Ok(Exit(2));
        }
    };
    if !valid_run_tag(run_tag) {
        eprintln!("overhead: --run-tag fuori da [a-z0-9]{{8}}");
        return Ok(Exit(2));
    }
    let ids: Vec<String> = levels
        .iter()
        .flat_map(|n| {
            (1..=runs).flat_map(move |r| {
                (0..OVERHEAD_DOCS_PER_RUN).map(move |k| format!("ovh-cpd-c{n}-r{r}-d{k}-{run_tag}"))
            })
        })
        .collect();
    if let Err(e) = check_job_ids(&ids) {
        eprintln!("overhead: {e:#}");
        return Ok(Exit(2));
    }
    let budget = Arc::new(budget()?);
    let limits = LimitsFile::load(limits_path)?.for_op("parse", &budget)?;
    let mut w = csv_writer(out, OVERHEAD_HEADER)?;
    let mut not_isolated: Vec<String> = Vec::new();

    for &n in &levels {
        for run in 1..=runs {
            let mut specs = Vec::new();
            for k in 0..OVERHEAD_DOCS_PER_RUN {
                let id = format!("ovh-cpd-c{n}-r{run}-d{k}-{run_tag}");
                match prepare_job(&job_bytes(&id, "parse", input, "isolation", &json!({}))) {
                    Ok(s) => specs.push(s),
                    Err((jid, e)) => {
                        print_job_error(jid.as_deref(), &e);
                        return Ok(Exit(2));
                    }
                }
            }
            let t0 = Instant::now();
            let results = run_all(specs, limits.clone(), budget.clone(), n).await;
            let wall = t0.elapsed();
            let mut init_ms: Vec<f64> = Vec::new();
            for (id, r) in &results {
                if let Ok(rep) = r {
                    if !rep.oom_adj_set {
                        not_isolated.push(id.clone());
                    }
                    if let ChildOutcome::Ok { data: Some(d), .. } = &rep.outcome
                        && let Some(p) = d["parse_ms"].as_u64()
                    {
                        init_ms.push(rep.wall_ms.saturating_sub(p) as f64);
                    }
                } else if let Err(e) = r {
                    eprintln!("overhead: job_id={id}: {e:#}");
                }
            }
            let spawn_init = if init_ms.is_empty() {
                String::new()
            } else {
                format!("{:.1}", init_ms.iter().sum::<f64>() / init_ms.len() as f64)
            };
            let row = [
                "child-per-doc".to_owned(),
                n.to_string(),
                run.to_string(),
                OVERHEAD_DOCS_PER_RUN.to_string(),
                wall.as_millis().to_string(),
                spawn_init,
                format!("{:.2}", OVERHEAD_DOCS_PER_RUN as f64 / wall.as_secs_f64()),
            ];
            let refs: Vec<&str> = row.iter().map(String::as_str).collect();
            w.write_row(&refs)?;
        }
    }

    let bytes = std::fs::read(input).with_context(|| format!("input {}", input.display()))?;
    let dir = std::env::var_os("PDFIUM_DIR").unwrap_or_else(|| "/opt/pdfium/lib".into());
    let bindings = pdfium_render::prelude::Pdfium::bind_to_library(
        pdfium_render::prelude::Pdfium::pdfium_platform_library_name_at_path(&dir),
    )
    .map_err(|e| anyhow::anyhow!("pdfium_bind: {e}"))?;
    let pdfium = pdfium_render::prelude::Pdfium::new(bindings);
    for &n in &levels {
        for run in 1..=runs {
            let t0 = Instant::now();
            in_process_run(&pdfium, &bytes, OVERHEAD_DOCS_PER_RUN, n)?;
            let wall = t0.elapsed();
            let row = [
                "in-process-mutex".to_owned(),
                n.to_string(),
                run.to_string(),
                OVERHEAD_DOCS_PER_RUN.to_string(),
                wall.as_millis().to_string(),
                String::new(),
                format!("{:.2}", OVERHEAD_DOCS_PER_RUN as f64 / wall.as_secs_f64()),
            ];
            let refs: Vec<&str> = row.iter().map(String::as_str).collect();
            w.write_row(&refs)?;
        }
    }
    if !not_isolated.is_empty() {
        eprintln!("oom_adj_set=false: {}", not_isolated.join(" "));
        return Ok(Exit(3));
    }
    Ok(Exit(0))
}

#[cfg(test)]
mod tests {
    //! (j) Tabella di `hostile_verdict` (TEST-4, TEST-6).
    use super::*;
    use s02_parser::protocol::Kind;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    fn ok() -> ChildOutcome {
        ChildOutcome::Ok {
            pages: Some(1),
            output: None,
            data: None,
        }
    }

    fn failed(kind: Kind) -> ChildOutcome {
        ChildOutcome::Failed {
            kind,
            detail: "x".into(),
        }
    }

    fn quarantined(reasons: &[&str]) -> ChildOutcome {
        ChildOutcome::Quarantined {
            reasons: s(reasons),
        }
    }

    #[test]
    fn ok_expected_ok_with_oom_adj_passes() {
        assert_eq!(hostile_verdict("ok", &[], &ok(), true), (true, vec![]));
    }

    #[test]
    fn expected_outcome_without_oom_adj_fails() {
        assert!(!hostile_verdict("ok", &[], &ok(), false).0);
        assert!(!hostile_verdict("failed", &s(&["Corrupt"]), &failed(Kind::Corrupt), false).0);
        assert!(
            !hostile_verdict(
                "quarantined",
                &s(&["javascript"]),
                &quarantined(&["javascript"]),
                false
            )
            .0
        );
    }

    #[test]
    fn failed_kind_among_alternatives_passes() {
        let alt = s(&["Corrupt", "Crashed"]);
        assert!(hostile_verdict("failed", &alt, &failed(Kind::Corrupt), true).0);
        assert!(hostile_verdict("failed", &alt, &failed(Kind::Crashed), true).0);
    }

    #[test]
    fn failed_kind_outside_alternatives_fails() {
        let alt = s(&["Corrupt", "Crashed"]);
        assert!(!hostile_verdict("failed", &alt, &failed(Kind::Timeout), true).0);
    }

    #[test]
    fn quarantined_superset_passes_with_extra_reasons() {
        let (pass, extra) = hostile_verdict(
            "quarantined",
            &s(&["javascript", "launch"]),
            &quarantined(&["launch", "embedded_file", "javascript"]),
            true,
        );
        assert!(pass);
        assert_eq!(extra, s(&["embedded_file"]));
    }

    #[test]
    fn quarantined_missing_expected_reason_fails() {
        let (pass, _) = hostile_verdict(
            "quarantined",
            &s(&["javascript", "launch"]),
            &quarantined(&["javascript"]),
            true,
        );
        assert!(!pass);
    }

    #[test]
    fn different_status_fails() {
        assert!(!hostile_verdict("failed", &s(&["Corrupt"]), &ok(), true).0);
        assert!(
            !hostile_verdict(
                "quarantined",
                &s(&["javascript"]),
                &failed(Kind::Corrupt),
                true
            )
            .0
        );
        assert!(!hostile_verdict("ok", &[], &quarantined(&["javascript"]), true).0);
    }
}
