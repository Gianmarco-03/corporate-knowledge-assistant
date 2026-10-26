//! Test d'integrazione dei driver di misura `budget` e `overhead` di `parse-parent` (fase 1.4,
//! piano 01.4-01, D-01 e D-02), lanciati come processo come nel worker.
//!
//! - D-01: il driver `budget` non lascia `selftest-input-<tag>` nella radice dei job, né sui
//!   percorsi d'errore (CSV senza `hw.json` accanto, `S02_BUDGET_TOTAL_BYTES` non numerico) né
//!   su quello riuscito; i codici d'uscita restano quelli di prima (1 e 0).
//! - D-02: il driver `overhead` scrive `overhead.csv` con la colonna `status` e una riga per
//!   ogni ripetizione di entrambi i modi; un errore di PDFium in-process (input non PDF) è un
//!   risultato registrato, non interrompe il driver e non cambia il codice d'uscita.
//!
//! Solo Linux, nel runner, dentro `bash tools/session.sh with 01-4-01 --wait 1800 -- …`:
//! `bash tools/session.sh oneshot "$CKA_RUN_ID" -- cargo test -p s02-parser --test drivers`.
//! Il processo del test non imposta variabili d'ambiente globali: ogni variabile (`S02_BIN_DIR`,
//! `S02_WORK_ROOT`, `S02_RESULTS_RAW`, budget fisso, `PDFIUM_DIR`) passa al solo figlio. Ogni
//! test ha una radice dei job propria, così i test paralleli non si vedono. Le cartelle stanno
//! sotto `/work/tmp` (`S02_TEST_BASE` per spostarle).
//!
//! Il guasto end-to-end si ottiene con un input non PDF passato a `overhead`, senza alcun gancio
//! nel driver di misura (hard_rules 5 del piano).
#![cfg(target_os = "linux")]

// Modifica: Claude (test-write 01.4-01) · 2026-10-22 · creazione: test rossi dei driver budget e overhead (gap-closure 1.4)
// Modifica: Claude (gsd-executor 01.6-07) · 2026-10-26 · test deterministici sotto carico e pulizia delle cartelle temporanee (gap-closure 1.6, D-11)

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

use s02_parser::isolate::Limits;
use serde_json::json;

const MIB: u64 = 1024 * 1024;

/// Bin `parse-parent` del runner (percorso letterale, come in `tests/isolation.rs`).
const PARSE_PARENT: &str = "/target/debug/parse-parent";

/// Budget fisso del padre (il runner non ha `memory.max`), letto solo da `parse-parent`.
const BUDGET_TOTAL_VAR: &str = "S02_BUDGET_TOTAL_BYTES";
const PARENT_RESERVE_VAR: &str = "S02_PARENT_RESERVE_BYTES";

/// Header di `budget.csv`, copiato da `parse-parent.rs` (invariato dalla 1.4).
const BUDGET_HEADER: &str = "requested_concurrency,jobs,max_admitted,completed,oom_kill_delta,parent_vm_hwm_kb,parent_alive";
/// Header nuovo di `overhead.csv` (D-03 della fase 1.4).
const OVERHEAD_HEADER_NEW: &str =
    "mode,concurrency,run,docs,wall_ms,spawn_init_ms,docs_per_s,status";

/// Manifest hardware minimo valido, richiesto accanto a ogni CSV da `common::csv::Writer`.
const HW_JSON: &str = r#"{"timestamp_utc":"2026-10-19T00:00:00Z"}"#;

/// `parse-child` finto (argv: --input <in> --out <out>): scrive `result.txt` e riesce.
const PARSE_CHILD_SCRIPT: &str = r#"#!/bin/sh
# parse-child finto dei test dei driver (argv: --input <in> --out <out>).
printf 'ok' > "$4/result.txt"
printf '{"status":"ok","pages":null,"output":null,"data":null}\n'
"#;

// ---- pulizia della radice di processo e delle cartelle di caso (piano 01.6-07, D-11) --------

/// Prefisso della radice di processo di questo binario: `<root>/s02-drivers-<pid>`.
const ROOT_PREFIX: &str = "s02-drivers-";

/// Radice di processo da rimuovere all'uscita, letta dalla callback di `atexit`.
static PROCESS_ROOT: OnceLock<PathBuf> = OnceLock::new();

unsafe extern "C" {
    fn atexit(cb: extern "C" fn()) -> std::ffi::c_int;
}

/// Callback di `atexit`: rimuove la radice di processo. Non panica (nessun `unwrap`, errore
/// ignorato) e legge solo uno stato `'static`.
extern "C" fn remove_process_root() {
    if let Some(p) = PROCESS_ROOT.get() {
        let _ = fs::remove_dir_all(p);
    }
}

/// `name` è la radice orfana di un'altra esecuzione di questo binario: esattamente
/// `<prefix><pid>` con `<pid>` decimale canonico (niente zeri iniziali, niente suffissi),
/// diverso da `own_pid` e non vivo secondo `alive`. Funzione pura (TEST-8 del test design).
fn is_orphan_root(name: &str, prefix: &str, own_pid: u32, alive: impl Fn(u32) -> bool) -> bool {
    let Some(rest) = name.strip_prefix(prefix) else {
        return false;
    };
    let Ok(pid) = rest.parse::<u32>() else {
        return false;
    };
    if pid.to_string() != rest {
        return false;
    }
    pid != own_pid && !alive(pid)
}

fn pid_alive(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

/// Pota sotto `root` le radici orfane di questo binario lasciate da run interrotte. Tocca solo
/// i nomi `s02-drivers-<pid>` con pid diverso dal corrente e assente da `/proc`. `/proc` è
/// quello del container: la potatura è sicura perché i binari di test di una sessione girano
/// in sequenza dentro un solo `oneshot`, e la prenotazione di `session.sh` esclude sessioni
/// concorrenti sul volume `work-tmp` (hard rule 6 del piano).
fn prune_orphan_roots(root: &Path) {
    let own = std::process::id();
    let Ok(rd) = fs::read_dir(root) else {
        return;
    };
    for e in rd.flatten() {
        let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
        if is_dir
            && e.file_name()
                .to_str()
                .is_some_and(|n| is_orphan_root(n, ROOT_PREFIX, own, pid_alive))
        {
            let _ = fs::remove_dir_all(e.path());
        }
    }
}

/// Guardia di una cartella di caso: la rimuove al `drop`, anche quando il test va in panico
/// (sul modello di `SelftestInput` di `parse-parent.rs`). Chi la usa la lega a un nome
/// (`let d = case_dir(..)`) e la tiene viva fino alla fine del test.
#[must_use = "la guardia rimuove la cartella al drop: va legata a un nome, non a `_`"]
struct CaseDir(PathBuf);

impl Drop for CaseDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl std::ops::Deref for CaseDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

// ---- ambiente dei test --------------------------------------------------------------------

/// Radice dei test: `bin/` (`fake-child`, `parse-child` finto), `raw/fixtures/not-a-pdf.pdf`,
/// `limits.json`.
fn base() -> &'static PathBuf {
    static BASE: OnceLock<PathBuf> = OnceLock::new();
    BASE.get_or_init(|| {
        let root = std::env::var_os("S02_TEST_BASE")
            .map_or_else(|| PathBuf::from("/work/tmp"), PathBuf::from);
        prune_orphan_roots(&root);
        let base = root.join(format!("{ROOT_PREFIX}{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        for d in ["bin", "raw/fixtures"] {
            fs::create_dir_all(base.join(d)).unwrap();
        }
        let base = base.canonicalize().unwrap();
        if PROCESS_ROOT.set(base.clone()).is_ok() {
            // SAFETY: `atexit` della libc, già collegata da std; registrata una sola volta (dentro
            // `OnceLock`). La callback non panica e legge solo `PROCESS_ROOT`, uno stato
            // `'static` mai rilasciato.
            unsafe {
                atexit(remove_process_root);
            }
        }

        let bin = base.join("bin");
        fs::copy(env!("CARGO_BIN_EXE_fake-child"), bin.join("fake-child")).unwrap();
        let script = bin.join("parse-child");
        fs::write(&script, PARSE_CHILD_SCRIPT).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

        fs::write(base.join("raw/fixtures/not-a-pdf.pdf"), b"not a pdf\n").unwrap();

        // Valori di `limits()` di `tests/isolation.rs`, prenotazione a 64 MiB; scritto come in
        // `write_limits`.
        let limits = Limits {
            address_space_bytes: 256 * MIB,
            memory_reservation_bytes: 64 * MIB,
            cpu_seconds: 10,
            wall_timeout: Duration::from_secs(10),
            file_size_bytes: 16 * MIB,
            max_open_files: 64,
            out_quota_bytes: MIB,
        };
        fs::write(
            base.join("limits.json"),
            serde_json::to_vec(&json!({ "default": limits })).unwrap(),
        )
        .unwrap();
        base
    })
}

/// Cartella di un caso: `<base>/<name>/jobs` (radice dei job, vuota) e `<base>/<name>/out`,
/// dentro una guardia che la rimuove al `drop`.
fn case_dir(name: &str) -> CaseDir {
    let d = base().join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("jobs")).unwrap();
    fs::create_dir_all(d.join("out")).unwrap();
    CaseDir(d)
}

/// Nomi delle voci di una cartella.
fn entries(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("lettura di {}: {e}", dir.display()))
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// Budget fisso del figlio: 1 GiB totale, 256 MiB di riserva del padre.
fn fixed_budget() -> Vec<(&'static str, String)> {
    vec![
        (BUDGET_TOTAL_VAR, (1024 * MIB).to_string()),
        (PARENT_RESERVE_VAR, (256 * MIB).to_string()),
    ]
}

/// `parse-parent <args>` con le radici del caso e le variabili in più; stdin nullo.
fn run_parent(args: &[&str], work_root: &Path, extra: &[(&str, String)]) -> Output {
    assert_eq!(
        env!("CARGO_BIN_EXE_parse-parent"),
        PARSE_PARENT,
        "test pensato per il runner (CARGO_TARGET_DIR=/target)"
    );
    let b = base();
    let mut c = Command::new("/target/debug/parse-parent");
    c.args(args)
        .env("S02_BIN_DIR", b.join("bin"))
        .env("S02_WORK_ROOT", work_root)
        .env("S02_RESULTS_RAW", b.join("raw"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra {
        c.env(k, v);
    }
    c.output().unwrap()
}

/// Cartella di `libpdfium.so` (copiata da `tests/isolation.rs`): `S02_TEST_PDFIUM_DIR`,
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

fn stderr_of(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn assert_work_root_empty(jobs: &Path, o: &Output) {
    let left = entries(jobs);
    assert!(
        left.is_empty(),
        "radice dei job non vuota dopo il driver: voci {left:?}; stderr del driver: {}",
        stderr_of(o)
    );
}

// ---- D-01: budget -------------------------------------------------------------------------

/// CSV senza `hw.json` accanto: `csv_writer` fallisce dopo la creazione dell'input del
/// selftest. Exit 1 come prima, e nessun residuo nella radice dei job.
#[test]
fn budget_csv_error_cleans_work_root() {
    let d = case_dir("budget-csv-error");
    let jobs = d.join("jobs");
    let out = d.join("out").join("budget.csv");
    let o = run_parent(
        &[
            "budget",
            "--concurrency",
            "1",
            "--jobs",
            "1",
            "--reserve-fraction",
            "0.25",
            "--run-tag",
            "dr1a0001",
            "--out",
            out.to_str().unwrap(),
        ],
        &jobs,
        &fixed_budget(),
    );
    let err = stderr_of(&o);
    assert_eq!(o.status.code(), Some(1), "exit di budget; stderr: {err}");
    assert!(err.starts_with("parse-parent:"), "stderr: {err}");
    assert_work_root_empty(&jobs, &o);
}

/// `S02_BUDGET_TOTAL_BYTES` non numerico: `budget()` fallisce dopo `csv_writer`. Exit 1 come
/// prima, e nessun residuo nella radice dei job.
#[test]
fn budget_budget_error_cleans_work_root() {
    let d = case_dir("budget-budget-error");
    let jobs = d.join("jobs");
    fs::write(d.join("out").join("hw.json"), HW_JSON).unwrap();
    let out = d.join("out").join("budget.csv");
    let o = run_parent(
        &[
            "budget",
            "--concurrency",
            "1",
            "--jobs",
            "1",
            "--reserve-fraction",
            "0.25",
            "--run-tag",
            "dr1b0001",
            "--out",
            out.to_str().unwrap(),
        ],
        &jobs,
        &[
            (BUDGET_TOTAL_VAR, "abc".to_owned()),
            (PARENT_RESERVE_VAR, (256 * MIB).to_string()),
        ],
    );
    let err = stderr_of(&o);
    assert_eq!(o.status.code(), Some(1), "exit di budget; stderr: {err}");
    assert!(err.contains("S02_BUDGET_TOTAL_BYTES"), "stderr: {err}");
    assert_work_root_empty(&jobs, &o);
}

/// Percorso riuscito (regressione, verde anche sulla base): exit 0, CSV con l'header di
/// `budget` e una riga, radice dei job vuota.
#[test]
fn budget_success_cleans_work_root() {
    let d = case_dir("budget-success");
    let jobs = d.join("jobs");
    fs::write(d.join("out").join("hw.json"), HW_JSON).unwrap();
    let out = d.join("out").join("budget.csv");
    let o = run_parent(
        &[
            "budget",
            "--concurrency",
            "1",
            "--jobs",
            "1",
            "--reserve-fraction",
            "0.25",
            "--run-tag",
            "dr1c0001",
            "--out",
            out.to_str().unwrap(),
        ],
        &jobs,
        &fixed_budget(),
    );
    let err = stderr_of(&o);
    assert_eq!(o.status.code(), Some(0), "exit di budget; stderr: {err}");
    let csv = fs::read_to_string(&out).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.first().copied(), Some(BUDGET_HEADER), "csv: {csv}");
    assert_eq!(lines.len(), 2, "header più una riga; csv: {csv}");
    assert_work_root_empty(&jobs, &o);
}

// ---- D-02: overhead -----------------------------------------------------------------------

/// Input non PDF: i job `child-per-doc` (parse-child finto) riescono, ogni ripetizione
/// in-process fallisce con `FormatError`. Il driver esce 0 con 8 righe di 8 campi, le righe
/// in-process hanno `status` `error:pdfium_format` e i campi di tempo vuoti.
#[test]
fn overhead_in_process_errors_keep_all_rows() {
    let d = case_dir("overhead-in-process-errors");
    let jobs = d.join("jobs");
    fs::write(d.join("out").join("hw.json"), HW_JSON).unwrap();
    let out = d.join("out").join("overhead.csv");
    let input = base().join("raw/fixtures/not-a-pdf.pdf");
    let limits = base().join("limits.json");
    let mut extra = fixed_budget();
    extra.push(("PDFIUM_DIR", pdfium_dir().to_string_lossy().into_owned()));
    let o = run_parent(
        &[
            "overhead",
            "--input",
            input.to_str().unwrap(),
            "--runs",
            "2",
            "--concurrency",
            "1,2",
            "--run-tag",
            "dr2a0001",
            "--out",
            out.to_str().unwrap(),
            "--limits",
            limits.to_str().unwrap(),
        ],
        &jobs,
        &extra,
    );
    let err = stderr_of(&o);
    let csv = fs::read_to_string(&out).unwrap_or_default();
    assert_eq!(
        o.status.code(),
        Some(0),
        "exit di overhead; stderr: {err}\ncsv:\n{csv}"
    );

    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some(OVERHEAD_HEADER_NEW),
        "header; csv:\n{csv}"
    );
    let rows: Vec<Vec<&str>> = lines[1..].iter().map(|l| l.split(',').collect()).collect();
    assert_eq!(rows.len(), 8, "2 modi × 2 concorrenze × 2 run; csv:\n{csv}");
    for r in &rows {
        assert_eq!(r.len(), 8, "campi della riga {r:?}");
    }

    let cpd: Vec<&Vec<&str>> = rows.iter().filter(|r| r[0] == "child-per-doc").collect();
    let inp: Vec<&Vec<&str>> = rows.iter().filter(|r| r[0] == "in-process-mutex").collect();
    assert_eq!(cpd.len(), 4, "righe child-per-doc; csv:\n{csv}");
    assert_eq!(inp.len(), 4, "righe in-process-mutex; csv:\n{csv}");
    for r in &cpd {
        assert_eq!(r[7], "ok", "status child-per-doc {r:?}");
        assert!(!r[4].is_empty(), "wall_ms child-per-doc {r:?}");
    }
    let expected = [("1", "1"), ("1", "2"), ("2", "1"), ("2", "2")];
    for (r, (n, run)) in inp.iter().zip(expected) {
        assert_eq!((r[1], r[2]), (n, run), "concorrenza e run {r:?}");
        assert_eq!(r[7], "error:pdfium_format", "status in-process {r:?}");
        assert_eq!(
            (r[4], r[5], r[6]),
            ("", "", ""),
            "campi di tempo vuoti in errore {r:?}"
        );
    }
    assert_work_root_empty(&jobs, &o);
}

// ---- D-11 (piano 01.6-07): guardia delle cartelle di caso ------------------------------------

// Modifica: Claude (test-write 01.6-07) · 2026-10-26 · test rosso della guardia `CaseDir` col panico (gap-closure 1.6, D-11)

/// La guardia di una cartella di caso la rimuove anche quando il test va in panico: la
/// cartella esiste dentro la chiusura prima del `panic!` (il test non passa a vuoto), e dopo
/// l'unwind non esiste più. Il percorso esce dalla chiusura copiato in una variabile esterna.
#[test]
fn case_dir_guard_cleans_on_panic() {
    let mut seen: Option<PathBuf> = None;
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let guard: CaseDir = case_dir("panic-guard");
        assert!(
            guard.0.is_dir(),
            "cartella di caso non creata: {}",
            guard.0.display()
        );
        seen = Some(guard.0.clone());
        panic!("panico voluto del test della guardia");
    }));
    assert!(res.is_err(), "la chiusura doveva andare in panico");
    let path = seen.expect("percorso della cartella di caso non registrato prima del panico");
    assert!(
        !path.exists(),
        "cartella di caso rimasta dopo il panico: {}",
        path.display()
    );
}

/// Predicato della potatura (TEST-8): solo `s02-drivers-<pid>` canonico, con pid diverso dal
/// corrente e morto. Nomi con suffissi, vuoti, non numerici, con zeri iniziali o di un altro
/// binario non si potano mai.
#[test]
fn orphan_root_predicate_accepts_only_dead_foreign_canonical_pids() {
    let own = 4242;
    let dead = |_: u32| false;
    let alive = |_: u32| true;
    assert!(is_orphan_root("s02-drivers-999999", ROOT_PREFIX, own, dead));
    assert!(
        !is_orphan_root("s02-drivers-4242", ROOT_PREFIX, own, dead),
        "pid corrente"
    );
    assert!(
        !is_orphan_root("s02-drivers-999999", ROOT_PREFIX, own, alive),
        "pid vivo"
    );
    for name in [
        "s02-drivers-123-keep",
        "s02-drivers-",
        "s02-drivers-abc",
        "s02-drivers-0012",
        "s02-drivers-+12",
        "s02-drivers-99999999999",
        "s02-isolation-999999",
        "x-s02-drivers-999999",
    ] {
        assert!(
            !is_orphan_root(name, ROOT_PREFIX, own, dead),
            "potato a torto: {name}"
        );
    }
}
