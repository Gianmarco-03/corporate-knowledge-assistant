//! Handler `POST /upload` in streaming (RESEARCH §3), `GET /healthz`, `GET /bench/stats`.
//!
//! Ordine (D-02, D-03, RISK-003): permesso di upload → stream su tempfile con deadline
//! → permesso di validazione → `validate::check` dentro `spawn_blocking` →
//! `gc::adopt_begin(sha)` → `sink.put(sha, path)` (sempre, anche se l'oggetto esiste:
//! stessa sequenza con o senza deduplica) → transazione breve con riga `upload`, riga
//! `outbox` e `gc::adopt_commit` → risposta. Nessuna chiamata esterna dentro una
//! transazione.
//!
//! Limite di 25 MiB su tre livelli (D-04): `RequestBodyLimitLayer` globale,
//! `DefaultBodyLimit` sulla sola route di upload, contatore manuale sul contenuto del
//! campo `file`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use tokio::io::AsyncWriteExt;
use tower_http::limit::RequestBodyLimitLayer;
use uuid::Uuid;

use crate::admission::{Admission, AdmissionConfig, Busy, TmpBytes};
use crate::sink::BlobSink;
use crate::{gc, validate};

/// Dimensione massima del contenuto del campo `file` (25 MiB, D-04).
pub const MAX_FILE: u64 = 26_214_400;

/// Margine per boundary e header dei campi multipart.
pub const MP_OVERHEAD: u64 = 64 * 1024;

/// Limite del corpo HTTP intero, per `RequestBodyLimitLayer` e `DefaultBodyLimit`.
pub const BODY_LIMIT: usize = (MAX_FILE + MP_OVERHEAD) as usize;

/// Risposta di un upload riuscito: stessa forma con o senza deduplica (D-02, T-11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResponse {
    pub upload_id: Uuid,
    pub sha256: String,
    pub size_bytes: u64,
    /// `accepted` | `quarantined`.
    pub status: String,
}

/// Configurazione del server di upload.
#[derive(Debug, Clone)]
pub struct UploadConfig {
    /// Cartella dei tempfile (`/work/tmp` nel runner, D-03).
    pub tmp_dir: PathBuf,
    /// `false` = `--validate off` (solo con `NullSink`, solo puro streaming).
    pub validate: bool,
    /// `true` = `--bench` (cronometri per fase).
    pub bench: bool,
    pub admission: AdmissionConfig,
}

/// Opzioni del solo binario di misura.
#[derive(Debug, Clone, Copy, Default)]
pub struct ServerOptions {
    /// `--fault after_put`: `std::process::abort()` dopo il PUT, prima della
    /// transazione (solo con `--bench`, per il caso crash-fra-put-e-tx).
    pub fault_after_put: bool,
}

/// Tempi per fase in `--bench` (µs), esposti da `GET /bench/phases`.
#[derive(Default)]
struct Phases {
    receive_hash: common::hist::Recorder,
    validate: common::hist::Recorder,
    persist_original: common::hist::Recorder,
    tx_outbox: common::hist::Recorder,
    total: common::hist::Recorder,
}

#[derive(Serialize)]
struct PhaseSummary {
    n: u64,
    p50_us: u64,
    p95_us: u64,
    p99_us: u64,
    max_us: u64,
}

impl From<common::hist::Summary> for PhaseSummary {
    fn from(s: common::hist::Summary) -> Self {
        PhaseSummary {
            n: s.n,
            p50_us: s.p50_us,
            p95_us: s.p95_us,
            p99_us: s.p99_us,
            max_us: s.max_us,
        }
    }
}

/// Contatori di correttezza in `--bench`.
#[derive(Default)]
struct Counters {
    /// Massimo di byte del campo letti prima di un 413 del contatore manuale.
    max_bytes_read_rejected: AtomicU64,
    status_2xx: AtomicU64,
    status_4xx: AtomicU64,
    status_5xx: AtomicU64,
}

struct AppState {
    cfg: UploadConfig,
    opts: ServerOptions,
    sink: Arc<dyn BlobSink>,
    db: Option<sqlx::PgPool>,
    adm: Admission,
    phases: Mutex<Phases>,
    counters: Counters,
}

/// Router con `POST /upload`, `GET /healthz`, `GET /bench/stats` e i limiti di corpo.
/// `db = None`: nessun metadato né GC (puro streaming e test in-process).
pub fn router(
    cfg: UploadConfig,
    sink: Arc<dyn BlobSink>,
    db: Option<sqlx::PgPool>,
) -> anyhow::Result<axum::Router> {
    router_with(cfg, sink, db, ServerOptions::default())
}

/// Come [`router`], con le opzioni del binario di misura.
pub fn router_with(
    cfg: UploadConfig,
    sink: Arc<dyn BlobSink>,
    db: Option<sqlx::PgPool>,
    opts: ServerOptions,
) -> anyhow::Result<axum::Router> {
    anyhow::ensure!(
        cfg.admission.max_inflight > 0 && cfg.admission.max_validations > 0,
        "max_inflight e max_validations devono essere positivi"
    );
    anyhow::ensure!(
        !opts.fault_after_put || cfg.bench,
        "--fault after_put è ammesso solo con --bench"
    );
    let bench = cfg.bench;
    let state = Arc::new(AppState {
        adm: Admission::new(cfg.admission.clone()),
        cfg,
        opts,
        sink,
        db,
        phases: Mutex::default(),
        counters: Counters::default(),
    });
    let mut app = Router::new()
        .route(
            "/upload",
            post(upload).layer(DefaultBodyLimit::max(BODY_LIMIT)),
        )
        .route("/healthz", get(|| async { "ok" }));
    if bench {
        app = app
            .route("/bench/stats", get(bench_stats))
            .route("/bench/phases", get(bench_phases));
    }
    Ok(app
        .layer(RequestBodyLimitLayer::new(BODY_LIMIT))
        .with_state(state))
}

// ---------------------------------------------------------------------- errori

/// Errore con la forma comune `{code, message, request_id}`.
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        ApiError {
            status,
            code,
            message: message.into(),
        }
    }
    fn busy(what: &str) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "busy",
            format!("{what}: riprovare più tardi"),
        )
    }
    fn internal(what: &str) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", what)
    }
    fn into_response(self, request_id: &str) -> Response {
        let body = serde_json::json!({
            "code": self.code,
            "message": self.message,
            "request_id": request_id,
        });
        let mut resp = (self.status, Json(body)).into_response();
        if self.status == StatusCode::SERVICE_UNAVAILABLE {
            resp.headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
        }
        resp
    }
}

impl From<Busy> for ApiError {
    fn from(_: Busy) -> Self {
        ApiError::busy("capacità esaurita")
    }
}

fn request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .filter(|v| {
            !v.is_empty()
                && v.len() <= 64
                && v.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn multipart_err(e: axum::extract::multipart::MultipartError) -> ApiError {
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "too_large",
            "corpo oltre 25 MiB",
        )
    } else {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "bad_multipart",
            "multipart non valido",
        )
    }
}

fn timeout_err() -> ApiError {
    ApiError::new(
        StatusCode::REQUEST_TIMEOUT,
        "receive_timeout",
        "ricezione del corpo oltre la deadline",
    )
}

// ---------------------------------------------------------------------- tempfile

/// Tempfile dell'upload con il suo conteggio di byte temporanei. Al drop il file si
/// cancella (`NamedTempFile`) e i byte escono dal budget.
struct TmpUpload {
    file: NamedTempFile,
    _bytes: TmpBytes,
}

impl TmpUpload {
    fn path(&self) -> &Path {
        self.file.path()
    }
}

/// Cancella il tempfile nel pool blocking (unlink è una syscall bloccante) e attende la
/// fine: la risposta parte solo dopo la cancellazione. Se l'handler viene abbandonato,
/// il task blocking completa comunque la cancellazione.
async fn discard(tmp: TmpUpload) {
    let _ = tokio::task::spawn_blocking(move || drop(tmp)).await;
}

/// Imposta la cancellazione della validazione quando l'handler viene abbandonato.
struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

struct Received {
    tmp: TmpUpload,
    sha256: String,
    size: u64,
}

/// Riceve l'unico campo `file` in streaming su tempfile, con sha256 e contatore.
// security-scanner: public-endpoint - server di misura dello spike S-01, raggiungibile solo sulla rete compose interna senza porte pubblicate; autenticazione e RBAC sono fuori dallo scope dello spike (Identity, fase successiva)
async fn receive(st: &AppState, mp: &mut Multipart) -> Result<Received, ApiError> {
    let idle = st.cfg.admission.idle_timeout;
    let mut field = tokio::time::timeout(idle, mp.next_field())
        .await
        .map_err(|_| timeout_err())?
        .map_err(multipart_err)?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "missing_file",
                "campo file assente",
            )
        })?;
    if field.name() != Some("file") {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "unexpected_field",
            "atteso un solo campo `file`",
        ));
    }
    // Il nome file del client non si legge né si usa (T-08).

    let dir = st.cfg.tmp_dir.clone();
    let (named, std_file) = tokio::task::spawn_blocking(move || {
        let named = NamedTempFile::new_in(dir)?;
        let f = named.as_file().try_clone()?;
        Ok::<_, std::io::Error>((named, f))
    })
    .await
    .map_err(|_| ApiError::internal("tempfile"))?
    .map_err(|_| ApiError::internal("tempfile"))?;
    let mut tmp = TmpUpload {
        file: named,
        _bytes: st.adm.tmp_bytes(),
    };
    let mut out = tokio::fs::File::from_std(std_file);
    let mut hasher = Sha256::new();
    let mut n: u64 = 0;

    loop {
        let chunk = match tokio::time::timeout(idle, field.chunk()).await {
            Err(_) => {
                discard(tmp).await;
                return Err(timeout_err());
            }
            Ok(Err(e)) => {
                discard(tmp).await;
                return Err(multipart_err(e));
            }
            Ok(Ok(None)) => break,
            Ok(Ok(Some(c))) => c,
        };
        n += chunk.len() as u64;
        if n > MAX_FILE {
            st.counters
                .max_bytes_read_rejected
                .fetch_max(n, Ordering::Relaxed);
            discard(tmp).await;
            return Err(ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too_large",
                "file oltre 25 MiB",
            ));
        }
        hasher.update(&chunk);
        if out.write_all(&chunk).await.is_err() {
            discard(tmp).await;
            return Err(ApiError::internal("scrittura del tempfile"));
        }
        tmp._bytes.add(chunk.len() as u64);
    }
    // `flush` attende la fine delle scritture in corso nel pool blocking.
    if out.flush().await.is_err() {
        discard(tmp).await;
        return Err(ApiError::internal("flush del tempfile"));
    }
    drop(out);
    drop(field);
    match tokio::time::timeout(idle, mp.next_field()).await {
        Err(_) => {
            discard(tmp).await;
            return Err(timeout_err());
        }
        Ok(Err(e)) => {
            discard(tmp).await;
            return Err(multipart_err(e));
        }
        Ok(Ok(Some(_))) => {
            discard(tmp).await;
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "unexpected_field",
                "atteso un solo campo `file`",
            ));
        }
        Ok(Ok(None)) => {}
    }
    Ok(Received {
        tmp,
        sha256: hex::encode(hasher.finalize()),
        size: n,
    })
}

// ---------------------------------------------------------------------- handler

// security-scanner: public-endpoint - server di misura dello spike S-01, raggiungibile solo sulla rete compose interna senza porte pubblicate; autenticazione e RBAC sono fuori dallo scope dello spike (Identity, fase successiva)
async fn upload(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    mut mp: Multipart,
) -> Response {
    let rid = request_id(&headers);
    let res = handle(&st, &mut mp).await;
    let (status_class, resp) = match res {
        Ok(body) => (2, (StatusCode::OK, Json(body)).into_response()),
        Err(e) => (e.status.as_u16() / 100, e.into_response(&rid)),
    };
    if st.cfg.bench {
        let c = match status_class {
            2 => &st.counters.status_2xx,
            4 => &st.counters.status_4xx,
            _ => &st.counters.status_5xx,
        };
        c.fetch_add(1, Ordering::Relaxed);
    }
    resp
}

// security-scanner: public-endpoint - server di misura dello spike S-01, raggiungibile solo sulla rete compose interna senza porte pubblicate; autenticazione e RBAC sono fuori dallo scope dello spike (Identity, fase successiva)
async fn handle(st: &Arc<AppState>, mp: &mut Multipart) -> Result<UploadResponse, ApiError> {
    let _upload_permit = st.adm.try_upload()?;
    let total = common::timer::Stopwatch::start();

    // Ricezione con deadline totale; l'inattività è controllata chunk per chunk.
    let sw = common::timer::Stopwatch::start();
    let rec = tokio::time::timeout(st.cfg.admission.receive_deadline, receive(st, mp))
        .await
        .map_err(|_| timeout_err())??;
    let t_receive = sw.elapsed_us();

    // Validazione nel pool blocking: permesso e tempfile si spostano nella closure.
    let sw = common::timer::Stopwatch::start();
    let (verdict, tmp) = if st.cfg.validate {
        let permit = match st.adm.validation().await {
            Ok(p) => p,
            Err(b) => {
                discard(rec.tmp).await;
                return Err(b.into());
            }
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let _cancel_guard = CancelOnDrop(cancel.clone());
        let deadline = Instant::now() + st.cfg.admission.validation_deadline;
        let tmp = rec.tmp;
        let (outcome, tmp) = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let r = validate::check(tmp.path(), deadline, &cancel);
            (r, tmp)
        })
        .await
        .map_err(|_| ApiError::internal("validazione"))?;
        match outcome {
            Ok(v) => (v, tmp),
            Err(rej) => {
                discard(tmp).await;
                return Err(rejection(rej));
            }
        }
    } else {
        (validate::Verdict::Accepted, rec.tmp)
    };
    let t_validate = sw.elapsed_us();

    // Adozione A, poi PUT sempre (stessa sequenza con o senza deduplica).
    let sw = common::timer::Stopwatch::start();
    if let Some(db) = &st.db
        && let Err(e) = gc::adopt_begin(db, &rec.sha256).await
    {
        eprintln!("upload: adozione fallita: {e:#}");
        discard(tmp).await;
        return Err(if e.is::<gc::AdoptBusy>() {
            ApiError::busy("oggetto in cancellazione")
        } else {
            ApiError::internal("adozione")
        });
    }
    // Errori su stderr del server (diagnosi dei 500 di Garage nella deduplica
    // concorrente): i messaggi di object_store e sqlx non contengono credenziali.
    if let Err(e) = st.sink.put(&rec.sha256, tmp.path()).await {
        eprintln!("upload: put fallito: {e:#}");
        discard(tmp).await;
        return Err(ApiError::internal("storage"));
    }
    let t_persist = sw.elapsed_us();
    if st.opts.fault_after_put {
        // Crash simulato fra PUT e transazione: resta un'adozione abbandonata.
        std::process::abort();
    }
    discard(tmp).await;

    let upload_id = Uuid::new_v4();
    let status = verdict.as_str().to_string();
    let sw = common::timer::Stopwatch::start();
    if let Some(db) = &st.db {
        commit_metadata(db, upload_id, &rec.sha256, rec.size, &status)
            .await
            .map_err(|e| {
                eprintln!("upload: transazione dei metadati fallita: {e:#}");
                ApiError::internal("metadati")
            })?;
    }
    let t_tx = sw.elapsed_us();

    if st.cfg.bench {
        let mut p = st.phases.lock().unwrap_or_else(|e| e.into_inner());
        p.receive_hash.record_us(t_receive);
        p.validate.record_us(t_validate);
        p.persist_original.record_us(t_persist);
        p.tx_outbox.record_us(t_tx);
        p.total.record_us(total.elapsed_us());
    }
    Ok(UploadResponse {
        upload_id,
        sha256: rec.sha256,
        size_bytes: rec.size,
        status,
    })
}

fn rejection(r: validate::Rejection) -> ApiError {
    let status = StatusCode::from_u16(r.http_status).unwrap_or(StatusCode::UNPROCESSABLE_ENTITY);
    let code: &'static str = match r.code.as_str() {
        "unsupported_format" => "unsupported_format",
        "validation_timeout" => "validation_timeout",
        "validation_cancelled" => "validation_cancelled",
        "pdf_encrypted" => "pdf_encrypted",
        "pdf_truncated" => "pdf_truncated",
        "docx_dtd" => "docx_dtd",
        "docx_macro" => "docx_macro",
        _ => "invalid_file",
    };
    ApiError::new(status, code, format!("file rifiutato: {}", r.code))
}

/// Transazione breve: riga `upload`, riga `outbox` con `event_id` univoco, adozione B.
async fn commit_metadata(
    db: &sqlx::PgPool,
    upload_id: Uuid,
    sha: &str,
    size: u64,
    status: &str,
) -> anyhow::Result<()> {
    let payload = serde_json::json!({
        "type": "upload.accepted",
        "upload_id": upload_id,
        "sha256": sha,
        "size_bytes": size,
        "status": status,
    });
    let mut tx = db.begin().await?;
    sqlx::query(
        "INSERT INTO upload (id, sha256, size_bytes, status, created_at) \
         VALUES ($1, $2, $3, $4, now())",
    )
    .bind(upload_id)
    .bind(sha)
    .bind(size as i64)
    .bind(status)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO outbox (event_id, payload, created_at) VALUES ($1, $2::text::jsonb, now())",
    )
    .bind(Uuid::new_v4())
    .bind(payload.to_string())
    .execute(&mut *tx)
    .await?;
    gc::adopt_commit(&mut tx, sha).await?;
    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------- bench

// security-scanner: public-endpoint - server di misura dello spike S-01, raggiungibile solo sulla rete compose interna senza porte pubblicate; autenticazione e RBAC sono fuori dallo scope dello spike (Identity, fase successiva)
async fn bench_stats(State(st): State<Arc<AppState>>) -> Json<crate::admission::Stats> {
    Json(st.adm.stats())
}

// security-scanner: public-endpoint - server di misura dello spike S-01, raggiungibile solo sulla rete compose interna senza porte pubblicate; autenticazione e RBAC sono fuori dallo scope dello spike (Identity, fase successiva)
async fn bench_phases(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let p = st.phases.lock().unwrap_or_else(|e| e.into_inner());
    let s = |r: &common::hist::Recorder| PhaseSummary::from(r.summary());
    let c = &st.counters;
    Json(serde_json::json!({
        "receive_hash": s(&p.receive_hash),
        "validate": s(&p.validate),
        "persist_original": s(&p.persist_original),
        "tx_outbox": s(&p.tx_outbox),
        "total": s(&p.total),
        "max_bytes_read_rejected": c.max_bytes_read_rejected.load(Ordering::Relaxed),
        "status_2xx": c.status_2xx.load(Ordering::Relaxed),
        "status_4xx": c.status_4xx.load(Ordering::Relaxed),
        "status_5xx": c.status_5xx.load(Ordering::Relaxed),
        "uptime_note": "contatori dall'avvio del processo",
    }))
}

/// Durata in secondi per la CLI.
pub fn secs(s: u64) -> Duration {
    Duration::from_secs(s)
}
