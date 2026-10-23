//! Bin `ocr` (piano 01-13, spike S-02 OCR, TTQ-parse e prova ostile sul render).
//!
//! STUB del test-write (agente Claude, 2026-10-23): le firme sotto sono il contratto che i
//! test `#[cfg(test)]` esercitano; i corpi `todo!()` li scrive l'esecutore del Task 2.
//!
//! - `render --input <pdf> --dpi <dpi> --out <dir>` e `tesseract --image <png>`: i soli
//!   sottocomandi da figlio isolato, con la CLI esatta di `job::resolve` di 01-07;
//! - `map`, `score`, `hostile-verdict`, `ttq-merge`: bin normali su dati derivati.
mod geom;
mod score;
mod tsv;
mod ttq;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

/// Sottocomandi del bin. Solo `Render` e `Tesseract` girano come figli isolati.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Cmd {
    Render {
        input: PathBuf,
        dpi: u32,
        out: PathBuf,
    },
    Tesseract {
        image: PathBuf,
    },
    Map(Vec<OsString>),
    Score(Vec<OsString>),
    HostileVerdict(Vec<OsString>),
    TtqMerge(Vec<OsString>),
}

/// Argomenti senza il nome del programma. Forme diverse da quelle dell'allowlist → `Err`.
pub(crate) fn parse_cli(_args: Vec<OsString>) -> Result<Cmd, String> {
    todo!("01-13 Task 2: parse_cli del bin ocr")
}

/// argv di `tesseract` (senza il nome del programma) per un PNG a `dpi` effettivi:
/// `<png> stdout --tessdata-dir /opt/tessdata_best -l ita+eng --oem 1 --psm 3 --dpi <dpi>
/// -c preserve_interword_spaces=1 tsv`. Nessuna shell; `OMP_THREAD_LIMIT` arriva dal padre.
pub(crate) fn tesseract_args(_png: &std::path::Path, _dpi: u32) -> Vec<OsString> {
    todo!("01-13 Task 2: tesseract_args")
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match parse_cli(args) {
        Ok(_) => todo!("01-13 Task 2: dispatch dei sottocomandi"),
        Err(e) => {
            eprintln!("ocr: {e}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn os(v: &[&str]) -> Vec<OsString> {
        v.iter().map(OsString::from).collect()
    }

    // ---- TEST-8: CLI del figlio e argv di tesseract ----------------------------------------

    #[test]
    fn test8_child_cli_matches_allowlist_argv() {
        // Stessa forma che `job::resolve` di 01-07 costruisce per `ocr_render` e `ocr_tesseract`.
        assert_eq!(
            parse_cli(os(&[
                "render",
                "--input",
                "/data/a.pdf",
                "--dpi",
                "300",
                "--out",
                "/results/raw/ocr/j1"
            ])),
            Ok(Cmd::Render {
                input: "/data/a.pdf".into(),
                dpi: 300,
                out: "/results/raw/ocr/j1".into()
            })
        );
        assert_eq!(
            parse_cli(os(&["tesseract", "--image", "/results/raw/ocr/j1/p1.png"])),
            Ok(Cmd::Tesseract {
                image: "/results/raw/ocr/j1/p1.png".into()
            })
        );
    }

    #[test]
    fn test8_child_cli_rejects_other_forms() {
        for bad in [
            vec!["render", "--input", "/data/a.pdf", "--out", "/r"],
            vec![
                "render",
                "--dpi",
                "300",
                "--input",
                "/data/a.pdf",
                "--out",
                "/r",
            ],
            vec![
                "render",
                "--input",
                "/data/a.pdf",
                "--dpi",
                "abc",
                "--out",
                "/r",
            ],
            vec!["tesseract", "--image"],
            vec!["tesseract", "--image", "/a.png", "--lang", "eng"],
            vec!["shell"],
            vec![],
        ] {
            assert!(parse_cli(os(&bad)).is_err(), "accettato: {bad:?}");
        }
    }

    #[test]
    fn test8_tesseract_argv_is_tsv_ita_eng_best_with_effective_dpi() {
        let a: Vec<String> = tesseract_args(Path::new("/x/p1.png"), 300)
            .into_iter()
            .map(|s| s.into_string().unwrap())
            .collect();
        let want = [
            "/x/p1.png",
            "stdout",
            "--tessdata-dir",
            "/opt/tessdata_best",
            "-l",
            "ita+eng",
            "--oem",
            "1",
            "--psm",
            "3",
            "--dpi",
            "300",
            "-c",
            "preserve_interword_spaces=1",
            "tsv",
        ];
        assert_eq!(a, want);
        assert_eq!(
            tesseract_args(Path::new("/x/p.png"), 200)[11],
            OsString::from("200")
        );
    }

    #[test]
    fn test8_no_shell_and_no_omp_override_in_ocr_sources() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bin/ocr");
        for f in ["main.rs", "tsv.rs", "geom.rs", "score.rs", "ttq.rs"] {
            let src = std::fs::read_to_string(dir.join(f)).unwrap();
            let code: String = src.split("#[cfg(test)]").next().unwrap().to_owned();
            assert!(!code.contains(concat!("Command::new", "(\"sh\")")), "{f}");
            assert!(!code.contains(concat!("Command::new", "(\"bash\")")), "{f}");
            // OMP_THREAD_LIMIT lo mette solo il padre (job::resolve di 01-07).
            assert!(!code.contains(".env(\"OMP_THREAD_LIMIT\""), "{f}");
        }
    }

    // ---- TEST-1..3, TEST-7: immagine paddle (verifica statica, prima di ogni build) ---------

    fn docker_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../docker")
    }

    fn read(p: PathBuf) -> String {
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }

    /// Righe logiche del Dockerfile (continuazioni `\` unite).
    fn dockerfile_steps() -> Vec<String> {
        let text = read(docker_dir().join("paddle.Dockerfile"));
        text.replace("\\\r\n", " ")
            .replace("\\\n", " ")
            .lines()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn test1_pip_installs_only_the_hashed_lock() {
        let lock = read(docker_dir().join("paddle-requirements.lock"));
        assert_eq!(
            lock.lines()
                .filter(|l| l.starts_with("paddlepaddle==3.3.1"))
                .count(),
            1
        );
        assert_eq!(
            lock.lines()
                .filter(|l| l.starts_with("paddleocr==3.7.0"))
                .count(),
            1
        );
        assert!(lock.matches("--hash=sha256:").count() >= 2);
        let head = lock
            .lines()
            .find(|l| l.starts_with("# base:"))
            .expect("commento # base: nel lock");
        assert!(
            head.contains("pip-tools==7.4.1") && head.contains("pip==24.3.1"),
            "{head}"
        );
        let digest = head
            .split("python:3.12-slim@sha256:")
            .nth(1)
            .map(|r| {
                r.chars()
                    .take_while(|c| c.is_ascii_hexdigit())
                    .collect::<String>()
            })
            .expect("digest nel lock");
        assert_eq!(digest.len(), 64);

        let steps = dockerfile_steps();
        let from: Vec<&String> = steps.iter().filter(|l| l.starts_with("FROM ")).collect();
        assert_eq!(from.len(), 1);
        assert_eq!(
            from[0].trim(),
            format!("FROM python:3.12-slim@sha256:{digest}")
        );
        let installs: Vec<&String> = steps.iter().filter(|l| l.contains("pip install")).collect();
        assert!(!installs.is_empty());
        for l in installs {
            // Ogni pip install passa dal solo lock con --require-hashes: nessun nome fuori lock.
            assert!(
                l.contains("--require-hashes") && l.contains("-r /tmp/requirements.lock"),
                "{l}"
            );
            assert_eq!(l.matches("pip install").count(), 1, "{l}");
        }
        assert!(
            steps
                .iter()
                .any(|l| l.contains("COPY docker/paddle-requirements.lock /tmp/requirements.lock"))
        );
    }

    #[test]
    fn test2_build_selects_v6_explicitly_and_stops_without_fallback() {
        let text = dockerfile_steps().join("\n");
        assert!(text.contains("PP-OCRv6"));
        assert!(text.contains("v6.selection.json"));
        assert!(
            text.contains("selectable"),
            "selectable=false previsto in build"
        );
        assert!(text.contains("v6.resolved.json"));
        // La pipeline v6 riceve ocr_version e lang da v6.selection.json, mai dal default.
        assert!(text.contains("ocr_version") && text.contains("lang"));
        // v5 solo riferimento secondario, con file propri.
        if text.contains("PP-OCRv5") {
            assert!(text.contains("v5.resolved.json"));
        }
    }

    #[test]
    fn test3_build_asserts_v6_family_of_det_and_rec() {
        let text = dockerfile_steps().join("\n");
        assert!(text.contains("family_check"));
        assert!(text.contains("PP-OCRv6_"));
        assert!(text.contains("model_name"));
    }

    #[test]
    fn test7_image_preloads_weights_and_runs_offline() {
        let steps = dockerfile_steps();
        let text = steps.join("\n");
        assert!(text.contains("/opt/paddle-models"));
        assert!(text.contains("manifest.sha256"));
        assert!(text.contains("sha256sum"));
        assert!(text.contains("device=\"cpu\"") || text.contains("device='cpu'"));
        assert!(steps.iter().any(|l| l.starts_with("USER 10001")));
        assert_eq!(
            steps
                .iter()
                .filter(|l| l.trim() == "CMD [\"sleep\",\"infinity\"]")
                .count(),
            1
        );
        // Rete negata a runtime: servizio `paddle` del compose di 01-02 (sola lettura).
        let compose = read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../compose.yaml"));
        let svc = compose
            .split("\n  paddle:")
            .nth(1)
            .expect("servizio paddle");
        let svc = svc.split("\n\n").next().unwrap();
        assert!(svc.contains("network_mode: none") && svc.contains("docker/paddle.Dockerfile"));
    }

    // ---- TEST-10, TEST-13: script host (verifiche senza Docker) ------------------------------

    fn script(rel: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
    }

    #[test]
    fn test13_hostile_render_script_shape() {
        let s = read(script("ocr-run/run_hostile_render.sh"));
        assert!(s.contains("parse-parent run"));
        assert!(s.contains("manifest.json"));
        assert!(s.contains("sha256sum"));
        assert!(s.contains("/results/raw/ocr/"));
        assert!(s.contains("--assert-only"));
        assert!(s.contains("hostile-verdict"));
        assert!(
            !s.contains("docker run") && !s.contains("--profile") && !s.contains("compose down")
        );
    }

    #[test]
    fn test13_hostile_render_script_exits_2_without_run_id() {
        let out = std::process::Command::new("bash")
            .arg(script("ocr-run/run_hostile_render.sh"))
            .arg("3")
            .env_remove("CKA_RUN_ID")
            .output()
            .expect("bash");
        assert_eq!(out.status.code(), Some(2));
    }

    #[test]
    fn test10_ttq_script_shape_and_exits_2_without_run_id() {
        let p = script("ocr-run/run_ttq.sh");
        let s = read(p.clone());
        for k in [
            "session.sh",
            "parse-parent",
            "run_id",
            "/results/raw/ttq/",
            "ttq-merge",
            "TTQ_INJECT_DELAY_MS",
            "date +%s%N",
        ] {
            assert!(s.contains(k), "{k}");
        }
        assert!(
            !s.contains("ocr render") && !s.contains("ocr tesseract") && !s.contains("docker run")
        );
        let out = std::process::Command::new("bash")
            .arg(p)
            .args(["doc", "1"])
            .env_remove("CKA_RUN_ID")
            .output()
            .expect("bash");
        assert_eq!(out.status.code(), Some(2));
    }

    // ---- Evidenze del Task 3 (#[ignore]: si lanciano a misura fatta, sull'host) --------------

    fn repo(rel: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel)
    }

    fn csv_rows(rel: &str, header: &str) -> Vec<Vec<String>> {
        let text = read(script(rel));
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some(header), "{rel}");
        lines
            .filter(|l| !l.is_empty())
            .map(|l| l.split(',').map(str::to_owned).collect())
            .collect()
    }

    #[test]
    #[ignore = "evidenza Task 3: hostile-render.csv"]
    fn test13_evidence_hostile_render_csv() {
        let rows = csv_rows(
            "results/ocr/hostile-render.csv",
            "case_id,run,expected,outcome,kind,detail,peak_rss_kb,wall_ms,parent_alive,leftover_procs,oom_kill_delta,oom_adj_set,pass",
        );
        for case in ["jbig2-malformed", "deep-nesting"] {
            let r: Vec<_> = rows.iter().filter(|r| r[0] == case).collect();
            assert_eq!(r.len(), 3, "{case}: tre ripetizioni, nessuna scartata");
            for row in r {
                assert_eq!(row[2], "failed/Corrupt|Crashed");
                assert!(row[8] == "true" || row[8] == "false");
                assert!(row[9].parse::<u32>().is_ok());
            }
        }
        let status = score::t09_status(
            &rows
                .iter()
                .filter(|r| r[0] == "jbig2-malformed")
                .map(|r| {
                    (
                        r[12] == "true",
                        r[8] == "true",
                        r[9].parse::<u32>().unwrap(),
                    )
                })
                .collect::<Vec<_>>(),
        );
        let ris = read(script("RISULTATI.md"));
        assert!(
            ris.contains("JBIG2") && ris.contains(status.as_str()),
            "T-09 in RISULTATI.md = {}",
            status.as_str()
        );
    }

    #[test]
    #[ignore = "evidenza Task 3: ocr_throughput.csv"]
    fn test11_test15_evidence_throughput_csv() {
        let rows = csv_rows(
            "results/ocr/ocr_throughput.csv",
            "engine,version,pages,requested_concurrency,max_admitted,run,wall_s,s_per_page,failed_jobs,oom_kill_delta,job_errors,leftover_procs_max,reservation_kb,peak_rss_kb_max,reserve_peak_ratio_min,parent_exit,m10_pass",
        );
        let limits: serde_json::Value =
            serde_json::from_str(&read(script("results/ocr/limits-ocr_tesseract.json"))).unwrap();
        let want_kb = score::ocr_tesseract_reservation_bytes(&limits) / 1024;
        for n in ["1", "4", "8", "16"] {
            assert!(rows.iter().any(|r| r[3] == n && !r[4].is_empty()), "N={n}");
        }
        for r in &rows {
            assert!(r[16] == "true" || r[16] == "false");
            // TEST-15: la prenotazione del batch è quella dei limiti correnti (freschezza).
            assert_eq!(
                r[12].parse::<u64>().unwrap(),
                want_kb,
                "reservation_kb stantia: {r:?}"
            );
        }
    }

    #[test]
    #[ignore = "evidenza Task 3: ADR-0004 e RISULTATI.md (sull'host: docs/ non è montato nel runner)"]
    fn test12_evidence_adr_0004() {
        let adr = read(repo("docs/adr/0004-parser-ocr-isolamento.md"));
        for t in ["T-06", "T-07", "T-09", "T-10"] {
            assert!(adr.contains(t), "{t}");
        }
        assert!(adr.to_lowercase().contains("estrapolat"));
        assert!(adr.contains("Job Object"));
        assert!(adr.contains("RISULTATI-isolamento.md") || adr.contains("s02-parser/RISULTATI.md"));
        assert!(adr.contains("max_admitted"));
        assert!(adr.contains("2 GiB"));
        let ris = read(script("RISULTATI.md"));
        for f in [
            "RISULTATI-isolamento.md",
            "RISULTATI-estrazione.md",
            "RISULTATI-locator.md",
            "RISULTATI-ocr.md",
        ] {
            assert!(ris.contains(f), "{f}");
        }
        for s in [
            "Esito",
            "Impatto su porte (S-07)",
            "Impatto threat model",
            "Destinazione",
        ] {
            assert!(ris.contains(s), "{s}");
        }
        for h in [
            "results/isolation/hw.json",
            "results/extract/hw.json",
            "results/locator/hw.json",
            "results/ocr/hw.json",
        ] {
            assert!(adr.contains(h), "{h}");
            assert!(script(h).exists(), "{h}");
        }
    }

    #[test]
    #[ignore = "evidenza Task 3: pesi PaddleOCR e righe v6"]
    fn test4_test6_evidence_paddle_rows() {
        let rows = csv_rows(
            "results/ocr/ocr.csv",
            "engine,version,doc_id,page,scan_quality,run,chars_expected,cer,seconds",
        );
        let paddle: Vec<_> = rows.iter().filter(|r| r[0] != "tesseract").collect();
        for q in ["good", "degraded"] {
            assert!(rows.iter().any(|r| r[0] == "tesseract" && r[4] == q), "{q}");
        }
        if !paddle.is_empty() {
            let w = read(script("results/ocr/paddle-weights.sha256"));
            score::weights_manifest_check(&w, &w).expect("manifest dei pesi ben formato");
            assert!(paddle.iter().any(|r| r[1] == "PP-OCRv6"));
            assert!(
                paddle
                    .iter()
                    .all(|r| r[1] == "PP-OCRv6" || r[1] == "PP-OCRv5"),
                "versione implicita"
            );
        }
    }
}
