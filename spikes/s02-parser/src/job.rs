//! Job JSON tipizzato e allowlist operazione → bin (contratto W17, punto 1).
//!
//! Un job è `{"job_id","op","input","out_dir","params"}` (al più 64 KiB, nessun campo in
//! più). [`parse`] lo decodifica e valida dominio dei parametri, `job_id` e presenza di
//! `out_dir`; [`resolve`] applica l'allowlist chiusa (bin fisso, argv ed env costruiti qui),
//! canonicalizza i percorsi dentro le radici ammesse e verifica che il bin esista.
//! Nessun nome di bin né argomento libero arriva dal chiamante: `params` ha uno schema chiuso
//! per operazione e ogni valore finisce nell'argv come singolo `OsString`, senza shell.
//!
//! Radici e cartelle le legge solo il padre dall'ambiente: `S02_BIN_DIR` (default
//! `/usr/local/bin`), `S02_WORK_ROOT` (default `/work`), `S02_RESULTS_RAW` (default
//! `/results/raw`). La stringa `out_dir` del job resta sempre `/results/raw/<area>/<job_id>`:
//! il padre la rimappa sotto `S02_RESULTS_RAW`, cosa che serve solo ai test nel runner.
use std::ffi::OsString;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

/// Dimensione massima di un job JSON.
pub const MAX_JOB_BYTES: usize = 64 * 1024;
/// Prefisso fisso della stringa `out_dir` nel job.
pub const OUT_DIR_PREFIX: &str = "/results/raw/";

/// Codice di `job_error` del contratto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobErrorCode {
    UnknownOp,
    InvalidParams,
    PathNotAllowed,
    ChildMissing,
    BudgetExceeded,
}

impl JobErrorCode {
    /// Nome del contratto (uguale alla variante).
    pub fn as_str(self) -> &'static str {
        match self {
            JobErrorCode::UnknownOp => "UnknownOp",
            JobErrorCode::InvalidParams => "InvalidParams",
            JobErrorCode::PathNotAllowed => "PathNotAllowed",
            JobErrorCode::ChildMissing => "ChildMissing",
            JobErrorCode::BudgetExceeded => "BudgetExceeded",
        }
    }
}

/// Job non valido: nessun processo lanciato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobError {
    pub code: JobErrorCode,
    pub detail: String,
}

impl JobError {
    pub fn new(code: JobErrorCode, detail: impl Into<String>) -> JobError {
        JobError {
            code,
            detail: detail.into(),
        }
    }

    fn invalid(detail: impl Into<String>) -> JobError {
        JobError::new(JobErrorCode::InvalidParams, detail)
    }

    fn path(detail: impl Into<String>) -> JobError {
        JobError::new(JobErrorCode::PathNotAllowed, detail)
    }

    /// Riga `job_error` del contratto (stampata da `parse-parent`, che esce 2).
    pub fn to_line(&self, job_id: Option<&str>) -> String {
        serde_json::json!({
            "job_id": job_id,
            "status": "job_error",
            "code": self.code.as_str(),
            "detail": self.detail,
        })
        .to_string()
    }
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "job_error {}: {}", self.code.as_str(), self.detail)
    }
}

impl std::error::Error for JobError {}

/// Bin figli ammessi: registro chiuso. Il nome è sempre un letterale del codice, mai un
/// valore del chiamante; `isolate` costruisce il `Command` con un letterale per variante.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChildBin {
    ParseChild,
    Extract,
    Locator,
    Ocr,
    FakeChild,
}

impl ChildBin {
    pub(crate) fn program(self) -> &'static str {
        match self {
            ChildBin::ParseChild => "parse-child",
            ChildBin::Extract => "extract",
            ChildBin::Locator => "locator",
            ChildBin::Ocr => "ocr",
            ChildBin::FakeChild => "fake-child",
        }
    }
}

/// Modi del figlio di prova `fake-child` (op `selftest`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SelftestMode {
    Alloc,
    SpawnGrandchild,
    Flood,
    ArgvEcho,
    Sleep,
    Write,
}

impl SelftestMode {
    fn as_str(self) -> &'static str {
        match self {
            SelftestMode::Alloc => "alloc",
            SelftestMode::SpawnGrandchild => "spawn-grandchild",
            SelftestMode::Flood => "flood",
            SelftestMode::ArgvEcho => "argv-echo",
            SelftestMode::Sleep => "sleep",
            SelftestMode::Write => "write",
        }
    }

    /// I modi numerici vogliono un intero decimale come `arg`.
    fn numeric_arg(self) -> bool {
        matches!(
            self,
            SelftestMode::Alloc | SelftestMode::Flood | SelftestMode::Sleep | SelftestMode::Write
        )
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct NoParams {}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocateParams {
    excerpt_file: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderParams {
    dpi: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelftestParams {
    mode: SelftestMode,
    #[serde(default)]
    arg: Option<String>,
}

/// Operazione in allowlist con i suoi parametri (enum serde etichettato: `op` + `params`).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", content = "params", deny_unknown_fields)]
enum Op {
    #[serde(rename = "parse")]
    Parse(NoParams),
    #[serde(rename = "extract_pdf")]
    ExtractPdf(NoParams),
    #[serde(rename = "locate")]
    Locate(LocateParams),
    #[serde(rename = "ocr_render")]
    OcrRender(RenderParams),
    #[serde(rename = "ocr_tesseract")]
    OcrTesseract(NoParams),
    #[serde(rename = "selftest")]
    Selftest(SelftestParams),
}

/// Nomi delle operazioni in allowlist.
pub const OPS: [&str; 6] = [
    "parse",
    "extract_pdf",
    "locate",
    "ocr_render",
    "ocr_tesseract",
    "selftest",
];

impl Op {
    fn name(&self) -> &'static str {
        match self {
            Op::Parse(_) => "parse",
            Op::ExtractPdf(_) => "extract_pdf",
            Op::Locate(_) => "locate",
            Op::OcrRender(_) => "ocr_render",
            Op::OcrTesseract(_) => "ocr_tesseract",
            Op::Selftest(_) => "selftest",
        }
    }

    fn bin(&self) -> ChildBin {
        match self {
            Op::Parse(_) => ChildBin::ParseChild,
            Op::ExtractPdf(_) => ChildBin::Extract,
            Op::Locate(_) => ChildBin::Locator,
            Op::OcrRender(_) | Op::OcrTesseract(_) => ChildBin::Ocr,
            Op::Selftest(_) => ChildBin::FakeChild,
        }
    }

    /// Le op con output vogliono `out_dir`; le altre lo vogliono `null`.
    fn has_output(&self) -> bool {
        matches!(self, Op::Parse(_) | Op::ExtractPdf(_) | Op::OcrRender(_))
    }

    fn descendants(&self) -> u32 {
        match self {
            Op::OcrTesseract(_) => 1,
            _ => 0,
        }
    }
}

/// Forma del job sul filo: campi chiusi, `op` e `params` decodificati dopo.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JobWire {
    job_id: String,
    op: String,
    input: String,
    #[serde(default)]
    out_dir: Option<String>,
    params: serde_json::Value,
}

/// Job decodificato e validato nei campi che non toccano il filesystem.
#[derive(Debug, Clone)]
pub struct Job {
    job_id: String,
    op: Op,
    input: String,
    out_dir: Option<OutDir>,
}

#[derive(Debug, Clone)]
struct OutDir {
    area: String,
}

impl Job {
    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    /// Nome dell'operazione (`parse`, `selftest`, …).
    pub fn op(&self) -> &'static str {
        self.op.name()
    }
}

/// `job_id` valido: `[a-z0-9-]{1,64}`.
pub fn valid_job_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.bytes().all(is_id_byte)
}

fn valid_area(area: &str) -> bool {
    (1..=32).contains(&area.len()) && area.bytes().all(is_id_byte)
}

fn is_id_byte(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'
}

/// `job_id` del job, se leggibile, anche quando il job non è valido (per la riga `job_error`).
pub fn job_id_hint(bytes: &[u8]) -> Option<String> {
    if bytes.len() > MAX_JOB_BYTES {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    v.get("job_id")?.as_str().map(str::to_owned)
}

/// Decodifica un job JSON (al più 64 KiB, `deny_unknown_fields`).
pub fn parse(bytes: &[u8]) -> Result<Job, JobError> {
    if bytes.len() > MAX_JOB_BYTES {
        return Err(JobError::invalid(format!(
            "job_too_large: {} byte oltre {MAX_JOB_BYTES}",
            bytes.len()
        )));
    }
    let wire: JobWire =
        serde_json::from_slice(bytes).map_err(|e| JobError::invalid(format!("json: {e}")))?;
    if !valid_job_id(&wire.job_id) {
        return Err(JobError::invalid("job_id fuori da [a-z0-9-]{1,64}"));
    }
    if !OPS.contains(&wire.op.as_str()) {
        return Err(JobError::new(JobErrorCode::UnknownOp, "op fuori allowlist"));
    }
    let op: Op = serde_json::from_value(serde_json::json!({
        "op": wire.op,
        "params": wire.params,
    }))
    .map_err(|e| JobError::invalid(format!("params: {e}")))?;
    validate_params(&op)?;

    let out_dir = match (op.has_output(), wire.out_dir) {
        (true, Some(s)) => Some(parse_out_dir(&s, &wire.job_id)?),
        (false, None) => None,
        (true, None) => return Err(JobError::invalid("out_dir richiesto dall'op")),
        (false, Some(_)) => return Err(JobError::invalid("out_dir deve essere null per l'op")),
    };
    Ok(Job {
        job_id: wire.job_id,
        op,
        input: wire.input,
        out_dir,
    })
}

fn validate_params(op: &Op) -> Result<(), JobError> {
    match op {
        Op::OcrRender(p) if !(72..=600).contains(&p.dpi) => {
            Err(JobError::invalid("dpi fuori da 72..=600"))
        }
        Op::Selftest(p) if p.mode.numeric_arg() => match &p.arg {
            Some(a) if !a.is_empty() && a.bytes().all(|b| b.is_ascii_digit()) && a.len() <= 19 => {
                Ok(())
            }
            _ => Err(JobError::invalid("arg intero decimale richiesto dal modo")),
        },
        Op::Selftest(p) if p.mode == SelftestMode::SpawnGrandchild && p.arg.is_some() => {
            Err(JobError::invalid("spawn-grandchild non ha arg"))
        }
        _ => Ok(()),
    }
}

/// `out_dir` solo nella forma esatta `/results/raw/<area>/<job_id>`, senza canonicalizzare.
fn parse_out_dir(s: &str, job_id: &str) -> Result<OutDir, JobError> {
    let rest = s
        .strip_prefix(OUT_DIR_PREFIX)
        .ok_or_else(|| JobError::path("out_dir_form"))?;
    let (area, id) = rest
        .split_once('/')
        .ok_or_else(|| JobError::path("out_dir_form"))?;
    if !valid_area(area) || id != job_id {
        return Err(JobError::path("out_dir_form"));
    }
    Ok(OutDir {
        area: area.to_owned(),
    })
}

/// Cartelle lette dal padre.
#[derive(Debug, Clone)]
pub(crate) struct Roots {
    pub(crate) bin_dir: PathBuf,
    pub(crate) work_root: PathBuf,
    pub(crate) results_raw: PathBuf,
}

impl Roots {
    pub(crate) fn from_env() -> Roots {
        let var = |k: &str, d: &str| {
            std::env::var_os(k)
                .filter(|v| !v.is_empty())
                .map_or_else(|| PathBuf::from(d), PathBuf::from)
        };
        Roots {
            bin_dir: var("S02_BIN_DIR", "/usr/local/bin"),
            work_root: var("S02_WORK_ROOT", "/work"),
            results_raw: var("S02_RESULTS_RAW", "/results/raw"),
        }
    }

    /// Radici canonicalizzate entro cui stanno `input` ed `excerpt_file`.
    fn allowed(&self) -> Vec<PathBuf> {
        [
            Path::new("/data"),
            self.results_raw.as_path(),
            self.work_root.as_path(),
        ]
        .iter()
        .filter_map(|p| std::fs::canonicalize(p).ok())
        .collect()
    }
}

/// Figlio risolto dall'allowlist: bin fisso, argv ed env costruiti dal padre.
///
/// I campi pubblici sono in sola lettura per contratto: `run_child` li confronta con la
/// copia sigillata da `resolve` e rifiuta uno spec modificato. I campi privati impediscono
/// di costruire uno spec fuori da `resolve`.
#[derive(Debug, Clone)]
pub struct ChildSpec {
    pub job_id: String,
    pub bin: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(OsString, OsString)>,
    /// Discendenti ammessi dall'op, oltre al figlio diretto.
    pub descendants: u32,
    pub(crate) sealed: Sealed,
}

/// Copia di `resolve` più i percorsi che `run_child` ricontrolla prima dello spawn.
#[derive(Debug, Clone)]
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) struct Sealed {
    pub(crate) job_id: String,
    pub(crate) bin_kind: ChildBin,
    pub(crate) bin: PathBuf,
    pub(crate) args: Vec<OsString>,
    pub(crate) env: Vec<(OsString, OsString)>,
    pub(crate) descendants: u32,
    pub(crate) bin_dir: PathBuf,
    pub(crate) job_dir: PathBuf,
    pub(crate) results_raw: PathBuf,
    /// `(cartella dell'area, out_dir)` reali, se l'op ha output.
    pub(crate) out: Option<(PathBuf, PathBuf)>,
    /// Percorsi canonici di `input` ed `excerpt_file` con `(dev, inode)` visti da `resolve`,
    /// ricontrollati con `lstat` subito prima dello spawn.
    pub(crate) checked: Vec<(PathBuf, Option<(u64, u64)>)>,
}

impl ChildSpec {
    /// `true` se i campi pubblici sono quelli costruiti da `resolve`.
    pub(crate) fn untampered(&self) -> bool {
        let s = &self.sealed;
        self.job_id == s.job_id
            && self.bin == s.bin
            && self.args == s.args
            && self.env == s.env
            && self.descendants == s.descendants
            && s.bin == s.bin_dir.join(s.bin_kind.program())
    }

    /// `out_dir` reale (rimappato sotto `S02_RESULTS_RAW`), se l'op ha output.
    pub fn out_dir(&self) -> Option<&Path> {
        self.sealed.out.as_ref().map(|(_, o)| o.as_path())
    }
}

/// Applica l'allowlist e valida parametri e percorsi.
pub fn resolve(job: &Job) -> Result<ChildSpec, JobError> {
    resolve_in(job, &Roots::from_env())
}

pub(crate) fn resolve_in(job: &Job, roots: &Roots) -> Result<ChildSpec, JobError> {
    let allowed = roots.allowed();
    let input = admitted_file(&job.input, &allowed, "input")?;
    let mut checked = vec![(input.clone(), identity(&input))];

    let out = job.out_dir.as_ref().map(|o| {
        let area_dir = roots.results_raw.join(&o.area);
        let out_dir = area_dir.join(&job.job_id);
        (area_dir, out_dir)
    });
    let out_os = || {
        out.as_ref()
            .map(|(_, o)| o.clone().into_os_string())
            .unwrap_or_default()
    };
    let inp = input.clone().into_os_string();

    let mut env: Vec<(OsString, OsString)> = vec![
        // Il bin si cerca solo nella cartella dei bin ammessi (nome letterale del registro).
        ("PATH".into(), roots.bin_dir.clone().into_os_string()),
        (
            "PDFIUM_DIR".into(),
            std::env::var_os("PDFIUM_DIR")
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "/opt/pdfium/lib".into()),
        ),
    ];
    let args: Vec<OsString> = match &job.op {
        Op::Parse(_) => vec!["--input".into(), inp, "--out".into(), out_os()],
        Op::ExtractPdf(_) => vec![
            "pdf".into(),
            "--input".into(),
            inp,
            "--out".into(),
            out_os(),
        ],
        Op::Locate(p) => {
            let ex = admitted_file(&p.excerpt_file, &allowed, "excerpt_file")?;
            checked.push((ex.clone(), identity(&ex)));
            vec![
                "locate".into(),
                "--input".into(),
                inp,
                "--excerpt-file".into(),
                ex.into_os_string(),
            ]
        }
        Op::OcrRender(p) => vec![
            "render".into(),
            "--input".into(),
            inp,
            "--dpi".into(),
            p.dpi.to_string().into(),
            "--out".into(),
            out_os(),
        ],
        Op::OcrTesseract(_) => {
            env.push(("OMP_THREAD_LIMIT".into(), "1".into()));
            vec!["tesseract".into(), "--image".into(), inp]
        }
        Op::Selftest(p) => {
            let mut v: Vec<OsString> = vec![p.mode.as_str().into()];
            if let Some(a) = &p.arg {
                v.push(a.into());
            }
            v
        }
    };

    let bin_kind = job.op.bin();
    let bin = roots.bin_dir.join(bin_kind.program());
    if !std::fs::metadata(&bin).is_ok_and(|m| m.is_file()) {
        return Err(JobError::new(
            JobErrorCode::ChildMissing,
            format!("bin {} assente in S02_BIN_DIR", bin_kind.program()),
        ));
    }

    let sealed = Sealed {
        job_id: job.job_id.clone(),
        bin_kind,
        bin: bin.clone(),
        args: args.clone(),
        env: env.clone(),
        descendants: job.op.descendants(),
        bin_dir: roots.bin_dir.clone(),
        job_dir: roots.work_root.join(&job.job_id),
        results_raw: roots.results_raw.clone(),
        out,
        checked,
    };
    Ok(ChildSpec {
        job_id: job.job_id.clone(),
        bin,
        args,
        env,
        descendants: job.op.descendants(),
        sealed,
    })
}

/// Percorso assoluto, canonicalizzato, file regolare sotto una radice ammessa.
fn admitted_file(raw: &str, allowed: &[PathBuf], what: &str) -> Result<PathBuf, JobError> {
    let p = Path::new(raw);
    if !p.is_absolute() {
        return Err(JobError::path(format!("{what}_not_absolute")));
    }
    let canon =
        std::fs::canonicalize(p).map_err(|_| JobError::path(format!("{what}_not_found")))?;
    if !allowed.iter().any(|root| canon.starts_with(root)) {
        return Err(JobError::path(format!("{what}_outside_roots")));
    }
    if !std::fs::metadata(&canon).is_ok_and(|m| m.is_file()) {
        return Err(JobError::path(format!("{what}_not_file")));
    }
    Ok(canon)
}

/// `(dev, inode)` del file, per riconoscere un componente sostituito dopo `resolve`.
#[cfg(unix)]
fn identity(p: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(p).ok().map(|m| (m.dev(), m.ino()))
}

#[cfg(not(unix))]
fn identity(_p: &Path) -> Option<(u64, u64)> {
    None
}

/// Componenti di un percorso assoluto, dalla radice al percorso stesso (`/a`, `/a/b`, …).
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) fn ancestors_from_root(p: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut cur = PathBuf::new();
    for c in p.components() {
        match c {
            Component::RootDir | Component::Prefix(_) => cur.push(c.as_os_str()),
            Component::Normal(n) => {
                cur.push(n);
                out.push(cur.clone());
            }
            Component::CurDir | Component::ParentDir => {
                cur.push(c.as_os_str());
                out.push(cur.clone());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(op: &str, out_dir: Option<&str>, params: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "job_id": "j-1",
            "op": op,
            "input": "/data/x.pdf",
            "out_dir": out_dir,
            "params": params,
        }))
        .unwrap()
    }

    #[test]
    fn known_ops_parse_and_unknown_is_unknown_op() {
        assert!(
            parse(&wire(
                "parse",
                Some("/results/raw/a/j-1"),
                serde_json::json!({})
            ))
            .is_ok()
        );
        let e = parse(&wire("exec", None, serde_json::json!({}))).unwrap_err();
        assert_eq!(e.code, JobErrorCode::UnknownOp);
    }

    #[test]
    fn out_dir_must_be_exact_form() {
        for bad in [
            "/results/raw/a/j-2",
            "/results/raw/a/j-1/",
            "/results/raw/A/j-1",
            "/results/raw//j-1",
            "/results/raw/a/b/j-1",
        ] {
            let e = parse(&wire("parse", Some(bad), serde_json::json!({}))).unwrap_err();
            assert_eq!(e.code, JobErrorCode::PathNotAllowed, "{bad}");
        }
    }

    #[test]
    fn numeric_selftest_modes_need_decimal_arg() {
        for (mode, arg) in [
            ("alloc", None),
            ("sleep", Some("1s")),
            ("write", Some("-1")),
        ] {
            let p = match arg {
                Some(a) => serde_json::json!({"mode": mode, "arg": a}),
                None => serde_json::json!({"mode": mode}),
            };
            let e = parse(&wire("selftest", None, p)).unwrap_err();
            assert_eq!(e.code, JobErrorCode::InvalidParams, "{mode}");
        }
    }

    #[test]
    fn job_error_line_has_contract_fields() {
        let line = JobError::new(JobErrorCode::ChildMissing, "x").to_line(Some("j-1"));
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["status"], "job_error");
        assert_eq!(v["code"], "ChildMissing");
        assert_eq!(v["job_id"], "j-1");
    }

    #[test]
    fn ancestors_start_at_root() {
        let a = ancestors_from_root(Path::new("/a/b/c"));
        assert_eq!(
            a,
            vec![
                PathBuf::from("/a"),
                PathBuf::from("/a/b"),
                PathBuf::from("/a/b/c")
            ]
        );
    }
}
