//! Bin `locator` (piano 01-09, spike S-02 / A-10 locator di citazione).
//!
//! Sottocomandi:
//! - `locate --input <md|docx> --excerpt-file <f>`: l'unico che apre un documento non fidato;
//!   gira solo come figlio isolato, via job `op: locate` del contratto di 01-07 (argv
//!   costruito da `job::resolve`), stampa una riga con `protocol::emit` e non scrive file;
//! - `roundtrip prepare|check`: bin normali che compongono job `locate` e ne leggono i
//!   `RunReport`, senza mai aprire DOCX o Markdown;
//! - `rects`: tre stadi di verifica dei rettangoli sui PDF sintetici di `gen_pages.py`;
//! - `offsets`: conversione scalar → UTF-16 dei vettori della fixture, solo campi `got_*`.
mod canon;
mod docx;
mod md;
mod pdfrect;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use s02_parser::prescan::{Caps, Rejection};
use s02_parser::protocol::{ChildOutcome, Kind, emit};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::canon::{CharRange, Projection, SectionStep};

/// Tetto di lettura di un Markdown in `locate`.
pub(crate) const MAX_MD_BYTES: u64 = 8 * 1024 * 1024;
/// Tetto di lettura del file excerpt in `locate`.
pub(crate) const MAX_EXCERPT_BYTES: u64 = 64 * 1024;

/// Radice dei documenti sintetici nel worker (mount `./data:/data:ro` di compose).
const SYNTHETIC_ROOT: &str = "/data/corpus/synthetic";
/// Manifest del corpus (piano 01-03) nel worker; `S02_CORPUS_MANIFEST` lo sostituisce.
const CORPUS_MANIFEST: &str = "/data/corpus/manifest.json";
/// File esistente fuori dalle radici ammesse: `excerpt_file` del job negativo. Il padre lo
/// rifiuta in `resolve` (`PathNotAllowed`) e non lo legge mai.
const NEG_EXCERPT: &str = "/etc/hostname";

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

/// Coppie `--flag valore` di `rest`: ogni flag di `names` esattamente una volta, nient'altro.
fn flags<const N: usize>(rest: &[OsString], names: [&str; N]) -> Result<[PathBuf; N], String> {
    if rest.len() != 2 * N {
        return Err(format!("attesi esattamente i flag {names:?}"));
    }
    let mut got: [Option<PathBuf>; N] = std::array::from_fn(|_| None);
    for pair in rest.chunks(2) {
        let flag = pair[0].to_str().unwrap_or("");
        let i = names
            .iter()
            .position(|n| *n == flag)
            .ok_or_else(|| format!("flag sconosciuto: {flag:?}"))?;
        if got[i].replace(PathBuf::from(&pair[1])).is_some() {
            return Err(format!("flag ripetuto: {flag}"));
        }
    }
    let mut out: [PathBuf; N] = std::array::from_fn(|_| PathBuf::new());
    for (i, g) in got.into_iter().enumerate() {
        out[i] = g.ok_or_else(|| format!("flag mancante: {}", names[i]))?;
    }
    Ok(out)
}

/// Argomenti senza il nome del programma. Argomenti non riconosciuti → `Err`.
pub(crate) fn parse_cli(args: Vec<OsString>) -> Result<Cmd, String> {
    let Some((sub, rest)) = args.split_first() else {
        return Err("sottocomando assente: locate | roundtrip | rects | offsets".into());
    };
    match sub.to_str() {
        Some("locate") => match rest {
            [a, input, b, ex] if a == "--input" && b == "--excerpt-file" => Ok(Cmd::Locate {
                input: PathBuf::from(input),
                excerpt_file: PathBuf::from(ex),
            }),
            _ => Err("locate: attesi esattamente --input <file> --excerpt-file <file>".into()),
        },
        Some("roundtrip") => match rest.split_first() {
            Some((p, r)) if p == "prepare" => {
                let [samples, excerpt_dir, jobs_out, run_tag] =
                    flags(r, ["--samples", "--excerpt-dir", "--jobs-out", "--run-tag"])?;
                Ok(Cmd::RoundtripPrepare {
                    samples,
                    excerpt_dir,
                    jobs_out,
                    run_tag: run_tag.to_string_lossy().into_owned(),
                })
            }
            Some((c, r)) if c == "check" => {
                let [samples, reports, out_dir] =
                    flags(r, ["--samples", "--reports", "--out-dir"])?;
                Ok(Cmd::RoundtripCheck {
                    samples,
                    reports,
                    out_dir,
                })
            }
            _ => Err("roundtrip: atteso prepare | check".into()),
        },
        Some("rects") => {
            let [cases, pdf_dir, out] = flags(rest, ["--cases", "--pdf-dir", "--out"])?;
            Ok(Cmd::Rects {
                cases,
                pdf_dir,
                out,
            })
        }
        Some("offsets") => {
            let [vectors, out] = flags(rest, ["--vectors", "--out"])?;
            Ok(Cmd::Offsets { vectors, out })
        }
        _ => Err(format!("sottocomando sconosciuto: {sub:?}")),
    }
}

// ---- locate (figlio) -------------------------------------------------------------------

fn rejection(r: Rejection) -> ChildOutcome {
    match r {
        Rejection::Failed(kind, detail) => ChildOutcome::Failed { kind, detail },
        Rejection::Quarantined(reasons) => ChildOutcome::Quarantined { reasons },
    }
}

/// Legge al più `cap` byte; oltre → `failed/ResourceLimit`.
fn read_capped(path: &Path, cap: u64, what: &str) -> Result<Vec<u8>, ChildOutcome> {
    let mut buf = Vec::new();
    read_bounded(path, cap, what, |chunk| buf.extend_from_slice(chunk))?;
    Ok(buf)
}

/// Lettura a blocchi di 64 KiB, al più `cap` byte (`take(cap + 1)` per vedere lo sforamento):
/// ogni blocco va a `sink`; oltre il tetto → `failed/ResourceLimit`.
fn read_bounded(
    path: &Path,
    cap: u64,
    what: &str,
    mut sink: impl FnMut(&[u8]),
) -> Result<(), ChildOutcome> {
    let file = std::fs::File::open(path)
        .map_err(|e| ChildOutcome::failed(Kind::Corrupt, format!("{what}_open: {e}")))?;
    let mut r = file.take(cap + 1);
    let mut chunk = vec![0u8; 64 * 1024];
    let mut n = 0u64;
    loop {
        let k = r
            .read(&mut chunk)
            .map_err(|e| ChildOutcome::failed(Kind::Corrupt, format!("{what}_read: {e}")))?;
        if k == 0 {
            return Ok(());
        }
        n += k as u64;
        if n > cap {
            return Err(ChildOutcome::failed(
                Kind::ResourceLimit,
                format!("{what}_over_{cap}_bytes"),
            ));
        }
        sink(&chunk[..k]);
    }
}

fn utf8(bytes: Vec<u8>, what: &str) -> Result<String, ChildOutcome> {
    String::from_utf8(bytes)
        .map_err(|_| ChildOutcome::failed(Kind::Corrupt, format!("{what}_not_utf8")))
}

/// Sha256 dei byte del file, letti in streaming entro `cap`.
fn file_sha256(path: &Path, cap: u64) -> Result<String, ChildOutcome> {
    let mut h = Sha256::new();
    read_bounded(path, cap, "input", |chunk| h.update(chunk))?;
    Ok(canon::hex(&h.finalize()))
}

/// Proiezione `cka-text-v1` del documento e sha256 dei suoi byte. È l'unico punto che legge
/// un MD o un DOCX; `run_locate` la chiama due volte, per cercare e per risolvere.
fn project(input: &Path) -> Result<(&'static str, Projection, String), ChildOutcome> {
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("md") => {
            let bytes = read_capped(input, MAX_MD_BYTES, "input")?;
            let sha = canon::sha256_hex(&bytes);
            let text = utf8(bytes, "input")?;
            Ok(("md", canon::sections(&md::parse(&text)), sha))
        }
        Some("docx") => {
            let caps = Caps::default();
            // Prescan prima di tutto (dentro `parse_file`), poi l'hash dei byte.
            let parsed = docx::parse_file(input, &caps).map_err(rejection)?;
            let sha = file_sha256(input, caps.docx_max_total_bytes)?;
            Ok(("docx", canon::sections(&parsed), sha))
        }
        _ => Err(ChildOutcome::failed(
            Kind::Unsupported,
            "format_not_md_or_docx",
        )),
    }
}

/// Sottocomando `locate` (figlio): esito da stampare con `protocol::emit`. `ok` con
/// `data = {projection, format, doc_sha256, matches: [{section_path, char_range,
/// section_text_sha256, reextracted}], unsupported}`; mai il testo completo del documento.
pub(crate) fn run_locate(input: &Path, excerpt_file: &Path) -> ChildOutcome {
    match locate_inner(input, excerpt_file) {
        Ok(o) | Err(o) => o,
    }
}

fn locate_inner(input: &Path, excerpt_file: &Path) -> Result<ChildOutcome, ChildOutcome> {
    let excerpt = utf8(
        read_capped(excerpt_file, MAX_EXCERPT_BYTES, "excerpt")?,
        "excerpt",
    )?;
    let (format, found_in, sha) = project(input)?;
    let locators = canon::locate(&found_in, &sha, &excerpt);
    // `resolve` riparte dai byte del file: seconda lettura e seconda proiezione, percorso di
    // codice distinto dalla ricerca.
    let (_, fresh, fresh_sha) = project(input)?;
    let matches: Vec<Value> = locators
        .iter()
        .map(|l| {
            let again = canon::resolve(l, &fresh, &fresh_sha).ok();
            json!({
                "section_path": l.section_path,
                "char_range": l.char_range,
                "section_text_sha256": l.section_text_sha256,
                "reextracted": again,
            })
        })
        .collect();
    Ok(ChildOutcome::Ok {
        pages: None,
        output: None,
        data: Some(json!({
            "projection": canon::PROJECTION,
            "format": format,
            "doc_sha256": sha,
            "matches": matches,
            "unsupported": found_in.unsupported,
        })),
    })
}

// ---- roundtrip prepare -----------------------------------------------------------------

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

/// Riga di un job sul filo, nell'ordine dei campi del contratto di 01-07:
/// `{"job_id":"…","op":"locate","input":"…","out_dir":null,"params":{"excerpt_file":"…"}}`.
#[derive(Serialize)]
struct LocateJob<'a> {
    job_id: &'a str,
    op: &'static str,
    input: &'a str,
    out_dir: Option<&'a str>,
    params: LocateParams<'a>,
}

#[derive(Serialize)]
struct LocateParams<'a> {
    excerpt_file: &'a str,
}

fn job_line(job_id: &str, input: &str, excerpt_file: &str) -> String {
    serde_json::to_string(&LocateJob {
        job_id,
        op: "locate",
        input,
        out_dir: None,
        params: LocateParams { excerpt_file },
    })
    .expect("serializzazione del job")
}

fn valid_run_tag(t: &str) -> bool {
    t.len() == 8
        && t.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// Un job `op: locate` per campione e formato (`job_id` = `<sample_id>-<format>-<run_tag>`,
/// `out_dir: null`, input `/data/corpus/synthetic/<doc_id>.<format>`), più il job negativo
/// `neg-path-<run_tag>` con `excerpt_file` fuori dalle radici ammesse. `run_tag` fuori da
/// `[a-z0-9]{8}`, `job_id` non valido o duplicato → `Err` (exit 2).
pub(crate) fn prepare_jobs(
    samples: &[Sample],
    excerpt_dir: &str,
    run_tag: &str,
) -> Result<Prepared, String> {
    if !valid_run_tag(run_tag) {
        return Err(format!("run_tag {run_tag:?} fuori da [a-z0-9]{{8}}"));
    }
    let dir = excerpt_dir.trim_end_matches('/');
    let mut seen = BTreeSet::new();
    let mut check_id = |id: &str| -> Result<(), String> {
        if !s02_parser::job::valid_job_id(id) {
            return Err(format!("job_id {id:?} fuori da [a-z0-9-]{{1,64}}"));
        }
        if !seen.insert(id.to_owned()) {
            return Err(format!("job_id duplicato: {id}"));
        }
        Ok(())
    };
    let mut jobs = Vec::new();
    let mut excerpts = Vec::new();
    for (i, s) in samples.iter().enumerate() {
        if !s02_parser::job::valid_job_id(&s.sample_id) {
            return Err(format!("sample_id {:?} non valido", s.sample_id));
        }
        // Il primo file ha spazi e caratteri non ASCII nel nome: il percorso attraversa
        // padre e figlio senza shell e senza perdere byte.
        let name = if i == 0 {
            format!("{} estratto città.txt", s.sample_id)
        } else {
            format!("{}.txt", s.sample_id)
        };
        let path = format!("{dir}/{name}");
        for f in &s.formats {
            if f != "md" && f != "docx" {
                return Err(format!("{}: formato {f:?} non ammesso", s.sample_id));
            }
            let id = format!("{}-{f}-{run_tag}", s.sample_id);
            check_id(&id)?;
            let input = format!("{SYNTHETIC_ROOT}/{}.{f}", s.doc_id);
            jobs.push(job_line(&id, &input, &path));
        }
        excerpts.push((path, s.excerpt.clone()));
    }
    let neg = format!("neg-path-{run_tag}");
    check_id(&neg)?;
    let first = samples.first().ok_or("nessun campione")?;
    let input = format!("{SYNTHETIC_ROOT}/{}.md", first.doc_id);
    jobs.push(job_line(&neg, &input, NEG_EXCERPT));
    Ok(Prepared { jobs, excerpts })
}

fn run_prepare(
    samples: &Path,
    excerpt_dir: &Path,
    jobs_out: &Path,
    run_tag: &str,
) -> anyhow::Result<()> {
    let s: Vec<Sample> = serde_json::from_slice(&std::fs::read(samples)?)?;
    let dir = excerpt_dir.to_string_lossy();
    let p = prepare_jobs(&s, &dir, run_tag).map_err(|e| anyhow::anyhow!(e))?;
    std::fs::create_dir_all(excerpt_dir)?;
    for (path, content) in &p.excerpts {
        std::fs::write(path, content)?;
    }
    let mut body = p.jobs.join("\n");
    body.push('\n');
    std::fs::write(jobs_out, body)?;
    eprintln!(
        "roundtrip prepare: {} job, {} excerpt",
        p.jobs.len(),
        p.excerpts.len()
    );
    Ok(())
}

// ---- roundtrip check -------------------------------------------------------------------

/// Lega ogni riga di `reports.jsonl` al proprio job per `job_id`, uno a uno: mancante,
/// duplicato o sconosciuto → `Err`. La riga finale `batch_summary` del padre non è un job.
pub(crate) fn bind_reports(
    job_ids: &[String],
    lines: &[String],
) -> Result<BTreeMap<String, Value>, String> {
    let wanted: BTreeSet<&str> = job_ids.iter().map(String::as_str).collect();
    let mut out = BTreeMap::new();
    for line in lines.iter().filter(|l| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(line).map_err(|e| format!("riga non JSON: {e}"))?;
        if v["status"] == "batch_summary" {
            continue;
        }
        let id = v["job_id"]
            .as_str()
            .ok_or_else(|| format!("riga senza job_id: {line}"))?
            .to_owned();
        if !wanted.contains(id.as_str()) {
            return Err(format!("job_id sconosciuto: {id}"));
        }
        if out.insert(id.clone(), v).is_some() {
            return Err(format!("job_id duplicato: {id}"));
        }
    }
    if let Some(missing) = wanted.iter().find(|id| !out.contains_key(**id)) {
        return Err(format!("rapporto mancante: {missing}"));
    }
    Ok(out)
}

/// sha256 del manifest di 01-03 per `(doc_id, formato)`: la voce MD ha `doc_id` uguale, la
/// voce DOCX ha `derived_from` uguale e `format: docx`.
fn manifest_sha(manifest: &Value, doc_id: &str, format: &str) -> Option<String> {
    manifest.as_array()?.iter().find_map(|d| {
        let hit = d["format"] == format
            && match format {
                "md" => d["doc_id"] == doc_id,
                _ => d["derived_from"] == doc_id,
            };
        hit.then(|| d["sha256"].as_str().map(str::to_owned))
            .flatten()
    })
}

/// Riga di evidenza per un job `locate`.
struct Measured {
    sample_id: String,
    doc_id: String,
    format: String,
    job_id: String,
    excerpt: String,
    expected: Vec<SectionStep>,
    status: String,
    sha_ok: bool,
    match_count: usize,
    section_path: Option<Vec<SectionStep>>,
    char_range: Option<CharRange>,
    section_hash: Option<String>,
    reextracted: Option<String>,
    unsupported: Vec<String>,
}

impl Measured {
    fn equal(&self) -> bool {
        self.status == "ok"
            && self.sha_ok
            && self.match_count == 1
            && self.reextracted.as_deref() == Some(self.excerpt.as_str())
    }

    fn path_matches(&self) -> bool {
        self.section_path.as_ref() == Some(&self.expected)
    }
}

fn measure(s: &Sample, format: &str, job_id: &str, report: &Value, manifest: &Value) -> Measured {
    let outcome = &report["outcome"];
    let status = outcome["status"]
        .as_str()
        .or(report["status"].as_str())
        .unwrap_or("missing")
        .to_owned();
    let data = &outcome["data"];
    let expected_sha = manifest_sha(manifest, &s.doc_id, format);
    let sha_ok = expected_sha.is_some() && data["doc_sha256"].as_str() == expected_sha.as_deref();
    let matches = data["matches"].as_array().cloned().unwrap_or_default();
    let first = matches.first();
    let decode = |k: &str| first.map(|m| m[k].clone());
    Measured {
        sample_id: s.sample_id.clone(),
        doc_id: s.doc_id.clone(),
        format: format.to_owned(),
        job_id: job_id.to_owned(),
        excerpt: s.excerpt.clone(),
        expected: s.expected_section_path.clone(),
        status,
        sha_ok,
        match_count: matches.len(),
        section_path: decode("section_path").and_then(|v| serde_json::from_value(v).ok()),
        char_range: decode("char_range").and_then(|v| serde_json::from_value(v).ok()),
        section_hash: decode("section_text_sha256").and_then(|v| v.as_str().map(str::to_owned)),
        reextracted: decode("reextracted").and_then(|v| v.as_str().map(str::to_owned)),
        unsupported: data["unsupported"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\t']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

fn opt_bool(b: Option<bool>) -> &'static str {
    match b {
        Some(true) => "true",
        Some(false) => "false",
        None => "",
    }
}

fn run_check(samples: &Path, reports: &Path, out_dir: &Path) -> anyhow::Result<()> {
    let s: Vec<Sample> = serde_json::from_slice(&std::fs::read(samples)?)?;
    let lines: Vec<String> = std::fs::read_to_string(reports)?
        .lines()
        .map(str::to_owned)
        .collect();
    // Il run_tag si ricava dal job negativo: è l'unico job senza campione.
    let run_tag = lines
        .iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find_map(|v| {
            v["job_id"]
                .as_str()
                .and_then(|id| id.strip_prefix("neg-path-"))
                .map(str::to_owned)
        })
        .ok_or_else(|| anyhow::anyhow!("rapporto del job neg-path assente"))?;
    let neg_id = format!("neg-path-{run_tag}");
    let mut ids: Vec<String> = s
        .iter()
        .flat_map(|x| {
            x.formats
                .iter()
                .map(|f| format!("{}-{f}-{run_tag}", x.sample_id))
                .collect::<Vec<_>>()
        })
        .collect();
    ids.push(neg_id.clone());
    let bound = bind_reports(&ids, &lines).map_err(|e| anyhow::anyhow!(e))?;

    let neg = &bound[&neg_id];
    let neg_ok = neg["status"] == "job_error" && neg["code"] == "PathNotAllowed";
    let out_dir_exists = bound
        .values()
        .any(|v| v["detail"].as_str() == Some("out_dir_exists"));

    let manifest_path =
        std::env::var("S02_CORPUS_MANIFEST").unwrap_or_else(|_| CORPUS_MANIFEST.to_owned());
    let manifest: Value = serde_json::from_slice(&std::fs::read(&manifest_path)?)?;

    let mut rows: Vec<Measured> = Vec::new();
    for x in &s {
        for f in &x.formats {
            let id = format!("{}-{f}-{run_tag}", x.sample_id);
            rows.push(measure(x, f, &id, &bound[&id], &manifest));
        }
    }

    // Concordanza MD/DOCX per campione.
    let twin = |r: &Measured| -> Option<&Measured> {
        rows.iter()
            .find(|o| o.sample_id == r.sample_id && o.format != r.format)
    };
    let mut csv = String::from(
        "doc_id,format,sample_id,job_id,section_path_depth,char_start,char_end,match_count,equal,same_path_md_docx,same_range_md_docx,same_section_hash_md_docx,path_matches_expected\n",
    );
    let mut samples_json = Vec::new();
    for r in &rows {
        let t = twin(r);
        let same_path = t.map(|o| o.section_path.is_some() && o.section_path == r.section_path);
        let same_range = t.map(|o| o.char_range.is_some() && o.char_range == r.char_range);
        let same_hash = t.map(|o| o.section_hash.is_some() && o.section_hash == r.section_hash);
        let (cs, ce) = r.char_range.map_or((String::new(), String::new()), |c| {
            (c.start.to_string(), c.end.to_string())
        });
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_field(&r.doc_id),
            r.format,
            r.sample_id,
            r.job_id,
            r.section_path.as_ref().map_or(0, Vec::len),
            cs,
            ce,
            r.match_count,
            r.equal(),
            opt_bool(same_path),
            opt_bool(same_range),
            opt_bool(same_hash),
            r.path_matches(),
        ));
        samples_json.push(json!({
            "sample_id": r.sample_id,
            "doc_id": r.doc_id,
            "format": r.format,
            "job_id": r.job_id,
            "status": r.status,
            "doc_sha256_matches_manifest": r.sha_ok,
            "excerpt": r.excerpt,
            "expected_section_path": r.expected,
            "section_path": r.section_path,
            "char_range": r.char_range,
            "section_text_sha256": r.section_hash,
            "reextracted": r.reextracted,
            "equal": r.equal(),
            "unsupported": r.unsupported,
        }));
    }
    std::fs::create_dir_all(out_dir)?;
    std::fs::write(out_dir.join("roundtrip.csv"), csv)?;
    let mut js = serde_json::to_string_pretty(&samples_json)?;
    js.push('\n');
    std::fs::write(out_dir.join("locator_samples.json"), js)?;

    let with_docx: Vec<&Measured> = rows
        .iter()
        .filter(|r| r.format == "md" && twin(r).is_some())
        .collect();
    let count = |f: &dyn Fn(&Measured) -> bool| rows.iter().filter(|r| f(r)).count();
    let summary = json!({
        "run_tag": run_tag,
        "jobs": rows.len(),
        "samples": s.len(),
        "status_ok": count(&|r| r.status == "ok"),
        "doc_sha256_matches_manifest": count(&|r| r.sha_ok),
        "roundtrip_equal": count(&Measured::equal),
        "path_matches_expected": count(&Measured::path_matches),
        "pairs_md_docx": with_docx.len(),
        "pairs_same_path": with_docx.iter().filter(|r| twin(r).is_some_and(|o| o.section_path.is_some() && o.section_path == r.section_path)).count(),
        "pairs_same_range": with_docx.iter().filter(|r| twin(r).is_some_and(|o| o.char_range.is_some() && o.char_range == r.char_range)).count(),
        "pairs_same_section_hash": with_docx.iter().filter(|r| twin(r).is_some_and(|o| o.section_hash.is_some() && o.section_hash == r.section_hash)).count(),
        "neg_path": {"status": neg["status"], "code": neg["code"], "detail": neg["detail"], "ok": neg_ok},
        "out_dir_exists_seen": out_dir_exists,
    });
    println!("{summary}");
    if !neg_ok || out_dir_exists {
        anyhow::bail!("contratto del lanciatore non rispettato (neg-path o out_dir_exists)");
    }
    Ok(())
}

// ---- offsets ---------------------------------------------------------------------------

/// Sottocomando `offsets`: dalla fixture legge solo `case_id`, `text`, `scalar_range` e
/// produce per caso soltanto `{case_id, got_utf16_range, got_text}`.
pub(crate) fn offsets_rows(fixture_json: &str) -> Result<Vec<Value>, String> {
    #[derive(Deserialize)]
    struct Fixture {
        vectors: Vec<Vector>,
    }
    // Campi letti: nessun valore atteso (`utf16_range`, `expected`) entra nel bin.
    #[derive(Deserialize)]
    struct Vector {
        case_id: String,
        text: String,
        scalar_range: [usize; 2],
    }
    let f: Fixture = serde_json::from_str(fixture_json).map_err(|e| format!("fixture: {e}"))?;
    Ok(f.vectors
        .into_iter()
        .map(|v| {
            let r = CharRange {
                start: v.scalar_range[0],
                end: v.scalar_range[1],
            };
            let got = canon::to_utf16_range(&v.text, r).map(|u| [u.start, u.end]);
            json!({
                "case_id": v.case_id,
                "got_utf16_range": got,
                "got_text": canon::slice_scalars(&v.text, r),
            })
        })
        .collect())
}

fn run_offsets(vectors: &Path, out: &Path) -> anyhow::Result<()> {
    let rows = offsets_rows(&std::fs::read_to_string(vectors)?).map_err(|e| anyhow::anyhow!(e))?;
    let mut body = serde_json::to_string_pretty(&rows)?;
    body.push('\n');
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(out, body)?;
    eprintln!("offsets: {} vettori", rows.len());
    Ok(())
}

// ---- rects -----------------------------------------------------------------------------

/// Caso di `cases.json` (copia verbatim della fixture più `pdf_file`).
#[derive(Deserialize)]
struct RectCase {
    case_id: String,
    rotate: i32,
    mediabox: [f64; 4],
    cropbox: [f64; 4],
    user_rect: [f64; 4],
    expected_viewport_rect: [f64; 4],
    pdf_file: String,
}

const TOL_PT: f64 = 0.5;

fn max_err(a: [f64; 4], b: [f64; 4]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
}

fn rect_row(c: &RectCase, stage: &str, got: Option<[f64; 4]>, extra_err: f64) -> String {
    let e = c.expected_viewport_rect;
    let differs = c.cropbox != c.mediabox;
    let (g, err, pass) = match got {
        Some(g) => {
            let err = max_err(g, e).max(extra_err);
            (
                g.iter().map(|v| format!("{v:.3}")).collect::<Vec<_>>(),
                format!("{err:.3}"),
                err <= TOL_PT,
            )
        }
        None => (vec![String::new(); 4], String::new(), false),
    };
    format!(
        "{},{stage},{},{differs},{},{},{},{},{},{},{},{},{err},{pass}\n",
        c.case_id, c.rotate, e[0], e[1], e[2], e[3], g[0], g[1], g[2], g[3]
    )
}

/// Bounds grezzi (spazio utente) del tracciato e dei glifi della pagina 1, con la geometria
/// della pagina letta da PDFium: `(path, text, mediabox, cropbox, rotate)`.
type PageGeometry = (Option<[f64; 4]>, Option<[f64; 4]>, [f64; 4], [f64; 4], i32);

fn pdf_geometry(
    pdfium: &pdfium_render::prelude::Pdfium,
    path: &Path,
) -> anyhow::Result<PageGeometry> {
    use pdfium_render::prelude::*;
    let r4 = |r: PdfRect| {
        [
            r.left().value as f64,
            r.bottom().value as f64,
            r.right().value as f64,
            r.top().value as f64,
        ]
    };
    let unite = |acc: Option<[f64; 4]>, b: [f64; 4]| {
        Some(match acc {
            None => b,
            Some(a) => [
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[2].max(b[2]),
                a[3].max(b[3]),
            ],
        })
    };
    let doc = pdfium.load_pdf_from_file(path, None)?;
    let page = doc.pages().get(0)?;
    let rotate = match page.rotation()? {
        PdfPageRenderRotation::None => 0,
        PdfPageRenderRotation::Degrees90 => 90,
        PdfPageRenderRotation::Degrees180 => 180,
        PdfPageRenderRotation::Degrees270 => 270,
    };
    let media = r4(page.boundaries().media()?.bounds);
    let crop = page
        .boundaries()
        .crop()
        .map(|b| r4(b.bounds))
        .unwrap_or(media);
    let mut path_box = None;
    for obj in page.objects().iter() {
        if obj.object_type() == PdfPageObjectType::Path {
            path_box = unite(path_box, r4(obj.bounds()?.to_rect()));
        }
    }
    let mut text_box = None;
    for ch in page.text()?.chars().iter() {
        let s = ch.unicode_string().unwrap_or_default();
        if s.trim().is_empty() {
            continue;
        }
        if let Ok(b) = ch.tight_bounds() {
            text_box = unite(text_box, r4(b));
        }
    }
    Ok((path_box, text_box, media, crop, rotate))
}

fn run_rects(cases: &Path, pdf_dir: &Path, out: &Path) -> anyhow::Result<()> {
    let raw: Value = serde_json::from_slice(&std::fs::read(cases)?)?;
    let list = raw.get("cases").cloned().unwrap_or(raw);
    let cases: Vec<RectCase> = serde_json::from_value(list)?;
    let dir = std::env::var_os("PDFIUM_DIR")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/opt/pdfium/lib".into());
    let pdfium =
        pdfium_render::prelude::Pdfium::new(pdfium_render::prelude::Pdfium::bind_to_library(
            pdfium_render::prelude::Pdfium::pdfium_platform_library_name_at_path(&dir),
        )?);
    let mut csv = String::from(
        "case_id,stage,rotate,cropbox_differs,expected_x0,expected_y0,expected_x1,expected_y1,got_x0,got_y0,got_x1,got_y1,max_abs_err_pt,pass\n",
    );
    let mut text_rows = String::new();
    let mut notes = Vec::new();
    for c in &cases {
        // Stadio 1: trasformazione pura, nessun PDFium.
        let t = pdfrect::to_viewport(c.user_rect, c.mediabox, c.cropbox, c.rotate);
        csv.push_str(&rect_row(c, "transform", Some(t), 0.0));

        let (path_box, text_box, media, crop, rotate) =
            pdf_geometry(&pdfium, &pdf_dir.join(&c.pdf_file))?;
        // Stadio 2: bounds grezzi del tracciato contro il rettangolo disegnato (spazio
        // utente), poi una sola trasformazione con la geometria letta da PDFium.
        let raw_err = path_box.map_or(f64::INFINITY, |b| max_err(b, c.user_rect));
        let got = path_box.map(|b| pdfrect::to_viewport(b, media, crop, rotate));
        csv.push_str(&rect_row(c, "extract_path", got, raw_err));
        notes.push(json!({
            "case_id": c.case_id,
            "pdfium_rotate": rotate,
            "pdfium_mediabox": media,
            "pdfium_cropbox": crop,
            "viewport_size": pdfrect::viewport_size(media, crop, rotate),
            "path_bounds_raw": path_box,
            "path_raw_err_vs_user_rect_pt": path_box.map(|b| max_err(b, c.user_rect)),
            "text_bounds_raw": text_box,
        }));
        // Stadio 3 (informativo): unione dei char box della stringa marcatore. Il valore
        // atteso resta quello della fixture: la riga misura quanto la bbox dei glifi si
        // discosta dal rettangolo.
        let got_text = text_box.map(|b| pdfrect::to_viewport(b, media, crop, rotate));
        text_rows.push_str(&rect_row(c, "extract_text", got_text, 0.0));
    }
    csv.push_str(&text_rows);
    if let Some(d) = out.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(out, csv)?;
    println!("{}", json!({"rects": notes}));
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let cmd = match parse_cli(args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("locator: {e}");
            return ExitCode::from(2);
        }
    };
    let result = match cmd {
        Cmd::Locate {
            input,
            excerpt_file,
        } => {
            let outcome = run_locate(&input, &excerpt_file);
            return match emit(&outcome) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::from(1),
            };
        }
        Cmd::RoundtripPrepare {
            samples,
            excerpt_dir,
            jobs_out,
            run_tag,
        } => {
            if !valid_run_tag(&run_tag) {
                eprintln!("locator: --run-tag fuori da [a-z0-9]{{8}}");
                return ExitCode::from(2);
            }
            match run_prepare(&samples, &excerpt_dir, &jobs_out, &run_tag) {
                Ok(()) => return ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("locator: roundtrip prepare: {e:#}");
                    return ExitCode::from(2);
                }
            }
        }
        Cmd::RoundtripCheck {
            samples,
            reports,
            out_dir,
        } => run_check(&samples, &reports, &out_dir),
        Cmd::Rects {
            cases,
            pdf_dir,
            out,
        } => run_rects(&cases, &pdf_dir, &out),
        Cmd::Offsets { vectors, out } => run_offsets(&vectors, &out),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("locator: {e:#}");
            ExitCode::from(1)
        }
    }
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
