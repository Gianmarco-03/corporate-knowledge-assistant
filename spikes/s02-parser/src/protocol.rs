//! Esiti tipizzati del figlio (contratto W17, punto 2).
//!
//! Il figlio stampa su stdout una sola riga JSON con [`emit`]; il padre la rilegge con
//! [`parse_line`] dentro `isolate::classify`. Forma della riga:
//! `{"status":"ok","pages":N|null,"output":"…"|null,"data":<json>|null,"vm_hwm_kb":N|null}`,
//! `{"status":"failed","kind":"<Kind>","detail":"…","vm_hwm_kb":…}` oppure
//! `{"status":"quarantined","reasons":[…],"vm_hwm_kb":…}`.
use std::io::Write;

use serde::{Deserialize, Serialize};

/// Classe di fallimento del figlio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Encrypted,
    DecompressionLimit,
    Timeout,
    ResourceLimit,
    OutputLimit,
    Crashed,
    Corrupt,
    Unsupported,
}

impl Kind {
    /// Nome del contratto (uguale alla variante).
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Encrypted => "Encrypted",
            Kind::DecompressionLimit => "DecompressionLimit",
            Kind::Timeout => "Timeout",
            Kind::ResourceLimit => "ResourceLimit",
            Kind::OutputLimit => "OutputLimit",
            Kind::Crashed => "Crashed",
            Kind::Corrupt => "Corrupt",
            Kind::Unsupported => "Unsupported",
        }
    }
}

/// Esito di un job, una riga JSON su stdout del figlio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum ChildOutcome {
    Ok {
        pages: Option<u64>,
        output: Option<String>,
        data: Option<serde_json::Value>,
    },
    Failed {
        kind: Kind,
        detail: String,
    },
    Quarantined {
        reasons: Vec<String>,
    },
}

impl ChildOutcome {
    /// Esito `failed` con il dettaglio dato.
    pub fn failed(kind: Kind, detail: impl Into<String>) -> ChildOutcome {
        ChildOutcome::Failed {
            kind,
            detail: detail.into(),
        }
    }

    /// `ok`, `failed` o `quarantined`.
    pub fn status(&self) -> &'static str {
        match self {
            ChildOutcome::Ok { .. } => "ok",
            ChildOutcome::Failed { .. } => "failed",
            ChildOutcome::Quarantined { .. } => "quarantined",
        }
    }
}

/// Riga del figlio con il campo aggiunto da [`emit`].
#[derive(Serialize)]
struct Line<'a> {
    #[serde(flatten)]
    outcome: &'a ChildOutcome,
    vm_hwm_kb: Option<u64>,
}

/// Campo `vm_hwm_kb` della riga, letto senza toccare il resto.
#[derive(Deserialize)]
struct HwmOnly {
    #[serde(default)]
    vm_hwm_kb: Option<u64>,
}

/// Stampa l'esito come una sola riga JSON con `vm_hwm_kb`.
pub fn emit(outcome: &ChildOutcome) -> anyhow::Result<()> {
    let line = serde_json::to_string(&Line {
        outcome,
        vm_hwm_kb: vm_hwm_kb(),
    })?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    out.write_all(line.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}

/// Rilegge una riga del figlio: esito e `vm_hwm_kb`; `None` se assente o malformata.
pub fn parse_line(line: &str) -> Option<(ChildOutcome, Option<u64>)> {
    let outcome: ChildOutcome = serde_json::from_str(line).ok()?;
    let hwm = serde_json::from_str::<HwmOnly>(line)
        .ok()
        .and_then(|h| h.vm_hwm_kb);
    Some((outcome, hwm))
}

/// `VmHWM` del processo corrente in KiB (`/proc/self/status`), se disponibile.
pub fn vm_hwm_kb() -> Option<u64> {
    proc_status_kb("/proc/self/status", "VmHWM:")
}

/// `VmPeak` del processo corrente in KiB (`/proc/self/status`), se disponibile.
pub fn vm_peak_kb() -> Option<u64> {
    proc_status_kb("/proc/self/status", "VmPeak:")
}

/// Valore in KiB della chiave `key` di un file in formato `/proc/<pid>/status`.
pub fn proc_status_kb(path: &str, key: &str) -> Option<u64> {
    let text = std::fs::read_to_string(path).ok()?;
    status_value_kb(&text, key)
}

/// Valore in KiB della chiave `key` nel testo di `/proc/<pid>/status`.
pub fn status_value_kb(text: &str, key: &str) -> Option<u64> {
    text.lines()
        .find_map(|l| l.strip_prefix(key))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|v| v.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emitted_line_round_trips() {
        let o = ChildOutcome::failed(Kind::DecompressionLimit, "per_stream");
        let line = serde_json::to_string(&Line {
            outcome: &o,
            vm_hwm_kb: Some(12),
        })
        .unwrap();
        assert_eq!(
            line,
            r#"{"status":"failed","kind":"DecompressionLimit","detail":"per_stream","vm_hwm_kb":12}"#
        );
        assert_eq!(parse_line(&line), Some((o, Some(12))));
    }

    #[test]
    fn status_value_is_read_in_kib() {
        let text = "Name:\tx\nVmPeak:\t  20480 kB\nVmHWM:\t    1024 kB\n";
        assert_eq!(status_value_kb(text, "VmHWM:"), Some(1024));
        assert_eq!(status_value_kb(text, "VmPeak:"), Some(20480));
        assert_eq!(status_value_kb(text, "VmRSS:"), None);
    }
}
