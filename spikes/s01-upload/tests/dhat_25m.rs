//! TEST-1 (puro streaming, 25 MiB = MAX_FILE): picco di heap dhat dell'upload con `NullSink` e
//! validazione spenta, corpo artificiale generato in streaming. Un test per file: il
//! profiler dhat è globale al processo. Scrive `results/dhat_25m.json`, poi legge
//! `dhat_1m.json` e `dhat_10m.json` e asserisce la soglia di RISULTATI: max_bytes(25 MiB) - max_bytes(1 MiB) < 2 MiB.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use s01_upload::admission::AdmissionConfig;
use s01_upload::sink::NullSink;
use s01_upload::upload::{UploadConfig, UploadResponse, router};
use sha2::{Digest, Sha256};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const SIZE: u64 = s01_upload::upload::MAX_FILE;
const LABEL: &str = "25m";

/// Corpo artificiale: blocchi sha256(seed ‖ contatore), chunk da 64 KiB, nessun file.
fn artificial_body(size: u64) -> reqwest::Body {
    let stream = futures::stream::unfold((0u64, 0u64), move |(sent, counter)| async move {
        if sent >= size {
            return None;
        }
        let n = (size - sent).min(64 * 1024) as usize;
        let mut chunk = Vec::with_capacity(n);
        let mut c = counter;
        while chunk.len() < n {
            let block = Sha256::digest([b"s01-dhat".as_slice(), &c.to_le_bytes()].concat());
            let take = (n - chunk.len()).min(block.len());
            chunk.extend_from_slice(&block[..take]);
            c += 1;
        }
        Some((
            Ok::<Bytes, std::io::Error>(Bytes::from(chunk)),
            (sent + n as u64, c),
        ))
    });
    reqwest::Body::wrap_stream(stream)
}

fn tmp_root() -> PathBuf {
    let work = PathBuf::from("/work/tmp");
    if work.is_dir() {
        work
    } else {
        std::env::temp_dir()
    }
}

async fn upload_artificial(size: u64) -> UploadResponse {
    let tmp = tempfile::tempdir_in(tmp_root()).expect("tempdir");
    let cfg = UploadConfig {
        tmp_dir: tmp.path().to_path_buf(),
        validate: false,
        bench: false,
        admission: AdmissionConfig {
            max_inflight: 4,
            max_validations: 2,
            validation_wait: Duration::from_secs(10),
            receive_deadline: Duration::from_secs(120),
            idle_timeout: Duration::from_secs(15),
            validation_deadline: Duration::from_secs(10),
        },
    };
    let app = router(cfg, Arc::new(NullSink), None).expect("router");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, app).await });

    let part = reqwest::multipart::Part::stream_with_length(artificial_body(size), size)
        .file_name("artificiale.bin");
    let form = reqwest::multipart::Form::new().part("file", part);
    let resp = reqwest::Client::new()
        .post(format!("http://{addr}/upload"))
        .multipart(form)
        .send()
        .await
        .expect("send");
    assert!(
        resp.status().is_success(),
        "upload {size} B: {}",
        resp.status()
    );
    resp.json::<UploadResponse>().await.expect("json")
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "dhat e /work/tmp: nel runner Linux via session.sh oneshot"
)]
fn test1_puro_streaming_dhat_25m_e_soglia() {
    let _profiler = dhat::Profiler::builder().testing().build();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");
    let body = rt.block_on(upload_artificial(SIZE));
    drop(rt);
    assert_eq!(body.size_bytes, SIZE);

    let stats = dhat::HeapStats::get();
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("results");
    std::fs::create_dir_all(&out).expect("results/");
    let json = serde_json::json!({
        "size_bytes": SIZE,
        "mode": "streaming",
        "max_bytes": stats.max_bytes,
        "max_blocks": stats.max_blocks,
        "total_bytes": stats.total_bytes,
    });
    std::fs::write(
        out.join(format!("dhat_{LABEL}.json")),
        serde_json::to_vec_pretty(&json).expect("json"),
    )
    .expect("write dhat json");

    // Soglia scritta prima della misura (RISULTATI §Soglie): la crescita del picco fra
    // 1 MiB e 25 MiB resta sotto 2 MiB. Cargo esegue i binari di test in ordine
    // alfabetico (dhat_10m, dhat_1m, dhat_25m): gli altri due JSON esistono già.
    let read_max = |label: &str| -> u64 {
        let p = out.join(format!("dhat_{label}.json"));
        let raw = std::fs::read(&p).unwrap_or_else(|e| {
            panic!(
                "{} assente ({e}): eseguire prima dhat_1m e dhat_10m",
                p.display()
            )
        });
        let v: serde_json::Value = serde_json::from_slice(&raw).expect("json dhat");
        v["max_bytes"].as_u64().expect("max_bytes")
    };
    let max_1m = read_max("1m");
    let max_10m = read_max("10m");
    let max_25m = stats.max_bytes as u64;
    const SOGLIA: u64 = 2 * 1024 * 1024;
    assert!(
        max_25m.saturating_sub(max_1m) < SOGLIA,
        "picco heap non costante: 1m={max_1m} 10m={max_10m} 25m={max_25m} (soglia delta < {SOGLIA})"
    );
}
