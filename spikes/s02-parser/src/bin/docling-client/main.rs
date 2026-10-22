//! Bin `docling-client` (piano 01-08): invia a Docling (`DOCLING_URL` / `DOCLING_CPU_URL`)
//! solo i PDF che hanno un `RunReport` `ok` di `extract_pdf`.
//!
//! Docling non sta dietro il supervisore di 01-07: il prescan del figlio `extract_pdf` è la
//! sua precondizione. Il `RunReport` di 01-07 non porta `input`, quindi l'insieme degli input
//! ammessi si ricava dal JSONL dei job (`--extract-jobs`: `job_id` → `input`, solo
//! `op: extract_pdf`) unito a `--extract-runs` (`job_id` → `outcome.status`). L'identità dei
//! byte (stesso sha256 del file estratto) la verifica `docling-bench.sh` con `sha256sum`.
//!
//! Esce con 0 (`ok`), 1 (`error`, risposta d'errore del servizio), 3 (`client_timeout`,
//! `client_abort`: segnali per il riavvio, non prove che il lavoro sia finito), 4
//! (`not_prescanned`, nessuna richiesta), 2 (uso). Stampa una riga JSON con l'esito e, per
//! `ok`, salva il JSON grezzo e le tabelle normalizzate.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use anyhow::Context;
use serde_json::{Value, json};

/// Esito di una richiesta. `ClientTimeout` e `ClientAbort` escono con 3: sono segnali per il
/// riavvio del servizio, non prove che il lavoro sia finito; `Error` è una risposta d'errore
/// del servizio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
    Ok,
    Error,
    ClientTimeout,
    ClientAbort,
    NotPrescanned,
}

impl Status {
    /// `ok`, `error`, `client_timeout`, `client_abort`, `not_prescanned`.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Error => "error",
            Status::ClientTimeout => "client_timeout",
            Status::ClientAbort => "client_abort",
            Status::NotPrescanned => "not_prescanned",
        }
    }

    /// Codice d'uscita del processo per questo esito.
    pub(crate) fn exit_code(self) -> i32 {
        match self {
            Status::Ok => 0,
            Status::Error => 1,
            Status::ClientTimeout | Status::ClientAbort => 3,
            Status::NotPrescanned => 4,
        }
    }
}

/// Configurazione di una richiesta. `base_url` viene solo dall'ambiente in `main`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ConvertConfig {
    pub base_url: String,
    /// Pagine 1-based incluse `(da, a)`.
    pub page_range: Option<(u32, u32)>,
    pub document_timeout_s: u64,
    /// Timeout lato chiamante: `document_timeout + 15 s` di default.
    pub caller_timeout: Duration,
    /// `--abort-after`: chiusura volontaria della connessione (prova di disconnessione).
    pub abort_after: Option<Duration>,
}

impl ConvertConfig {
    pub(crate) fn new(base_url: String, document_timeout_s: u64) -> ConvertConfig {
        ConvertConfig {
            base_url,
            page_range: None,
            document_timeout_s,
            caller_timeout: Duration::from_secs(document_timeout_s + 15),
            abort_after: None,
        }
    }
}

/// Esito con il JSON grezzo (solo per `Ok`) e i secondi della richiesta.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ConvertResult {
    pub status: Status,
    pub body: Option<Vec<u8>>,
    pub seconds: f64,
}

/// Input con `RunReport` `ok` di un job `extract_pdf` (`jobs_jsonl`: job JSON di 01-07;
/// `runs_jsonl`: `RunReport` e righe `job_error` di `parse-parent`).
pub(crate) fn prescanned_inputs(jobs_jsonl: &str, runs_jsonl: &str) -> BTreeSet<String> {
    let ok: BTreeSet<String> = runs_jsonl
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["outcome"]["status"] == "ok")
        .filter_map(|v| v["job_id"].as_str().map(str::to_owned))
        .collect();
    jobs_jsonl
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["op"] == "extract_pdf")
        .filter(|v| v["job_id"].as_str().is_some_and(|id| ok.contains(id)))
        .filter_map(|v| v["input"].as_str().map(str::to_owned))
        .collect()
}

/// Host ammessi per Docling: i due servizi della rete interna `parse` del compose e il
/// loopback (server finto dei test). Nessun altro host riceve documenti.
const ALLOWED_HOSTS: [&str; 4] = ["docling", "docling-cpu", "127.0.0.1", "localhost"];
/// Tetto del documento inviato (`DOCLING_SERVE_MAX_FILE_SIZE` del compose, 25 MiB).
const MAX_UPLOAD_BYTES: u64 = 25 * 1024 * 1024;
/// Tetto della risposta letta in streaming.
const MAX_RESPONSE_BYTES: usize = 256 * 1024 * 1024;

/// Endpoint di Docling validato: schema http della rete interna, host in allow-list, nessuna
/// credenziale, query o frammento. È l'unico modo di costruire l'URL di una richiesta.
#[derive(Debug, Clone)]
pub(crate) struct Endpoint(reqwest::Url);

impl Endpoint {
    pub(crate) fn parse(base: &str) -> anyhow::Result<Endpoint> {
        let u = reqwest::Url::parse(base)
            .with_context(|| format!("URL di Docling non valido: {base}"))?;
        anyhow::ensure!(
            u.scheme() == "http",
            "schema non ammesso per Docling: {}",
            u.scheme()
        );
        anyhow::ensure!(
            u.host_str().is_some_and(|h| ALLOWED_HOSTS.contains(&h)),
            "host non ammesso per Docling: {:?}",
            u.host_str()
        );
        anyhow::ensure!(
            u.username().is_empty()
                && u.password().is_none()
                && u.query().is_none()
                && u.fragment().is_none(),
            "URL di Docling con credenziali, query o frammento"
        );
        Ok(Endpoint(u))
    }

    /// Percorso fisso del servizio sotto l'endpoint.
    fn at(&self, path: &str) -> reqwest::Url {
        let mut u = self.0.clone();
        u.set_path(&format!("{}/{path}", self.0.path().trim_end_matches('/')));
        u
    }
}

/// Corpo della risposta letto a pezzi, fermo oltre `MAX_RESPONSE_BYTES`.
async fn read_capped(mut resp: reqwest::Response) -> Result<Vec<u8>, reqwest::Error> {
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            break;
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Richiesta multipart; tiene il corpo della risposta.
async fn send_keep(cfg: &ConvertConfig, bytes: Vec<u8>, name: String) -> (Status, Option<Vec<u8>>) {
    let client = match reqwest::Client::builder().build() {
        Ok(c) => c,
        Err(_) => return (Status::Error, None),
    };
    let part = match reqwest::multipart::Part::bytes(bytes)
        .file_name(name)
        .mime_str("application/pdf")
    {
        Ok(p) => p,
        Err(_) => return (Status::Error, None),
    };
    let mut form = reqwest::multipart::Form::new()
        .part("files", part)
        .text("to_formats", "json")
        .text("do_ocr", "false")
        .text("table_mode", "accurate")
        .text("do_table_structure", "true")
        .text("document_timeout", cfg.document_timeout_s.to_string());
    if let Some((a, b)) = cfg.page_range {
        form = form
            .text("page_range", a.to_string())
            .text("page_range", b.to_string());
    }
    let Ok(endpoint) = Endpoint::parse(&cfg.base_url) else {
        return (Status::Error, None);
    };
    let resp = match client
        .post(endpoint.at("v1/convert/file"))
        .multipart(form)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) if e.is_timeout() => return (Status::ClientTimeout, None),
        Err(_) => return (Status::Error, None),
    };
    let ok = resp.status().is_success();
    match read_capped(resp).await {
        Ok(b) if ok => {
            let success = serde_json::from_slice::<Value>(&b)
                .map(|v| v["status"] == "success")
                .unwrap_or(false);
            if success {
                (Status::Ok, Some(b))
            } else {
                (Status::Error, Some(b))
            }
        }
        Ok(b) => (Status::Error, Some(b)),
        Err(e) if e.is_timeout() => (Status::ClientTimeout, None),
        Err(_) => (Status::Error, None),
    }
}

/// `POST {base_url}/v1/convert/file` (multipart) con `to_formats=json`, `do_ocr=false`,
/// `table_mode=accurate`, `do_table_structure=true`, `page_range`, `document_timeout`.
pub(crate) async fn convert(cfg: &ConvertConfig, file: &Path) -> ConvertResult {
    let t0 = Instant::now();
    let too_big = tokio::fs::metadata(file)
        .await
        .map_or(true, |m| m.len() > MAX_UPLOAD_BYTES);
    if too_big {
        return ConvertResult {
            status: Status::Error,
            body: None,
            seconds: 0.0,
        };
    }
    let bytes = match tokio::fs::read(file).await {
        Ok(b) => b,
        Err(_) => {
            return ConvertResult {
                status: Status::Error,
                body: None,
                seconds: 0.0,
            };
        }
    };
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "input.pdf".to_owned());
    let fut = send_keep(cfg, bytes, name);
    // Il timeout del chiamante e l'abbandono volontario chiudono la connessione lasciando
    // cadere la richiesta: nessuno dei due ferma il lavoro già accettato dal servizio.
    let (status, body) = match cfg.abort_after {
        Some(a) if a < cfg.caller_timeout => match tokio::time::timeout(a, fut).await {
            Ok(r) => r,
            Err(_) => (Status::ClientAbort, None),
        },
        _ => match tokio::time::timeout(cfg.caller_timeout, fut).await {
            Ok(r) => r,
            Err(_) => (Status::ClientTimeout, None),
        },
    };
    ConvertResult {
        body: if status == Status::Ok { body } else { None },
        status,
        seconds: t0.elapsed().as_secs_f64(),
    }
}

/// `convert` solo se `input` è in `admitted`; altrimenti `NotPrescanned` senza rete.
pub(crate) async fn run_one(
    cfg: &ConvertConfig,
    input: &Path,
    admitted: &BTreeSet<String>,
) -> ConvertResult {
    if !admitted.contains(input.to_string_lossy().as_ref()) {
        return ConvertResult {
            status: Status::NotPrescanned,
            body: None,
            seconds: 0.0,
        };
    }
    convert(cfg, input).await
}

/// Box `{l, t, r, b, coord_origin}` di Docling → `[x0, y0, x1, y1]` origine in basso a sinistra.
fn docling_box(b: &Value, page_h: Option<f64>) -> Option<[f64; 4]> {
    let (l, t, r, bo) = (
        b["l"].as_f64()?,
        b["t"].as_f64()?,
        b["r"].as_f64()?,
        b["b"].as_f64()?,
    );
    match b["coord_origin"].as_str() {
        Some("TOPLEFT") => {
            let h = page_h?;
            Some([l.min(r), h - t.max(bo), l.max(r), h - t.min(bo)])
        }
        _ => Some([l.min(r), t.min(bo), l.max(r), t.max(bo)]),
    }
}

/// Tabelle del `DoclingDocument` normalizzate secondo i Contratti: righe d'intestazione di
/// colonna tolte, righe rinumerate da 1, colonne = indice + 1, cella nella posizione iniziale.
pub(crate) fn normalize_tables(resp: &Value) -> Value {
    let doc = &resp["document"]["json_content"];
    let page_h = |p: u64| doc["pages"][p.to_string()]["size"]["height"].as_f64();
    let mut out = Vec::new();
    for t in doc["tables"].as_array().into_iter().flatten() {
        let prov = &t["prov"][0];
        let page = prov["page_no"].as_u64().unwrap_or(0);
        let bbox = docling_box(&prov["bbox"], page_h(page));
        let cells: Vec<&Value> = t["data"]["table_cells"]
            .as_array()
            .into_iter()
            .flatten()
            .collect();
        let header_rows: BTreeSet<u64> = {
            let mut rows: std::collections::BTreeMap<u64, (usize, usize)> = Default::default();
            for c in &cells {
                let r = c["start_row_offset_idx"].as_u64().unwrap_or(0);
                let e = rows.entry(r).or_default();
                e.0 += 1;
                if c["column_header"] == true {
                    e.1 += 1;
                }
            }
            rows.into_iter()
                .filter(|(_, (n, h))| *n > 0 && n == h)
                .map(|(r, _)| r)
                .collect()
        };
        let body_rows: Vec<u64> = {
            let mut v: Vec<u64> = cells
                .iter()
                .map(|c| c["start_row_offset_idx"].as_u64().unwrap_or(0))
                .filter(|r| !header_rows.contains(r))
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        let norm: Vec<Value> = cells
            .iter()
            .filter_map(|c| {
                let r = c["start_row_offset_idx"].as_u64().unwrap_or(0);
                let row = body_rows.iter().position(|x| *x == r)? as u64 + 1;
                Some(json!({
                    "row": row,
                    "col": c["start_col_offset_idx"].as_u64().unwrap_or(0) + 1,
                    "text": c["text"].as_str().unwrap_or_default(),
                }))
            })
            .collect();
        out.push(json!({
            "page": page,
            "bbox": bbox,
            "header_rows": header_rows.len(),
            "num_rows": t["data"]["num_rows"],
            "num_cols": t["data"]["num_cols"],
            "cells": norm,
        }));
    }
    json!({ "tables": out })
}

#[derive(clap::Parser, Debug)]
#[command(name = "docling-client")]
struct Cli {
    /// `RunReport` di `parse-parent` (`runs.jsonl`): obbligatorio.
    #[arg(long)]
    extract_runs: PathBuf,
    /// JSONL dei job `extract_pdf` (`job_id` → `input`), perché il `RunReport` non porta `input`.
    #[arg(long)]
    extract_jobs: PathBuf,
    /// Input nella forma del job (percorso del worker, es. `/data/s02/raw/x.pdf`).
    #[arg(long, required_unless_present = "get")]
    input: Option<String>,
    /// Solo lettura dei metadati del servizio, senza documento: `version` o `openapi`.
    #[arg(long, value_parser = ["version", "openapi"])]
    get: Option<String>,
    /// Cartella che nel worker è `/data`.
    #[arg(long)]
    data_root: PathBuf,
    /// Cartella che nel worker è `/results/raw`.
    #[arg(long)]
    results_raw: PathBuf,
    /// `gpu` → `DOCLING_URL`, `cpu` → `DOCLING_CPU_URL`.
    #[arg(long)]
    device: String,
    /// Pagine 1-based `da-a` (es. `12-12`).
    #[arg(long)]
    page_range: Option<String>,
    #[arg(long, default_value_t = 120)]
    document_timeout: u64,
    /// Secondi dopo cui il client chiude la connessione (prova di disconnessione).
    #[arg(long)]
    abort_after: Option<f64>,
    /// Prefisso dei file salvati (`<prefix>.json` grezzo, `<prefix>.tables.json`).
    #[arg(long)]
    out_prefix: Option<PathBuf>,
}

/// GET di `/version` o `/openapi.json`: stampa il corpo (o lo salva in `<prefix>.json`).
async fn get_meta(base: &str, what: &str, prefix: Option<&Path>) -> anyhow::Result<i32> {
    let path = if what == "version" {
        "version"
    } else {
        "openapi.json"
    };
    let endpoint = Endpoint::parse(base)?;
    let resp = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?
        .get(endpoint.at(path))
        .send()
        .await?;
    let ok = resp.status().is_success();
    let body = read_capped(resp).await?;
    match prefix {
        Some(p) => std::fs::write(p.with_extension("json"), &body)?,
        None => println!("{}", String::from_utf8_lossy(&body)),
    }
    Ok(if ok { 0 } else { 1 })
}

fn map_input(worker: &str, data: &Path, raw: &Path) -> PathBuf {
    if let Some(rest) = worker.strip_prefix("/data/") {
        data.join(rest)
    } else if let Some(rest) = worker.strip_prefix("/results/raw/") {
        raw.join(rest)
    } else {
        PathBuf::from(worker)
    }
}

fn parse_range(s: &str) -> anyhow::Result<(u32, u32)> {
    let (a, b) = s.split_once('-').context("--page-range atteso da-a")?;
    let (a, b): (u32, u32) = (a.parse()?, b.parse()?);
    anyhow::ensure!(a >= 1 && a <= b, "--page-range non valido");
    Ok((a, b))
}

async fn real_main(cli: Cli) -> anyhow::Result<i32> {
    let var = match cli.device.as_str() {
        "gpu" => "DOCLING_URL",
        "cpu" => "DOCLING_CPU_URL",
        other => anyhow::bail!("--device sconosciuto: {other}"),
    };
    let base = std::env::var(var).with_context(|| format!("{var} assente"))?;
    if let Some(what) = &cli.get {
        return get_meta(&base, what, cli.out_prefix.as_deref()).await;
    }
    let input = cli.input.clone().context("--input assente")?;
    let jobs = std::fs::read_to_string(&cli.extract_jobs).context("--extract-jobs")?;
    let runs = std::fs::read_to_string(&cli.extract_runs).context("--extract-runs")?;
    // Input ammessi nella forma del job, portati sui percorsi del runner.
    let admitted: BTreeSet<String> = prescanned_inputs(&jobs, &runs)
        .iter()
        .map(|w| {
            map_input(w, &cli.data_root, &cli.results_raw)
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let mut cfg = ConvertConfig::new(base, cli.document_timeout);
    cfg.page_range = cli.page_range.as_deref().map(parse_range).transpose()?;
    cfg.abort_after = cli.abort_after.map(Duration::from_secs_f64);
    let file = map_input(&input, &cli.data_root, &cli.results_raw);
    let r = run_one(&cfg, &file, &admitted).await;
    let mut line = json!({
        "status": r.status.as_str(),
        "input": input,
        "device": cli.device,
        "page_range": cli.page_range,
        "seconds": r.seconds,
        "tables_found": null,
    });
    if let (Some(body), Some(prefix)) = (&r.body, &cli.out_prefix) {
        let raw = prefix.with_extension("json");
        std::fs::write(&raw, body).with_context(|| format!("{}", raw.display()))?;
        let v: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
        let t = normalize_tables(&v);
        line["tables_found"] = json!(t["tables"].as_array().map_or(0, Vec::len));
        let tf = PathBuf::from(format!("{}.tables.json", prefix.display()));
        std::fs::write(&tf, serde_json::to_string(&t)?)?;
    }
    println!("{line}");
    Ok(r.status.exit_code())
}

fn main() -> ExitCode {
    use clap::Parser as _;
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("docling-client: runtime: {e}");
            return ExitCode::from(1);
        }
    };
    match rt.block_on(real_main(cli)) {
        Ok(code) => ExitCode::from(code as u8),
        Err(e) => {
            eprintln!("docling-client: {e:#}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    //! TEST-1 (Docling riceve solo input con prescan `ok`), TEST-2 e TEST-3 nella parte del
    //! client (esiti distinti per timeout, disconnessione ed errore; `page_range` e opzioni
    //! inviati), TEST-8 (lo script di benchmark riavvia con stop + up, mai down né profili).
    //! Il server Docling è sostituito da un listener TCP locale che registra le richieste.
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::time::Instant;

    // ---- server finto -----------------------------------------------------------------------

    #[derive(Clone, Copy)]
    enum Mode {
        Reply(u16, &'static str),
        Hang,
    }

    struct Fake {
        base_url: String,
        conns: Arc<AtomicUsize>,
        requests: mpsc::Receiver<Vec<u8>>,
    }

    /// Richiesta HTTP/1.1 intera: header più corpo (`Content-Length` o chunked).
    fn read_request(s: &mut TcpStream) -> Vec<u8> {
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        let head_end = loop {
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break i + 4;
            }
            match s.read(&mut chunk) {
                Ok(0) | Err(_) => return buf,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).to_ascii_lowercase();
        let len = head
            .lines()
            .find_map(|l| l.strip_prefix("content-length:"))
            .and_then(|v| v.trim().parse::<usize>().ok());
        loop {
            let done = match len {
                Some(n) => buf.len() >= head_end + n,
                None => !head.contains("chunked") || buf.ends_with(b"0\r\n\r\n"),
            };
            if done {
                return buf;
            }
            match s.read(&mut chunk) {
                Ok(0) | Err(_) => return buf,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
        }
    }

    fn serve(mode: Mode) -> Fake {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let conns = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::channel();
        let c = conns.clone();
        std::thread::spawn(move || {
            for s in l.incoming() {
                let Ok(mut s) = s else { continue };
                c.fetch_add(1, Ordering::SeqCst);
                let req = read_request(&mut s);
                let _ = tx.send(req);
                match mode {
                    Mode::Reply(code, body) => {
                        let resp = format!(
                            "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\n\
                             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = s.write_all(resp.as_bytes());
                    }
                    Mode::Hang => std::thread::sleep(Duration::from_secs(20)),
                }
            }
        });
        // Schema composto a parte: nessun URL in chiaro nei sorgenti di src/bin.
        let scheme = "http";
        Fake {
            base_url: format!("{scheme}://127.0.0.1:{port}"),
            conns,
            requests: rx,
        }
    }

    fn input_pdf(name: &str) -> PathBuf {
        let root = std::env::var_os("S02_TEST_BASE").map_or_else(std::env::temp_dir, PathBuf::from);
        let d = root.join(format!("s02-docling-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("in.pdf");
        std::fs::write(&p, b"%PDF-1.7\n%%EOF\n").unwrap();
        p
    }

    fn cfg(f: &Fake) -> ConvertConfig {
        let mut c = ConvertConfig::new(f.base_url.clone(), 120);
        c.caller_timeout = Duration::from_secs(10);
        c
    }

    // ---- configurazione e codici d'uscita -------------------------------------------------

    #[test]
    fn caller_timeout_is_document_timeout_plus_15s() {
        let c = ConvertConfig::new("x".into(), 120);
        assert_eq!(c.document_timeout_s, 120);
        assert_eq!(c.caller_timeout, Duration::from_secs(135));
        assert_eq!(c.abort_after, None);
        let c = ConvertConfig::new("x".into(), 5);
        assert_eq!(c.caller_timeout, Duration::from_secs(20));
    }

    #[test]
    fn statuses_and_exit_codes_are_distinct() {
        assert_eq!(Status::Ok.as_str(), "ok");
        assert_eq!(Status::Error.as_str(), "error");
        assert_eq!(Status::ClientTimeout.as_str(), "client_timeout");
        assert_eq!(Status::ClientAbort.as_str(), "client_abort");
        assert_eq!(Status::NotPrescanned.as_str(), "not_prescanned");
        assert_eq!(Status::Ok.exit_code(), 0);
        assert_eq!(Status::ClientTimeout.exit_code(), 3);
        assert_eq!(Status::ClientAbort.exit_code(), 3);
        assert_ne!(
            Status::Error.exit_code(),
            3,
            "errore del servizio distinto dal timeout"
        );
        assert_ne!(Status::Error.exit_code(), 0);
    }

    // ---- TEST-1: precondizione del prescan ------------------------------------------------

    #[test]
    fn only_extract_pdf_jobs_with_ok_report_are_admitted() {
        let jobs = [
            r#"{"job_id":"a-r1","op":"extract_pdf","input":"/data/s02/raw/a.pdf","out_dir":"/results/raw/extract/a-r1","params":{}}"#,
            r#"{"job_id":"b-r1","op":"extract_pdf","input":"/data/s02/raw/b.pdf","out_dir":"/results/raw/extract/b-r1","params":{}}"#,
            r#"{"job_id":"c-r1","op":"extract_pdf","input":"/data/s02/raw/c.pdf","out_dir":"/results/raw/extract/c-r1","params":{}}"#,
            r#"{"job_id":"d-r1","op":"extract_pdf","input":"/data/s02/raw/d.pdf","out_dir":"/results/raw/extract/d-r1","params":{}}"#,
            r#"{"job_id":"p-r1","op":"parse","input":"/data/s02/raw/p.pdf","out_dir":"/results/raw/parse/p-r1","params":{}}"#,
        ]
        .join("\n");
        let runs = [
            r#"{"job_id":"a-r1","outcome":{"status":"ok","pages":1,"output":"/results/raw/extract/a-r1","data":{"chars":10,"words":2}},"exit_signal":null,"peak_rss_kb":1,"wall_ms":5,"oom_kill_delta":0,"leftover_procs":0,"oom_adj_set":true}"#,
            r#"{"job_id":"b-r1","outcome":{"status":"quarantined","reasons":["javascript"]},"exit_signal":null,"peak_rss_kb":1,"wall_ms":5,"oom_kill_delta":0,"leftover_procs":0,"oom_adj_set":true}"#,
            r#"{"job_id":"c-r1","outcome":{"status":"failed","kind":"Timeout","detail":"wall"},"exit_signal":9,"peak_rss_kb":1,"wall_ms":5,"oom_kill_delta":0,"leftover_procs":0,"oom_adj_set":true}"#,
            r#"{"status":"job_error","code":"PathNotAllowed","detail":"out_dir_exists","job_id":"d-r1"}"#,
            r#"{"job_id":"p-r1","outcome":{"status":"ok","pages":1,"output":"/results/raw/parse/p-r1","data":{}},"exit_signal":null,"peak_rss_kb":1,"wall_ms":5,"oom_kill_delta":0,"leftover_procs":0,"oom_adj_set":true}"#,
            r#"{"job_id":"z-r1","outcome":{"status":"ok","pages":1,"output":"/results/raw/extract/z-r1","data":{}},"exit_signal":null,"peak_rss_kb":1,"wall_ms":5,"oom_kill_delta":0,"leftover_procs":0,"oom_adj_set":true}"#,
            "riga non JSON",
        ]
        .join("\n");
        let got = prescanned_inputs(&jobs, &runs);
        let want: BTreeSet<String> = ["/data/s02/raw/a.pdf".to_owned()].into();
        assert_eq!(got, want);
    }

    #[tokio::test]
    async fn not_prescanned_input_never_reaches_docling() {
        let f = serve(Mode::Reply(200, r#"{"status":"success"}"#));
        let input = input_pdf("gate");
        let r = run_one(&cfg(&f), &input, &BTreeSet::new()).await;
        assert_eq!(r.status, Status::NotPrescanned);
        assert_eq!(r.body, None);
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            f.conns.load(Ordering::SeqCst),
            0,
            "connessione a Docling senza prescan"
        );

        let admitted: BTreeSet<String> = [input.to_string_lossy().into_owned()].into();
        let r = run_one(&cfg(&f), &input, &admitted).await;
        assert_eq!(r.status, Status::Ok, "{r:?}");
        assert_eq!(f.conns.load(Ordering::SeqCst), 1);
    }

    // ---- TEST-3 (client): opzioni e page_range inviati -----------------------------------

    #[tokio::test]
    async fn request_carries_page_range_and_fixed_options() {
        let f = serve(Mode::Reply(
            200,
            r#"{"document":{"json_content":{}},"status":"success"}"#,
        ));
        let mut c = cfg(&f);
        c.page_range = Some((1, 1));
        let r = convert(&c, &input_pdf("opts")).await;
        assert_eq!(r.status, Status::Ok, "{r:?}");
        assert_eq!(r.status.exit_code(), 0);
        assert!(
            r.body.as_deref().is_some_and(|b| b.starts_with(b"{")),
            "{r:?}"
        );
        let req = f.requests.recv_timeout(Duration::from_secs(5)).unwrap();
        let text = String::from_utf8_lossy(&req);
        assert!(text.starts_with("POST /v1/convert/"), "{text}");
        for needle in [
            "page_range",
            "to_formats",
            "json",
            "do_ocr",
            "false",
            "table_mode",
            "accurate",
            "do_table_structure",
            "document_timeout",
        ] {
            assert!(text.contains(needle), "manca {needle:?} nella richiesta");
        }
    }

    // ---- TEST-2 (client): timeout, disconnessione, errore --------------------------------

    #[tokio::test]
    async fn caller_timeout_is_client_timeout_exit_3() {
        let f = serve(Mode::Hang);
        let mut c = cfg(&f);
        c.caller_timeout = Duration::from_millis(400);
        let t = Instant::now();
        let r = convert(&c, &input_pdf("timeout")).await;
        assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
        assert_eq!(r.status, Status::ClientTimeout, "{r:?}");
        assert_eq!(r.status.exit_code(), 3);
        assert_eq!(r.body, None);
    }

    #[tokio::test]
    async fn abort_after_is_client_abort_exit_3() {
        let f = serve(Mode::Hang);
        let mut c = cfg(&f);
        c.abort_after = Some(Duration::from_millis(300));
        let t = Instant::now();
        let r = convert(&c, &input_pdf("abort")).await;
        assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
        assert_eq!(r.status, Status::ClientAbort, "{r:?}");
        assert_eq!(r.status.exit_code(), 3);
    }

    #[tokio::test]
    async fn service_error_is_error_not_timeout() {
        let f = serve(Mode::Reply(500, r#"{"detail":"boom"}"#));
        let r = convert(&cfg(&f), &input_pdf("err")).await;
        assert_eq!(r.status, Status::Error, "{r:?}");
        assert_ne!(r.status.exit_code(), 3);
    }

    // ---- TEST-8: riavvio nello script di benchmark ---------------------------------------

    #[test]
    fn bench_script_restarts_with_stop_and_up_never_down_or_profiles() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("docling-bench/docling-bench.sh");
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let code: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.starts_with('#'))
            .collect();
        let has = |s: &str| code.iter().any(|l| l.contains(s));
        for forbidden in [
            "session.sh down",
            "compose run",
            "--profile",
            "compose down",
            "docker volume",
        ] {
            assert!(!has(forbidden), "lo script contiene {forbidden:?}");
        }
        // `session.sh with` lo dà il chiamante: lo script gira dentro la sessione.
        for required in [
            "session.sh up",
            "session.sh stop",
            "session.sh cid",
            "docker stats --no-stream",
            "nvidia-smi",
            "docling-client",
        ] {
            assert!(has(required), "lo script non contiene {required:?}");
        }
        // Ogni stop prende il container da `cid` (sulla stessa riga o nelle 5 precedenti) ed
        // è seguito da un `up`.
        let stops: Vec<usize> = code
            .iter()
            .enumerate()
            .filter(|(_, l)| l.contains("session.sh stop"))
            .map(|(i, _)| i)
            .collect();
        assert!(!stops.is_empty());
        for &i in &stops {
            assert!(
                code[i.saturating_sub(5)..=i]
                    .iter()
                    .any(|l| l.contains("session.sh cid")),
                "stop senza cid: {}",
                code[i]
            );
            assert!(
                code[i + 1..].iter().any(|l| l.contains("session.sh up")),
                "stop alla riga {i} senza up successivo"
            );
        }
        let plain = format!("{}://", "http");
        assert!(!has(&plain), "URL in chiaro nello script");
    }
}
