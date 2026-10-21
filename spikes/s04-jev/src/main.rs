//! Spike S-04/A-09: forma della chiamata `rank` a JEV, concorrenza, alias.
//!
//! Sottocomandi (da `spikes/`, nel runner via `session.sh oneshot`):
//! - `sets`: elenca le 30 query scelte con il seme (nessuna rete);
//! - `estimate --form-reps 3 --levels 4,8,16 --runs 3`: stima token e costo (nessuna rete);
//! - `aliases --names jev-latest,jev-1.13.0,jev-preview`;
//! - `concurrency --levels 4,8,16 --runs 3`;
//! - `forms --form a|b|c --reps 3 --concurrency <n>`;
//! - `collect`: dai grezzi di `results/jev/raw/` scrive `forms.csv`,
//!   `concurrency.csv`, `aliases.csv` (serve `hw.json` accanto) e stampa le tabelle.
//!
//! Ogni invio passa dalla guardia di visibilità del client; il budget
//! `S04_MAX_COST_USD` vale per l'intera sessione grazie al registro di spesa
//! `raw/spend.json`, che ogni sottocomando legge prima di partire e aggiorna
//! dopo ogni unità di lavoro.
//!
//! Modifica: Claude (gsd-executor 01-11) · 2026-10-21 · CLI dello spike S-04

use anyhow::{Context, bail};
use clap::{Parser, Subcommand, ValueEnum};
use common::hist::Recorder;
use common::timer::Stopwatch;
use s04_jev::candidates::{self, Acl, CandidateSet};
use s04_jev::jev::{Client, ClientConfig, Form, JevError, NOUL_TRUE, RankOutcome};
use s04_jev::metrics::{ndcg_at_k, recall_at_k};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Seme della selezione delle query, registrato in RISULTATI.
const SEED: u64 = 0x0111_5EED;
const SET_SIZE: usize = 20;

const FORMS_HEADER: [&str; 16] = [
    "form",
    "query_id",
    "rep",
    "concurrency",
    "status",
    "latency_ms",
    "input_tokens",
    "output_tokens",
    "model_resolved",
    "retry_after",
    "ndcg_at_5",
    "recall_at_8",
    "noul_relevant_std",
    "instructs_model_flagged",
    "unauthorized_sent",
    "questions_sha256",
];

const CONCURRENCY_HEADER: [&str; 13] = [
    "level",
    "run",
    "requests",
    "ok",
    "status_429",
    "status_529",
    "other_errors",
    "p50_ms",
    "p95_ms",
    "p99_ms",
    "throughput_rps",
    "retries",
    "cost_usd",
];

const ALIASES_HEADER: [&str; 5] = [
    "alias",
    "request_id",
    "model_resolved",
    "status",
    "latency_ms",
];

#[derive(Parser)]
#[command(about = "Spike S-04: forma del rank JEV, concorrenza e alias")]
struct Cli {
    /// Cartella del corpus comune (sola lettura).
    #[arg(long, default_value = "data/corpus")]
    corpus: PathBuf,
    /// Cartella dei risultati della parte JEV.
    #[arg(long, default_value = "s04-jev/results/jev")]
    results: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
enum FormArg {
    A,
    B,
    C,
}

impl From<FormArg> for Form {
    fn from(f: FormArg) -> Form {
        match f {
            FormArg::A => Form::A,
            FormArg::B => Form::B,
            FormArg::C => Form::C,
        }
    }
}

#[derive(Subcommand)]
enum Cmd {
    /// Elenca le query scelte con il seme, senza rete.
    Sets {
        #[arg(long, default_value_t = 30)]
        queries: usize,
    },
    /// Stima di token e costo della sessione, senza rete.
    Estimate {
        #[arg(long, default_value_t = 30)]
        queries: usize,
        #[arg(long, default_value_t = 3)]
        form_reps: usize,
        #[arg(long, value_delimiter = ',', default_value = "4,8,16")]
        levels: Vec<usize>,
        #[arg(long, default_value_t = 3)]
        runs: usize,
        #[arg(long, default_value_t = 10)]
        concurrency_queries: usize,
    },
    /// Una richiesta di forma A per alias, `response.model` registrato.
    Aliases {
        #[arg(
            long,
            value_delimiter = ',',
            default_value = "jev-latest,jev-1.13.0,jev-preview"
        )]
        names: Vec<String>,
    },
    /// Forma A a vari livelli di concorrenza.
    Concurrency {
        #[arg(long, value_delimiter = ',', default_value = "4,8,16")]
        levels: Vec<usize>,
        #[arg(long, default_value_t = 3)]
        runs: usize,
        /// Query per run (20 candidati ciascuna).
        #[arg(long, default_value_t = 10)]
        queries: usize,
        #[arg(long, default_value_t = 10_000)]
        deadline_ms: u64,
    },
    /// Una forma su tutte le query, con ripetizioni e warm-up.
    Forms {
        #[arg(long, value_enum)]
        form: FormArg,
        #[arg(long, default_value_t = 3)]
        reps: usize,
        #[arg(long, default_value_t = 8)]
        concurrency: usize,
        #[arg(long, default_value_t = 30)]
        queries: usize,
        #[arg(long, default_value_t = 10_000)]
        deadline_ms: u64,
    },
    /// Grezzi → CSV e tabelle.
    Collect,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let raw = cli.results.join("raw");
    match cli.cmd {
        Cmd::Sets { queries } => cmd_sets(&cli.corpus, queries),
        Cmd::Estimate {
            queries,
            form_reps,
            levels,
            runs,
            concurrency_queries,
        } => cmd_estimate(
            &cli.corpus,
            &raw,
            queries,
            form_reps,
            &levels,
            runs,
            concurrency_queries,
        ),
        Cmd::Aliases { names } => cmd_aliases(&cli.corpus, &raw, &names).await,
        Cmd::Concurrency {
            levels,
            runs,
            queries,
            deadline_ms,
        } => {
            cmd_concurrency(
                &cli.corpus,
                &raw,
                &levels,
                runs,
                queries,
                Duration::from_millis(deadline_ms),
            )
            .await
        }
        Cmd::Forms {
            form,
            reps,
            concurrency,
            queries,
            deadline_ms,
        } => {
            cmd_forms(
                &cli.corpus,
                &raw,
                form.into(),
                reps,
                concurrency,
                queries,
                Duration::from_millis(deadline_ms),
            )
            .await
        }
        Cmd::Collect => cmd_collect(&cli.corpus, &cli.results),
    }
}

// ---------------------------------------------------------------- registro di spesa

fn ledger_path(raw: &Path) -> PathBuf {
    raw.join("spend.json")
}

fn ledger_read(raw: &Path) -> anyhow::Result<f64> {
    let path = ledger_path(raw);
    if !path.exists() {
        return Ok(0.0);
    }
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)
        .with_context(|| format!("{} non valido", path.display()))?;
    v["spent_usd"]
        .as_f64()
        .with_context(|| format!("{}: manca spent_usd", path.display()))
}

fn ledger_write(raw: &Path, spent: f64) -> anyhow::Result<()> {
    std::fs::create_dir_all(raw)?;
    std::fs::write(
        ledger_path(raw),
        serde_json::to_string(&json!({"spent_usd": spent}))?,
    )?;
    Ok(())
}

/// Client con il budget residuo della sessione (budget − spesa registrata).
fn client_for(corpus: &Path, raw: &Path, model: Option<&str>) -> anyhow::Result<(Client, f64)> {
    let mut cfg = ClientConfig::from_env()?;
    let before = ledger_read(raw)?;
    let remaining = cfg.max_cost_usd - before;
    if remaining <= 0.0 {
        bail!("budget S04_MAX_COST_USD esaurito dalla sessione (registro raw/spend.json)");
    }
    cfg.max_cost_usd = remaining;
    if let Some(m) = model {
        cfg.model = m.to_owned();
    }
    let client = Client::new(cfg, Acl::load(corpus)?)?;
    Ok((client, before))
}

fn sets(corpus: &Path, n: usize) -> anyhow::Result<Vec<CandidateSet>> {
    candidates::build_candidate_sets(corpus, n, SET_SIZE, SEED)
}

fn append_jsonl(path: &Path, v: &Value) -> anyhow::Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("apertura di {}", path.display()))?;
    writeln!(f, "{}", serde_json::to_string(v)?)?;
    Ok(())
}

fn fresh(path: &Path) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- sets, estimate

fn cmd_sets(corpus: &Path, n: usize) -> anyhow::Result<()> {
    let sets = sets(corpus, n)?;
    let inj = candidates::injection_passage_ids(corpus)?;
    println!(
        "seme 0x{SEED:X}, {} query, {SET_SIZE} passaggi ciascuna",
        sets.len()
    );
    let mut by_lang: BTreeMap<&str, usize> = BTreeMap::new();
    for s in &sets {
        *by_lang.entry(s.lang.as_str()).or_default() += 1;
        let n_inj = s.passages.iter().filter(|p| inj.contains(&p.id)).count();
        println!(
            "{} lang={} role={} rilevanti={} injection={}",
            s.query.id,
            s.lang,
            s.query.role_profile,
            s.relevant_ids.len(),
            n_inj
        );
    }
    println!("mix: {by_lang:?}");
    Ok(())
}

/// Token stimati di una richiesta: costo fisso osservato in 01-05 (332 token
/// per circa 270 caratteri) arrotondato per eccesso, più 1 token ogni 3 caratteri.
fn est_tokens(chars: usize) -> u64 {
    300 + (chars as u64).div_ceil(3)
}

fn cmd_estimate(
    corpus: &Path,
    raw: &Path,
    queries: usize,
    form_reps: usize,
    levels: &[usize],
    runs: usize,
    conc_queries: usize,
) -> anyhow::Result<()> {
    let sets = sets(corpus, queries)?;
    let wa = s04_jev::jev::FORM_A_JSON.len();
    let wb = s04_jev::jev::FORM_B_JSON.len();
    let wc = s04_jev::jev::FORM_C_JSON.len();
    let per_query_a = |s: &CandidateSet| -> u64 {
        s.passages
            .iter()
            .map(|p| est_tokens(s.query.text.len() + p.text.len() + wa))
            .sum()
    };
    let all_text = |s: &CandidateSet| -> usize {
        s.query.text.len() + s.passages.iter().map(|p| p.text.len()).sum::<usize>()
    };
    let reps = form_reps as u64 + 1; // warm-up compreso, per eccesso su ogni query
    let a: u64 = sets.iter().map(per_query_a).sum::<u64>() * reps;
    let b: u64 = sets
        .iter()
        .map(|s| est_tokens(all_text(s) + wb * SET_SIZE))
        .sum::<u64>()
        * reps;
    let c: u64 = sets
        .iter()
        .map(|s| est_tokens(all_text(s) + wc + 16 * SET_SIZE))
        .sum::<u64>()
        * reps;
    let conc: u64 =
        sets.iter().take(conc_queries).map(per_query_a).sum::<u64>() * (levels.len() * runs) as u64;
    let aliases = 3 * est_tokens(400 + wa);
    let total = a + b + c + conc + aliases;
    let usd = s04_jev::jev::cost_usd(total, 0);
    let cfg_budget = ClientConfig::from_env().map(|c| c.max_cost_usd).ok();
    let spent = ledger_read(raw)?;
    println!("stima token input: A {a}, B {b}, C {c}, concorrenza {conc}, alias {aliases}");
    println!("stima totale: {total} token, {usd:.4} USD (output gratuito)");
    match cfg_budget {
        Some(budget) => println!(
            "stima + spesa registrata entro S04_MAX_COST_USD: {}",
            if usd + spent <= budget { "sì" } else { "NO" }
        ),
        None => println!("S04_MAX_COST_USD non leggibile: confronto non eseguito"),
    }
    Ok(())
}

// ---------------------------------------------------------------- aliases

async fn cmd_aliases(corpus: &Path, raw: &Path, names: &[String]) -> anyhow::Result<()> {
    let path = raw.join("aliases.jsonl");
    fresh(&path)?;
    // Una query con un passaggio pubblico: il più piccolo stato possibile.
    let all = sets(corpus, 30)?;
    let set = all
        .iter()
        .find(|s| s.query.role_profile == "guest")
        .context("nessuna query guest nel campione")?;
    let passage = std::slice::from_ref(&set.passages[0]);
    let mut spent = ledger_read(raw)?;
    for name in names {
        let (client, before) = client_for(corpus, raw, Some(name))?;
        let res = client
            .rank_form_a_each(&set.query, passage, 1, Duration::from_secs(30))
            .await;
        let (status, meta) = match res {
            Err(e) => bail!("alias {name}: {e}"),
            Ok(mut v) => {
                let o = v.remove(0);
                let status = match &o.result {
                    Ok(_) => o.meta.status.to_string(),
                    Err(e) => e.label(),
                };
                (status, o.meta)
            }
        };
        spent = before + client.spent_usd();
        ledger_write(raw, spent)?;
        if client.unauthorized_sent() != 0 {
            bail!("hard gate: unauthorized_sent > 0");
        }
        println!(
            "{name} → {} (status {status}, {} ms)",
            meta.model_resolved.as_deref().unwrap_or("-"),
            meta.latency_ms
        );
        append_jsonl(
            &path,
            &json!({
                "alias": name,
                "request_id": meta.request_id,
                "model_resolved": meta.model_resolved,
                "status": status,
                "latency_ms": meta.latency_ms,
                "input_tokens": meta.input_tokens,
            }),
        )?;
    }
    println!("spesa registrata della sessione: {spent:.6} USD");
    Ok(())
}

// ---------------------------------------------------------------- concurrency

async fn cmd_concurrency(
    corpus: &Path,
    raw: &Path,
    levels: &[usize],
    runs: usize,
    queries: usize,
    deadline: Duration,
) -> anyhow::Result<()> {
    let path = raw.join("concurrency.jsonl");
    fresh(&path)?;
    let all = sets(corpus, queries)?;
    let (client, before) = client_for(corpus, raw, None)?;
    for &level in levels {
        for run in 1..=runs {
            let wall = Stopwatch::start();
            let mut stop: Option<String> = None;
            for set in &all {
                let outcomes = client
                    .rank_form_a_each(&set.query, &set.passages, level, deadline)
                    .await
                    .map_err(|e| anyhow::anyhow!("{}: {e}", set.query.id))?;
                for o in outcomes {
                    let (ok, label) = match &o.result {
                        Ok(_) => (true, o.meta.status.to_string()),
                        Err(e) => (false, e.label()),
                    };
                    if matches!(o.result, Err(JevError::BudgetExceeded { .. })) {
                        stop = Some("budget esaurito".to_owned());
                    }
                    append_jsonl(
                        &path,
                        &json!({
                            "kind": "request", "level": level, "run": run,
                            "query_id": set.query.id, "passage_id": o.passage_id,
                            "ok": ok, "status": label,
                            "rate_limited": matches!(o.result, Err(JevError::RateLimited { .. })),
                            "latency_ms": o.meta.latency_ms,
                            "throttled_429": o.meta.throttled_429,
                            "throttled_529": o.meta.throttled_529,
                            "retries": o.meta.retries,
                            "retry_after": o.meta.retry_after,
                            "input_tokens": o.meta.input_tokens,
                            "model_resolved": o.meta.model_resolved,
                        }),
                    )?;
                }
                ledger_write(raw, before + client.spent_usd())?;
                if stop.is_some() {
                    break;
                }
            }
            let cost = before + client.spent_usd();
            append_jsonl(
                &path,
                &json!({"kind": "run", "level": level, "run": run,
                        "wall_ms": wall.elapsed_ms_f64(), "cost_usd_cum": cost,
                        "unauthorized_sent": client.unauthorized_sent()}),
            )?;
            println!(
                "livello {level} run {run}: {:.0} ms, spesa cumulata {cost:.6} USD",
                wall.elapsed_ms_f64()
            );
            if client.unauthorized_sent() != 0 {
                bail!("hard gate: unauthorized_sent > 0");
            }
            if let Some(why) = stop {
                bail!("sessione interrotta: {why}");
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- forms

async fn cmd_forms(
    corpus: &Path,
    raw: &Path,
    form: Form,
    reps: usize,
    concurrency: usize,
    queries: usize,
    deadline: Duration,
) -> anyhow::Result<()> {
    let path = raw.join(format!("forms-{}.jsonl", form.as_str()));
    fresh(&path)?;
    let all = sets(corpus, queries)?;
    let (client, before) = client_for(corpus, raw, None)?;
    let sha = form.questions_sha256();
    // rep 0 = warm-up sulla prima query, registrato e poi escluso dagli aggregati.
    let mut plan: Vec<(&CandidateSet, usize)> = vec![(&all[0], 0)];
    for set in &all {
        for rep in 1..=reps {
            plan.push((set, rep));
        }
    }
    for (set, rep) in plan {
        let unauth_before = client.unauthorized_sent();
        let sw = Stopwatch::start();
        let mut rec = json!({
            "form": form.as_str(), "query_id": set.query.id, "rep": rep,
            "concurrency": if form == Form::A { concurrency } else { 1 },
            "questions_sha256": sha,
            "passages": set.passages.iter().map(|p| p.id.clone()).collect::<Vec<_>>(),
            "relevant_ids": set.relevant_ids,
        });
        let mut budget_hit = false;
        match form {
            Form::A => {
                let outcomes = client
                    .rank_form_a_each(&set.query, &set.passages, concurrency, deadline)
                    .await
                    .map_err(|e| anyhow::anyhow!("{}: {e}", set.query.id))?;
                let mut scores = serde_json::Map::new();
                let mut instructs = serde_json::Map::new();
                let mut first_err: Option<String> = None;
                let metas: Vec<_> = outcomes.iter().map(|o| o.meta.clone()).collect();
                for o in &outcomes {
                    match &o.result {
                        Ok(s) => {
                            scores.insert(o.passage_id.clone(), s.relevant.into());
                            instructs.insert(o.passage_id.clone(), s.instructs_model.into());
                        }
                        Err(e) => {
                            budget_hit |= matches!(e, JevError::BudgetExceeded { .. });
                            first_err.get_or_insert_with(|| e.label());
                        }
                    }
                }
                fill_meta(&mut rec, &metas);
                rec["status"] = first_err.unwrap_or_else(|| "200".to_owned()).into();
                rec["scores"] = scores.into();
                rec["instructs"] = instructs.into();
            }
            Form::B | Form::C => {
                let res = if form == Form::B {
                    client
                        .rank_form_b(&set.query, &set.passages, deadline)
                        .await
                } else {
                    client
                        .rank_form_c(&set.query, &set.passages, deadline)
                        .await
                };
                match res {
                    Ok(RankOutcome { relevance, calls }) => {
                        fill_meta(&mut rec, &calls);
                        rec["status"] = "200".into();
                        rec["scores"] = relevance
                            .into_iter()
                            .map(|(id, v)| (id, Value::from(v)))
                            .collect::<serde_json::Map<_, _>>()
                            .into();
                    }
                    Err(JevError::Unauthorized { reason }) => {
                        bail!("{}: guardia di visibilità: {reason}", set.query.id)
                    }
                    Err(e) => {
                        budget_hit |= matches!(e, JevError::BudgetExceeded { .. });
                        rec["status"] = e.label().into();
                        rec["scores"] = json!({});
                    }
                }
            }
        }
        rec["latency_ms"] = (sw.elapsed_ms_f64().round() as u64).into();
        rec["unauthorized_sent"] = (client.unauthorized_sent() - unauth_before).into();
        append_jsonl(&path, &rec)?;
        ledger_write(raw, before + client.spent_usd())?;
        println!(
            "forma {} {} rep {rep}: status {} in {} ms",
            form.as_str(),
            set.query.id,
            rec["status"].as_str().unwrap_or("?"),
            rec["latency_ms"]
        );
        if client.unauthorized_sent() != 0 {
            bail!("hard gate: unauthorized_sent > 0");
        }
        if budget_hit {
            bail!("sessione interrotta: budget esaurito");
        }
    }
    println!(
        "spesa registrata della sessione: {:.6} USD",
        before + client.spent_usd()
    );
    Ok(())
}

fn fill_meta(rec: &mut Value, metas: &[s04_jev::jev::CallMeta]) {
    let models: BTreeSet<&str> = metas
        .iter()
        .filter_map(|m| m.model_resolved.as_deref())
        .collect();
    rec["model_resolved"] = models.into_iter().collect::<Vec<_>>().join("|").into();
    rec["input_tokens"] = metas
        .iter()
        .filter_map(|m| m.input_tokens)
        .sum::<u64>()
        .into();
    rec["output_tokens"] = metas
        .iter()
        .filter_map(|m| m.output_tokens)
        .sum::<u64>()
        .into();
    rec["retry_after"] = metas
        .iter()
        .rev()
        .find_map(|m| m.retry_after.clone())
        .unwrap_or_default()
        .into();
    rec["retries"] = metas.iter().map(|m| m.retries).sum::<u32>().into();
    rec["throttled_429"] = metas.iter().map(|m| m.throttled_429).sum::<u32>().into();
    rec["throttled_529"] = metas.iter().map(|m| m.throttled_529).sum::<u32>().into();
}

// ---------------------------------------------------------------- collect

fn read_jsonl(path: &Path) -> anyhow::Result<Vec<Value>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    std::fs::read_to_string(path)?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).with_context(|| format!("riga di {}", path.display())))
        .collect()
}

/// Ordine per segnale decrescente; a parità, l'ordine dell'insieme.
fn ranking(rec: &Value) -> Vec<String> {
    let order: Vec<String> = rec["passages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    let mut scored: Vec<(usize, String, f64)> = order
        .into_iter()
        .enumerate()
        .filter_map(|(i, id)| rec["scores"][&id].as_f64().map(|s| (i, id, s)))
        .collect();
    scored.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
    scored.into_iter().map(|(_, id, _)| id).collect()
}

fn sample_std(v: &[f64]) -> Option<f64> {
    if v.len() < 2 {
        return None;
    }
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (v.len() - 1) as f64;
    Some(var.sqrt())
}

/// Percentile nearest-rank su valori ordinati.
fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = ((p / 100.0) * sorted.len() as f64).ceil().max(1.0) as usize;
    sorted[rank.min(sorted.len()) - 1]
}

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        f64::NAN
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

fn s(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(x) => x.clone(),
        other => other.to_string(),
    }
}

fn cmd_collect(corpus: &Path, results: &Path) -> anyhow::Result<()> {
    let raw = results.join("raw");
    let injection = candidates::injection_passage_ids(corpus)?;

    // ---- forms.csv
    let mut forms = common::csv::Writer::create(&results.join("forms.csv"), &FORMS_HEADER)?;
    println!("## Forme (rep 1..3, warm-up escluso)\n");
    println!(
        "| Forma | righe ok / totali | nDCG@5 | recall@8 | latenza p50 / p95 / p99 ms | token input medi | std Noul per candidato media / p95 / max | sopra 0,05 |"
    );
    println!("|---|---|---|---|---|---|---|---|");
    let mut inj_report = String::new();
    for form in ["a", "b", "c"] {
        let recs = read_jsonl(&raw.join(format!("forms-{form}.jsonl")))?;
        if recs.is_empty() {
            continue;
        }
        // std per candidato fra le ripetizioni 1..n.
        let mut per_cand: HashMap<(String, String), Vec<f64>> = HashMap::new();
        for r in recs.iter().filter(|r| r["rep"].as_u64() != Some(0)) {
            if let Some(m) = r["scores"].as_object() {
                for (pid, v) in m {
                    if let Some(x) = v.as_f64() {
                        per_cand
                            .entry((s(&r["query_id"]), pid.clone()))
                            .or_default()
                            .push(x);
                    }
                }
            }
        }
        let mut std_by_query: HashMap<String, f64> = HashMap::new();
        let mut stds: Vec<f64> = Vec::new();
        for ((q, _), v) in &per_cand {
            if let Some(sd) = sample_std(v) {
                stds.push(sd);
                let e = std_by_query.entry(q.clone()).or_insert(0.0);
                *e = e.max(sd);
            }
        }
        stds.sort_by(f64::total_cmp);

        let (mut nd, mut rc, mut lat, mut tok) = (vec![], vec![], vec![], vec![]);
        let (mut ok_rows, mut rows) = (0usize, 0usize);
        let (mut inj_sent, mut inj_flag, mut clean_sent, mut clean_flag) = (0, 0, 0, 0);
        for r in &recs {
            let ok = r["status"] == "200";
            let ranked = ranking(r);
            let ranked_ref: Vec<&str> = ranked.iter().map(String::as_str).collect();
            let rel: Vec<&str> = r["relevant_ids"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            let (n5, r8) = if ok {
                (
                    format!("{:.4}", ndcg_at_k(&ranked_ref, &rel, 5)),
                    format!("{:.4}", recall_at_k(&ranked_ref, &rel, 8)),
                )
            } else {
                (String::new(), String::new())
            };
            let flagged = if form == "a" {
                let mut n = 0;
                for (pid, v) in r["instructs"].as_object().into_iter().flatten() {
                    let hit = v.as_f64().is_some_and(|x| x >= NOUL_TRUE);
                    n += usize::from(hit);
                    if r["rep"].as_u64() != Some(0) {
                        if injection.contains(pid) {
                            inj_sent += 1;
                            inj_flag += usize::from(hit);
                        } else {
                            clean_sent += 1;
                            clean_flag += usize::from(hit);
                        }
                    }
                }
                n.to_string()
            } else {
                "na".to_owned()
            };
            let std_q = if r["rep"].as_u64() == Some(0) {
                String::new()
            } else {
                std_by_query
                    .get(&s(&r["query_id"]))
                    .map(|v| format!("{v:.4}"))
                    .unwrap_or_default()
            };
            forms.write_row(&[
                form,
                &s(&r["query_id"]),
                &s(&r["rep"]),
                &s(&r["concurrency"]),
                &s(&r["status"]),
                &s(&r["latency_ms"]),
                &s(&r["input_tokens"]),
                &s(&r["output_tokens"]),
                &s(&r["model_resolved"]),
                &s(&r["retry_after"]),
                &n5,
                &r8,
                &std_q,
                &flagged,
                &s(&r["unauthorized_sent"]),
                &s(&r["questions_sha256"]),
            ])?;
            if r["rep"].as_u64() == Some(0) {
                continue;
            }
            rows += 1;
            if ok {
                ok_rows += 1;
                nd.push(ndcg_at_k(&ranked_ref, &rel, 5));
                rc.push(recall_at_k(&ranked_ref, &rel, 8));
                lat.push(r["latency_ms"].as_f64().unwrap_or(f64::NAN));
                tok.push(r["input_tokens"].as_f64().unwrap_or(f64::NAN));
            }
        }
        lat.sort_by(f64::total_cmp);
        let over = stds.iter().filter(|x| **x > 0.05).count();
        println!(
            "| {} | {ok_rows} / {rows} | {:.4} | {:.4} | {:.0} / {:.0} / {:.0} | {:.0} | {:.4} / {:.4} / {:.4} | {over} su {} |",
            form.to_uppercase(),
            mean(&nd),
            mean(&rc),
            pct(&lat, 50.0),
            pct(&lat, 95.0),
            pct(&lat, 99.0),
            mean(&tok),
            mean(&stds),
            pct(&stds, 95.0),
            stds.last().copied().unwrap_or(f64::NAN),
            stds.len()
        );
        if form == "a" {
            inj_report = format!(
                "instructs_model ≥ {NOUL_TRUE}: excerpt injection-case {inj_flag} su {inj_sent} invii, altri excerpt {clean_flag} su {clean_sent}"
            );
        }
    }
    println!("\n{inj_report}\n");

    // ---- concurrency.csv
    let recs = read_jsonl(&raw.join("concurrency.jsonl"))?;
    if !recs.is_empty() {
        let mut w =
            common::csv::Writer::create(&results.join("concurrency.csv"), &CONCURRENCY_HEADER)?;
        println!("## Concorrenza\n");
        println!(
            "| Livello | run | richieste | ok | 429 | 529 | altri errori | p50 / p95 / p99 ms | throughput rps | retry | spesa cumulata USD |"
        );
        println!("|---|---|---|---|---|---|---|---|---|---|---|");
        let runs: Vec<&Value> = recs.iter().filter(|r| r["kind"] == "run").collect();
        for run in runs {
            let (level, n) = (run["level"].as_u64(), run["run"].as_u64());
            let reqs: Vec<&Value> = recs
                .iter()
                .filter(|r| {
                    r["kind"] == "request" && r["level"].as_u64() == level && r["run"].as_u64() == n
                })
                .collect();
            let ok = reqs.iter().filter(|r| r["ok"] == true).count();
            let sum = |k: &str| reqs.iter().map(|r| r[k].as_u64().unwrap_or(0)).sum::<u64>();
            let other = reqs
                .iter()
                .filter(|r| r["ok"] == false && r["rate_limited"] != true)
                .count();
            let mut hist = Recorder::new();
            for r in &reqs {
                hist.record_us(r["latency_ms"].as_u64().unwrap_or(0) * 1000);
            }
            let sm = hist.summary();
            let wall_s = run["wall_ms"].as_f64().unwrap_or(f64::NAN) / 1000.0;
            let fields = [
                s(&run["level"]),
                s(&run["run"]),
                reqs.len().to_string(),
                ok.to_string(),
                sum("throttled_429").to_string(),
                sum("throttled_529").to_string(),
                other.to_string(),
                (sm.p50_us / 1000).to_string(),
                (sm.p95_us / 1000).to_string(),
                (sm.p99_us / 1000).to_string(),
                format!("{:.2}", ok as f64 / wall_s),
                sum("retries").to_string(),
                format!("{:.6}", run["cost_usd_cum"].as_f64().unwrap_or(f64::NAN)),
            ];
            let refs: Vec<&str> = fields.iter().map(String::as_str).collect();
            w.write_row(&refs)?;
            println!("| {} |", fields.join(" | "));
        }
        let models: BTreeSet<String> = recs
            .iter()
            .filter(|r| r["kind"] == "request")
            .map(|r| s(&r["model_resolved"]))
            .collect();
        println!("\nmodelli risolti (concorrenza): {models:?}\n");
    }

    // ---- aliases.csv
    let recs = read_jsonl(&raw.join("aliases.jsonl"))?;
    if !recs.is_empty() {
        let mut w = common::csv::Writer::create(&results.join("aliases.csv"), &ALIASES_HEADER)?;
        println!("## Alias\n");
        for r in &recs {
            let fields = [
                s(&r["alias"]),
                s(&r["request_id"]),
                s(&r["model_resolved"]),
                s(&r["status"]),
                s(&r["latency_ms"]),
            ];
            let refs: Vec<&str> = fields.iter().map(String::as_str).collect();
            w.write_row(&refs)?;
            println!("- {}", fields.join(" · "));
        }
    }
    let seen: HashSet<String> = injection.into_iter().collect();
    println!(
        "\nexcerpt injection-case nel corpus ratificato: {}",
        seen.len()
    );
    Ok(())
}
