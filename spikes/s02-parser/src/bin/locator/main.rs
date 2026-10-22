//! Bin `locator` (piano 01-09, spike S-02 / A-10 locator di citazione).
//!
//! STUB di `test-write`: tipi e firme minime perché i test rossi compilino; ogni funzione è
//! `todo!()`. L'implementazione è dell'esecutore. Sottocomandi: `locate` (solo come figlio,
//! job `op: locate` del contratto di 01-07), `roundtrip prepare|check`, `rects`, `offsets`
//! (bin normali che non aprono DOCX né MD).
#![allow(dead_code)] // stub di test-write: l'esecutore lo rimuove con l'implementazione

mod canon;
mod docx;
mod md;
mod pdfrect;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use s02_parser::protocol::ChildOutcome;
use serde::Deserialize;

use crate::canon::SectionStep;

/// Tetto di lettura di un Markdown in `locate`.
pub(crate) const MAX_MD_BYTES: u64 = 8 * 1024 * 1024;
/// Tetto di lettura del file excerpt in `locate`.
pub(crate) const MAX_EXCERPT_BYTES: u64 = 64 * 1024;

/// Sottocomandi. Solo `Locate` gira come figlio isolato (argv di `job::resolve`:
/// `locate --input <input> --excerpt-file <excerpt_file>`), senza alcun percorso di output.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Cmd {
    Locate {
        input: PathBuf,
        excerpt_file: PathBuf,
    },
    RoundtripPrepare {
        samples: PathBuf,
        excerpt_dir: PathBuf,
        jobs_out: PathBuf,
        run_tag: String,
    },
    RoundtripCheck {
        samples: PathBuf,
        reports: PathBuf,
        out_dir: PathBuf,
    },
    Rects {
        cases: PathBuf,
        pdf_dir: PathBuf,
        out: PathBuf,
    },
    Offsets {
        vectors: PathBuf,
        out: PathBuf,
    },
}

/// Argomenti senza il nome del programma. Argomenti non riconosciuti → `Err`.
pub(crate) fn parse_cli(_args: Vec<OsString>) -> Result<Cmd, String> {
    todo!("01-09: CLI di locator")
}

/// Sottocomando `locate` (figlio): esito da stampare con `protocol::emit`. `ok` con
/// `data = {projection, format, doc_sha256, matches: [{section_path, char_range,
/// section_text_sha256, reextracted}], unsupported}`; mai il testo completo del documento.
pub(crate) fn run_locate(_input: &Path, _excerpt_file: &Path) -> ChildOutcome {
    todo!("01-09: locate")
}

/// Campione scritto a mano in `locator-gen/expected_samples.json`.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Sample {
    pub(crate) sample_id: String,
    pub(crate) doc_id: String,
    pub(crate) formats: Vec<String>,
    pub(crate) excerpt: String,
    pub(crate) expected_section_path: Vec<SectionStep>,
}

/// Esito di `roundtrip prepare`: righe JSONL dei job e file excerpt `(percorso, contenuto)`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Prepared {
    pub(crate) jobs: Vec<String>,
    pub(crate) excerpts: Vec<(String, String)>,
}

/// Un job `op: locate` per campione e formato (`job_id` = `<sample_id>-<format>-<run_tag>`,
/// `out_dir: null`, input `/data/corpus/synthetic/<doc_id>.<format>`), più il job negativo
/// `neg-path-<run_tag>` con `excerpt_file` fuori dalle radici ammesse. `run_tag` fuori da
/// `[a-z0-9]{8}`, `job_id` non valido o duplicato → `Err` (exit 2).
pub(crate) fn prepare_jobs(
    _samples: &[Sample],
    _excerpt_dir: &str,
    _run_tag: &str,
) -> Result<Prepared, String> {
    todo!("01-09: roundtrip prepare")
}

/// Lega ogni riga di `reports.jsonl` al proprio job per `job_id`, uno a uno: mancante,
/// duplicato o sconosciuto → `Err`.
pub(crate) fn bind_reports(
    _job_ids: &[String],
    _lines: &[String],
) -> Result<BTreeMap<String, serde_json::Value>, String> {
    todo!("01-09: roundtrip check, legame dei RunReport")
}

/// Sottocomando `offsets`: dalla fixture legge solo `case_id`, `text`, `scalar_range` e
/// produce per caso soltanto `{case_id, got_utf16_range, got_text}`.
pub(crate) fn offsets_rows(_fixture_json: &str) -> Result<Vec<serde_json::Value>, String> {
    todo!("01-09: offsets")
}

fn main() {
    todo!("01-09: bin locator")
}

#[cfg(test)]
mod tests {
    //! TEST-6 (fixture e bin che non produce valori attesi), TEST-7 (payload senza testo
    //! completo, DOCX ostili), TEST-8 (job `locate` con `out_dir: null`, radici ammesse,
    //! `job_id` unici, CLI del figlio senza output), TEST-1 (locate end-to-end sul figlio).
    use super::*;
    use crate::docx::tests::{EXTERNAL_RELS, scratch_dir, twin_document_xml, write_docx};
    use s02_parser::protocol::Kind;
    use std::collections::BTreeSet;

    const SAMPLES: &str = include_str!("../../../locator-gen/expected_samples.json");
    const OFFSET_FIXTURE: &str = include_str!("../../../locator-gen/offset_vectors.expected.json");
    const ALLOWED_ROOTS: [&str; 3] = ["/data/", "/results/raw/", "/work/"];
    const RUN_TAG: &str = "a1b2c3d4";

    fn args(v: &[&str]) -> Vec<OsString> {
        v.iter().map(OsString::from).collect()
    }

    fn samples() -> Vec<Sample> {
        serde_json::from_str(SAMPLES).expect("expected_samples.json")
    }

    // ---- CLI (TEST-8) ------------------------------------------------------------------

    /// Il ramo figlio accetta esattamente l'argv di `job::resolve`.
    #[test]
    fn locate_accepts_exactly_the_resolve_argv() {
        let c = parse_cli(args(&[
            "locate",
            "--input",
            "/data/corpus/synthetic/syn-01-budget-interno.md",
            "--excerpt-file",
            "/results/raw/locator/excerpts/s01 scenario \u{e8}.txt",
        ]));
        assert_eq!(
            c,
            Ok(Cmd::Locate {
                input: "/data/corpus/synthetic/syn-01-budget-interno.md".into(),
                excerpt_file: "/results/raw/locator/excerpts/s01 scenario \u{e8}.txt".into(),
            })
        );
    }

    /// TEST-8: `locate` non ha alcun percorso di output; argomenti in più o mancanti → Err.
    #[test]
    fn locate_rejects_output_flags_and_extra_args() {
        let base = [
            "locate",
            "--input",
            "/data/a.md",
            "--excerpt-file",
            "/work/j/e.txt",
        ];
        for extra in [
            &["--out", "/results/raw/locator/x"][..],
            &["--out-dir", "/tmp"][..],
            &["altro"][..],
        ] {
            let mut v: Vec<&str> = base.to_vec();
            v.extend_from_slice(extra);
            assert!(parse_cli(args(&v)).is_err(), "accettato {v:?}");
        }
        assert!(parse_cli(args(&["locate", "--input", "/data/a.md"])).is_err());
        assert!(parse_cli(args(&["sconosciuto"])).is_err());
        assert!(parse_cli(args(&[])).is_err());
    }

    /// Sottocomandi normali del piano.
    #[test]
    fn normal_subcommands_parse() {
        assert_eq!(
            parse_cli(args(&[
                "roundtrip",
                "prepare",
                "--samples",
                "s.json",
                "--excerpt-dir",
                "ex",
                "--jobs-out",
                "j.jsonl",
                "--run-tag",
                RUN_TAG,
            ])),
            Ok(Cmd::RoundtripPrepare {
                samples: "s.json".into(),
                excerpt_dir: "ex".into(),
                jobs_out: "j.jsonl".into(),
                run_tag: RUN_TAG.into(),
            })
        );
        assert_eq!(
            parse_cli(args(&[
                "roundtrip",
                "check",
                "--samples",
                "s.json",
                "--reports",
                "r.jsonl",
                "--out-dir",
                "o",
            ])),
            Ok(Cmd::RoundtripCheck {
                samples: "s.json".into(),
                reports: "r.jsonl".into(),
                out_dir: "o".into(),
            })
        );
        assert_eq!(
            parse_cli(args(&[
                "rects",
                "--cases",
                "c.json",
                "--pdf-dir",
                "p",
                "--out",
                "r.csv"
            ])),
            Ok(Cmd::Rects {
                cases: "c.json".into(),
                pdf_dir: "p".into(),
                out: "r.csv".into(),
            })
        );
        assert_eq!(
            parse_cli(args(&["offsets", "--vectors", "v.json", "--out", "o.json"])),
            Ok(Cmd::Offsets {
                vectors: "v.json".into(),
                out: "o.json".into(),
            })
        );
    }

    // ---- fixture dei campioni (TEST-6) -------------------------------------------------

    /// TEST-6: i campioni attesi sono quelli a mano e coprono i casi del Task 2.
    #[test]
    fn expected_samples_cover_the_plan_cases() {
        let raw: serde_json::Value = serde_json::from_str(SAMPLES).unwrap();
        let s = samples();
        assert!(s.len() >= 10, "almeno 10 campioni");
        let ids: BTreeSet<&str> = s.iter().map(|x| x.sample_id.as_str()).collect();
        assert_eq!(ids.len(), s.len(), "sample_id duplicati");
        for (x, r) in s.iter().zip(raw.as_array().unwrap()) {
            assert_eq!(r["class"], "estratto-sorgente", "{}", x.sample_id);
            assert!(s02_parser::job::valid_job_id(&x.sample_id));
            assert!(!x.expected_section_path.is_empty(), "{}", x.sample_id);
            assert!(x.expected_section_path.iter().all(|p| p.ordinal >= 1));
            assert!(!x.excerpt.contains('\''), "{}: apostrofo", x.sample_id);
            assert!(x.formats.iter().all(|f| f == "md" || f == "docx"));
            assert!(x.formats.contains(&"md".to_owned()));
        }
        let with_docx = s.iter().filter(|x| x.formats.iter().any(|f| f == "docx"));
        assert!(with_docx.count() >= 8);
        assert!(
            s.iter()
                .filter(|x| x.expected_section_path.len() >= 3)
                .count()
                >= 3
        );
        assert!(
            s.iter().any(|x| x.excerpt.contains('\t')),
            "campione a cavallo di celle"
        );
        assert!(
            s.iter().filter(|x| !x.excerpt.is_ascii()).count() >= 2,
            "accentati"
        );
    }

    // ---- roundtrip prepare (TEST-8) ----------------------------------------------------

    fn job_fields(line: &str) -> serde_json::Value {
        serde_json::from_str(line).expect("riga JSONL")
    }

    /// TEST-8: ogni job è `op: locate`, `out_dir: null`, `job_id` unico e valido che finisce
    /// col `run_tag`, percorsi sotto le radici ammesse; il contratto di 01-07 lo accetta.
    #[test]
    fn prepared_jobs_are_locate_with_null_out_dir_and_unique_ids() {
        let s = samples();
        let p = prepare_jobs(&s, "/results/raw/locator/excerpts", RUN_TAG).expect("prepare");
        let expected_jobs: usize = s.iter().map(|x| x.formats.len()).sum::<usize>() + 1;
        assert_eq!(p.jobs.len(), expected_jobs);
        let mut ids = BTreeSet::new();
        let excerpt_paths: BTreeSet<&str> = p.excerpts.iter().map(|(f, _)| f.as_str()).collect();
        for line in &p.jobs {
            let j = job_fields(line);
            let id = j["job_id"].as_str().unwrap().to_owned();
            assert!(ids.insert(id.clone()), "job_id duplicato {id}");
            assert!(s02_parser::job::valid_job_id(&id), "{id}");
            assert!(id.ends_with(&format!("-{RUN_TAG}")), "{id}");
            assert_eq!(j["op"], "locate", "{id}");
            assert!(j["out_dir"].is_null(), "{id}: out_dir non nullo");
            let job = s02_parser::job::parse(line.as_bytes()).expect("job valido per 01-07");
            assert_eq!(job.op(), "locate");
            if id.starts_with("neg-path-") {
                continue;
            }
            let input = j["input"].as_str().unwrap();
            let ex = j["params"]["excerpt_file"].as_str().unwrap();
            assert!(
                ALLOWED_ROOTS.iter().any(|r| input.starts_with(r)),
                "{input}"
            );
            assert!(ALLOWED_ROOTS.iter().any(|r| ex.starts_with(r)), "{ex}");
            assert!(excerpt_paths.contains(ex), "{ex} non scritto da prepare");
        }
        for x in &s {
            for f in &x.formats {
                let id = format!("{}-{}-{RUN_TAG}", x.sample_id, f);
                let line = p
                    .jobs
                    .iter()
                    .find(|l| job_fields(l)["job_id"] == id.as_str())
                    .unwrap_or_else(|| panic!("job {id} assente"));
                let j = job_fields(line);
                assert_eq!(
                    j["input"],
                    format!("/data/corpus/synthetic/{}.{}", x.doc_id, f).as_str()
                );
                let ex = j["params"]["excerpt_file"].as_str().unwrap();
                let content = p.excerpts.iter().find(|(f, _)| f == ex).map(|(_, c)| c);
                assert_eq!(content, Some(&x.excerpt), "excerpt di {id}");
            }
        }
    }

    /// TEST-8: il job negativo ha `excerpt_file` fuori dalle radici ammesse (atteso
    /// `job_error`/`PathNotAllowed` dal padre) e anch'esso `out_dir: null`.
    #[test]
    fn negative_path_job_points_outside_allowed_roots() {
        let p = prepare_jobs(&samples(), "/results/raw/locator/excerpts", RUN_TAG).unwrap();
        let neg: Vec<serde_json::Value> = p
            .jobs
            .iter()
            .map(|l| job_fields(l))
            .filter(|j| j["job_id"] == format!("neg-path-{RUN_TAG}").as_str())
            .collect();
        assert_eq!(neg.len(), 1);
        assert!(neg[0]["out_dir"].is_null());
        let ex = neg[0]["params"]["excerpt_file"].as_str().unwrap();
        assert!(ex.starts_with('/'), "percorso assoluto: {ex}");
        assert!(!ALLOWED_ROOTS.iter().any(|r| ex.starts_with(r)), "{ex}");
    }

    /// Almeno un file excerpt ha nel nome spazi e caratteri non ASCII; tutti stanno in
    /// `excerpt_dir`.
    #[test]
    fn excerpt_files_live_in_excerpt_dir_with_one_unusual_name() {
        let dir = "/results/raw/locator/excerpts";
        let p = prepare_jobs(&samples(), dir, RUN_TAG).unwrap();
        assert!(
            p.excerpts
                .iter()
                .all(|(f, _)| f.starts_with(&format!("{dir}/")))
        );
        assert!(
            p.excerpts.iter().any(|(f, _)| {
                let name = &f[dir.len() + 1..];
                name.contains(' ') && !name.is_ascii()
            }),
            "nessun nome con spazi e non ASCII"
        );
    }

    /// TEST-8: `run_tag` fuori da `[a-z0-9]{8}` o `job_id` duplicato → Err.
    #[test]
    fn prepare_rejects_bad_run_tag_and_duplicate_ids() {
        let s = samples();
        for bad in [
            "",
            "a1b2c3d",
            "a1b2c3d4e",
            "A1B2C3D4",
            "a1b2-3d4",
            "a1b2c3d/",
        ] {
            assert!(
                prepare_jobs(&s, "/results/raw/locator/excerpts", bad).is_err(),
                "run_tag {bad:?} accettato"
            );
        }
        let mut dup = s.clone();
        dup.push(s[0].clone());
        assert!(prepare_jobs(&dup, "/results/raw/locator/excerpts", RUN_TAG).is_err());
        let mut bad_id = s.clone();
        bad_id[0].sample_id = "S01 Maiuscolo".to_owned();
        assert!(prepare_jobs(&bad_id, "/results/raw/locator/excerpts", RUN_TAG).is_err());
    }

    /// Regressione del contratto 01-07 (già verde): un job `locate` con `out_dir` non nullo
    /// è rifiutato da `job::parse`.
    #[test]
    fn locate_job_with_out_dir_is_rejected_by_the_contract() {
        let line = serde_json::json!({
            "job_id": "x-md-a1b2c3d4",
            "op": "locate",
            "input": "/data/corpus/synthetic/syn-01-budget-interno.md",
            "out_dir": "/results/raw/locator/x-md-a1b2c3d4",
            "params": {"excerpt_file": "/results/raw/locator/excerpts/x.txt"},
        })
        .to_string();
        assert!(s02_parser::job::parse(line.as_bytes()).is_err());
    }

    // ---- roundtrip check: legame dei RunReport -----------------------------------------

    fn report(id: &str) -> String {
        serde_json::json!({"job_id": id, "outcome": {"status": "ok", "pages": null, "output": null, "data": {}}})
            .to_string()
    }

    #[test]
    fn reports_bind_one_to_one_by_job_id() {
        let ids = vec!["a-md-x".to_owned(), "a-docx-x".to_owned()];
        let ok = bind_reports(&ids, &[report("a-docx-x"), report("a-md-x")]).expect("legame");
        assert_eq!(ok.keys().cloned().collect::<Vec<_>>(), {
            let mut v = ids.clone();
            v.sort();
            v
        });
        assert!(bind_reports(&ids, &[report("a-md-x")]).is_err(), "mancante");
        assert!(
            bind_reports(
                &ids,
                &[report("a-md-x"), report("a-md-x"), report("a-docx-x")]
            )
            .is_err(),
            "duplicato"
        );
        assert!(
            bind_reports(
                &ids,
                &[report("a-md-x"), report("a-docx-x"), report("altro")]
            )
            .is_err(),
            "sconosciuto"
        );
    }

    // ---- offsets: il bin non produce valori attesi (TEST-6) ----------------------------

    #[test]
    fn offsets_rows_carry_only_got_fields_matching_the_fixture() {
        let rows = offsets_rows(OFFSET_FIXTURE).expect("offsets");
        let fx: serde_json::Value = serde_json::from_str(OFFSET_FIXTURE).unwrap();
        let vs = fx["vectors"].as_array().unwrap();
        assert_eq!(rows.len(), vs.len());
        for r in &rows {
            let keys: BTreeSet<&str> = r.as_object().unwrap().keys().map(String::as_str).collect();
            assert_eq!(
                keys,
                BTreeSet::from(["case_id", "got_utf16_range", "got_text"]),
                "{r}"
            );
            let v = vs.iter().find(|v| v["case_id"] == r["case_id"]).unwrap();
            assert_eq!(r["got_utf16_range"], v["utf16_range"], "{}", r["case_id"]);
            assert_eq!(r["got_text"], v["expected"], "{}", r["case_id"]);
        }
    }

    /// Il bin ignora `utf16_range` ed `expected` della fixture: cambiarli non cambia l'uscita.
    #[test]
    fn offsets_ignore_expected_fields_of_the_input() {
        let mut fx: serde_json::Value = serde_json::from_str(OFFSET_FIXTURE).unwrap();
        for v in fx["vectors"].as_array_mut().unwrap() {
            v["utf16_range"] = serde_json::json!([0, 0]);
            v["expected"] = serde_json::json!("FALSO");
        }
        assert_eq!(
            offsets_rows(&fx.to_string()).unwrap(),
            offsets_rows(OFFSET_FIXTURE).unwrap()
        );
    }

    // ---- locate nel figlio (TEST-1, TEST-7) --------------------------------------------

    const LOCATE_MD: &str =
        "# Titolo\n\nRiga segreta che non deve uscire.\n\nIl margine sale al 15%.\n";
    /// `sha256sum` dei byte di `LOCATE_MD`.
    const LOCATE_MD_SHA: &str = "af6d3932ae21e9626fe82e9926070d59ac4ce3260ab9854f588377a8d5fa376c";

    fn listing(dir: &Path) -> BTreeSet<PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect()
    }

    fn ok_data(o: ChildOutcome) -> serde_json::Value {
        match o {
            ChildOutcome::Ok { data: Some(d), .. } => d,
            other => panic!("atteso ok con data, ottenuto {other:?}"),
        }
    }

    /// TEST-1 e T-01-09-05: locate su MD dà il payload del piano, hash dei byte, locator
    /// riestratto; il payload non contiene il resto del documento; nessun file scritto.
    #[test]
    fn locate_markdown_payload_is_minimal_and_exact() {
        let dir = scratch_dir();
        let input = dir.join("doc.md");
        let excerpt = dir.join("excerpt con spazi \u{e8}.txt");
        std::fs::write(&input, LOCATE_MD).unwrap();
        std::fs::write(&excerpt, "margine sale al 15%").unwrap();
        let before = listing(&dir);
        let out = run_locate(&input, &excerpt);
        assert_eq!(listing(&dir), before, "locate ha scritto file");
        let line = serde_json::to_string(&out).unwrap();
        assert!(
            !line.contains("Riga segreta"),
            "testo completo nel payload: {line}"
        );
        let d = ok_data(out);
        let keys: BTreeSet<&str> = d.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            BTreeSet::from([
                "projection",
                "format",
                "doc_sha256",
                "matches",
                "unsupported"
            ])
        );
        assert_eq!(d["projection"], "cka-text-v1");
        assert_eq!(d["format"], "md");
        assert_eq!(d["doc_sha256"], LOCATE_MD_SHA);
        let m = d["matches"].as_array().unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(
            m[0]["section_path"],
            serde_json::json!([{"title": "Titolo", "ordinal": 1}])
        );
        // «Riga segreta che non deve uscire.» = 33 scalar, `\n` = 33, «Il » = 34..37.
        assert_eq!(
            m[0]["char_range"],
            serde_json::json!({"start": 37, "end": 56})
        );
        assert_eq!(m[0]["reextracted"], "margine sale al 15%");
        assert!(
            m[0]["section_text_sha256"]
                .as_str()
                .is_some_and(|h| h.len() == 64)
        );
    }

    /// TEST-1: locate sul DOCX gemello trova lo stesso locator del test (3).
    #[test]
    fn locate_docx_twin_finds_the_same_locator() {
        let dir = scratch_dir();
        let input = write_docx(&dir, "twin.docx", &twin_document_xml(), None);
        let excerpt = dir.join("e.txt");
        std::fs::write(&excerpt, "cresciuto del 5%.\nVoce\t2025").unwrap();
        let d = ok_data(run_locate(&input, &excerpt));
        assert_eq!(d["format"], "docx");
        let m = d["matches"].as_array().unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(
            m[0]["section_path"],
            serde_json::json!([
                {"title": "Relazione", "ordinal": 1},
                {"title": "Risultati chiave del periodo", "ordinal": 1}
            ])
        );
        assert_eq!(
            m[0]["char_range"],
            serde_json::json!({"start": 15, "end": 42})
        );
        assert_eq!(m[0]["reextracted"], "cresciuto del 5%.\nVoce\t2025");
    }

    /// TEST-7: DOCX con relazione esterna → `quarantined`, nessun testo.
    #[test]
    fn locate_quarantines_docx_with_external_relationship() {
        let dir = scratch_dir();
        let input = write_docx(&dir, "ext.docx", &twin_document_xml(), Some(EXTERNAL_RELS));
        let excerpt = dir.join("e.txt");
        std::fs::write(&excerpt, "cresciuto").unwrap();
        match run_locate(&input, &excerpt) {
            ChildOutcome::Quarantined { reasons } => {
                assert!(reasons.contains(&"external_link".to_owned()), "{reasons:?}")
            }
            other => panic!("atteso quarantined, ottenuto {other:?}"),
        }
    }

    /// Letture con `take`: excerpt oltre 64 KiB e Markdown oltre 8 MiB → `ResourceLimit`.
    #[test]
    fn locate_enforces_read_caps() {
        let dir = scratch_dir();
        let input = dir.join("doc.md");
        std::fs::write(&input, LOCATE_MD).unwrap();
        let big_excerpt = dir.join("big.txt");
        std::fs::write(&big_excerpt, vec![b'a'; MAX_EXCERPT_BYTES as usize + 1]).unwrap();
        match run_locate(&input, &big_excerpt) {
            ChildOutcome::Failed { kind, .. } => assert_eq!(kind, Kind::ResourceLimit),
            other => panic!("excerpt: atteso failed, ottenuto {other:?}"),
        }
        let big_md = dir.join("big.md");
        let mut body = b"# T\n\n".to_vec();
        body.resize(MAX_MD_BYTES as usize + 1, b'x');
        std::fs::write(&big_md, body).unwrap();
        let excerpt = dir.join("e.txt");
        std::fs::write(&excerpt, "xxx").unwrap();
        match run_locate(&big_md, &excerpt) {
            ChildOutcome::Failed { kind, .. } => assert_eq!(kind, Kind::ResourceLimit),
            other => panic!("md: atteso failed, ottenuto {other:?}"),
        }
    }

    /// Formato non MD/DOCX → `failed`/`Unsupported`.
    #[test]
    fn locate_rejects_other_formats() {
        let dir = scratch_dir();
        let input = dir.join("doc.pdf");
        std::fs::write(&input, b"%PDF-1.7\n").unwrap();
        let excerpt = dir.join("e.txt");
        std::fs::write(&excerpt, "x").unwrap();
        match run_locate(&input, &excerpt) {
            ChildOutcome::Failed { kind, .. } => assert_eq!(kind, Kind::Unsupported),
            other => panic!("atteso failed, ottenuto {other:?}"),
        }
    }
}
