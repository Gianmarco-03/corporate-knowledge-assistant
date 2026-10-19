//! Admission control dell'upload (RISK-003, T-05): tetto di upload in corso, semaforo
//! di validazione, budget di byte temporanei, deadline di ricezione e di validazione.

use std::time::Duration;

use serde::{Deserialize, Serialize};

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
