//! Sonde di leak: task vivi, file descriptor, connessioni stabilite (spike S-05).
//!
//! `open_fds` e `established_conns` funzionano solo su Linux (`/proc`, `ss` di
//! iproute2, presenti nel runner): altrove restituiscono `None` e la colonna del CSV
//! resta vuota.

use std::process::Command;

/// Task vivi nel runtime tokio corrente (`RuntimeMetrics::num_alive_tasks`, stabile in
/// tokio 1.53 senza `tokio_unstable`).
pub fn alive_tasks() -> usize {
    tokio::runtime::Handle::current()
        .metrics()
        .num_alive_tasks()
}

/// File descriptor aperti dal processo (voci di `/proc/self/fd`, compresa quella della
/// lettura stessa: costante fra un campione e l'altro).
pub fn open_fds() -> Option<usize> {
    let dir = std::fs::read_dir("/proc/self/fd").ok()?;
    Some(dir.count())
}

/// Connessioni TCP stabilite con porta di destinazione `port` (lato client del mock):
/// `ss -Htn state established '( dport = :<port> )'`.
pub fn established_conns(port: u16) -> Option<usize> {
    let filter = format!("( dport = :{port} )");
    let out = Command::new("ss")
        .args(["-Htn", "state", "established", &filter])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(text.lines().filter(|l| !l.trim().is_empty()).count())
}
