//! Writer CSV a header fisso: rifiuta di creare il file senza `hw.json` gemello.
use std::path::Path;

pub struct Writer {
    _private: (),
}

impl Writer {
    pub fn create(_path: &Path, _header: &[&str]) -> anyhow::Result<Writer> {
        todo!()
    }

    pub fn write_row(&mut self, _fields: &[&str]) -> anyhow::Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_dir(test: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("cka_spike_{}_{}_{}", std::process::id(), test, n));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_valid_hw(dir: &Path) {
        std::fs::write(dir.join("hw.json"), r#"{"timestamp_utc":"2026-10-19T00:00:00Z"}"#).unwrap();
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
