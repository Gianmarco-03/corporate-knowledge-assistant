//! Test OOM controllato (piano 01-18, Task 3; soglia M11; TEST-10, TEST-11 di `test_design_12`).
//!
//! Il test `oom_kills_child_not_parent` è `#[ignore]`: gira solo nel container a memoria nota
//! lanciato da `measure/run_oom.sh` (`--memory 256m`, rete spenta, read-only, uid 10001).
//! Usa solo l'API pubblica di 01-07 (`job::{parse, resolve}`, `isolate::{run_child, Limits,
//! Budget}`); il processo del test fa le veci di `parse-parent`, come in `tests/isolation.rs`.
//! Stampa una sola riga `OOM_RESULT {json}` con l'esito tristato di M11: `verificato`,
//! `fallito` (riga stampata, poi `panic!`) o `non_esercitato` (con `motivo`).
//!
//! Le funzioni pure `classify_m11` e `result_json` decidono l'esito e la forma della riga;
//! i test non ignorati in coda al file ne fissano il contratto senza container.
#![cfg(target_os = "linux")]

// Modifica: Claude (gsd-executor 01.6-07) · 2026-10-26 · test deterministici sotto carico e pulizia delle cartelle temporanee (gap-closure 1.6, D-11)

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use nix::errno::Errno;
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::Pid;
use s02_parser::isolate::{Budget, Limits, RunReport, run_child};
use s02_parser::job::{self, JobError, JobErrorCode};
use s02_parser::protocol::ChildOutcome;
use serde_json::{Value, json};

const MIB: u64 = 1024 * 1024;
const JOB_ID: &str = "oom-alloc";
const WORK_ROOT: &str = "/work/tmp/oom";
const RAW_ROOT: &str = "/work/tmp/oom-raw";
const RESERVATION_BYTES: u64 = 64 * MIB;
const PARENT_RESERVE_BYTES: u64 = 64 * MIB;

// ---- pulizia delle radici del test (piano 01.6-07, D-11) ------------------------------------

unsafe extern "C" {
    fn atexit(cb: extern "C" fn()) -> std::ffi::c_int;
}

/// Callback di `atexit`: rimuove `WORK_ROOT` e `RAW_ROOT`. Non panica (errori ignorati) e usa
/// solo costanti `'static`.
extern "C" fn remove_test_roots() {
    let _ = fs::remove_dir_all(WORK_ROOT);
    let _ = fs::remove_dir_all(RAW_ROOT);
}

/// Rimuove i residui di run precedenti e registra una volta la pulizia all'uscita del binario.
/// Si chiama all'inizio del test, prima di ogni ritorno `non_esercitato` e di `create_dir_all`.
fn clean_roots_now_and_at_exit() {
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    remove_test_roots();
    REGISTERED.call_once(|| {
        // SAFETY: `atexit` della libc, già collegata da std; registrata una sola volta (dentro
        // `Once`). La callback non panica e non usa stato di Rust non `'static`.
        unsafe {
            atexit(remove_test_roots);
        }
    });
}

/// Chiavi della riga `OOM_RESULT`, nell'ordine del piano (Task 3). `run_oom.sh` e la verifica
/// del piano leggono questi nomi.
const RESULT_KEYS: [&str; 15] = [
    "esito",
    "motivo",
    "memory_max_bytes",
    "memory_reservation_bytes",
    "oom_group",
    "oom_kill_delta",
    "outcome",
    "kind",
    "detail",
    "exit_signal",
    "oom_adj_set",
    "leftover_procs",
    "job_dir_removed",
    "unreaped_children",
    "parent_oom_score_adj",
];

/// Esito tristato di M11.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Esito {
    Verificato,
    Fallito,
    NonEsercitato,
}

impl Esito {
    fn as_str(self) -> &'static str {
        match self {
            Esito::Verificato => "verificato",
            Esito::Fallito => "fallito",
            Esito::NonEsercitato => "non_esercitato",
        }
    }
}

/// Quanto il test osserva dopo il ritorno di `run_child`.
#[derive(Debug, Clone, PartialEq)]
struct Observed {
    memory_max_bytes: u64,
    memory_reservation_bytes: u64,
    oom_group: Option<String>,
    /// Differenza del contatore `oom_kill` di `memory.events` letta dal test (prima e dopo).
    oom_kill_delta: u64,
    /// `ok` | `failed` | `quarantined`.
    outcome: String,
    kind: Option<String>,
    detail: Option<String>,
    exit_signal: Option<i32>,
    oom_adj_set: bool,
    leftover_procs: u32,
    job_dir_removed: bool,
    unreaped_children: u32,
    parent_oom_score_adj: i32,
}

/// Classificazione di M11 (Task 1 e Task 3 del piano).
///
/// - `oom_kill_delta` = 0 → `NonEsercitato`, con motivo non vuoto, qualunque sia il resto;
/// - OOM avvenuto (`oom_kill_delta` ≥ 1) e tutti i controlli veri → `Verificato`, motivo vuoto:
///   `memory_reservation_bytes` < `memory_max_bytes`, `outcome` = `failed`, `kind` =
///   `ResourceLimit`, `detail` = `cgroup_oom`, `exit_signal` = 9, `oom_adj_set`,
///   `leftover_procs` = 0, `unreaped_children` = 0, `job_dir_removed`,
///   `parent_oom_score_adj` = 0;
/// - OOM avvenuto e un controllo falso → `Fallito`, con motivo che contiene il nome (chiave
///   della riga `OOM_RESULT`) del primo controllo violato, nell'ordine sopra.
fn classify_m11(obs: &Observed) -> (Esito, String) {
    if obs.oom_kill_delta == 0 {
        return (
            Esito::NonEsercitato,
            "oom_kill_delta=0: il cgroup non è andato in OOM".to_owned(),
        );
    }
    let checks: [(&str, bool); 10] = [
        (
            "memory_reservation_bytes",
            obs.memory_reservation_bytes < obs.memory_max_bytes,
        ),
        ("outcome", obs.outcome == "failed"),
        ("kind", obs.kind.as_deref() == Some("ResourceLimit")),
        ("detail", obs.detail.as_deref() == Some("cgroup_oom")),
        ("exit_signal", obs.exit_signal == Some(9)),
        ("oom_adj_set", obs.oom_adj_set),
        ("leftover_procs", obs.leftover_procs == 0),
        ("unreaped_children", obs.unreaped_children == 0),
        ("job_dir_removed", obs.job_dir_removed),
        ("parent_oom_score_adj", obs.parent_oom_score_adj == 0),
    ];
    match checks.iter().find(|(_, ok)| !ok) {
        Some((key, _)) => (Esito::Fallito, format!("controllo violato: {key}")),
        None => (Esito::Verificato, String::new()),
    }
}

/// Oggetto JSON della riga `OOM_RESULT`: sempre tutte le chiavi di [`RESULT_KEYS`] e nessun'altra.
///
/// Con `obs` = `None` (precondizione mancata prima del lancio: subreaper, cgroup illimitato,
/// job non ammesso, API non pubblica) i campi diversi da `esito` e `motivo` valgono `null`.
/// Con `obs` presente i tipi sono quelli che la verifica del piano confronta con `===`:
/// numeri per byte, delta, segnale, contatori e `oom_score_adj`; booleani per `oom_adj_set` e
/// `job_dir_removed`; stringhe (o `null`) per `outcome`, `kind`, `detail`, `oom_group`.
fn result_json(esito: Esito, motivo: &str, obs: Option<&Observed>) -> Value {
    let mut v = json!({"esito": esito.as_str(), "motivo": motivo});
    let fields = match obs {
        Some(o) => json!({
            "memory_max_bytes": o.memory_max_bytes,
            "memory_reservation_bytes": o.memory_reservation_bytes,
            "oom_group": o.oom_group,
            "oom_kill_delta": o.oom_kill_delta,
            "outcome": o.outcome,
            "kind": o.kind,
            "detail": o.detail,
            "exit_signal": o.exit_signal,
            "oom_adj_set": o.oom_adj_set,
            "leftover_procs": o.leftover_procs,
            "job_dir_removed": o.job_dir_removed,
            "unreaped_children": o.unreaped_children,
            "parent_oom_score_adj": o.parent_oom_score_adj,
        }),
        None => Value::Object(
            RESULT_KEYS
                .iter()
                .filter(|k| !matches!(**k, "esito" | "motivo"))
                .map(|k| ((*k).to_owned(), Value::Null))
                .collect(),
        ),
    };
    if let (Some(dst), Value::Object(src)) = (v.as_object_mut(), fields) {
        dst.extend(src);
    }
    v
}

fn emit(esito: Esito, motivo: &str, obs: Option<&Observed>) {
    println!("OOM_RESULT {}", result_json(esito, motivo, obs));
}

/// Precondizione mancata: riga `non_esercitato` con il motivo, il test non dichiara nulla.
fn not_exercised(motivo: &str) {
    emit(Esito::NonEsercitato, motivo, None);
}

fn read_trim(path: &str) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_owned())
}

/// `memory.max` del cgroup in byte; `None` se vale `max` o non è leggibile.
fn memory_max() -> Option<u64> {
    read_trim("/sys/fs/cgroup/memory.max").and_then(|v| v.parse().ok())
}

/// Contatore `oom_kill` di `memory.events`.
fn oom_kills() -> Option<u64> {
    let text = fs::read_to_string("/sys/fs/cgroup/memory.events").ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix("oom_kill "))
        .and_then(|v| v.trim().parse().ok())
}

/// Figli non raccolti del test: `waitpid(-1, WNOHANG)` finché dà `ECHILD`.
fn unreaped_children() -> u32 {
    let mut n = 0;
    loop {
        match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)) {
            Err(Errno::ECHILD) => return n,
            Ok(WaitStatus::StillAlive) => return n + 1,
            Ok(_) => n += 1,
            Err(_) => return n + 1,
        }
    }
}

/// Nessuna cartella di job sotto la radice di lavoro.
fn job_dirs_removed(work_root: &Path) -> bool {
    fs::read_dir(work_root)
        .map(|rd| rd.filter_map(Result::ok).all(|e| !e.path().is_dir()))
        .unwrap_or(false)
}

fn outcome_fields(o: &ChildOutcome) -> (String, Option<String>, Option<String>) {
    match o {
        ChildOutcome::Failed { kind, detail } => (
            "failed".to_owned(),
            Some(kind.as_str().to_owned()),
            Some(detail.clone()),
        ),
        other => (other.status().to_owned(), None, None),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "solo nel container a memoria bassa di 01-18"]
async fn oom_kills_child_not_parent() {
    // (0) Radici pulite all'avvio e all'uscita del binario (piano 01.6-07, D-11).
    clean_roots_now_and_at_exit();

    // (1) Il test fa le veci di `parse-parent`: subreaper.
    if nix::sys::prctl::set_child_subreaper(true).is_err() {
        return not_exercised("subreaper_unavailable");
    }

    // (2) Cgroup del container: memoria nota, gruppo OOM, contatore iniziale.
    let Some(max_bytes) = memory_max() else {
        return not_exercised("cgroup_illimitato");
    };
    let oom_group = read_trim("/sys/fs/cgroup/memory.oom.group");
    let Some(oom_before) = oom_kills() else {
        return not_exercised("memory_events_illeggibile");
    };

    // (3) Radici del padre e job `selftest alloc` di 2 × memory.max.
    let work_root = PathBuf::from(WORK_ROOT);
    let raw = PathBuf::from(RAW_ROOT);
    let fixtures = raw.join("fixtures");
    fs::create_dir_all(&work_root).expect("radice di lavoro del test");
    fs::create_dir_all(&fixtures).expect("cartella degli input del test");
    let input = fixtures.join("in.bin");
    fs::write(&input, b"oom").expect("input del job");
    let fake = PathBuf::from(env!("CARGO_BIN_EXE_fake-child"));
    let bin_dir = fake.parent().expect("cartella di fake-child").to_path_buf();
    // SAFETY: impostate una sola volta all'inizio dell'unico test del file, prima di ogni uso
    // del lanciatore; nessun altro thread del processo legge l'ambiente in parallelo.
    unsafe {
        std::env::set_var("S02_BIN_DIR", &bin_dir);
        std::env::set_var("S02_WORK_ROOT", &work_root);
        std::env::set_var("S02_RESULTS_RAW", &raw);
    }

    let alloc_mib = 2 * max_bytes.div_ceil(MIB);
    let job_bytes = serde_json::to_vec(&json!({
        "job_id": JOB_ID,
        "op": "selftest",
        "input": input.to_str().expect("percorso UTF-8"),
        "out_dir": null,
        "params": {"mode": "alloc", "arg": alloc_mib.to_string()},
    }))
    .expect("job JSON");
    let job = match job::parse(&job_bytes) {
        Ok(j) => j,
        Err(e) => {
            return not_exercised(&format!("api_non_pubblica job::parse {}", e.code.as_str()));
        }
    };
    let spec = match job::resolve(&job) {
        Ok(s) => s,
        Err(e) => {
            return not_exercised(&format!(
                "api_non_pubblica job::resolve {}",
                e.code.as_str()
            ));
        }
    };
    let limits = Limits {
        address_space_bytes: (alloc_mib + 256) * MIB,
        memory_reservation_bytes: RESERVATION_BYTES,
        cpu_seconds: 30,
        wall_timeout: Duration::from_secs(30),
        file_size_bytes: MIB,
        max_open_files: 64,
        out_quota_bytes: MIB,
    };
    let budget = Budget::fixed(4 * max_bytes, PARENT_RESERVE_BYTES);

    // (4) Lancio: l'unico limite che interviene deve essere quello del cgroup.
    let report: RunReport = match run_child(&spec, &limits, &budget).await {
        Ok(r) => r,
        Err(e) => {
            let motivo = match e.downcast_ref::<JobError>() {
                Some(je) if je.code == JobErrorCode::BudgetExceeded => "non_ammesso".to_owned(),
                Some(je) => format!("run_child_job_error {}", je.code.as_str()),
                None => "run_child_errore".to_owned(),
            };
            return not_exercised(&motivo);
        }
    };

    // (5) Dopo il ritorno: contatore OOM, reaping, cartelle, oom_score_adj del test.
    let oom_after = oom_kills().unwrap_or(oom_before);
    let (outcome, kind, detail) = outcome_fields(&report.outcome);
    let obs = Observed {
        memory_max_bytes: max_bytes,
        memory_reservation_bytes: limits.memory_reservation_bytes,
        oom_group,
        oom_kill_delta: oom_after.saturating_sub(oom_before),
        outcome,
        kind,
        detail,
        exit_signal: report.exit_signal,
        oom_adj_set: report.oom_adj_set,
        leftover_procs: report.leftover_procs,
        job_dir_removed: !work_root.join(JOB_ID).exists() && job_dirs_removed(&work_root),
        unreaped_children: unreaped_children(),
        parent_oom_score_adj: read_trim("/proc/self/oom_score_adj")
            .and_then(|v| v.parse().ok())
            .unwrap_or(i32::MIN),
    };
    let (esito, motivo) = classify_m11(&obs);
    emit(esito, &motivo, Some(&obs));
    assert_ne!(
        esito,
        Esito::Fallito,
        "M11 fallito ({motivo}): {obs:?} {report:?}"
    );
}

// ---- contratto di classify_m11 e result_json (TEST-10, TEST-11) ---------------------------

/// Osservazione di un OOM riuscito secondo M11 (cgroup da 256 MiB come in `run_oom.sh`).
fn verified() -> Observed {
    Observed {
        memory_max_bytes: 256 * MIB,
        memory_reservation_bytes: RESERVATION_BYTES,
        oom_group: Some("0".to_owned()),
        oom_kill_delta: 1,
        outcome: "failed".to_owned(),
        kind: Some("ResourceLimit".to_owned()),
        detail: Some("cgroup_oom".to_owned()),
        exit_signal: Some(9),
        oom_adj_set: true,
        leftover_procs: 0,
        job_dir_removed: true,
        unreaped_children: 0,
        parent_oom_score_adj: 0,
    }
}

/// Una mutazione per controllo di M11, con la chiave che il motivo deve nominare.
fn violations() -> Vec<(&'static str, Observed)> {
    let v = verified;
    vec![
        ("memory_reservation_bytes", {
            let mut o = v();
            o.memory_reservation_bytes = o.memory_max_bytes;
            o
        }),
        ("outcome", {
            let mut o = v();
            o.outcome = "ok".to_owned();
            o.kind = None;
            o.detail = None;
            o
        }),
        ("kind", {
            let mut o = v();
            o.kind = Some("Crashed".to_owned());
            o
        }),
        ("detail", {
            let mut o = v();
            o.detail = Some("rlimit_as".to_owned());
            o
        }),
        ("exit_signal", {
            let mut o = v();
            o.exit_signal = Some(6);
            o
        }),
        ("exit_signal", {
            let mut o = v();
            o.exit_signal = None;
            o
        }),
        ("oom_adj_set", {
            let mut o = v();
            o.oom_adj_set = false;
            o
        }),
        ("leftover_procs", {
            let mut o = v();
            o.leftover_procs = 1;
            o
        }),
        ("unreaped_children", {
            let mut o = v();
            o.unreaped_children = 1;
            o
        }),
        ("job_dir_removed", {
            let mut o = v();
            o.job_dir_removed = false;
            o
        }),
        ("parent_oom_score_adj", {
            let mut o = v();
            o.parent_oom_score_adj = 1000;
            o
        }),
    ]
}

#[test]
fn m11_all_checks_with_oom_is_verificato() {
    let (esito, motivo) = classify_m11(&verified());
    assert_eq!(esito, Esito::Verificato, "motivo: {motivo}");
    assert!(
        motivo.is_empty(),
        "verificato senza motivo, ottenuto {motivo:?}"
    );
}

#[test]
fn m11_more_than_one_oom_kill_is_still_verificato() {
    let mut o = verified();
    o.oom_kill_delta = 3;
    assert_eq!(classify_m11(&o).0, Esito::Verificato);
}

#[test]
fn m11_oom_group_does_not_decide_the_outcome() {
    for g in [None, Some("1".to_owned())] {
        let mut o = verified();
        o.oom_group = g;
        assert_eq!(classify_m11(&o).0, Esito::Verificato, "{o:?}");
    }
}

#[test]
fn m11_no_oom_kill_is_non_esercitato_with_reason() {
    let mut o = verified();
    o.oom_kill_delta = 0;
    let (esito, motivo) = classify_m11(&o);
    assert_eq!(esito, Esito::NonEsercitato);
    assert!(!motivo.is_empty(), "non_esercitato vuole un motivo");
}

#[test]
fn m11_without_oom_never_verificato_nor_fallito() {
    // Senza OOM il test non dichiara nulla, anche se i controlli sono falsi (TEST-11).
    for (key, mut o) in violations() {
        o.oom_kill_delta = 0;
        let (esito, motivo) = classify_m11(&o);
        assert_eq!(esito, Esito::NonEsercitato, "{key}: {o:?}");
        assert!(!motivo.is_empty(), "{key}: motivo vuoto");
    }
}

#[test]
fn m11_each_violated_check_with_oom_is_fallito_naming_the_check() {
    for (key, o) in violations() {
        let (esito, motivo) = classify_m11(&o);
        assert_eq!(esito, Esito::Fallito, "{key}: {o:?}");
        assert!(
            motivo.contains(key),
            "{key}: il motivo deve nominare il controllo, ottenuto {motivo:?}"
        );
    }
}

#[test]
fn m11_reservation_not_below_cgroup_is_fallito() {
    // Il job deve essere ammesso con una prenotazione sotto memory.max (rischio residuo di 01-07).
    let mut o = verified();
    o.memory_reservation_bytes = o.memory_max_bytes + 1;
    let (esito, motivo) = classify_m11(&o);
    assert_eq!(esito, Esito::Fallito);
    assert!(motivo.contains("memory_reservation_bytes"), "{motivo:?}");
}

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v
        .as_object()
        .expect("OOM_RESULT è un oggetto JSON")
        .keys()
        .cloned()
        .collect();
    k.sort();
    k
}

fn expected_keys() -> Vec<String> {
    let mut k: Vec<String> = RESULT_KEYS.iter().map(|s| (*s).to_owned()).collect();
    k.sort();
    k
}

#[test]
fn result_json_verificato_has_exact_keys_and_types_checked_by_the_plan() {
    let o = verified();
    let v = result_json(Esito::Verificato, "", Some(&o));
    assert_eq!(keys(&v), expected_keys());
    assert_eq!(v["esito"], json!("verificato"));
    assert_eq!(v["memory_max_bytes"], json!(256 * MIB));
    assert_eq!(v["memory_reservation_bytes"], json!(RESERVATION_BYTES));
    assert_eq!(v["oom_group"], json!("0"));
    assert_eq!(v["oom_kill_delta"], json!(1));
    assert_eq!(v["outcome"], json!("failed"));
    assert_eq!(v["kind"], json!("ResourceLimit"));
    assert_eq!(v["detail"], json!("cgroup_oom"));
    assert_eq!(v["exit_signal"], json!(9));
    assert_eq!(v["oom_adj_set"], json!(true));
    assert_eq!(v["leftover_procs"], json!(0));
    assert_eq!(v["job_dir_removed"], json!(true));
    assert_eq!(v["unreaped_children"], json!(0));
    assert_eq!(v["parent_oom_score_adj"], json!(0));
    assert!(
        v["memory_reservation_bytes"].as_u64() < v["memory_max_bytes"].as_u64(),
        "prenotazione sotto memory.max nella riga"
    );
}

#[test]
fn result_json_fallito_carries_reason_and_observed_values() {
    let mut o = verified();
    o.leftover_procs = 2;
    let v = result_json(Esito::Fallito, "leftover_procs", Some(&o));
    assert_eq!(keys(&v), expected_keys());
    assert_eq!(v["esito"], json!("fallito"));
    assert_eq!(v["motivo"], json!("leftover_procs"));
    assert_eq!(v["leftover_procs"], json!(2));
}

#[test]
fn result_json_precondition_is_non_esercitato_with_nulls() {
    for motivo in [
        "subreaper_unavailable",
        "cgroup_illimitato",
        "non_ammesso",
        "api_non_pubblica Limits",
    ] {
        let v = result_json(Esito::NonEsercitato, motivo, None);
        assert_eq!(keys(&v), expected_keys(), "{motivo}");
        assert_eq!(v["esito"], json!("non_esercitato"));
        assert_eq!(v["motivo"], json!(motivo));
        for k in RESULT_KEYS
            .iter()
            .filter(|k| !matches!(**k, "esito" | "motivo"))
        {
            assert!(
                v[*k].is_null(),
                "{motivo}: {k} deve essere null, è {}",
                v[*k]
            );
        }
    }
}

#[test]
fn result_json_is_one_line_without_environment_values() {
    let v = result_json(Esito::NonEsercitato, "cgroup_illimitato", None);
    let line = format!("OOM_RESULT {v}");
    assert!(!line.contains('\n'), "una sola riga");
    for secret_name in [
        "CKA_SPIKES_ENV",
        "DATABASE_URL",
        "TYPESAFE_API_KEY",
        "PATH=",
    ] {
        assert!(!line.contains(secret_name), "{secret_name} nella riga");
    }
}
