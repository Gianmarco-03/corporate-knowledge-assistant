//! Bin `extract` (piano 01-08, spike S-02 estrazione).
//!
//! STUB di `test-write`: firme minime perché i test rossi compilino. L'implementazione è
//! dell'esecutore (sottocomando `pdf` figlio di `extract_pdf`, `score`, `select-tables`).
#![allow(dead_code)] // stub di test-write: l'esecutore lo rimuove con l'implementazione

mod pdf;
mod score;
mod tables;

use std::ffi::OsString;
use std::path::PathBuf;

/// Sottocomandi del bin. Solo `Pdf` gira come figlio isolato (argv costruito da
/// `job::resolve` di 01-07: `pdf --input <input> --out <out_dir>`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Cmd {
    Pdf { input: PathBuf, out: PathBuf },
    Score,
    SelectTables,
}

/// Argomenti senza il nome del programma. Argomenti non riconosciuti → `Err`.
pub(crate) fn parse_cli(_args: Vec<OsString>) -> Result<Cmd, String> {
    todo!("01-08: CLI di extract")
}

fn main() {
    todo!("01-08: bin extract")
}

#[cfg(test)]
mod tests {
    //! TEST-1 (parte CLI): il ramo figlio accetta solo l'argv di `job::resolve`.
    use super::*;

    fn args(v: &[&str]) -> Vec<OsString> {
        v.iter().map(OsString::from).collect()
    }

    #[test]
    fn pdf_accepts_exactly_the_resolve_argv() {
        let c = parse_cli(args(&[
            "pdf",
            "--input",
            "/data/s02/raw/a.pdf",
            "--out",
            "/results/raw/extract/a-run1",
        ]))
        .expect("argv di job::resolve accettato");
        assert_eq!(
            c,
            Cmd::Pdf {
                input: PathBuf::from("/data/s02/raw/a.pdf"),
                out: PathBuf::from("/results/raw/extract/a-run1"),
            }
        );
    }

    #[test]
    fn pdf_rejects_extra_unknown_and_missing_args() {
        for bad in [
            vec!["pdf", "--input", "/a.pdf", "--out", "/o", "--dpi", "300"],
            vec!["pdf", "--input", "/a.pdf"],
            vec!["pdf", "--out", "/o"],
            vec!["pdf", "--input", "/a.pdf", "--out", "/o", "extra"],
            vec!["--input", "/a.pdf", "--out", "/o"],
            vec!["shell", "--input", "/a.pdf", "--out", "/o"],
            vec![],
        ] {
            assert!(parse_cli(args(&bad)).is_err(), "accettato {bad:?}");
        }
    }
}
