//! Spike S-05: cancellazione e deadline delle richieste HTTP verso i provider.
//!
//! Sottocomandi (da `spikes/`, nel runner Linux via `session.sh oneshot`):
//! - `mock --proto h1|h2 --cancel-at-ms 50,200,500 --cycles 10000`
//! - `wiremock --cancel-at-ms 50,200,500 --cycles 10000`
//! - `deadline --variant a|b --fanout 8,16,32 --budget-ms 200,500,1000 --runs 3`
//! - `jev --cancel-at-ms 50` (numero di richieste da `S05_JEV_CANCEL_REQUESTS`)
//! - `collect`: unisce i grezzi di `results/raw/` in `cancel.csv` e `deadline.csv`
//!
//! Ogni sottocomando verifica `results/hw.json` prima di misurare.

mod client;
mod jev;
mod probe;
mod server;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand, ValueEnum};
use client::{FanoutOutcome, Proto};
use common::hist::Recorder;
use futures::StreamExt;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CANCEL_HEADER: [&str; 11] = [
    "scenario",
    "proto",
    "cancel_at_ms",
    "cycle",
    "exit_after_cancel_us",
    "server_saw_cancel_ms",
    "server_dropped",
    "server_completed",
    "alive_tasks",
    "open_fds",
    "established_conns",
];

const DEADLINE_HEADER: [&str; 9] = [
    "variant",
    "fanout",
    "concurrency",
    "budget_ms",
    "run",
    "exit_after_deadline_us",
    "completed",
    "cancelled",
    "alive_tasks_after",
];

const JEV_HEADER: [&str; 10] = [
    "req_id",
    "kind",
    "cancel_at_ms",
    "sent_at_utc",
    "status",
    "dropped",
    "latency_ms",
    "model_resolved",
    "input_tokens",
    "output_tokens",
];

const CYCLES_HEADER: &str =
    "cycle,id,exit_after_cancel_us,server_saw_cancel_ms,finished_before_cancel";

/// Un campione ogni `SAMPLE_EVERY` cicli completati (più la base e l'ultimo).
const SAMPLE_EVERY: usize = 100;
/// Assestamento prima del campione finale (soglia di leak: «dopo 1 s»).
const SETTLE: Duration = Duration::from_secs(1);
/// Tempi di cancellazione della matrice, nell'ordine di `collect`.
const MATRIX_CANCEL_MS: [u64; 3] = [50, 200, 500];

#[derive(Parser)]
#[command(about = "Spike S-05: cancellazione e deadline HTTP")]
struct Cli {
    /// Cartella dei risultati (deve contenere hw.json).
    #[arg(long, default_value = "s05-cancel/results", global = true)]
    results_dir: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
enum ProtoArg {
    H1,
    H2,
}

impl ProtoArg {
    fn proto(self) -> Proto {
        match self {
            ProtoArg::H1 => Proto::H1,
            ProtoArg::H2 => Proto::H2,
        }
    }
    fn label(self) -> &'static str {
        match self {
            ProtoArg::H1 => "h1",
            ProtoArg::H2 => "h2",
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Variant {
    A,
    B,
}

impl Variant {
    fn label(self) -> &'static str {
        match self {
            Variant::A => "a",
            Variant::B => "b",
        }
    }
}

#[derive(Subcommand)]
enum Cmd {
    /// Cicli di cancellazione verso il server axum lento con guardia di drop.
    Mock {
        #[arg(long, value_enum)]
        proto: ProtoArg,
        #[arg(long, value_delimiter = ',', default_value = "50,200,500")]
        cancel_at_ms: Vec<u64>,
        #[arg(long, default_value_t = 10_000)]
        cycles: usize,
        /// Cicli in volo contemporaneamente (= dimensione del pool).
        #[arg(long, default_value_t = client::POOL_MAX_IDLE_PER_HOST)]
        concurrency: usize,
        /// Ritardo dell'handler (maggiore di ogni tempo di cancellazione).
        #[arg(long, default_value_t = 2_000)]
        delay_ms: u64,
    },
    /// Cicli di cancellazione verso wiremock (`ResponseTemplate::set_delay`), HTTP/1.
    Wiremock {
        #[arg(long, value_delimiter = ',', default_value = "50,200,500")]
        cancel_at_ms: Vec<u64>,
        #[arg(long, default_value_t = 10_000)]
        cycles: usize,
        #[arg(long, default_value_t = client::POOL_MAX_IDLE_PER_HOST)]
        concurrency: usize,
        #[arg(long, default_value_t = 2_000)]
        delay_ms: u64,
    },
    /// Fan-out con deadline globale, variante A o B.
    Deadline {
        #[arg(long, value_enum)]
        variant: Variant,
        #[arg(long, value_delimiter = ',', default_value = "8,16,32")]
        fanout: Vec<usize>,
        #[arg(long, value_delimiter = ',', default_value = "200,500,1000")]
        budget_ms: Vec<u64>,
        #[arg(long, default_value_t = 3)]
        runs: u32,
        #[arg(long, default_value_t = 8)]
        concurrency: usize,
        #[arg(long, value_enum, default_value = "h2")]
        proto: ProtoArg,
        /// Ritardi del mock distribuiti in [min, max) ms.
        #[arg(long, default_value_t = 50)]
        delay_min_ms: u64,
        #[arg(long, default_value_t = 2_000)]
        delay_max_ms: u64,
    },
    /// Serie JEV reale: richieste cancellate più richieste di controllo, nessun retry.
    Jev {
        /// Richieste cancellate; default da `S05_JEV_CANCEL_REQUESTS`.
        #[arg(long)]
        requests: Option<usize>,
        #[arg(long, default_value_t = 50)]
        cancel_at_ms: u64,
        #[arg(long, default_value_t = 5)]
        controls: usize,
        /// Pausa fra una richiesta e la successiva.
        #[arg(long, default_value_t = 2_000)]
        spacing_ms: u64,
        /// Pausa fra la serie cancellata e quella di controllo (lettura in console).
        #[arg(long, default_value_t = 120)]
        gap_s: u64,
    },
    /// Unisce i grezzi in `cancel.csv` e `deadline.csv` e stampa i percentili.
    Collect {
        #[arg(long, default_value_t = 10_000)]
        cycles: usize,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    common::manifest::ensure_present(&cli.results_dir)
        .context("results/hw.json mancante: lanciare prima tools/hw-manifest.sh")?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("runtime tokio")?;
    let dir = cli.results_dir;
    rt.block_on(async move {
        match cli.cmd {
            Cmd::Mock {
                proto,
                cancel_at_ms,
                cycles,
                concurrency,
                delay_ms,
            } => {
                for ms in cancel_at_ms {
                    let target = Target::Axum { proto, delay_ms };
                    run_cancel_scenario(&dir, target, ms, cycles, concurrency).await?;
                }
                Ok(())
            }
            Cmd::Wiremock {
                cancel_at_ms,
                cycles,
                concurrency,
                delay_ms,
            } => {
                for ms in cancel_at_ms {
                    let target = Target::Wiremock { delay_ms };
                    run_cancel_scenario(&dir, target, ms, cycles, concurrency).await?;
                }
                Ok(())
            }
            Cmd::Deadline {
                variant,
                fanout,
                budget_ms,
                runs,
                concurrency,
                proto,
                delay_min_ms,
                delay_max_ms,
            } => {
                let cfg = DeadlineCfg {
                    variant,
                    fanouts: fanout,
                    budgets_ms: budget_ms,
                    runs,
                    concurrency,
                    proto,
                    delay_min_ms,
                    delay_max_ms,
                };
                run_deadline(&dir, &cfg).await
            }
            Cmd::Jev {
                requests,
                cancel_at_ms,
                controls,
                spacing_ms,
                gap_s,
            } => {
                let requests = match requests {
                    Some(n) => n,
                    None => common::env::required("S05_JEV_CANCEL_REQUESTS")?
                        .parse()
                        .context("S05_JEV_CANCEL_REQUESTS non è un intero")?,
                };
                let cfg = JevCfg {
                    requests,
                    cancel_at_ms,
                    controls,
                    spacing_ms,
                    gap_s,
                };
                run_jev(&dir, &cfg).await
            }
            Cmd::Collect { cycles } => collect(&dir, cycles),
        }
    })
}

// ---------------------------------------------------------------------------
// Cicli di cancellazione
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Target {
    Axum { proto: ProtoArg, delay_ms: u64 },
    Wiremock { delay_ms: u64 },
}

impl Target {
    fn scenario(self) -> &'static str {
        match self {
            Target::Axum { .. } => "mock",
            Target::Wiremock { .. } => "wiremock",
        }
    }
    fn proto(self) -> ProtoArg {
        match self {
            Target::Axum { proto, .. } => proto,
            Target::Wiremock { .. } => ProtoArg::H1,
        }
    }
    fn delay(self) -> Duration {
        match self {
            Target::Axum { delay_ms, .. } | Target::Wiremock { delay_ms } => {
                Duration::from_millis(delay_ms)
            }
        }
    }
}

/// Server sotto misura: axum con contatori, oppure wiremock (contatori non osservabili).
enum Mock {
    Axum(Arc<server::Metrics>),
    Wiremock(wiremock::MockServer),
}

impl Mock {
    fn metrics(&self) -> Option<&server::Metrics> {
        match self {
            Mock::Axum(m) => Some(m),
            Mock::Wiremock(_) => None,
        }
    }
}

struct Sample {
    cycle: usize,
    id: Option<u64>,
    alive: usize,
    fds: Option<usize>,
    conns: Option<usize>,
    dropped: Option<u64>,
    completed: Option<u64>,
}

fn take_sample(cycle: usize, id: Option<u64>, port: u16, mock: &Mock, base: (u64, u64)) -> Sample {
    Sample {
        cycle,
        id,
        alive: probe::alive_tasks(),
        fds: probe::open_fds(),
        conns: probe::established_conns(port),
        dropped: mock.metrics().map(|m| m.dropped() - base.0),
        completed: mock.metrics().map(|m| m.completed() - base.1),
    }
}

/// Attende che il mock non abbia handler in corso, al massimo `max`.
async fn wait_quiescent(mock: &Mock, max: Duration) -> bool {
    let Some(m) = mock.metrics() else {
        tokio::time::sleep(max).await;
        return true;
    };
    let end = tokio::time::Instant::now() + max;
    while !m.quiescent() {
        if tokio::time::Instant::now() >= end {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    true
}

fn opt<T: ToString>(v: Option<T>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

/// Millisecondi (tre decimali) fra la cancellazione programmata e il drop visto dal server.
fn saw_ms(mock: &Mock, id: u64, cancel_at: std::time::Instant) -> Option<String> {
    let drop_at = mock.metrics()?.drop_time(id)?;
    let ms = match drop_at.checked_duration_since(cancel_at) {
        Some(d) => d.as_secs_f64() * 1000.0,
        None => -(cancel_at - drop_at).as_secs_f64() * 1000.0,
    };
    Some(format!("{ms:.3}"))
}

async fn run_cancel_scenario(
    dir: &Path,
    target: Target,
    cancel_ms: u64,
    cycles: usize,
    concurrency: usize,
) -> anyhow::Result<()> {
    if cycles == 0 {
        bail!("--cycles deve essere > 0");
    }
    let delay = target.delay();
    let cancel_after = Duration::from_millis(cancel_ms);
    if delay <= cancel_after {
        bail!("il ritardo del mock deve superare il tempo di cancellazione");
    }
    let (port, mock) = match target {
        Target::Axum { .. } => {
            let (addr, metrics) = server::spawn_slow_server(delay).await?;
            (addr.port(), Mock::Axum(metrics))
        }
        Target::Wiremock { .. } => {
            use wiremock::matchers::{method, path};
            let srv = wiremock::MockServer::start().await;
            wiremock::Mock::given(method("GET"))
                .and(path(server::SLOW_PATH))
                .respond_with(
                    wiremock::ResponseTemplate::new(200)
                        .set_body_string("ok")
                        .set_delay(delay),
                )
                .mount(&srv)
                .await;
            (srv.address().port(), Mock::Wiremock(srv))
        }
    };
    let proto = target.proto();
    let client = client::build_client(proto.proto())?;
    let tag = format!("{}-{}-{cancel_ms}", target.scenario(), proto.label());
    println!("[{tag}] porta {port}, {cycles} cicli, concorrenza {concurrency}");

    // Riscaldamento (escluso dalla misura): una richiesta cancellata, poi quiete.
    let warm = format!("http://127.0.0.1:{port}{}", server::SLOW_PATH);
    let _ = client::cancel_cycle(&client, &warm, cancel_after).await;
    wait_quiescent(&mock, delay + SETTLE).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let base = mock
        .metrics()
        .map(|m| (m.dropped(), m.completed()))
        .unwrap_or((0, 0));
    let mut samples = vec![take_sample(0, None, port, &mock, base)];

    let mut outcomes: Vec<Option<client::CycleOutcome>> = vec![None; cycles + 1];
    let mut order: Vec<u64> = Vec::with_capacity(cycles);
    {
        let client = &client;
        let mut stream = futures::stream::iter(1..=cycles as u64)
            .map(|id| async move {
                let endpoint = format!("http://127.0.0.1:{port}{}?id={id}", server::SLOW_PATH);
                (
                    id,
                    client::cancel_cycle(client, &endpoint, cancel_after).await,
                )
            })
            .buffer_unordered(concurrency.max(1));
        while let Some((id, out)) = stream.next().await {
            let idx = usize::try_from(id).context("id del ciclo")?;
            outcomes[idx] = Some(out);
            order.push(id);
            let done = order.len();
            if done.is_multiple_of(SAMPLE_EVERY) && done != cycles {
                samples.push(take_sample(done, Some(id), port, &mock, base));
            }
        }
    }
    tokio::time::sleep(SETTLE).await;
    let quiet = wait_quiescent(&mock, delay + SETTLE).await;
    let last = order.last().copied();
    samples.push(take_sample(cycles, last, port, &mock, base));
    if !quiet {
        eprintln!("[{tag}] il mock ha ancora handler in corso al campione finale");
    }

    // Grezzi per ciclo.
    let raw = dir.join("raw");
    std::fs::create_dir_all(&raw).context("cartella results/raw")?;
    let mut cyc = String::from(CYCLES_HEADER);
    cyc.push('\n');
    let mut exit_rec = Recorder::new();
    let (mut seen, mut seen_100, mut early) = (0usize, 0usize, 0usize);
    for (n, id) in order.iter().enumerate() {
        let Some(out) = outcomes[usize::try_from(*id)?] else {
            continue;
        };
        let saw = saw_ms(&mock, *id, out.cancel_at);
        if let Some(s) = &saw {
            seen += 1;
            if s.parse::<f64>().is_ok_and(|v| v <= 100.0) {
                seen_100 += 1;
            }
        }
        if out.finished_before_cancel {
            early += 1;
        }
        exit_rec.record_us(u64::try_from(out.exit_after_cancel_us.max(0)).unwrap_or(0));
        writeln!(
            cyc,
            "{},{id},{},{},{}",
            n + 1,
            out.exit_after_cancel_us,
            saw.unwrap_or_default(),
            out.finished_before_cancel
        )?;
    }
    std::fs::write(raw.join(format!("cancel-{tag}.cycles.csv")), cyc)?;

    let mut sampled = CANCEL_HEADER.join(",");
    sampled.push('\n');
    for s in &samples {
        let (exit, saw) = match s
            .id
            .and_then(|id| Some((id, outcomes[usize::try_from(id).ok()?]?)))
        {
            Some((id, out)) => (
                out.exit_after_cancel_us.to_string(),
                saw_ms(&mock, id, out.cancel_at).unwrap_or_default(),
            ),
            None => (String::new(), String::new()),
        };
        writeln!(
            sampled,
            "{},{},{cancel_ms},{},{exit},{saw},{},{},{},{},{}",
            target.scenario(),
            proto.label(),
            s.cycle,
            opt(s.dropped),
            opt(s.completed),
            s.alive,
            opt(s.fds),
            opt(s.conns)
        )?;
    }
    std::fs::write(raw.join(format!("cancel-{tag}.sampled.csv")), sampled)?;

    let received = match &mock {
        Mock::Wiremock(srv) => srv.received_requests().await.map(|v| v.len()),
        Mock::Axum(m) => Some(usize::try_from(m.started()).unwrap_or(usize::MAX)),
    };
    let sum = exit_rec.summary();
    let (b, f) = (&samples[0], &samples[samples.len() - 1]);
    println!(
        "[{tag}] exit_after_cancel_us p50 {} p95 {} p99 {} max {} (n {}); visti dal server {seen} \
         ({seen_100} entro 100 ms); terminati prima della cancellazione {early}; richieste \
         arrivate al server {} (riscaldamento compreso)",
        sum.p50_us,
        sum.p95_us,
        sum.p99_us,
        sum.max_us,
        sum.n,
        opt(received)
    );
    println!(
        "[{tag}] base/fine: task {}/{}, fd {}/{}, connessioni {}/{}, dropped {}, completed {}",
        b.alive,
        f.alive,
        opt(b.fds),
        opt(f.fds),
        opt(b.conns),
        opt(f.conns),
        opt(f.dropped),
        opt(f.completed)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Deadline sul fan-out
// ---------------------------------------------------------------------------

struct DeadlineCfg {
    variant: Variant,
    fanouts: Vec<usize>,
    budgets_ms: Vec<u64>,
    runs: u32,
    concurrency: usize,
    proto: ProtoArg,
    delay_min_ms: u64,
    delay_max_ms: u64,
}

async fn run_deadline(dir: &Path, cfg: &DeadlineCfg) -> anyhow::Result<()> {
    let plan = server::DelayPlan::Spread {
        min: Duration::from_millis(cfg.delay_min_ms),
        max: Duration::from_millis(cfg.delay_max_ms),
    };
    let (addr, metrics) = server::spawn_server(plan).await?;
    let client = client::build_client(cfg.proto.proto())?;
    let endpoint = format!("http://127.0.0.1:{}{}", addr.port(), server::SLOW_PATH);
    // Riscaldamento: apre la connessione prima della base dei task.
    let _ = client::cancel_cycle(&client, &endpoint, Duration::from_millis(50)).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let base_tasks = probe::alive_tasks();
    println!(
        "[deadline-{}] proto {}, base task {base_tasks}",
        cfg.variant.label(),
        cfg.proto.label()
    );

    let mut out = DEADLINE_HEADER.join(",");
    out.push('\n');
    for &fanout in &cfg.fanouts {
        for &budget_ms in &cfg.budgets_ms {
            for run in 1..=cfg.runs {
                let start = tokio::time::Instant::now();
                let deadline = start + Duration::from_millis(budget_ms);
                let res: FanoutOutcome = match cfg.variant {
                    Variant::A => {
                        client::fanout_variant_a(
                            &client,
                            &endpoint,
                            fanout,
                            cfg.concurrency,
                            deadline,
                        )
                        .await
                    }
                    Variant::B => {
                        client::fanout_variant_b(
                            &client,
                            &endpoint,
                            fanout,
                            cfg.concurrency,
                            deadline,
                        )
                        .await
                    }
                };
                tokio::time::sleep(SETTLE).await;
                let alive = probe::alive_tasks();
                writeln!(
                    out,
                    "{},{fanout},{},{budget_ms},{run},{},{},{},{alive}",
                    cfg.variant.label(),
                    cfg.concurrency,
                    res.exit_after_deadline_us,
                    res.completed,
                    res.cancelled
                )?;
                println!(
                    "[deadline-{}] fanout {fanout} budget {budget_ms} run {run}: uscita {} us, \
                     completate {}, task dopo {alive}",
                    cfg.variant.label(),
                    res.exit_after_deadline_us,
                    res.completed
                );
            }
        }
    }
    let raw = dir.join("raw");
    std::fs::create_dir_all(&raw).context("cartella results/raw")?;
    std::fs::write(
        raw.join(format!("deadline-{}.csv", cfg.variant.label())),
        out,
    )?;
    println!(
        "[deadline-{}] server: avviate {}, completate {}, cancellate {}",
        cfg.variant.label(),
        metrics.started(),
        metrics.completed(),
        metrics.dropped()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Serie JEV reale
// ---------------------------------------------------------------------------

struct JevCfg {
    requests: usize,
    cancel_at_ms: u64,
    controls: usize,
    spacing_ms: u64,
    gap_s: u64,
}

/// Istante UTC corrente in RFC 3339 con millisecondi e `Z` finale.
fn utc_rfc3339_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let millis = now.subsec_millis();
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    // Algoritmo days-from-civil inverso di H. Hinnant (calendario gregoriano prolettico).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3_600,
        (rem % 3_600) / 60,
        rem % 60
    )
}

async fn run_jev(dir: &Path, cfg: &JevCfg) -> anyhow::Result<()> {
    let base_url = common::env::required("JEV_API_URL")?;
    let model = common::env::required("JEV_MODEL")?;
    let client = jev::JevClient::new(&base_url, common::env::required("TYPESAFE_API_KEY")?, model)?;
    let path = dir.join("jev_cancel.csv");
    let mut w = common::csv::Writer::create(&path, &JEV_HEADER)?;
    let cancel = Duration::from_millis(cfg.cancel_at_ms);
    let spacing = Duration::from_millis(cfg.spacing_ms);
    let cancel_ms = cfg.cancel_at_ms.to_string();
    let mut req_id = 0usize;

    // Serie cancellata: riscaldamento non addebitabile, poi una sola POST droppata a 50 ms.
    for _ in 0..cfg.requests {
        req_id += 1;
        let version = client
            .warm_up()
            .await
            .context("serie interrotta senza retry")?;
        let sent_at = utc_rfc3339_now();
        let t = std::time::Instant::now();
        let res = tokio::time::timeout(cancel, client.send()).await;
        let latency = format!("{:.1}", t.elapsed().as_secs_f64() * 1000.0);
        let (status, dropped, reply) = match res {
            Err(_) => (String::new(), "true", None),
            Ok(Ok(v)) => (
                "200".to_owned(),
                "false",
                Some(jev::JevReply::from_parts(200, version.clone(), Some(&v))),
            ),
            Ok(Err(e)) => {
                eprintln!("[jev] richiesta {req_id}: errore prima della cancellazione: {e:#}");
                ("error".to_owned(), "false", None)
            }
        };
        let model_resolved = reply.as_ref().and_then(|r| r.model_resolved.clone());
        let tin = reply.as_ref().and_then(|r| r.input_tokens);
        let tout = reply.as_ref().and_then(|r| r.output_tokens);
        w.write_row(&[
            &req_id.to_string(),
            "cancelled",
            &cancel_ms,
            &sent_at,
            &status,
            dropped,
            &latency,
            &opt(model_resolved),
            &opt(tin),
            &opt(tout),
        ])?;
        println!("[jev] {req_id} cancelled {sent_at} dropped={dropped} http={version}");
        tokio::time::sleep(spacing).await;
    }

    println!(
        "[jev] pausa di {} s prima delle richieste di controllo",
        cfg.gap_s
    );
    tokio::time::sleep(Duration::from_secs(cfg.gap_s)).await;

    // Controllo: richieste complete, mai cancellate né ripetute.
    for _ in 0..cfg.controls {
        req_id += 1;
        let sent_at = utc_rfc3339_now();
        let t = std::time::Instant::now();
        let res = client.send_reply().await;
        let latency = format!("{:.1}", t.elapsed().as_secs_f64() * 1000.0);
        let (status, reply) = match res {
            Ok(r) => (r.status.to_string(), Some(r)),
            Err(e) => {
                eprintln!("[jev] richiesta {req_id}: controllo non riuscito: {e:#}");
                ("error".to_owned(), None)
            }
        };
        let version = reply.as_ref().map(|r| r.http_version.clone());
        let model_resolved = reply.as_ref().and_then(|r| r.model_resolved.clone());
        let tin = reply.as_ref().and_then(|r| r.input_tokens);
        let tout = reply.as_ref().and_then(|r| r.output_tokens);
        w.write_row(&[
            &req_id.to_string(),
            "control",
            "",
            &sent_at,
            &status,
            "false",
            &latency,
            &opt(model_resolved),
            &opt(tin),
            &opt(tout),
        ])?;
        println!(
            "[jev] {req_id} control {sent_at} status={status} http={}",
            opt(version)
        );
        tokio::time::sleep(spacing).await;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Raccolta dei grezzi
// ---------------------------------------------------------------------------

fn read_rows(path: &Path, header: &[&str]) -> anyhow::Result<Vec<Vec<String>>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("grezzo {} assente", path.display()))?;
    let mut lines = text.lines();
    if lines.next() != Some(header.join(",").as_str()) {
        bail!("header inatteso in {}", path.display());
    }
    Ok(lines
        .filter(|l| !l.is_empty())
        .map(|l| l.split(',').map(str::to_owned).collect())
        .collect())
}

fn collect(dir: &Path, cycles: usize) -> anyhow::Result<()> {
    let raw = dir.join("raw");
    let mut cancel = common::csv::Writer::create(&dir.join("cancel.csv"), &CANCEL_HEADER)?;
    for (scenario, proto) in [("mock", "h1"), ("mock", "h2"), ("wiremock", "h1")] {
        for ms in MATRIX_CANCEL_MS {
            let p = raw.join(format!("cancel-{scenario}-{proto}-{ms}.sampled.csv"));
            let rows = read_rows(&p, &CANCEL_HEADER)?;
            let last = rows.last().map(|r| r[3].clone());
            if last.as_deref() != Some(cycles.to_string().as_str()) {
                bail!("{}: ultima riga non a cycle = {cycles}", p.display());
            }
            for r in &rows {
                let fields: Vec<&str> = r.iter().map(String::as_str).collect();
                cancel.write_row(&fields)?;
            }
        }
    }

    let mut deadline = common::csv::Writer::create(&dir.join("deadline.csv"), &DEADLINE_HEADER)?;
    let mut report = String::new();
    for v in ["a", "b"] {
        let rows = read_rows(&raw.join(format!("deadline-{v}.csv")), &DEADLINE_HEADER)?;
        let mut all = Recorder::new();
        for r in rows.iter().filter(|r| r[4] != "1") {
            let fields: Vec<&str> = r.iter().map(String::as_str).collect();
            deadline.write_row(&fields)?;
            let us: i64 = r[5].parse().context("exit_after_deadline_us")?;
            all.record_us(u64::try_from(us.max(0)).unwrap_or(0));
        }
        let s = all.summary();
        writeln!(
            report,
            "variante {v}: n {} p50 {} p95 {} p99 {} max {} us (run 1 esclusa, anticipi a 0)",
            s.n, s.p50_us, s.p95_us, s.p99_us, s.max_us
        )?;
    }
    print!("{report}");
    std::io::stdout().flush()?;
    Ok(())
}
