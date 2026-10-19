//! Server di upload in-process su 127.0.0.1:0.
//!
//! - TEST-4: admission control sotto carico ostile (12 client lenti a 1 KiB/s e 6 DOCX
//!   costosi insieme, metà dei client chiusi a metà): validazioni attive ≤ 2, upload
//!   attivi ≤ 4, codici solo 2xx/408/413/415/422/503, `Retry-After` sui 503, entro 15 s
//!   FD e byte temporanei al baseline. Scrive `results/admission.csv`.
//! - TEST-2: confini 26_214_399 e 26_214_400 byte → 2xx, 26_214_401 → 413.
//! - TEST-3 (parte): nessun file non valido raggiunge il sink (contatore dei `put`).
//! - TEST-8 (parte): risposta con la stessa forma con o senza deduplica, `upload_id`
//!   nuovo per richiesta (D-02, T-11). La misura dei tempi è in `dedup_timing.csv`.
//! - TEST-9 (parte): upload interrotto a metà ricezione → nessun `put`, nessun tempfile.
//!   Il crash fra PUT e transazione (`--fault after_put`) è in `correctness.csv`.
//!
//! Le deadline di ricezione sono accorciate per tenere il test breve (default del
//! piano: 120 s totali, 15 s di inattività).

use std::collections::BTreeSet;
use std::io::Write;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use bytes::Bytes;
use futures::future::BoxFuture;
use s01_upload::admission::{AdmissionConfig, Stats};
use s01_upload::sink::{BlobSink, NullSink};
use s01_upload::upload::{MAX_FILE, UploadConfig, UploadResponse, router};
use zip::write::SimpleFileOptions;

const MAX_INFLIGHT: usize = 4;
const MAX_VALIDATIONS: usize = 2;

/// Sink di test: conta i `put` e delega a `NullSink`.
struct CountingSink {
    puts: AtomicU64,
    inner: NullSink,
}

impl BlobSink for CountingSink {
    fn put<'a>(&'a self, key: &'a str, path: &'a Path) -> BoxFuture<'a, anyhow::Result<()>> {
        self.puts.fetch_add(1, Ordering::SeqCst);
        self.inner.put(key, path)
    }
    fn get_range<'a>(
        &'a self,
        key: &'a str,
        range: Range<u64>,
    ) -> BoxFuture<'a, anyhow::Result<Bytes>> {
        self.inner.get_range(key, range)
    }
    fn delete<'a>(&'a self, key: &'a str) -> BoxFuture<'a, anyhow::Result<()>> {
        self.inner.delete(key)
    }
    fn count(&self) -> BoxFuture<'_, anyhow::Result<u64>> {
        self.inner.count()
    }
}

fn tmp_root() -> PathBuf {
    let work = PathBuf::from("/work/tmp");
    if work.is_dir() {
        work
    } else {
        std::env::temp_dir()
    }
}

fn admission(receive: Duration, idle: Duration) -> AdmissionConfig {
    AdmissionConfig {
        max_inflight: MAX_INFLIGHT,
        max_validations: MAX_VALIDATIONS,
        validation_wait: Duration::from_secs(10),
        receive_deadline: receive,
        idle_timeout: idle,
        validation_deadline: Duration::from_secs(10),
    }
}

/// Avvia il server e restituisce `(base_url, sink)`.
async fn start(
    tmp_dir: &Path,
    validate: bool,
    adm: AdmissionConfig,
) -> (String, Arc<CountingSink>) {
    let sink = Arc::new(CountingSink {
        puts: AtomicU64::new(0),
        inner: NullSink,
    });
    let cfg = UploadConfig {
        tmp_dir: tmp_dir.to_path_buf(),
        validate,
        bench: true,
        admission: adm,
    };
    let app = router(cfg, sink.clone(), None).expect("router");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, app).await });
    (format!("http://{addr}"), sink)
}

fn no_pool_client() -> reqwest::Client {
    reqwest::Client::builder()
        .pool_max_idle_per_host(0)
        .build()
        .expect("client")
}

fn bytes_part(name: &str, data: Vec<u8>) -> reqwest::multipart::Form {
    let part = reqwest::multipart::Part::bytes(data).file_name(name.to_string());
    reqwest::multipart::Form::new().part("file", part)
}

/// Corpo artificiale di `size` byte con prefisso `%PDF-` (validazione spenta nei confini).
fn sized_body(size: u64) -> reqwest::multipart::Form {
    let stream = futures::stream::unfold(0u64, move |sent| async move {
        if sent >= size {
            return None;
        }
        let n = (size - sent).min(64 * 1024) as usize;
        let mut chunk = vec![b'7'; n];
        if sent == 0 {
            let head = b"%PDF-1.7\n";
            let k = head.len().min(n);
            chunk[..k].copy_from_slice(&head[..k]);
        }
        Some((
            Ok::<Bytes, std::io::Error>(Bytes::from(chunk)),
            sent + n as u64,
        ))
    });
    let part =
        reqwest::multipart::Part::stream_with_length(reqwest::Body::wrap_stream(stream), size)
            .file_name("confine.pdf");
    reqwest::multipart::Form::new().part("file", part)
}

/// Client lento: `total` byte a ~1 KiB/s.
fn slow_body(total: usize) -> reqwest::multipart::Form {
    let stream = futures::stream::unfold(0usize, move |sent| async move {
        if sent >= total {
            return None;
        }
        if sent > 0 {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        let n = (total - sent).min(1024);
        let mut chunk = vec![b'a'; n];
        if sent == 0 {
            chunk[..5].copy_from_slice(b"%PDF-");
        }
        Some((Ok::<Bytes, std::io::Error>(Bytes::from(chunk)), sent + n))
    });
    let part = reqwest::multipart::Part::stream_with_length(
        reqwest::Body::wrap_stream(stream),
        total as u64,
    )
    .file_name("lento.pdf");
    reqwest::multipart::Form::new().part("file", part)
}

/// DOCX costoso ma entro i limiti: 990 voci da 64 KiB di zeri (rapporto alto sotto 16 MiB).
fn costly_docx() -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opt = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    w.start_file("[Content_Types].xml", opt).expect("zip");
    w.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#)
        .expect("zip");
    w.start_file("word/document.xml", opt).expect("zip");
    w.write_all(br#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body/></w:document>"#)
        .expect("zip");
    let zeros = vec![0u8; 64 * 1024];
    for i in 0..990 {
        w.start_file(format!("word/media/z{i}.bin"), opt)
            .expect("zip");
        w.write_all(&zeros).expect("zip");
    }
    w.finish().expect("zip").into_inner()
}

async fn stats(client: &reqwest::Client, base: &str) -> Stats {
    client
        .get(format!("{base}/bench/stats"))
        .send()
        .await
        .expect("stats")
        .json::<Stats>()
        .await
        .expect("stats json")
}

fn dir_usage(dir: &Path) -> (u64, u64) {
    let mut files = 0;
    let mut bytes = 0;
    for e in std::fs::read_dir(dir).expect("read_dir").flatten() {
        if let Ok(m) = e.metadata()
            && m.is_file()
        {
            files += 1;
            bytes += m.len();
        }
    }
    (files, bytes)
}

#[derive(Debug)]
enum Outcome {
    Status(u16, bool),
    Transport,
    Aborted,
}

// ------------------------------------------------------------------- TEST-4

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "/proc/self/fd e /work/tmp: nel runner Linux via session.sh oneshot"
)]
async fn test4_admission_sotto_carico_ostile() {
    let tmp = tempfile::tempdir_in(tmp_root()).expect("tempdir");
    let (base, sink) = start(
        tmp.path(),
        true,
        admission(Duration::from_secs(6), Duration::from_secs(3)),
    )
    .await;
    let sampler = reqwest::Client::new();
    let baseline = stats(&sampler, &base).await;
    let (tmp_files_0, _) = dir_usage(tmp.path());

    // Campionamento di /bench/stats ogni 50 ms.
    let stop = Arc::new(tokio::sync::Notify::new());
    let sampler_task = {
        let (sampler, base, stop) = (sampler.clone(), base.clone(), stop.clone());
        tokio::spawn(async move {
            let (mut pu, mut pv) = (0u64, 0u64);
            loop {
                let s = stats(&sampler, &base).await;
                pu = pu.max(s.active_uploads).max(s.peak_active_uploads);
                pv = pv.max(s.active_validations).max(s.peak_active_validations);
                tokio::select! {
                    _ = stop.notified() => break,
                    _ = tokio::time::sleep(Duration::from_millis(50)) => {}
                }
            }
            (pu, pv)
        })
    };

    let docx = Arc::new(costly_docx());
    let mut tasks = Vec::new();
    for i in 0..12 {
        let base = base.clone();
        tasks.push(tokio::spawn(async move {
            let req = no_pool_client()
                .post(format!("{base}/upload"))
                .multipart(slow_body(64 * 1024))
                .send();
            if i % 2 == 0 {
                // Metà dei client chiusi a metà della ricezione.
                match tokio::time::timeout(Duration::from_secs(2), req).await {
                    Err(_) => Outcome::Aborted,
                    Ok(Ok(r)) => Outcome::Status(
                        r.status().as_u16(),
                        r.headers().contains_key("retry-after"),
                    ),
                    Ok(Err(_)) => Outcome::Transport,
                }
            } else {
                match req.await {
                    Ok(r) => Outcome::Status(
                        r.status().as_u16(),
                        r.headers().contains_key("retry-after"),
                    ),
                    // Il server chiude dopo il 408 mentre il client sta ancora scrivendo.
                    Err(_) => Outcome::Transport,
                }
            }
        }));
    }
    for i in 0..6 {
        let (base, docx) = (base.clone(), docx.clone());
        tasks.push(tokio::spawn(async move {
            let req = no_pool_client()
                .post(format!("{base}/upload"))
                .multipart(bytes_part("costoso.docx", docx.to_vec()))
                .send();
            if i % 2 == 0 {
                // Client che chiude durante la validazione.
                match tokio::time::timeout(Duration::from_millis(300), req).await {
                    Err(_) => Outcome::Aborted,
                    Ok(Ok(r)) => Outcome::Status(
                        r.status().as_u16(),
                        r.headers().contains_key("retry-after"),
                    ),
                    Ok(Err(_)) => Outcome::Transport,
                }
            } else {
                match req.await {
                    Ok(r) => Outcome::Status(
                        r.status().as_u16(),
                        r.headers().contains_key("retry-after"),
                    ),
                    Err(_) => Outcome::Transport,
                }
            }
        }));
    }
    let mut outcomes = Vec::new();
    for t in tasks {
        outcomes.push(t.await.expect("join"));
    }

    // Entro 15 s dalla fine del carico FD e tempfile tornano al baseline.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let (fd_after, tmp_files_after, tmp_bytes_after) = loop {
        let s = stats(&sampler, &base).await;
        let (files, bytes) = dir_usage(tmp.path());
        let ok = s.open_fds <= baseline.open_fds + 4 && files == tmp_files_0;
        if ok || tokio::time::Instant::now() >= deadline {
            break (s.open_fds, files, bytes);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    stop.notify_one();
    let (peak_uploads, peak_validations) = sampler_task.await.expect("sampler");

    let allowed: BTreeSet<u16> = [408, 413, 415, 422, 503].into_iter().collect();
    let mut n503 = 0;
    let mut n408 = 0;
    let mut n2xx = 0u64;
    let mut bad_codes = Vec::new();
    let mut missing_retry_after = 0;
    for o in &outcomes {
        if let Outcome::Status(code, retry_after) = o {
            match *code {
                200..=299 => n2xx += 1,
                503 => {
                    n503 += 1;
                    if !retry_after {
                        missing_retry_after += 1;
                    }
                }
                408 => n408 += 1,
                c if allowed.contains(&c) => {}
                c => bad_codes.push(c),
            }
        }
    }
    let puts = sink.puts.load(Ordering::SeqCst);
    let fd_ok = fd_after <= baseline.open_fds + 4;
    let tmp_ok = tmp_files_after == tmp_files_0;
    let pass = peak_validations <= MAX_VALIDATIONS as u64
        && peak_uploads <= MAX_INFLIGHT as u64
        && bad_codes.is_empty()
        && missing_retry_after == 0
        && puts <= 6
        && puts >= n2xx
        && fd_ok
        && tmp_ok;

    let csv_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("results/admission.csv");
    std::fs::create_dir_all(csv_path.parent().expect("parent")).expect("results/");
    let mut csv = common::csv::Writer::create(
        &csv_path,
        &[
            "case",
            "max_inflight",
            "max_validations",
            "peak_active_uploads",
            "peak_active_validations",
            "status_503",
            "status_408",
            "fd_baseline",
            "fd_after",
            "tmp_bytes_after",
            "pass",
        ],
    )
    .expect("csv");
    csv.write_row(&[
        "slow12_docx6_half_closed",
        &MAX_INFLIGHT.to_string(),
        &MAX_VALIDATIONS.to_string(),
        &peak_uploads.to_string(),
        &peak_validations.to_string(),
        &n503.to_string(),
        &n408.to_string(),
        &baseline.open_fds.to_string(),
        &fd_after.to_string(),
        &tmp_bytes_after.to_string(),
        &pass.to_string(),
    ])
    .expect("csv row");
    drop(csv);

    assert!(
        peak_validations <= MAX_VALIDATIONS as u64,
        "validazioni attive {peak_validations} > {MAX_VALIDATIONS}"
    );
    assert!(
        peak_uploads <= MAX_INFLIGHT as u64,
        "upload attivi {peak_uploads} > {MAX_INFLIGHT}"
    );
    assert!(bad_codes.is_empty(), "codici fuori elenco: {bad_codes:?}");
    assert_eq!(missing_retry_after, 0, "503 senza Retry-After");
    assert!(
        puts <= 6 && puts >= n2xx,
        "put={puts}, 2xx={n2xx}: solo i DOCX validi raggiungono il sink"
    );
    assert!(
        fd_ok,
        "FD {fd_after} oltre baseline {} + 4",
        baseline.open_fds
    );
    assert!(
        tmp_ok,
        "tempfile residui: {tmp_files_after} (baseline {tmp_files_0}), {tmp_bytes_after} B"
    );
    assert!(
        n408 > 0 || n503 > 0,
        "nessun rifiuto sotto carico: {outcomes:?}"
    );
}

// ------------------------------------------------------------------- TEST-2

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test2_confini_25mib_accettati_e_piu_uno_413() {
    let tmp = tempfile::tempdir_in(tmp_root()).expect("tempdir");
    let (base, sink) = start(
        tmp.path(),
        false,
        admission(Duration::from_secs(120), Duration::from_secs(15)),
    )
    .await;
    let client = reqwest::Client::new();
    for (size, expected_ok) in [
        (MAX_FILE - 1, true),
        (MAX_FILE, true),
        (MAX_FILE + 1, false),
    ] {
        let resp = client
            .post(format!("{base}/upload"))
            .multipart(sized_body(size))
            .send()
            .await
            .expect("send");
        let code = resp.status().as_u16();
        if expected_ok {
            assert!((200..300).contains(&code), "{size} B → {code}, atteso 2xx");
            let body: UploadResponse = resp.json().await.expect("json");
            assert_eq!(body.size_bytes, size);
        } else {
            assert_eq!(code, 413, "{size} B → {code}, atteso 413");
        }
    }
    assert_eq!(
        sink.puts.load(Ordering::SeqCst),
        2,
        "il file oltre limite non raggiunge il sink"
    );
    assert_eq!(dir_usage(tmp.path()).0, 0, "nessun tempfile residuo");
}

// ------------------------------------------------------------------- TEST-9

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test9_upload_interrotto_nessun_put_nessun_tempfile() {
    let tmp = tempfile::tempdir_in(tmp_root()).expect("tempdir");
    let (base, sink) = start(
        tmp.path(),
        true,
        admission(Duration::from_secs(120), Duration::from_secs(15)),
    )
    .await;
    // Il client invia 128 KiB di un PDF dichiarato da 1 MiB e poi si blocca: il test
    // chiude la connessione a metà ricezione.
    let total = 1024 * 1024u64;
    let stream = futures::stream::unfold(0u8, |step| async move {
        match step {
            0 => {
                let mut chunk = vec![b'5'; 128 * 1024];
                chunk[..9].copy_from_slice(b"%PDF-1.7\n");
                Some((Ok::<Bytes, std::io::Error>(Bytes::from(chunk)), 1))
            }
            _ => {
                std::future::pending::<()>().await;
                None
            }
        }
    });
    let part =
        reqwest::multipart::Part::stream_with_length(reqwest::Body::wrap_stream(stream), total)
            .file_name("interrotto.pdf");
    let form = reqwest::multipart::Form::new().part("file", part);
    let req = no_pool_client()
        .post(format!("{base}/upload"))
        .multipart(form)
        .send();
    let r = tokio::time::timeout(Duration::from_secs(1), req).await;
    assert!(
        r.is_err(),
        "il server non deve rispondere a un corpo incompleto"
    );

    // Dopo la chiusura il tempfile si cancella e nulla raggiunge il sink.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while dir_usage(tmp.path()).0 > 0 && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(
        dir_usage(tmp.path()).0,
        0,
        "tempfile residuo dopo l'interruzione"
    );
    assert_eq!(
        sink.puts.load(Ordering::SeqCst),
        0,
        "put di un upload interrotto"
    );
}

// ------------------------------------------------------------------- TEST-3 / TEST-8

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test3_file_non_valido_non_raggiunge_il_sink() {
    let tmp = tempfile::tempdir_in(tmp_root()).expect("tempdir");
    let (base, sink) = start(
        tmp.path(),
        true,
        admission(Duration::from_secs(120), Duration::from_secs(15)),
    )
    .await;
    let client = reqwest::Client::new();
    // Magic ZIP con nome .pdf (polyglot) e binario con NUL dichiarato Markdown.
    let casi: [(&str, Vec<u8>); 2] = [
        ("finto.pdf", b"PK\x03\x04\x14\x00\x00\x00junk".to_vec()),
        ("finto.md", vec![b'#', 0, 0, 0xff]),
    ];
    for (name, data) in casi {
        let resp = client
            .post(format!("{base}/upload"))
            .multipart(bytes_part(name, data))
            .send()
            .await
            .expect("send");
        assert_eq!(resp.status().as_u16(), 415, "caso {name}");
    }
    assert_eq!(
        sink.puts.load(Ordering::SeqCst),
        0,
        "nessun put per file rifiutati"
    );
    assert_eq!(dir_usage(tmp.path()).0, 0, "nessun tempfile residuo");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test8_risposta_identica_con_o_senza_deduplica() {
    let tmp = tempfile::tempdir_in(tmp_root()).expect("tempdir");
    let (base, sink) = start(
        tmp.path(),
        true,
        admission(Duration::from_secs(120), Duration::from_secs(15)),
    )
    .await;
    let pdf_path = tmp.path().join("fixture-dedup.pdf.src");
    s01_upload::pdfgen::generate(&pdf_path, 256 * 1024, 42).expect("generate");
    let data = std::fs::read(&pdf_path).expect("read");
    std::fs::remove_file(&pdf_path).expect("rm");

    let client = reqwest::Client::new();
    let mut raw = Vec::new();
    for _ in 0..2 {
        let resp = client
            .post(format!("{base}/upload"))
            .multipart(bytes_part("relazione.pdf", data.clone()))
            .send()
            .await
            .expect("send");
        assert!(resp.status().is_success(), "{}", resp.status());
        raw.push(resp.json::<serde_json::Value>().await.expect("json"));
    }
    let keys = |v: &serde_json::Value| -> BTreeSet<String> {
        v.as_object().expect("oggetto").keys().cloned().collect()
    };
    let expected: BTreeSet<String> = ["upload_id", "sha256", "size_bytes", "status"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(keys(&raw[0]), expected, "campi della prima risposta");
    assert_eq!(keys(&raw[1]), expected, "campi della risposta deduplicata");
    assert_eq!(raw[0]["sha256"], raw[1]["sha256"]);
    assert_eq!(raw[0]["status"], raw[1]["status"]);
    assert_eq!(raw[0]["size_bytes"], raw[1]["size_bytes"]);
    assert_ne!(
        raw[0]["upload_id"], raw[1]["upload_id"],
        "upload_id nuovo per richiesta"
    );
    // Stessa sequenza nei due casi: il PUT avviene anche per il duplicato.
    assert_eq!(sink.puts.load(Ordering::SeqCst), 2);
}
