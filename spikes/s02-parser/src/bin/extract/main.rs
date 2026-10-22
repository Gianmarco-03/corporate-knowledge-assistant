//! Bin `extract` (piano 01-08, spike S-02 estrazione).
//!
//! - `pdf --input <f> --out <dir>`: il solo sottocomando eseguito come figlio isolato, e solo
//!   tramite un job `op: extract_pdf` (argv costruito da `job::resolve` di 01-07). Stampa una
//!   riga con `protocol::emit`; argomenti diversi → exit 2 senza toccare alcun file.
//! - `score …` e `select-tables …`: bin normali nel runner, su JSON derivati e annotazioni,
//!   mai su PDF (vedi `score.rs` e `tables.rs`).
//! - `jobs …`: genera il JSONL dei job `extract_pdf` (un job per file, `job_id` unico per run).
mod pdf;
mod score;
mod tables;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use s02_parser::prescan::Caps;
use s02_parser::protocol::emit;

/// Sottocomandi del bin. Solo `Pdf` gira come figlio isolato (argv costruito da
/// `job::resolve` di 01-07: `pdf --input <input> --out <out_dir>`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Cmd {
    Pdf { input: PathBuf, out: PathBuf },
    Score(Vec<OsString>),
    SelectTables(Vec<OsString>),
    Jobs(Vec<OsString>),
}

/// Argomenti senza il nome del programma. Argomenti non riconosciuti → `Err`.
pub(crate) fn parse_cli(args: Vec<OsString>) -> Result<Cmd, String> {
    let Some((sub, rest)) = args.split_first() else {
        return Err("sottocomando assente: pdf | score | select-tables | jobs".into());
    };
    match sub.to_str() {
        Some("pdf") => match rest {
            [a, input, b, out] if a == "--input" && b == "--out" => Ok(Cmd::Pdf {
                input: PathBuf::from(input),
                out: PathBuf::from(out),
            }),
            _ => Err("pdf: attesi esattamente --input <file> --out <dir>".into()),
        },
        Some("score") => Ok(Cmd::Score(rest.to_vec())),
        Some("select-tables") => Ok(Cmd::SelectTables(rest.to_vec())),
        Some("jobs") => Ok(Cmd::Jobs(rest.to_vec())),
        _ => Err(format!("sottocomando sconosciuto: {sub:?}")),
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let cmd = match parse_cli(args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("extract: {e}");
            return ExitCode::from(2);
        }
    };
    let result = match cmd {
        Cmd::Pdf { input, out } => {
            let dir = std::env::var_os("PDFIUM_DIR")
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "/opt/pdfium/lib".into());
            let outcome = pdf::run_pdf(&input, &out, &Caps::default(), Path::new(&dir));
            return match emit(&outcome) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::from(1),
            };
        }
        Cmd::Score(a) => score::run(a),
        Cmd::SelectTables(a) => tables::run(a),
        Cmd::Jobs(a) => jobs(a),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("extract: {e:#}");
            ExitCode::from(1)
        }
    }
}

#[derive(clap::Parser, Debug)]
#[command(name = "extract jobs")]
struct JobsCli {
    /// Manifest di `spikes/data/s02` (voci `role=pdf`).
    #[arg(long)]
    manifest: PathBuf,
    /// Documenti del corpus comune (`/data/corpus/raw/<doc_id>.pdf`), ripetibile.
    #[arg(long)]
    corpus_doc: Vec<String>,
    /// Altri input già sotto `/data` o `/results/raw` (percorsi del worker), ripetibile.
    #[arg(long)]
    input: Vec<String>,
    /// Identificatore del run, suffisso dei `job_id` (`[a-z0-9-]`).
    #[arg(long)]
    run_id: String,
}

/// `job_id` dal nome: minuscole, `[a-z0-9-]`, al più 64 caratteri con il suffisso del run.
fn job_id(name: &str, run_id: &str) -> String {
    let mut base: String = name
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let max = 64usize.saturating_sub(run_id.len() + 1);
    base.truncate(max);
    let base = base.trim_matches('-');
    format!("{base}-{run_id}")
}

fn jobs(args: Vec<OsString>) -> anyhow::Result<()> {
    use clap::Parser as _;
    let cli = JobsCli::try_parse_from(std::iter::once(OsString::from("extract-jobs")).chain(args))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    anyhow::ensure!(
        !cli.run_id.is_empty()
            && cli
                .run_id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
        "--run-id fuori da [a-z0-9-]"
    );
    let mut inputs: Vec<(String, String)> = Vec::new();
    for m in score::load_manifest(&cli.manifest)?
        .into_iter()
        .filter(|m| m.role == "pdf")
    {
        inputs.push((m.item_id.clone(), format!("/data/s02/raw/{}", m.file)));
    }
    for d in &cli.corpus_doc {
        inputs.push((d.clone(), format!("/data/corpus/raw/{d}.pdf")));
    }
    for i in &cli.input {
        let name = Path::new(i)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        inputs.push((name, i.clone()));
    }
    let mut seen = std::collections::BTreeSet::new();
    for (name, input) in inputs {
        let id = job_id(&name, &cli.run_id);
        anyhow::ensure!(seen.insert(id.clone()), "job_id duplicato: {id}");
        println!(
            "{}",
            serde_json::json!({
                "job_id": id,
                "op": "extract_pdf",
                "input": input,
                "out_dir": format!("/results/raw/extract/{id}"),
                "params": {},
            })
        );
    }
    Ok(())
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
