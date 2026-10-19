//! Writer CSV a header fisso: rifiuta di creare il file senza `hw.json` gemello.
//!
//! Il controllo del manifest avviene prima di toccare il filesystem: se
//! `hw.json` manca o non è valido non resta alcun CSV parziale. Ogni riga ha
//! esattamente i campi dell'header; una riga di arità diversa viene rifiutata
//! senza scrivere nulla.
use anyhow::{Context, bail};
use std::fs::File;
use std::path::{Path, PathBuf};

pub struct Writer {
    inner: ::csv::Writer<File>,
    columns: usize,
    path: PathBuf,
}

impl Writer {
    /// Crea (o tronca) `path` e scrive l'header, dopo aver verificato il
    /// manifest `hw.json` nella stessa cartella.
    pub fn create(path: &Path, header: &[&str]) -> anyhow::Result<Writer> {
        if header.is_empty() {
            bail!("header vuoto per il CSV {}", path.display());
        }
        let dir = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => Path::new("."),
        };
        crate::manifest::ensure_present(dir).with_context(|| {
            format!(
                "CSV {} rifiutato: manca un manifest hardware valido accanto",
                path.display()
            )
        })?;
        let inner = ::csv::Writer::from_path(path)
            .with_context(|| format!("impossibile creare il CSV {}", path.display()))?;
        let mut writer = Writer {
            inner,
            columns: header.len(),
            path: path.to_path_buf(),
        };
        writer.write_record(header)?;
        Ok(writer)
    }

    /// Scrive una riga; errore se il numero di campi differisce dall'header.
    pub fn write_row(&mut self, fields: &[&str]) -> anyhow::Result<()> {
        if fields.len() != self.columns {
            bail!(
                "CSV {}: riga con {} campi, l'header ne ha {}",
                self.path.display(),
                fields.len(),
                self.columns
            );
        }
        self.write_record(fields)
    }

    /// Scrive e svuota il buffer: una misura interrotta lascia su disco le
    /// righe già completate.
    fn write_record(&mut self, fields: &[&str]) -> anyhow::Result<()> {
        self.inner
            .write_record(fields)
            .with_context(|| format!("scrittura nel CSV {} fallita", self.path.display()))?;
        self.inner
            .flush()
            .with_context(|| format!("flush del CSV {} fallito", self.path.display()))
    }
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

    fn write_valid_hw(dir: &Path) {
        std::fs::write(
            dir.join("hw.json"),
            r#"{"timestamp_utc":"2026-10-19T00:00:00Z"}"#,
        )
        .unwrap();
    }

    #[test]
    fn csv_create_without_hw_json_fails() {
        let dir = unique_dir("csv_create_without_hw_json_fails");
        let path = dir.join("x.csv");
        let res = Writer::create(&path, &["a", "b"]);
        assert!(res.is_err(), "senza hw.json la creazione deve fallire");
        assert!(!path.exists(), "nessun file parziale dopo il fallimento");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn csv_create_with_hw_json_writes_exact_header() {
        let dir = unique_dir("csv_create_with_hw_json_writes_exact_header");
        write_valid_hw(&dir);
        let path = dir.join("x.csv");
        {
            let mut w = Writer::create(&path, &["a", "b"]).unwrap();
            w.write_row(&["1", "2"]).unwrap();
        }
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().next(), Some("a,b"));
        assert_eq!(content.lines().nth(1), Some("1,2"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn csv_write_row_wrong_arity_fails() {
        let dir = unique_dir("csv_write_row_wrong_arity_fails");
        write_valid_hw(&dir);
        let path = dir.join("x.csv");
        {
            let mut w = Writer::create(&path, &["a", "b"]).unwrap();
            assert!(w.write_row(&["1"]).is_err());
            assert!(w.write_row(&["1", "2", "3"]).is_err());
        }
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 1, "solo l'header: {content}");
        assert!(!content.contains("1,2,3"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
