//! Admission control dell'upload (RISK-003, T-05): tetto di upload in corso, semaforo
//! di validazione, budget di byte temporanei, deadline di ricezione e di validazione.
//!
//! Due `tokio::sync::Semaphore`: `inflight` (upload in corso, `try_acquire` → 503 con
//! `Retry-After`) e `validations` (attesa ≤ `validation_wait`, poi 503). I contatori
//! atomici alimentano `GET /bench/stats`.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::upload::MAX_FILE;

/// Parametri di admission. Default del piano: `max_inflight` 32, `max_validations` 8,
/// `validation_wait` 10 s, `receive_deadline` 120 s, `idle_timeout` 15 s,
/// `validation_deadline` 10 s.
#[derive(Debug, Clone)]
pub struct AdmissionConfig {
    /// Upload in corso massimi; oltre, 503 con `Retry-After`.
    pub max_inflight: usize,
    /// Validazioni attive massime.
    pub max_validations: usize,
    /// Attesa massima di un permesso di validazione, poi 503.
    pub validation_wait: Duration,
    /// Deadline totale di ricezione del corpo, poi 408.
    pub receive_deadline: Duration,
    /// Inattività massima fra due chunk, poi 408.
    pub idle_timeout: Duration,
    /// Deadline di `validate::check`, poi 422 `validation_timeout`.
    pub validation_deadline: Duration,
}

impl Default for AdmissionConfig {
    fn default() -> Self {
        AdmissionConfig {
            max_inflight: 32,
            max_validations: 8,
            validation_wait: Duration::from_secs(10),
            receive_deadline: Duration::from_secs(120),
            idle_timeout: Duration::from_secs(15),
            validation_deadline: Duration::from_secs(10),
        }
    }
}

impl AdmissionConfig {
    /// Budget di byte temporanei: `max_inflight × MAX_FILE` (800 MiB coi default).
    pub fn tmp_budget_bytes(&self) -> u64 {
        self.max_inflight as u64 * MAX_FILE
    }
}

/// Corpo JSON di `GET /bench/stats`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub active_uploads: u64,
    pub peak_active_uploads: u64,
    pub active_validations: u64,
    pub peak_active_validations: u64,
    pub tmp_bytes: u64,
    pub open_fds: u64,
    pub vm_hwm_kb: u64,
    pub cgroup_peak_bytes: Option<u64>,
}

/// Stato condiviso dell'admission.
pub struct Admission {
    pub cfg: AdmissionConfig,
    inflight: Arc<Semaphore>,
    validations: Arc<Semaphore>,
    active_uploads: Arc<AtomicU64>,
    peak_uploads: Arc<AtomicU64>,
    active_validations: Arc<AtomicU64>,
    peak_validations: Arc<AtomicU64>,
    tmp_bytes: Arc<AtomicU64>,
}

/// Rifiuto di admission: 503 con `Retry-After`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Busy;

/// Permesso di un upload in corso; al drop libera il posto e il contatore.
pub struct UploadPermit {
    _permit: OwnedSemaphorePermit,
    active: Arc<AtomicU64>,
}

impl Drop for UploadPermit {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Permesso di validazione: si sposta dentro la closure di `spawn_blocking`, così il
/// lavoro già partito lo tiene fino alla fine anche se il client se n'è andato.
pub struct ValidationPermit {
    _permit: OwnedSemaphorePermit,
    active: Arc<AtomicU64>,
}

impl Drop for ValidationPermit {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Byte temporanei di un tempfile: al drop li sottrae dal contatore globale.
pub struct TmpBytes {
    n: u64,
    total: Arc<AtomicU64>,
}

impl TmpBytes {
    pub fn add(&mut self, k: u64) {
        self.n += k;
        self.total.fetch_add(k, Ordering::SeqCst);
    }
}

impl Drop for TmpBytes {
    fn drop(&mut self) {
        self.total.fetch_sub(self.n, Ordering::SeqCst);
    }
}

fn bump_peak(active: &AtomicU64, peak: &AtomicU64) {
    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
    peak.fetch_max(now, Ordering::SeqCst);
}

impl Admission {
    pub fn new(cfg: AdmissionConfig) -> Self {
        Admission {
            inflight: Arc::new(Semaphore::new(cfg.max_inflight)),
            validations: Arc::new(Semaphore::new(cfg.max_validations)),
            cfg,
            active_uploads: Arc::default(),
            peak_uploads: Arc::default(),
            active_validations: Arc::default(),
            peak_validations: Arc::default(),
            tmp_bytes: Arc::default(),
        }
    }

    /// Posto per un upload in corso, senza attesa.
    pub fn try_upload(&self) -> Result<UploadPermit, Busy> {
        let permit = self
            .inflight
            .clone()
            .try_acquire_owned()
            .map_err(|_| Busy)?;
        bump_peak(&self.active_uploads, &self.peak_uploads);
        Ok(UploadPermit {
            _permit: permit,
            active: self.active_uploads.clone(),
        })
    }

    /// Permesso di validazione, con attesa al più `validation_wait`.
    pub async fn validation(&self) -> Result<ValidationPermit, Busy> {
        let fut = self.validations.clone().acquire_owned();
        let permit = tokio::time::timeout(self.cfg.validation_wait, fut)
            .await
            .map_err(|_| Busy)?
            .map_err(|_| Busy)?;
        bump_peak(&self.active_validations, &self.peak_validations);
        Ok(ValidationPermit {
            _permit: permit,
            active: self.active_validations.clone(),
        })
    }

    /// Nuovo contatore di byte temporanei per un tempfile.
    pub fn tmp_bytes(&self) -> TmpBytes {
        TmpBytes {
            n: 0,
            total: self.tmp_bytes.clone(),
        }
    }

    pub fn stats(&self) -> Stats {
        Stats {
            active_uploads: self.active_uploads.load(Ordering::SeqCst),
            peak_active_uploads: self.peak_uploads.load(Ordering::SeqCst),
            active_validations: self.active_validations.load(Ordering::SeqCst),
            peak_active_validations: self.peak_validations.load(Ordering::SeqCst),
            tmp_bytes: self.tmp_bytes.load(Ordering::SeqCst),
            open_fds: open_fds(),
            vm_hwm_kb: vm_hwm_kb(),
            cgroup_peak_bytes: cgroup_peak_bytes(),
        }
    }
}

/// FD aperti dal processo (`/proc/self/fd`); 0 fuori Linux.
pub fn open_fds() -> u64 {
    std::fs::read_dir("/proc/self/fd")
        .map(|d| d.count() as u64)
        .unwrap_or(0)
}

/// Picco di RSS del processo (`VmHWM` di `/proc/self/status`), in KiB; 0 fuori Linux.
pub fn vm_hwm_kb() -> u64 {
    let Ok(s) = std::fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    s.lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.trim().trim_end_matches("kB").trim().parse().ok())
        .unwrap_or(0)
}

/// `memory.peak` del cgroup v2 del container, se disponibile (RESEARCH §11.4).
pub fn cgroup_peak_bytes() -> Option<u64> {
    std::fs::read_to_string("/sys/fs/cgroup/memory.peak")
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

/// Spazio libero in byte del filesystem di `dir`, da `df -Pk` (POSIX). `None` se non
/// determinabile: chi chiama rifiuta di partire (a sicurezza chiusa).
pub fn free_bytes(dir: &Path) -> Option<u64> {
    let out = std::process::Command::new("df")
        .arg("-Pk")
        .arg(dir)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().nth(1)?;
    let avail_kb: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
    Some(avail_kb * 1024)
}
