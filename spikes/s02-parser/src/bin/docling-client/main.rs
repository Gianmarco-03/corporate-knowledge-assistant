//! Bin `docling-client` (piano 01-08): invia a Docling (`DOCLING_URL` / `DOCLING_CPU_URL`)
//! solo i PDF che hanno un `RunReport` `ok` di `extract_pdf`.
//!
//! STUB di `test-write`: tipi e firme minime; i corpi sono dell'esecutore.
//! Nota per l'esecutore: il `RunReport` di 01-07 non porta `input`, quindi l'insieme degli
//! input ammessi si ricava dal JSONL dei job (`job_id` → `input`, solo `op: extract_pdf`)
//! unito a `runs.jsonl` (`job_id` → `outcome.status`).
#![allow(dead_code)] // stub di test-write: l'esecutore lo rimuove con l'implementazione

use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

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
        todo!("01-08: nomi degli esiti")
    }

    /// Codice d'uscita del processo per questo esito.
    pub(crate) fn exit_code(self) -> i32 {
        todo!("01-08: codici d'uscita")
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
    pub(crate) fn new(_base_url: String, _document_timeout_s: u64) -> ConvertConfig {
        todo!("01-08: configurazione di default")
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
pub(crate) fn prescanned_inputs(_jobs_jsonl: &str, _runs_jsonl: &str) -> BTreeSet<String> {
    todo!("01-08: input ammessi da extract_pdf")
}

/// `POST {base_url}/v1/convert/file` (multipart) con `to_formats=json`, `do_ocr=false`,
/// `table_mode=accurate`, `do_table_structure=true`, `page_range`, `document_timeout`.
pub(crate) async fn convert(_cfg: &ConvertConfig, _file: &Path) -> ConvertResult {
    todo!("01-08: richiesta a Docling")
}

/// `convert` solo se `input` è in `admitted`; altrimenti `NotPrescanned` senza rete.
pub(crate) async fn run_one(
    _cfg: &ConvertConfig,
    _input: &Path,
    _admitted: &BTreeSet<String>,
) -> ConvertResult {
    todo!("01-08: precondizione del prescan")
}

fn main() {
    todo!("01-08: bin docling-client")
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
