//! Test del lanciatore isolato (piano 01-07, Task 2): casi (a)-(h3) del piano e TEST-1..TEST-6,
//! TEST-9 del design `test_design_8`; dal design `test_design_11` (piano v6) i casi (f2), (f3)
//! di TEST-11 e `parse_child_data_schema` di TEST-15 (Task 3), in coda al file.
//!
//! Solo Linux, nel runner: `bash tools/session.sh oneshot "$CKA_RUN_ID" -- cargo test -p s02-parser
//! --test isolation`. Il processo di test fa le veci di `parse-parent`: si dichiara
//! `PR_SET_CHILD_SUBREAPER` e imposta una volta le tre variabili lette dal padre
//! (`S02_BIN_DIR`, `S02_WORK_ROOT`, `S02_RESULTS_RAW`) su cartelle sotto `/work/tmp`
//! (`S02_TEST_BASE` per spostarle). Le radici ammesse nei test sono quindi i valori rimappati:
//! gli input stanno sotto `<S02_RESULTS_RAW>/fixtures`, l'`out_dir` del job resta la stringa
//! `/results/raw/<area>/<job_id>` e il figlio la riceve rimappata.
//!
//! Cartella dei bin dei test: `fake-child` (copia del bin del crate), `ocr` e `locator` (copie
//! di `fake-child`, solo per far risolvere le op di 01-13 e 01-09), `parse-child` (script `sh`
//! di prova: scrive `result.txt` nell'`out_dir`; con un input `*.sleep` resta vivo), nessun
//! `extract` (per `ChildMissing`).
//!
//! Formato assunto per `argv-echo`: `data = {"argv": [...], "cwd": "..."}`, con l'ultimo
//! elemento di `argv` uguale all'`arg` del job.
#![cfg(target_os = "linux")]

// Modifica: Claude (test-write 01.6-07) · 2026-10-26 · budget_admits, dropped_future e quota_counts deterministici sotto carico, senza tolleranze allargate (gap-closure 1.6, D-11)
// Modifica: Claude (gsd-executor 01.6-07) · 2026-10-26 · test deterministici sotto carico e pulizia delle cartelle temporanee (gap-closure 1.6, D-11)

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use nix::sys::signal::{Signal, kill, killpg};
use nix::unistd::Pid;
use s02_parser::isolate::{Budget, Limits, RunReport, run_child};
use s02_parser::job::{self, ChildSpec, JobError, JobErrorCode};
use s02_parser::protocol::{ChildOutcome, Kind};
use serde_json::{Value, json};

const MIB: u64 = 1024 * 1024;
const KIB: u64 = 1024;

// ---- ambiente dei test --------------------------------------------------------------------

struct Env {
    base: PathBuf,
    work_root: PathBuf,
    raw: PathBuf,
    bin: PathBuf,
    outside: PathBuf,
}

const PARSE_CHILD_SCRIPT: &str = r#"#!/bin/sh
# parse-child finto dei test del lanciatore (argv: --input <in> --out <out>).
case "$2" in
  *.sleep) exec /bin/sleep 30 ;;
esac
printf 'ok' > "$4/result.txt"
printf '{"status":"ok","pages":null,"output":null,"data":null}\n'
"#;

fn env() -> &'static Env {
    static ENV: OnceLock<Env> = OnceLock::new();
    ENV.get_or_init(|| {
        let root = std::env::var_os("S02_TEST_BASE")
            .map_or_else(|| PathBuf::from("/work/tmp"), PathBuf::from);
        let base = root.join(format!("s02-isolation-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        for d in ["jobs", "raw/fixtures", "bin", "outside"] {
            fs::create_dir_all(base.join(d)).unwrap();
        }
        let base = base.canonicalize().unwrap();
        let e = Env {
            work_root: base.join("jobs"),
            raw: base.join("raw"),
            bin: base.join("bin"),
            outside: base.join("outside"),
            base,
        };

        let fake = PathBuf::from(env!("CARGO_BIN_EXE_fake-child"));
        for name in ["fake-child", "ocr", "locator"] {
            fs::copy(&fake, e.bin.join(name)).unwrap();
        }
        let script = e.bin.join("parse-child");
        fs::write(&script, PARSE_CHILD_SCRIPT).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

        let fx = e.raw.join("fixtures");
        fs::write(fx.join("in.pdf"), b"%PDF-1.7\n%%EOF\n").unwrap();
        fs::write(fx.join("in.sleep"), b"x").unwrap();
        fs::write(fx.join("excerpt.txt"), b"estratto").unwrap();
        fs::write(e.outside.join("secret.pdf"), b"%PDF-1.7\n%%EOF\n").unwrap();
        fs::write(e.outside.join("excerpt.txt"), b"fuori").unwrap();
        symlink(e.outside.join("secret.pdf"), fx.join("link.pdf")).unwrap();

        // SAFETY: impostate una sola volta, dentro OnceLock, prima di ogni uso del lanciatore;
        // nessun codice non Rust del processo di test legge l'ambiente in parallelo.
        unsafe {
            std::env::set_var("S02_BIN_DIR", &e.bin);
            std::env::set_var("S02_WORK_ROOT", &e.work_root);
            std::env::set_var("S02_RESULTS_RAW", &e.raw);
        }
        nix::sys::prctl::set_child_subreaper(true)
            .expect("PR_SET_CHILD_SUBREAPER nel processo di test");
        e
    })
}

fn fixture(name: &str) -> PathBuf {
    env().raw.join("fixtures").join(name)
}

fn job_dir(job_id: &str) -> PathBuf {
    env().work_root.join(job_id)
}

fn out_real(area: &str, job_id: &str) -> PathBuf {
    env().raw.join(area).join(job_id)
}

fn out_str(area: &str, job_id: &str) -> String {
    format!("/results/raw/{area}/{job_id}")
}

fn job_json(job_id: &str, op: &str, input: &Path, out_dir: Option<&str>, params: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "job_id": job_id,
        "op": op,
        "input": input.to_str().unwrap(),
        "out_dir": out_dir,
        "params": params,
    }))
    .unwrap()
}

fn selftest(job_id: &str, mode: &str, arg: Option<&str>) -> Vec<u8> {
    let params = match arg {
        Some(a) => json!({"mode": mode, "arg": a}),
        None => json!({"mode": mode}),
    };
    job_json(job_id, "selftest", &fixture("in.pdf"), None, params)
}

fn spec(bytes: &[u8]) -> ChildSpec {
    let _ = env();
    let job = job::parse(bytes).unwrap_or_else(|e| panic!("job non valido: {e:?}"));
    job::resolve(&job).unwrap_or_else(|e| panic!("resolve fallita: {e:?}"))
}

/// Errore di job da `parse`, `resolve` o, se queste passano, da `run_child` prima dello spawn.
async fn job_err(bytes: &[u8], limits: &Limits, budget: &Budget) -> JobError {
    let _ = env();
    let job = match job::parse(bytes) {
        Ok(j) => j,
        Err(e) => return e,
    };
    let spec = match job::resolve(&job) {
        Ok(s) => s,
        Err(e) => return e,
    };
    match run_child(&spec, limits, budget).await {
        Ok(r) => panic!("atteso job_error, il figlio è partito: {r:?}"),
        Err(e) => e
            .downcast_ref::<JobError>()
            .cloned()
            .unwrap_or_else(|| panic!("atteso JobError, ottenuto {e:#}")),
    }
}

/// Errore di job da `parse` o `resolve` (nessun lancio tentato).
fn static_err(bytes: &[u8]) -> JobError {
    let _ = env();
    match job::parse(bytes) {
        Err(e) => e,
        Ok(j) => match job::resolve(&j) {
            Err(e) => e,
            Ok(s) => panic!("atteso job_error, risolto {s:?}"),
        },
    }
}

fn limits() -> Limits {
    Limits {
        address_space_bytes: 256 * MIB,
        memory_reservation_bytes: 256 * MIB,
        cpu_seconds: 10,
        wall_timeout: Duration::from_secs(10),
        file_size_bytes: 16 * MIB,
        max_open_files: 64,
        out_quota_bytes: MIB,
    }
}

fn big_budget() -> Budget {
    Budget::fixed(2048 * MIB, 256 * MIB)
}

fn failed(o: &ChildOutcome) -> (Kind, String) {
    match o {
        ChildOutcome::Failed { kind, detail } => (*kind, detail.clone()),
        other => panic!("atteso failed, ottenuto {other:?}"),
    }
}

fn assert_ok_report(r: &RunReport, job_id: &str) {
    assert_eq!(r.job_id, job_id);
    assert!(
        matches!(r.outcome, ChildOutcome::Ok { .. }),
        "esito {:?}",
        r.outcome
    );
    assert!(r.oom_adj_set, "oom_score_adj non applicato: {r:?}");
    assert_eq!(r.leftover_procs, 0, "{r:?}");
}

/// Attende fino a `within` che la condizione valga.
async fn eventually(within: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + within;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= end {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Processi (pid, ppid, pgid) con la cartella corrente uguale a `dir`.
fn procs_with_cwd(dir: &Path) -> Vec<(i32, i32, i32)> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir("/proc") else {
        return out;
    };
    for entry in rd.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<i32>().ok())
        else {
            continue;
        };
        let Ok(cwd) = fs::read_link(format!("/proc/{pid}/cwd")) else {
            continue;
        };
        if cwd != dir {
            continue;
        }
        let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        let Some(rest) = stat.rsplit_once(')').map(|(_, r)| r) else {
            continue;
        };
        let f: Vec<&str> = rest.split_whitespace().collect();
        if let (Some(ppid), Some(pgid)) = (f.get(1), f.get(2)) {
            out.push((pid, ppid.parse().unwrap_or(0), pgid.parse().unwrap_or(0)));
        }
    }
    out
}

/// Raccoglie i pgid dei processi del job mentre gira, per `window`.
async fn watch_groups(dir: PathBuf, window: Duration) -> Vec<i32> {
    let end = Instant::now() + window;
    let mut groups = Vec::new();
    while Instant::now() < end {
        for (_, _, pgid) in procs_with_cwd(&dir) {
            if !groups.contains(&pgid) {
                groups.push(pgid);
            }
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    groups
}

fn group_alive(pgid: i32) -> bool {
    killpg(Pid::from_raw(pgid), None).is_ok()
}

// ---- TEST-1: job JSON chiuso, allowlist, argv del padre ---------------------------------

/// (a) Percorso con spazi e caratteri non ASCII arriva identico al figlio.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn argv_echo_passes_path_with_spaces_and_non_ascii_unchanged() {
    let id = "tw-argv-echo";
    let arg = env().raw.join("fixtures/dir con spazi àèìòù/file €ß ü.pdf");
    let arg = arg.to_str().unwrap();
    let s = spec(&selftest(id, "argv-echo", Some(arg)));
    let r = run_child(&s, &limits(), &big_budget()).await.unwrap();
    assert_ok_report(&r, id);
    let ChildOutcome::Ok {
        data: Some(data), ..
    } = &r.outcome
    else {
        panic!("argv-echo senza data: {:?}", r.outcome);
    };
    let argv: Vec<&str> = data["argv"]
        .as_array()
        .expect("data.argv")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(argv.last().copied(), Some(arg), "argv {argv:?}");
    assert!(argv.contains(&"argv-echo"), "argv {argv:?}");
    assert_eq!(data["cwd"].as_str(), Some(job_dir(id).to_str().unwrap()));
    assert!(!job_dir(id).exists(), "cartella del job non rimossa");
}

#[test]
fn unknown_op_is_unknown_op() {
    let e = static_err(&job_json(
        "tw-unknown-op",
        "shell",
        &fixture("in.pdf"),
        None,
        json!({}),
    ));
    assert_eq!(e.code, JobErrorCode::UnknownOp, "{e:?}");
}

#[test]
fn unknown_top_level_field_is_invalid_params() {
    let mut v: Value =
        serde_json::from_slice(&selftest("tw-unknown-field", "argv-echo", Some("x"))).unwrap();
    v["bin"] = json!("/bin/sh");
    let e = static_err(&serde_json::to_vec(&v).unwrap());
    assert_eq!(e.code, JobErrorCode::InvalidParams, "{e:?}");
}

#[test]
fn unknown_param_is_invalid_params() {
    let e = static_err(&job_json(
        "tw-unknown-param",
        "selftest",
        &fixture("in.pdf"),
        None,
        json!({"mode": "argv-echo", "arg": "x", "argv": ["-c", "id"]}),
    ));
    assert_eq!(e.code, JobErrorCode::InvalidParams, "{e:?}");
}

#[test]
fn selftest_mode_outside_list_is_invalid_params() {
    let e = static_err(&selftest("tw-bad-mode", "exec", Some("/bin/sh")));
    assert_eq!(e.code, JobErrorCode::InvalidParams, "{e:?}");
}

#[test]
fn oversized_job_is_rejected() {
    let big = "a".repeat(70 * 1024);
    let _ = static_err(&selftest("tw-oversized", "argv-echo", Some(&big)));
}

#[test]
fn job_id_outside_regex_is_rejected() {
    let long = "a".repeat(65);
    for id in ["../escape", "Upper", "a/b", "", "a b", long.as_str()] {
        let e = static_err(&selftest(id, "argv-echo", Some("x")));
        assert!(
            matches!(
                e.code,
                JobErrorCode::InvalidParams | JobErrorCode::PathNotAllowed
            ),
            "job_id {id:?}: {e:?}"
        );
    }
}

#[test]
fn resolve_builds_fixed_argv_per_op() {
    let e = env();
    let input = fixture("in.pdf").canonicalize().unwrap();
    let os = |v: &[&str]| v.iter().map(OsString::from).collect::<Vec<_>>();
    let inp = input.to_str().unwrap();

    let s = spec(&job_json(
        "tw-argv-parse",
        "parse",
        &input,
        Some(&out_str("test", "tw-argv-parse")),
        json!({}),
    ));
    assert_eq!(s.bin, e.bin.join("parse-child"));
    let out = out_real("test", "tw-argv-parse");
    assert_eq!(
        s.args,
        os(&["--input", inp, "--out", out.to_str().unwrap()])
    );
    assert_eq!(s.job_id, "tw-argv-parse");

    let s = spec(&job_json(
        "tw-argv-render",
        "ocr_render",
        &input,
        Some(&out_str("ocr", "tw-argv-render")),
        json!({"dpi": 150}),
    ));
    assert_eq!(s.bin, e.bin.join("ocr"));
    let out = out_real("ocr", "tw-argv-render");
    assert_eq!(
        s.args,
        os(&[
            "render",
            "--input",
            inp,
            "--dpi",
            "150",
            "--out",
            out.to_str().unwrap()
        ])
    );

    let s = spec(&job_json(
        "tw-argv-tess",
        "ocr_tesseract",
        &input,
        None,
        json!({}),
    ));
    assert_eq!(s.bin, e.bin.join("ocr"));
    assert_eq!(s.args, os(&["tesseract", "--image", inp]));
    assert!(
        s.env
            .contains(&(OsString::from("OMP_THREAD_LIMIT"), OsString::from("1"))),
        "env {:?}",
        s.env
    );
    assert_eq!(s.descendants, 1);

    let ex = fixture("excerpt.txt").canonicalize().unwrap();
    let s = spec(&job_json(
        "tw-argv-locate",
        "locate",
        &input,
        None,
        json!({"excerpt_file": ex.to_str().unwrap()}),
    ));
    assert_eq!(s.bin, e.bin.join("locator"));
    assert_eq!(
        s.args,
        os(&[
            "locate",
            "--input",
            inp,
            "--excerpt-file",
            ex.to_str().unwrap()
        ])
    );

    let s = spec(&selftest("tw-argv-self", "argv-echo", Some("--evil arg")));
    assert_eq!(s.bin, e.bin.join("fake-child"));
    assert_eq!(s.args, os(&["argv-echo", "--evil arg"]));
    assert_eq!(s.descendants, 0);
}

#[test]
fn missing_bin_is_child_missing() {
    let e = static_err(&job_json(
        "tw-no-extract",
        "extract_pdf",
        &fixture("in.pdf"),
        Some(&out_str("extract", "tw-no-extract")),
        json!({}),
    ));
    assert_eq!(e.code, JobErrorCode::ChildMissing, "{e:?}");
    assert!(!out_real("extract", "tw-no-extract").exists());
}

/// CLI di `parse-parent`: `job_error` su stdout ed exit 2 (assunzione: la validazione del job
/// non dipende dal file dei limiti, quindi il test non passa `--limits`). Il bin si lancia col
/// percorso letterale del runner (`CARGO_TARGET_DIR=/target`): lo scanner rifiuta
/// `Command::new` su un percorso non letterale.
#[test]
fn parse_parent_prints_job_error_and_exits_2() {
    const PARSE_PARENT: &str = "/target/debug/parse-parent";
    assert_eq!(
        env!("CARGO_BIN_EXE_parse-parent"),
        PARSE_PARENT,
        "test pensato per il runner (CARGO_TARGET_DIR=/target)"
    );
    let e = env();
    let run = |job: &[u8], args: &[&str]| {
        let mut c = Command::new("/target/debug/parse-parent");
        c.args(args)
            .env("S02_BIN_DIR", &e.bin)
            .env("S02_WORK_ROOT", &e.work_root)
            .env("S02_RESULTS_RAW", &e.raw)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = c.spawn().unwrap();
        child.stdin.take().unwrap().write_all(job).unwrap();
        child.wait_with_output().unwrap()
    };

    let o = run(
        &job_json(
            "tw-cli-unknown",
            "shell",
            &fixture("in.pdf"),
            None,
            json!({}),
        ),
        &["run", "--job", "-"],
    );
    assert_eq!(
        o.status.code(),
        Some(2),
        "stderr {}",
        String::from_utf8_lossy(&o.stderr)
    );
    let stdout = String::from_utf8(o.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 1, "stdout {stdout:?}");
    let v: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(v["status"], "job_error");
    assert_eq!(v["code"], "UnknownOp");
    assert_eq!(v["job_id"], "tw-cli-unknown");
    assert!(v["detail"].is_string());

    let path = e.base.join("cli-job.json");
    fs::write(
        &path,
        job_json(
            "tw-cli-missing",
            "extract_pdf",
            &fixture("in.pdf"),
            Some(&out_str("extract", "tw-cli-missing")),
            json!({}),
        ),
    )
    .unwrap();
    let o = run(b"", &["run", "--job", path.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(2));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["status"], "job_error");
    assert_eq!(v["code"], "ChildMissing");

    // Nessun nome di bin dal chiamante: la vecchia CLI `--child` non esiste più.
    let o = run(b"", &["run", "--child", "fake-child", "--input", "/x"]);
    assert!(!o.status.success());
}

// ---- TEST-2: percorsi, symlink, TOCTOU, out_dir esclusivo --------------------------------

#[test]
fn input_with_dotdot_outside_roots_is_path_not_allowed() {
    let input = env().raw.join("fixtures/../../outside/secret.pdf");
    let e = static_err(&job_json(
        "tw-dotdot",
        "selftest",
        &input,
        None,
        json!({"mode": "argv-echo"}),
    ));
    assert_eq!(e.code, JobErrorCode::PathNotAllowed, "{e:?}");
}

#[test]
fn input_symlink_outside_roots_is_path_not_allowed() {
    let e = static_err(&job_json(
        "tw-symlink-in",
        "selftest",
        &fixture("link.pdf"),
        None,
        json!({"mode": "argv-echo"}),
    ));
    assert_eq!(e.code, JobErrorCode::PathNotAllowed, "{e:?}");
}

#[test]
fn excerpt_file_outside_roots_is_path_not_allowed() {
    let ex = env().outside.join("excerpt.txt");
    let e = static_err(&job_json(
        "tw-excerpt-out",
        "locate",
        &fixture("in.pdf"),
        None,
        json!({"excerpt_file": ex.to_str().unwrap()}),
    ));
    assert_eq!(e.code, JobErrorCode::PathNotAllowed, "{e:?}");
}

/// (h3) `out_dir` di forma diversa da `/results/raw/<area>/<job_id>`.
#[test]
fn out_dir_wrong_forms_are_path_not_allowed() {
    let id = "tw-outform";
    let under_jobs = job_dir(id);
    let forms = [
        "/results/raw/test/other-id".to_owned(),
        format!("/results/raw/test/../test/{id}"),
        format!("/results/raw/Bad_Area/{id}"),
        format!("/results/raw/{id}"),
        format!("results/raw/test/{id}"),
        format!("/results/raw/test/{id}/"),
        under_jobs.to_str().unwrap().to_owned(),
        out_real("test", id).to_str().unwrap().to_owned(),
    ];
    for f in &forms {
        let e = static_err(&job_json(
            id,
            "parse",
            &fixture("in.pdf"),
            Some(f),
            json!({}),
        ));
        assert_eq!(e.code, JobErrorCode::PathNotAllowed, "out_dir {f:?}: {e:?}");
    }
}

#[test]
fn out_dir_presence_must_match_op() {
    let e = static_err(&job_json(
        "tw-self-out",
        "selftest",
        &fixture("in.pdf"),
        Some(&out_str("test", "tw-self-out")),
        json!({"mode": "argv-echo"}),
    ));
    assert!(
        matches!(
            e.code,
            JobErrorCode::InvalidParams | JobErrorCode::PathNotAllowed
        ),
        "{e:?}"
    );
    let e = static_err(&job_json(
        "tw-parse-null",
        "parse",
        &fixture("in.pdf"),
        None,
        json!({}),
    ));
    assert!(
        matches!(
            e.code,
            JobErrorCode::InvalidParams | JobErrorCode::PathNotAllowed
        ),
        "{e:?}"
    );
}

/// (h3) `out_dir` già esistente: rifiutato, nessun processo lanciato.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn existing_out_dir_is_out_dir_exists_and_launches_nothing() {
    let id = "tw-out-exists";
    fs::create_dir_all(out_real("test", id)).unwrap();
    let e = job_err(
        &job_json(
            id,
            "parse",
            &fixture("in.pdf"),
            Some(&out_str("test", id)),
            json!({}),
        ),
        &limits(),
        &big_budget(),
    )
    .await;
    assert_eq!(e.code, JobErrorCode::PathNotAllowed, "{e:?}");
    assert_eq!(e.detail, "out_dir_exists");
    assert!(
        !out_real("test", id).join("result.txt").exists(),
        "il figlio è partito"
    );
    assert!(!job_dir(id).exists());
}

/// (h3) Componente dell'`out_dir` sostituito da un symlink dopo `resolve`, prima dello spawn.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn out_dir_symlink_planted_after_resolve_is_rejected_before_spawn() {
    let id = "tw-out-swap";
    let s = spec(&job_json(
        id,
        "parse",
        &fixture("in.pdf"),
        Some(&out_str("swap", id)),
        json!({}),
    ));
    let target = env().outside.join("swap-target");
    fs::create_dir_all(&target).unwrap();
    let _ = fs::remove_dir_all(env().raw.join("swap"));
    symlink(&target, env().raw.join("swap")).unwrap();

    let err = run_child(&s, &limits(), &big_budget()).await.unwrap_err();
    let e = err
        .downcast_ref::<JobError>()
        .unwrap_or_else(|| panic!("atteso JobError: {err:#}"));
    assert_eq!(e.code, JobErrorCode::PathNotAllowed, "{e:?}");
    assert_eq!(e.detail, "symlink");
    assert!(
        !target.join(id).exists(),
        "out_dir creato seguendo il symlink"
    );
    assert!(!job_dir(id).exists());
}

/// TOCTOU controllato: componente dell'input sostituito da un symlink dopo `resolve`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn input_component_swapped_for_symlink_after_resolve_is_rejected() {
    let id = "tw-in-swap";
    let dir = env().raw.join("fixtures/swapdir");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("in.pdf"), b"%PDF-1.7\n").unwrap();
    let s = spec(&job_json(
        id,
        "parse",
        &dir.join("in.pdf"),
        Some(&out_str("test", id)),
        json!({}),
    ));

    let elsewhere = env().outside.join("swapdir");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::write(elsewhere.join("in.pdf"), b"%PDF-1.7\n").unwrap();
    fs::rename(&dir, env().raw.join("fixtures/swapdir-orig")).unwrap();
    symlink(&elsewhere, &dir).unwrap();

    let err = run_child(&s, &limits(), &big_budget()).await.unwrap_err();
    let e = err
        .downcast_ref::<JobError>()
        .unwrap_or_else(|| panic!("atteso JobError: {err:#}"));
    assert_eq!(e.code, JobErrorCode::PathNotAllowed, "{e:?}");
    assert_eq!(e.detail, "symlink");
    assert!(
        !out_real("test", id).join("result.txt").exists(),
        "il figlio è partito"
    );
    assert!(!job_dir(id).exists());
}

// ---- TEST-3: parametri, limiti e budget falliscono chiusi -------------------------------

#[test]
fn dpi_out_of_domain_is_invalid_params() {
    for (i, dpi) in [
        json!(0),
        json!(71),
        json!(601),
        json!(-1),
        json!("150"),
        json!(150.5),
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("tw-dpi-bad-{i}");
        let e = static_err(&job_json(
            &id,
            "ocr_render",
            &fixture("in.pdf"),
            Some(&out_str("ocr", &id)),
            json!({"dpi": dpi}),
        ));
        assert_eq!(e.code, JobErrorCode::InvalidParams, "dpi {dpi}: {e:?}");
    }
    for dpi in [72, 600] {
        let id = format!("tw-dpi-ok-{dpi}");
        let _ = spec(&job_json(
            &id,
            "ocr_render",
            &fixture("in.pdf"),
            Some(&out_str("ocr", &id)),
            json!({"dpi": dpi}),
        ));
    }
}

/// Prenotazione = `memory_reservation_bytes` × (1 + discendenti) + quota; oltre totale − riserva
/// → `BudgetExceeded` (dal piano v6 la prenotazione non usa più l'AS: i casi fissano la
/// prenotazione uguale all'AS, come la (f) del piano).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reservation_over_budget_is_budget_exceeded_without_spawn() {
    let budget = Budget::fixed(320 * MIB, 64 * MIB);

    let id = "tw-bud-as";
    let l = Limits {
        address_space_bytes: 512 * MIB,
        memory_reservation_bytes: 512 * MIB,
        out_quota_bytes: 0,
        ..limits()
    };
    let e = job_err(&selftest(id, "sleep", Some("0")), &l, &budget).await;
    assert_eq!(e.code, JobErrorCode::BudgetExceeded, "{e:?}");
    assert!(!job_dir(id).exists());

    // La quota dell'output (tmpfs) entra nella prenotazione.
    let id = "tw-bud-quota";
    let l = Limits {
        address_space_bytes: 200 * MIB,
        memory_reservation_bytes: 200 * MIB,
        out_quota_bytes: 64 * MIB,
        ..limits()
    };
    let e = job_err(&selftest(id, "sleep", Some("0")), &l, &budget).await;
    assert_eq!(e.code, JobErrorCode::BudgetExceeded, "{e:?}");

    // I discendenti ammessi dall'op moltiplicano la prenotazione: 2 × 130 MiB > 256 MiB.
    let id = "tw-bud-desc";
    let l = Limits {
        address_space_bytes: 130 * MIB,
        memory_reservation_bytes: 130 * MIB,
        out_quota_bytes: 0,
        ..limits()
    };
    let e = job_err(
        &job_json(id, "ocr_tesseract", &fixture("in.pdf"), None, json!({})),
        &l,
        &budget,
    )
    .await;
    assert_eq!(e.code, JobErrorCode::BudgetExceeded, "{e:?}");

    // Entro il budget lo stesso job parte.
    let id = "tw-bud-fits";
    let l = Limits {
        address_space_bytes: 200 * MIB,
        memory_reservation_bytes: 200 * MIB,
        out_quota_bytes: 0,
        ..limits()
    };
    let r = run_child(&spec(&selftest(id, "sleep", Some("0"))), &l, &budget)
        .await
        .unwrap();
    assert_ok_report(&r, id);
}

/// (f) 4 job `alloc 100` con `memory_reservation_bytes` 128 MiB, AS 128 MiB e
/// `Budget::fixed(320 MiB, 64 MiB)`: tutti `ok`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn budget_admits_alloc_jobs_and_all_complete() {
    let budget = Budget::fixed(320 * MIB, 64 * MIB);
    let l = Limits {
        address_space_bytes: 128 * MIB,
        memory_reservation_bytes: 128 * MIB,
        out_quota_bytes: 0,
        ..limits()
    };
    let specs: Vec<ChildSpec> = (0..4)
        .map(|i| spec(&selftest(&format!("tw-alloc-{i}"), "alloc", Some("100"))))
        .collect();
    let (a, b, c, d) = tokio::join!(
        run_child(&specs[0], &l, &budget),
        run_child(&specs[1], &l, &budget),
        run_child(&specs[2], &l, &budget),
        run_child(&specs[3], &l, &budget),
    );
    for (i, r) in [a, b, c, d].into_iter().enumerate() {
        assert_ok_report(&r.unwrap(), &format!("tw-alloc-{i}"));
    }
}

/// Numero di cartelle `tw-pair-*` vive nella radice dei job, da **una sola** `read_dir` della
/// cartella padre: quattro `is_dir` separati leggerebbero quattro istanti diversi e potrebbero
/// contare insieme una cartella appena rimossa e una appena creata (test design 01.6-07, RISK-2).
fn pair_dirs_alive(work_root: &Path) -> usize {
    fs::read_dir(work_root).map_or(0, |rd| {
        rd.flatten()
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("tw-pair-"))
            })
            .count()
    })
}

/// (f) Mai più di 2 ammessi insieme, e 2 davvero insieme: il contatore del budget arriva a 2
/// e non oltre, un osservatore concorrente vede al massimo 2 cartelle di job vive e ne vede 2
/// insieme; i 4 job `sleep 1` a coppie durano almeno ~2 s. Nessun limite superiore di tempo:
/// il tetto lo prova `max_admitted`, la coppia lo prova l'osservatore (piano 01.6-07, D-11).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn budget_admits_at_most_two_jobs_together() {
    let budget = Budget::fixed(320 * MIB, 64 * MIB);
    let l = Limits {
        address_space_bytes: 128 * MIB,
        memory_reservation_bytes: 128 * MIB,
        out_quota_bytes: 0,
        ..limits()
    };
    let specs: Vec<ChildSpec> = (0..4)
        .map(|i| spec(&selftest(&format!("tw-pair-{i}"), "sleep", Some("1"))))
        .collect();
    let work_root = env().work_root.clone();
    let stop = AtomicBool::new(false);
    let jobs = async {
        let out = tokio::join!(
            run_child(&specs[0], &l, &budget),
            run_child(&specs[1], &l, &budget),
            run_child(&specs[2], &l, &budget),
            run_child(&specs[3], &l, &budget),
        );
        stop.store(true, Ordering::SeqCst);
        out
    };
    // Campiona ogni 10 ms fino al termine dei 4 job; si ferma con `stop`, altrimenti il `join!`
    // non finirebbe.
    let observer = async {
        let mut max = 0usize;
        while !stop.load(Ordering::SeqCst) {
            max = max.max(pair_dirs_alive(&work_root));
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        max
    };
    let t0 = Instant::now();
    let ((a, b, c, d), osservati_max) = tokio::join!(jobs, observer);
    let wall = t0.elapsed();
    for (i, r) in [a, b, c, d].into_iter().enumerate() {
        assert_ok_report(&r.unwrap(), &format!("tw-pair-{i}"));
    }
    assert_eq!(budget.max_admitted(), 2, "tetto dei job ammessi insieme");
    assert_eq!(
        osservati_max, 2,
        "massimo di cartelle tw-pair vive insieme (attese 2: a coppie, mai 3)"
    );
    assert!(
        wall >= Duration::from_millis(1900),
        "4 job da 1 s a coppie: wall {wall:?}"
    );
}

/// (g) `alloc` oltre `RLIMIT_AS` → `ResourceLimit`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alloc_over_address_space_is_resource_limit() {
    let id = "tw-alloc-over";
    let l = Limits {
        address_space_bytes: 128 * MIB,
        ..limits()
    };
    let r = run_child(
        &spec(&selftest(id, "alloc", Some("256"))),
        &l,
        &big_budget(),
    )
    .await
    .unwrap();
    assert_eq!(r.job_id, id);
    assert_eq!(failed(&r.outcome).0, Kind::ResourceLimit, "{r:?}");
    assert!(!job_dir(id).exists());
}

// ---- TEST-4: limiti non applicabili non diventano un job «isolato» ------------------------

/// `setrlimit` che fallisce in `pre_exec` (NOFILE oltre `nr_open`): spawn fallito con errno,
/// nessun RunReport, cartella rimossa, budget non trattenuto.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unapplicable_setrlimit_fails_the_spawn_with_errno() {
    let budget = Budget::fixed(320 * MIB, 64 * MIB);
    let id = "tw-rlimit-fail";
    let l = Limits {
        address_space_bytes: 200 * MIB,
        memory_reservation_bytes: 200 * MIB,
        out_quota_bytes: 0,
        max_open_files: 1 << 40,
        ..limits()
    };
    let err = run_child(&spec(&selftest(id, "argv-echo", Some("x"))), &l, &budget)
        .await
        .expect_err("un figlio senza limiti non deve partire");
    assert!(
        err.downcast_ref::<JobError>().is_none(),
        "non è un job_error: {err:#}"
    );
    let msg = format!("{err:#}");
    assert!(
        msg.contains("os error") || msg.contains("errno"),
        "errno assente: {msg}"
    );
    assert!(
        eventually(Duration::from_secs(1), || !job_dir(id).exists()).await,
        "cartella del job non rimossa"
    );

    let l = Limits {
        address_space_bytes: 200 * MIB,
        out_quota_bytes: 0,
        ..limits()
    };
    let next = tokio::time::timeout(
        Duration::from_secs(5),
        run_child(
            &spec(&selftest("tw-rlimit-next", "sleep", Some("0"))),
            &l,
            &budget,
        ),
    )
    .await
    .expect("budget trattenuto dopo lo spawn fallito");
    assert_ok_report(&next.unwrap(), "tw-rlimit-next");
}

// ---- TEST-5: timeout, cancellazione, flood, crash, nipoti --------------------------------

/// (c) `spawn-grandchild` + timeout → `Timeout`, gruppo ucciso e raccolto, cartella rimossa.
/// TEST-6: un fallimento del figlio è un `RunReport`, non un `job_error`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn timeout_kills_grandchild_group_and_removes_job_dir() {
    let id = "tw-grandchild-timeout";
    let l = Limits {
        wall_timeout: Duration::from_secs(1),
        ..limits()
    };
    let s = spec(&selftest(id, "spawn-grandchild", None));
    let bb = big_budget();
    let (r, groups) = tokio::join!(
        run_child(&s, &l, &bb),
        watch_groups(job_dir(id), Duration::from_millis(900)),
    );
    let r = r.expect("un timeout è un esito del figlio, non un errore");
    assert_eq!(r.job_id, id);
    assert_eq!(failed(&r.outcome).0, Kind::Timeout, "{r:?}");
    assert_eq!(r.leftover_procs, 0);
    assert!(r.oom_adj_set);
    assert!(!groups.is_empty(), "nessun processo del job osservato");
    for g in groups {
        assert!(!group_alive(g), "gruppo {g} ancora vivo");
    }
    assert!(procs_with_cwd(&job_dir(id)).is_empty());
    assert!(!job_dir(id).exists());
}

/// (d) Futuro di `run_child` droppato dal chiamante quando figlio e nipote del job sono vivi:
/// entro 1 s gruppo assente e cartella rimossa; il permesso del budget restituito si prova con
/// la cartella del job successivo comparsa entro 1 s (la cartella nasce solo dopo `admit`,
/// `isolate.rs:218` poi `prepare`). Il budget ammette un solo job da 200 MiB alla volta
/// (capacità 256 MiB), quindi la cartella di `tw-drop-next` può nascere solo dal permesso
/// restituito (piano 01.6-07, D-11).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropped_future_kills_group_and_cleans_within_one_second() {
    let budget = Budget::fixed(320 * MIB, 64 * MIB);
    let id = "tw-grandchild-drop";
    let l = Limits {
        address_space_bytes: 200 * MIB,
        memory_reservation_bytes: 200 * MIB,
        out_quota_bytes: 0,
        // guardia: tetto di durata del figlio contro il blocco, invariato dalla base.
        wall_timeout: Duration::from_secs(30),
        ..limits()
    };
    let s = spec(&selftest(id, "spawn-grandchild", None));
    let dir = job_dir(id);
    // Il futuro di `run_child` passa per valore al `select!`: all'uscita del `select!` il ramo
    // perdente cade subito, non a fine scope (test design, RISK-4).
    let (pids, groups) = tokio::select! {
        r = run_child(&s, &l, &budget) => {
            panic!("il futuro doveva essere droppato: run_child è finito prima dell'evento, {r:?}")
        }
        seen = async {
            // Evento: figlio e nipote `sleep 300` vivi, entrambi con la cartella del job come
            // cwd (test design, RISK-3).
            // guardia di blocco di 30 s, non la proprietà: allo scadere il test fallisce.
            let both = eventually(Duration::from_secs(30), || procs_with_cwd(&dir).len() >= 2);
            assert!(both.await, "figlio e nipote del job non osservati entro la guardia di 30 s");
            let mut pids = Vec::new();
            let mut groups = Vec::new();
            for (pid, _, pgid) in procs_with_cwd(&dir) {
                pids.push(pid);
                if !groups.contains(&pgid) {
                    groups.push(pgid);
                }
            }
            for pgid in watch_groups(dir.clone(), Duration::from_millis(100)).await {
                if !groups.contains(&pgid) {
                    groups.push(pgid);
                }
            }
            (pids, groups)
        } => seen,
    };
    assert!(
        pids.len() >= 2,
        "processi del job all'evento: {pids:?} (attesi figlio e nipote)"
    );
    assert!(!groups.is_empty(), "nessun processo del job osservato");
    let gone = eventually(Duration::from_secs(1), || {
        groups.iter().all(|g| !group_alive(*g)) && !dir.exists()
    })
    .await;
    assert!(
        gone,
        "dopo 1 s: gruppi vivi {:?}, cartella {}",
        groups,
        dir.exists()
    );

    let next_spec = spec(&selftest("tw-drop-next", "sleep", Some("1")));
    let next_dir = job_dir("tw-drop-next");
    // guardia di blocco di 30 s, non la proprietà: allo scadere il test fallisce.
    let guard = Duration::from_secs(30);
    let (next, admitted) = tokio::time::timeout(guard, async {
        tokio::join!(
            run_child(&next_spec, &l, &budget),
            eventually(Duration::from_secs(1), || next_dir.is_dir()),
        )
    })
    .await
    .expect("guardia di 30 s scaduta: run_child di tw-drop-next bloccato");
    assert!(
        admitted,
        "permesso del budget non restituito entro 1 s dal drop: cartella di tw-drop-next assente"
    );
    assert_ok_report(&next.unwrap(), "tw-drop-next");
}

/// (e) `flood` di 4 MiB su stdout e stderr → `OutputLimit` entro 2 s, padre vivo.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn flood_on_both_pipes_is_output_limit_within_two_seconds() {
    let id = "tw-flood";
    let t0 = Instant::now();
    let r = run_child(
        &spec(&selftest(id, "flood", Some("4194304"))),
        &limits(),
        &big_budget(),
    )
    .await
    .unwrap();
    assert!(t0.elapsed() < Duration::from_secs(2), "{:?}", t0.elapsed());
    assert_eq!(r.job_id, id);
    assert_eq!(failed(&r.outcome).0, Kind::OutputLimit, "{r:?}");
    assert_eq!(r.leftover_procs, 0);
    assert!(!job_dir(id).exists());
}

/// Figlio ucciso da fuori con SIGKILL, senza OOM del cgroup → `Crashed`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn externally_killed_child_is_crashed() {
    let id = "tw-killed";
    let me = std::process::id() as i32;
    let dir = job_dir(id);
    let killer = async {
        let mut killed = false;
        let end = Instant::now() + Duration::from_secs(3);
        while !killed && Instant::now() < end {
            for (pid, ppid, _) in procs_with_cwd(&dir) {
                if ppid == me {
                    kill(Pid::from_raw(pid), Signal::SIGKILL).unwrap();
                    killed = true;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        killed
    };
    let (s, l, bb) = (
        spec(&selftest(id, "sleep", Some("5"))),
        limits(),
        big_budget(),
    );
    let (r, killed) = tokio::join!(run_child(&s, &l, &bb), killer);
    assert!(killed, "figlio non trovato");
    let r = r.unwrap();
    assert_eq!(r.exit_signal, Some(9));
    if r.oom_kill_delta == 0 {
        assert_eq!(failed(&r.outcome).0, Kind::Crashed, "{r:?}");
    }
    assert!(!dir.exists());
}

// ---- TEST-6: esiti completi -------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn successful_run_reports_isolation_fields() {
    let id = "tw-report";
    let r = run_child(
        &spec(&selftest(id, "argv-echo", Some("x"))),
        &limits(),
        &big_budget(),
    )
    .await
    .unwrap();
    assert_ok_report(&r, id);
    assert_eq!(r.exit_signal, None);
    assert!(r.peak_rss_kb.is_some_and(|k| k > 0), "{r:?}");
    assert_eq!(r.oom_kill_delta, 0);
}

// ---- TEST-9: quota sommata e pulizia ----------------------------------------------------

/// (h) `write` oltre `out_quota_bytes` → `ResourceLimit/out_quota`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn write_over_quota_is_resource_limit_out_quota() {
    let id = "tw-write-over";
    let l = Limits {
        out_quota_bytes: 64 * KIB,
        ..limits()
    };
    let r = run_child(
        &spec(&selftest(id, "write", Some("1048576"))),
        &l,
        &big_budget(),
    )
    .await
    .unwrap();
    assert_eq!(
        failed(&r.outcome),
        (Kind::ResourceLimit, "out_quota".to_owned()),
        "{r:?}"
    );
    assert!(!job_dir(id).exists());
}

fn write_bytes(path: &Path, n: u64) {
    fs::write(path, vec![b'x'; n as usize]).unwrap();
}

/// (h2) Quota sommata: metà nella cartella del job e metà più un byte nell'`out_dir`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quota_counts_job_dir_plus_out_dir() {
    let id = "tw-quota-sum";
    let quota = 64 * KIB;
    let l = Limits {
        out_quota_bytes: quota,
        wall_timeout: Duration::from_secs(10),
        ..limits()
    };
    let s = spec(&job_json(
        id,
        "parse",
        &fixture("in.sleep"),
        Some(&out_str("test", id)),
        json!({}),
    ));
    let (jd, od) = (job_dir(id), out_real("test", id));
    let writer = async {
        assert!(
            eventually(Duration::from_secs(5), || jd.is_dir() && od.is_dir()).await,
            "cartelle del job non create"
        );
        write_bytes(&jd.join("half.bin"), quota / 2);
        write_bytes(&od.join("half-plus-one.bin"), quota / 2 + 1);
        // Istante del termine delle scritture, restituito come valore del ramo del `join!`:
        // i 5 s si misurano da qui, non dall'avvio del job (piano 01.6-07, D-11).
        Instant::now()
    };
    let bb = big_budget();
    let (r, scritto) = tokio::join!(run_child(&s, &l, &bb), writer);
    let rilevata = scritto.elapsed();
    let r = r.unwrap();
    assert_eq!(
        failed(&r.outcome),
        (Kind::ResourceLimit, "out_quota".to_owned()),
        "{r:?}"
    );
    assert!(
        rilevata < Duration::from_secs(5),
        "quota non rilevata in tempo: {rilevata:?} dal termine delle scritture"
    );
    assert!(!jd.exists());
}

/// Controllo di (h2): la somma sotto quota non fa scattare la quota (il job finisce in timeout).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quota_sum_under_limit_is_not_out_quota() {
    let id = "tw-quota-under";
    let quota = 64 * KIB;
    let l = Limits {
        out_quota_bytes: quota,
        wall_timeout: Duration::from_secs(2),
        ..limits()
    };
    let s = spec(&job_json(
        id,
        "parse",
        &fixture("in.sleep"),
        Some(&out_str("test", id)),
        json!({}),
    ));
    let (jd, od) = (job_dir(id), out_real("test", id));
    let writer = async {
        assert!(eventually(Duration::from_secs(1), || jd.is_dir() && od.is_dir()).await);
        write_bytes(&jd.join("q.bin"), quota / 4);
        write_bytes(&od.join("q.bin"), quota / 4);
    };
    let bb = big_budget();
    let (r, ()) = tokio::join!(run_child(&s, &l, &bb), writer);
    assert_eq!(failed(&r.unwrap().outcome).0, Kind::Timeout);
}

/// Tetto di 10.000 voci visitate nella scansione della quota → `out_quota`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quota_scan_entry_cap_is_out_quota() {
    let id = "tw-quota-entries";
    let l = Limits {
        out_quota_bytes: 64 * MIB,
        wall_timeout: Duration::from_secs(20),
        ..limits()
    };
    let s = spec(&job_json(
        id,
        "parse",
        &fixture("in.sleep"),
        Some(&out_str("test", id)),
        json!({}),
    ));
    let od = out_real("test", id);
    let writer = async {
        assert!(eventually(Duration::from_secs(5), || od.is_dir()).await);
        for i in 0..10_050 {
            if fs::write(od.join(format!("e{i}")), b"").is_err() {
                break;
            }
        }
    };
    let bb = big_budget();
    let (r, ()) = tokio::join!(run_child(&s, &l, &bb), writer);
    assert_eq!(
        failed(&r.unwrap().outcome),
        (Kind::ResourceLimit, "out_quota".to_owned())
    );
}

/// Job riuscito: l'`out_dir` (il risultato) resta, la cartella del job no.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn successful_parse_keeps_out_dir_and_removes_job_dir() {
    let id = "tw-parse-ok";
    let s = spec(&job_json(
        id,
        "parse",
        &fixture("in.pdf"),
        Some(&out_str("test", id)),
        json!({}),
    ));
    let r = run_child(&s, &limits(), &big_budget()).await.unwrap();
    assert_ok_report(&r, id);
    assert_eq!(
        fs::read(out_real("test", id).join("result.txt")).unwrap(),
        b"ok"
    );
    assert!(!job_dir(id).exists());
}

// ---- TEST-11: vittima designata dell'OOM e riserva del padre (casi f2, f3) --------------

/// (f2) Con un job `selftest sleep` in corso, `oom_score_adj` del figlio vale 1000 e quello
/// del processo del test 0: all'OOM del cgroup la vittima designata è il figlio. Nel
/// `RunReport` `oom_adj_set` = `true` (RISK-004 di risk_audit_19; l'OOM vero è di 01-18).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn child_is_the_designated_oom_victim() {
    let id = "tw-oom-adj";
    let s = spec(&selftest(id, "sleep", Some("2")));
    let dir = job_dir(id);
    let watch = async {
        // Ultimo valore letto dal figlio: si attende il 1000 (la scrittura avviene fra
        // `chdir` ed `exec`, una lettura troppo presto vede ancora il valore ereditato).
        let mut last: Option<String> = None;
        let end = Instant::now() + Duration::from_millis(1500);
        while Instant::now() < end && last.as_deref() != Some("1000") {
            for (pid, _, _) in procs_with_cwd(&dir) {
                if let Ok(v) = fs::read_to_string(format!("/proc/{pid}/oom_score_adj")) {
                    last = Some(v.trim().to_owned());
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        last
    };
    let (l, bb) = (limits(), big_budget());
    let (r, child_adj) = tokio::join!(run_child(&s, &l, &bb), watch);
    let r = r.unwrap();
    assert_ok_report(&r, id);
    assert!(r.oom_adj_set, "{r:?}");
    assert_eq!(
        child_adj.as_deref(),
        Some("1000"),
        "oom_score_adj del figlio"
    );
    let own = fs::read_to_string("/proc/self/oom_score_adj").unwrap();
    assert_eq!(own.trim(), "0", "oom_score_adj del processo del test");
}

/// Bin `parse-parent` del runner (percorso letterale, come in
/// `parse_parent_prints_job_error_and_exits_2`).
const PARSE_PARENT: &str = "/target/debug/parse-parent";

/// Budget fisso del padre nei test CLI (il runner non ha `memory.max`): variabili lette solo
/// dal padre, come `S02_BIN_DIR`. Assunzione del test-write: il piano chiede `Budget::fixed`
/// per (f3) senza fissare come lo riceve `parse-parent`.
const BUDGET_TOTAL_VAR: &str = "S02_BUDGET_TOTAL_BYTES";
const PARENT_RESERVE_VAR: &str = "S02_PARENT_RESERVE_BYTES";

fn write_limits(name: &str, l: &Limits) -> PathBuf {
    let p = env().base.join(name);
    fs::write(&p, serde_json::to_vec(&json!({ "default": l })).unwrap()).unwrap();
    p
}

/// `parse-parent <args>` con le radici dei test, la cartella dei bin data e le variabili in
/// più; stdin nullo.
fn run_parent(args: &[&str], bin: &Path, extra: &[(&str, String)]) -> std::process::Output {
    assert_eq!(
        env!("CARGO_BIN_EXE_parse-parent"),
        PARSE_PARENT,
        "test pensato per il runner (CARGO_TARGET_DIR=/target)"
    );
    let e = env();
    let mut c = Command::new("/target/debug/parse-parent");
    c.args(args)
        .env("S02_BIN_DIR", bin)
        .env("S02_WORK_ROOT", &e.work_root)
        .env("S02_RESULTS_RAW", &e.raw)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra {
        c.env(k, v);
    }
    c.output().unwrap()
}

/// Righe JSON dello stdout di `parse-parent`.
fn json_lines(stdout: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("riga {l:?}: {e}")))
        .collect()
}

/// (f3) `batch` con `Budget::fixed` e una riserva del padre più piccola del suo `VmHWM`:
/// rapporti dei job scritti, poi `parent_reserve_exceeded vm_hwm_kb=<n>` su stderr ed exit 3.
/// Controllo: con una riserva ampia lo stesso batch esce 0 senza quella riga.
#[test]
fn batch_over_parent_reserve_exits_3_after_reports() {
    let e = env();
    let limits_path = write_limits("f3-limits.json", &limits());
    let batch = |tag: &str, reserve: u64| {
        let ids: Vec<String> = ["a", "b"]
            .iter()
            .map(|s| format!("tw-f3-{tag}-{s}"))
            .collect();
        let jobs = e.base.join(format!("f3-{tag}.jsonl"));
        let lines: Vec<String> = ids
            .iter()
            .map(|id| String::from_utf8(selftest(id, "argv-echo", Some("x"))).unwrap())
            .collect();
        fs::write(&jobs, lines.join("\n")).unwrap();
        let o = run_parent(
            &[
                "batch",
                "--jobs",
                jobs.to_str().unwrap(),
                "--concurrency",
                "2",
                "--limits",
                limits_path.to_str().unwrap(),
            ],
            &e.bin,
            &[
                (BUDGET_TOTAL_VAR, (1024 * MIB).to_string()),
                (PARENT_RESERVE_VAR, reserve.to_string()),
            ],
        );
        (ids, o)
    };

    let (ids, o) = batch("low", MIB);
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert_eq!(o.status.code(), Some(3), "stderr {stderr}");
    let hwm: u64 = stderr
        .lines()
        .find_map(|l| l.strip_prefix("parent_reserve_exceeded vm_hwm_kb="))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or_else(|| panic!("riga parent_reserve_exceeded assente: {stderr}"));
    assert!(
        hwm > 1024,
        "VmHWM del padre {hwm} kB non oltre la riserva di 1 MiB"
    );
    let lines = json_lines(&o.stdout);
    for id in &ids {
        let rep = lines
            .iter()
            .find(|v| v["job_id"] == id.as_str() && v.get("outcome").is_some())
            .unwrap_or_else(|| panic!("rapporto di {id} assente: {lines:?}"));
        assert_eq!(rep["outcome"]["status"], "ok", "{rep}");
    }

    let (_, o) = batch("high", 256 * MIB);
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert_eq!(o.status.code(), Some(0), "stderr {stderr}");
    assert!(!stderr.contains("parent_reserve_exceeded"), "{stderr}");
}

// ---- TEST-15: schema unico dei sei campi di `data` ---------------------------------------

/// I sei campi dello schema unico (punto 5 di `<interfaces>` del piano v6).
const SCHEMA_FIELDS: [&str; 6] = [
    "encryption",
    "embedded_files",
    "objstm_count",
    "objstm_decompressed_bytes",
    "xref_streams",
    "decompress_work_bytes",
];

/// Cartella dei bin con il `parse-child` vero (copia del bin del crate).
fn real_bin_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let d = env().base.join("bin-real");
        fs::create_dir_all(&d).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_parse-child"), d.join("parse-child")).unwrap();
        d
    })
    .clone()
}

/// Cartella di `libpdfium.so` (build `chromium/8100` del worker): `S02_TEST_PDFIUM_DIR`,
/// `PDFIUM_DIR`, `/opt/pdfium/lib` o `/src/target/pdfium/lib` (copia ignorata da git sotto
/// `spikes/target`, perché l'immagine del runner non ha PDFium).
fn pdfium_dir() -> PathBuf {
    let mut candidates: Vec<PathBuf> = ["S02_TEST_PDFIUM_DIR", "PDFIUM_DIR"]
        .iter()
        .filter_map(|k| std::env::var_os(k).filter(|v| !v.is_empty()))
        .map(PathBuf::from)
        .collect();
    candidates.push(PathBuf::from("/opt/pdfium/lib"));
    candidates.push(PathBuf::from("/src/target/pdfium/lib"));
    candidates
        .iter()
        .find(|d| d.join("libpdfium.so").is_file())
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "PDFium assente in {candidates:?}: estrarre pdfium-linux-x64.tgz di chromium/8100 \
                 (sha256 di docker/worker.Dockerfile) in spikes/target/pdfium"
            )
        })
}

/// `parse-parent run` di un job `parse` col `parse-child` vero; ritorna `outcome.data`.
fn parse_data(id: &str, input: &Path) -> Value {
    let e = env();
    let l = Limits {
        address_space_bytes: 1536 * MIB,
        cpu_seconds: 60,
        wall_timeout: Duration::from_secs(60),
        file_size_bytes: 64 * MIB,
        out_quota_bytes: 64 * MIB,
        ..limits()
    };
    let limits_path = write_limits(&format!("{id}-limits.json"), &l);
    let job = e.base.join(format!("{id}.json"));
    fs::write(
        &job,
        job_json(id, "parse", input, Some(&out_str("schema", id)), json!({})),
    )
    .unwrap();
    let mut extra = vec![
        (BUDGET_TOTAL_VAR, (2048 * MIB).to_string()),
        (PARENT_RESERVE_VAR, (256 * MIB).to_string()),
    ];
    if input.extension().is_some_and(|x| x == "pdf") {
        extra.push(("PDFIUM_DIR", pdfium_dir().to_string_lossy().into_owned()));
    }
    let o = run_parent(
        &[
            "run",
            "--job",
            job.to_str().unwrap(),
            "--limits",
            limits_path.to_str().unwrap(),
        ],
        &real_bin_dir(),
        &extra,
    );
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert_eq!(o.status.code(), Some(0), "stderr {stderr}");
    let lines = json_lines(&o.stdout);
    assert_eq!(lines.len(), 1, "{lines:?}");
    let outcome = &lines[0]["outcome"];
    assert_eq!(outcome["status"], "ok", "{outcome}");
    outcome["data"].clone()
}

/// I sei campi, tutti presenti e con questi nomi: `encryption` stringa, gli altri interi.
fn six_fields(data: &Value) -> (String, [u64; 5]) {
    for k in SCHEMA_FIELDS {
        assert!(data.get(k).is_some(), "campo {k} assente in data: {data}");
    }
    let enc = data["encryption"]
        .as_str()
        .unwrap_or_else(|| panic!("encryption non stringa: {data}"))
        .to_owned();
    let n = |k: &str| {
        data[k]
            .as_u64()
            .unwrap_or_else(|| panic!("{k} non intero: {data}"))
    };
    (
        enc,
        [
            n("embedded_files"),
            n("objstm_count"),
            n("objstm_decompressed_bytes"),
            n("xref_streams"),
            n("decompress_work_bytes"),
        ],
    )
}

/// PDF `objstm-many`: 7.000 dizionari raggiungibili dal catalogo, che `qpdf
/// --object-streams=generate` raccoglie in object stream da al più 100 oggetti (quindi più di
/// 64) con una cross-reference stream.
fn objstm_many_pdf(path: &Path) {
    let n = 7000u32;
    let mut out: Vec<u8> = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets: Vec<usize> = Vec::new();
    let refs: Vec<String> = (0..n).map(|i| format!("{} 0 R", 5 + i)).collect();
    let mut objs: Vec<(u32, String)> = vec![
        (
            1,
            format!(
                "<< /Type /Catalog /Pages 2 0 R /Extra [{}] >>",
                refs.join(" ")
            ),
        ),
        (2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned()),
        (
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R >>".to_owned(),
        ),
        (
            4,
            "<< /Length 35 >>\nstream\nBT /F1 12 Tf 72 712 Td (Ciao) Tj ET\nendstream".to_owned(),
        ),
    ];
    objs.extend((0..n).map(|i| (5 + i, format!("<< /K {i} >>"))));
    for (num, body) in &objs {
        offsets.push(out.len());
        out.extend_from_slice(format!("{num} 0 obj\n{body}\nendobj\n").as_bytes());
    }
    let xref = out.len();
    let size = objs.len() + 1;
    out.extend_from_slice(format!("xref\n0 {size}\n0000000000 65535 f \n").as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    let plain = env().base.join("objstm-many-plain.pdf");
    fs::write(&plain, out).unwrap();
    let st = Command::new("qpdf")
        .arg("--object-streams=generate")
        .arg(&plain)
        .arg(path)
        .status()
        .expect("qpdf nel runner");
    assert!(matches!(st.code(), Some(0 | 3)), "qpdf: {st:?}");
    let pdf = fs::read(path).unwrap();
    let objstm = pdf.windows(7).filter(|w| w == b"/ObjStm").count();
    assert!(objstm > 64, "fixture: {objstm} object stream");
}

/// DOCX minimo (namespace sostituiti da URN segnaposto, come nei test del prescan).
fn minimal_docx(path: &Path) {
    let mut z = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let parts: [(&str, &str); 3] = [
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="urn:test:ooxml/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="urn:test:ooxml/package/2006/relationships"><Relationship Id="rId1" Type="urn:test:ooxml/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="urn:test:ooxml/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Ciao</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ];
    for (name, data) in parts {
        z.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(data.as_bytes()).unwrap();
    }
    z.finish().unwrap();
}

/// TEST-15 (RISK-001 di risk_audit_21): `parse-parent run` con `op: parse` dà `ok` e in `data`
/// tutti i sei campi dello schema unico. DOCX: `none` e 0. PDF `objstm-many`:
/// `objstm_count` > 64, `xref_streams` ≥ 1, `objstm_decompressed_bytes` < 512 MiB,
/// `decompress_work_bytes` ≤ B = 2 × 512 MiB + 2 × `xref_streams` × 64 MiB (e mai sotto la
/// riespansione e le due passate delle xref stream del ramo non cifrato).
#[test]
fn parse_child_data_schema() {
    // Fixture e PDFium prima di ogni lancio: un errore di fixture non si confonde con il rosso.
    let docx = fixture("schema.docx");
    minimal_docx(&docx);
    let pdf = fixture("objstm-many.pdf");
    objstm_many_pdf(&pdf);
    let _ = pdfium_dir();

    let (enc, nums) = six_fields(&parse_data("tw-schema-docx", &docx));
    assert_eq!((enc.as_str(), nums), ("none", [0, 0, 0, 0, 0]));

    let (enc, [embedded, objstm, objstm_bytes, xref, work]) =
        six_fields(&parse_data("tw-schema-pdf", &pdf));
    assert_eq!(enc, "none");
    assert_eq!(embedded, 0);
    assert!(objstm > 64, "objstm_count {objstm}");
    assert!(xref >= 1, "xref_streams {xref}");
    assert!(
        objstm_bytes > 0 && objstm_bytes < 512 * MIB,
        "objstm_decompressed_bytes {objstm_bytes}"
    );
    let b = 2 * 512 * MIB + 2 * xref * 64 * MIB;
    assert!(work <= b, "decompress_work_bytes {work} oltre B {b}");
    let floor = 2 * objstm_bytes + 2 * xref * 64 * MIB;
    assert!(
        work >= floor,
        "decompress_work_bytes {work} sotto il minimo {floor}"
    );
}
