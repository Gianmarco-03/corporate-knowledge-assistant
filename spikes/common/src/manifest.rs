//! Manifest hardware (`hw.json`) che accompagna ogni misura.
use anyhow::Context;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct HwManifest {
    pub timestamp_utc: String,
    pub git_rev: Option<String>,
    pub cpu: Option<String>,
    pub ram_total: Option<String>,
    pub gpu: Option<String>,
    pub cuda_driver: Option<String>,
    pub wsl_version: Option<String>,
    pub docker_version: Option<String>,
    pub docker_desktop_resources: Option<String>,
    pub images: Option<BTreeMap<String, String>>,
    pub rustc: Option<String>,
    pub tools: Option<BTreeMap<String, String>>,
    pub power_plan: Option<String>,
}

/// Nome fisso del manifest hardware gemello di ogni CSV di misura.
const MANIFEST_NAME: &str = "hw.json";

/// Legge e valida `dir/hw.json`: errore se manca, non è JSON valido, non ha
/// `timestamp_utc` o lo ha vuoto. Gli errori nominano il percorso del file.
pub fn ensure_present(dir: &Path) -> anyhow::Result<HwManifest> {
    let path = dir.join(MANIFEST_NAME);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("manifest hardware {} assente o illeggibile", path.display()))?;
    let manifest: HwManifest = serde_json::from_str(&text)
        .with_context(|| format!("manifest hardware {} non valido", path.display()))?;
    if manifest.timestamp_utc.trim().is_empty() {
        anyhow::bail!("manifest hardware {}: timestamp_utc vuoto", path.display());
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_dir(test: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("cka_spike_{}_{}_{}", std::process::id(), test, n));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn manifest_ensure_present_rejects_malformed_json() {
        let dir = unique_dir("manifest_malformed");
        let file = dir.join("hw.json");
        std::fs::write(&file, "{not json").unwrap();
        let err = ensure_present(&dir).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("hw.json"),
            "il messaggio deve nominare il percorso: {msg}"
        );

        // JSON valido ma privo del campo obbligatorio timestamp_utc.
        std::fs::write(&file, r#"{"git_rev":"abc"}"#).unwrap();
        assert!(
            ensure_present(&dir).is_err(),
            "timestamp_utc e' obbligatorio"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
