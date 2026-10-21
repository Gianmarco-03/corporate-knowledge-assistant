//! Esiti tipizzati del figlio (contratto W17, punto 2).
// stub del test-write: da implementare in 01-07

/// Classe di fallimento del figlio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Esito di un job, una riga JSON su stdout del figlio.
#[derive(Debug, Clone, PartialEq)]
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

/// Stampa l'esito come una sola riga JSON con `vm_hwm_kb`.
pub fn emit(_outcome: &ChildOutcome) -> anyhow::Result<()> {
    // stub del test-write: da implementare in 01-07
    todo!()
}
