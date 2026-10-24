//! Bin `ocr` (piano 01-13, spike S-02 OCR, TTQ-parse e prova ostile sul render).
//!
//! - `render --input <pdf> --dpi <dpi> --out <dir>` e `tesseract --image <png>`: i soli
//!   sottocomandi da figlio isolato, con la CLI esatta di `job::resolve` di 01-07; girano solo
//!   tramite un job `ocr_render` / `ocr_tesseract` di `parse-parent`;
//! - `map`, `score`, `hostile-verdict`, `throughput-row`, `paddle-check`, `tess-jobs`,
//!   `ttq-select`, `ttq-merge`, `limits-from-runs`: bin normali su dati derivati, nel runner.
//!
//! `render`: `prescan::check_pdf` prima di PDFium (un rifiuto si emette com'è), poi
//! `render_with_config` con `scale_page_by_factor(dpi/72)`, PNG in scala di grigi a 8 bit con
//! chunk `pHYs` pari al DPI e sidecar `<pagina>.render.json`. Un errore di PDFium al
//! caricamento o alla render → `failed/Corrupt` con detail `pdfium_load` / `pdfium_render` e il
//! nome della variante; nessun controllo sul contenuto dei pixel decide l'esito.
//!
//! `tesseract`: DPI dal `pHYs` (assente → `failed/Corrupt` `dpi_missing`), `tesseract … tsv`
//! lanciato per percorso assoluto con `Command::arg` (nessuna shell; `OMP_THREAD_LIMIT` arriva
//! dal padre), stdout con tetto di 512 KiB, parole in `data`. Il figlio riporta come
//! `vm_hwm_kb` e `vm_peak_kb` il massimo fra il proprio valore e quello del processo
//! `tesseract`, campionato su `/proc/<pid>/status`: la calibrazione e il rapporto
//! prenotazione/picco vedono così la memoria del discendente (Soglie di RISULTATI-ocr.md).
//!
//! **Modifica:** Claude (gsd-executor 01-13) · 2026-10-24 · corpi del bin ocr (Task 2)
mod geom;
mod score;
mod tsv;
mod ttq;

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{ExitCode, Stdio};
use std::time::{Duration, Instant};

use anyhow::Context;
use pdfium_render::prelude::{PdfRenderConfig, Pdfium, PdfiumError, PdfiumInternalError};
use s02_parser::prescan::{self, Caps, Rejection};
use s02_parser::protocol::{ChildOutcome, Kind, emit, vm_hwm_kb, vm_peak_kb};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

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
    TtqSelect(Vec<OsString>),
    TessJobs(Vec<OsString>),
    ThroughputRow(Vec<OsString>),
    PaddleCheck(Vec<OsString>),
    LimitsFromRuns(Vec<OsString>),
}

/// Argomenti senza il nome del programma. Forme diverse da quelle dell'allowlist → `Err`.
pub(crate) fn parse_cli(args: Vec<OsString>) -> Result<Cmd, String> {
    let Some((sub, rest)) = args.split_first() else {
        return Err("sottocomando assente".into());
    };
    let rest = rest.to_vec();
    match sub.to_str() {
        Some("render") => match rest.as_slice() {
            [a, input, b, dpi, c, out] if a == "--input" && b == "--dpi" && c == "--out" => {
                let dpi = dpi
                    .to_str()
                    .and_then(|d| d.parse::<u32>().ok())
                    .ok_or("render: --dpi non è un intero")?;
                Ok(Cmd::Render {
                    input: PathBuf::from(input),
                    dpi,
                    out: PathBuf::from(out),
                })
            }
            _ => Err("render: attesi esattamente --input <pdf> --dpi <dpi> --out <dir>".into()),
        },
        Some("tesseract") => match rest.as_slice() {
            [a, image] if a == "--image" => Ok(Cmd::Tesseract {
                image: PathBuf::from(image),
            }),
            _ => Err("tesseract: atteso esattamente --image <png>".into()),
        },
        Some("map") => Ok(Cmd::Map(rest)),
        Some("score") => Ok(Cmd::Score(rest)),
        Some("hostile-verdict") => Ok(Cmd::HostileVerdict(rest)),
        Some("ttq-merge") => Ok(Cmd::TtqMerge(rest)),
        Some("ttq-select") => Ok(Cmd::TtqSelect(rest)),
        Some("tess-jobs") => Ok(Cmd::TessJobs(rest)),
        Some("throughput-row") => Ok(Cmd::ThroughputRow(rest)),
        Some("paddle-check") => Ok(Cmd::PaddleCheck(rest)),
        Some("limits-from-runs") => Ok(Cmd::LimitsFromRuns(rest)),
        _ => Err(format!("sottocomando sconosciuto: {sub:?}")),
    }
}

/// Cartella dei config del pacchetto Debian di Tesseract 5 (contiene `tsv`), cartella corrente
/// del discendente.
const TESS_CONFIG_DIR: &str = "/usr/share/tesseract-ocr/5/tessdata/configs";
/// Tetto dello stdout di Tesseract letto dal figlio.
const TSV_CAP: u64 = 512 * 1024;
/// Intervallo di campionamento della memoria del discendente.
const SAMPLE_EVERY: Duration = Duration::from_millis(5);

/// argv di `tesseract` (senza il nome del programma) per un PNG a `dpi` effettivi:
/// `<png> stdout --tessdata-dir /opt/tessdata_best -l ita+eng --oem 1 --psm 3 --dpi <dpi>
/// -c preserve_interword_spaces=1 tsv`. Nessuna shell; `OMP_THREAD_LIMIT` arriva dal padre.
pub(crate) fn tesseract_args(png: &Path, dpi: u32) -> Vec<OsString> {
    let mut v: Vec<OsString> = vec![png.as_os_str().to_owned()];
    v.extend(
        [
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
        ]
        .map(OsString::from),
    );
    v.push(dpi.to_string().into());
    v.extend(["-c", "preserve_interword_spaces=1", "tsv"].map(OsString::from));
    v
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let cmd = match parse_cli(args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ocr: {e}");
            return ExitCode::from(2);
        }
    };
    let result = match cmd {
        Cmd::Render { input, dpi, out } => {
            return child_exit(emit(&render(&input, dpi, &out)));
        }
        Cmd::Tesseract { image } => {
            let (outcome, hwm) = tesseract(&image);
            return child_exit(emit_with_hwm(&outcome, hwm));
        }
        Cmd::Map(a) => run_map(a),
        Cmd::Score(a) => score::run_score(a),
        Cmd::HostileVerdict(a) => score::run_hostile_verdict(a),
        Cmd::ThroughputRow(a) => score::run_throughput_row(a),
        Cmd::PaddleCheck(a) => score::run_paddle_check(a),
        Cmd::LimitsFromRuns(a) => score::run_limits_from_runs(a),
        Cmd::TtqMerge(a) => run_ttq_merge(a),
        Cmd::TtqSelect(a) => run_ttq_select(a),
        Cmd::TessJobs(a) => run_tess_jobs(a),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ocr: {e:#}");
            ExitCode::from(1)
        }
    }
}

fn child_exit(r: anyhow::Result<()>) -> ExitCode {
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(1),
    }
}

/// Riga del figlio nella forma di `protocol::emit`, con `vm_hwm_kb` dato (massimo fra figlio e
/// discendente).
#[derive(Serialize)]
struct Line<'a> {
    #[serde(flatten)]
    outcome: &'a ChildOutcome,
    vm_hwm_kb: Option<u64>,
}

fn emit_with_hwm(outcome: &ChildOutcome, hwm: Option<u64>) -> anyhow::Result<()> {
    let line = serde_json::to_string(&Line {
        outcome,
        vm_hwm_kb: hwm,
    })?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    out.write_all(line.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}

fn rejection(r: Rejection) -> ChildOutcome {
    match r {
        Rejection::Failed(kind, detail) => ChildOutcome::Failed { kind, detail },
        Rejection::Quarantined(reasons) => ChildOutcome::Quarantined { reasons },
    }
}

/// Nome della variante d'errore di PDFium (senza dati), per il detail.
fn pdfium_variant(e: &PdfiumError) -> String {
    let s = format!("{e:?}");
    s.chars()
        .take(200)
        .collect::<String>()
        .replace(['\n', '\r'], " ")
}

// ---- render ---------------------------------------------------------------------------------

fn render(input: &Path, dpi: u32, out: &Path) -> ChildOutcome {
    let t0 = Instant::now();
    if !(72..=600).contains(&dpi) {
        return ChildOutcome::failed(Kind::Unsupported, "dpi fuori da 72..=600");
    }
    let bytes = match std::fs::read(input) {
        Ok(b) => b,
        Err(e) => return ChildOutcome::failed(Kind::Corrupt, format!("read: {e}")),
    };
    // Prescan prima di qualunque contatto con PDFium: un rifiuto si emette com'è.
    let report = match prescan::check_pdf(&bytes, &Caps::default()) {
        Ok(r) => r,
        Err(r) => return rejection(r),
    };
    let dir = std::env::var_os("PDFIUM_DIR")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/opt/pdfium/lib".into());
    let bindings = match Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&dir))
    {
        Ok(b) => b,
        Err(e) => return ChildOutcome::failed(Kind::Crashed, format!("pdfium_bind: {e}")),
    };
    let pdfium = Pdfium::new(bindings);
    let doc = match pdfium.load_pdf_from_byte_slice(&bytes, None) {
        Ok(d) => d,
        Err(PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError)) => {
            return ChildOutcome::failed(Kind::Encrypted, "pdfium_password");
        }
        Err(e) => {
            return ChildOutcome::failed(
                Kind::Corrupt,
                format!("pdfium_load: {}", pdfium_variant(&e)),
            );
        }
    };
    let sha = hex_lower(&Sha256::digest(&bytes));
    let config = PdfRenderConfig::new().scale_page_by_factor(dpi as f32 / 72.0);
    let ppm = geom::dpi_to_ppm(dpi);
    let (mut pages, mut png_bytes) = (0u64, 0u64);
    for (i, page) in doc.pages().iter().enumerate() {
        let n = i as u32 + 1;
        let (w_pt, h_pt) = (
            f64::from(page.width().value),
            f64::from(page.height().value),
        );
        let bitmap = match page.render_with_config(&config) {
            Ok(b) => b,
            Err(e) => {
                return ChildOutcome::failed(
                    Kind::Corrupt,
                    format!("pdfium_render: pagina {n}: {}", pdfium_variant(&e)),
                );
            }
        };
        let (w, h) = (bitmap.width() as u32, bitmap.height() as u32);
        let rgba = bitmap.as_rgba_bytes();
        drop(bitmap);
        if rgba.len() != (w as usize) * (h as usize) * 4 {
            return ChildOutcome::failed(
                Kind::Corrupt,
                format!("pdfium_render: pagina {n}: buffer"),
            );
        }
        let gray: Vec<u8> = rgba
            .chunks_exact(4)
            .map(|p| {
                // Luma BT.601 su sfondo bianco (alpha composto sul bianco).
                let a = f32::from(p[3]) / 255.0;
                let l = 0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2]);
                (l * a + 255.0 * (1.0 - a)).round().clamp(0.0, 255.0) as u8
            })
            .collect();
        drop(rgba);
        let png = match encode_png_gray(&gray, w, h, ppm) {
            Ok(p) => p,
            Err(e) => return ChildOutcome::failed(Kind::ResourceLimit, format!("png: {e}")),
        };
        let stem = format!("page-{n:04}");
        let sidecar = json!({
            "source_sha256": sha,
            "page": n,
            "page_w_pt": w_pt,
            "page_h_pt": h_pt,
            "dpi": dpi,
            "width_px": w,
            "height_px": h,
        });
        let written = std::fs::write(out.join(format!("{stem}.png")), &png).and_then(|()| {
            std::fs::write(
                out.join(format!("{stem}.render.json")),
                serde_json::to_vec(&sidecar).unwrap_or_default(),
            )
        });
        if let Err(e) = written {
            return ChildOutcome::failed(Kind::ResourceLimit, format!("write: {e}"));
        }
        png_bytes += png.len() as u64;
        pages += 1;
    }
    ChildOutcome::Ok {
        pages: Some(pages),
        output: Some(out.to_string_lossy().into_owned()),
        data: Some(json!({
            "pages": pages,
            "dpi": dpi,
            "png_bytes": png_bytes,
            "render_ms": t0.elapsed().as_millis() as u64,
            "max_image_bytes_seen": report.max_stream_bytes,
            "vm_peak_kb": vm_peak_kb(),
            "vm_hwm_kb": vm_hwm_kb(),
        })),
    }
}

fn hex_lower(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// PNG in scala di grigi a 8 bit col crate `image`, più il chunk `pHYs` (unità metro) inserito
/// subito dopo `IHDR`, con CRC32 di `flate2`.
fn encode_png_gray(gray: &[u8], w: u32, h: u32, ppm: u32) -> anyhow::Result<Vec<u8>> {
    use image::ImageEncoder as _;
    let mut buf = Vec::new();
    image::codecs::png::PngEncoder::new(&mut buf).write_image(
        gray,
        w,
        h,
        image::ExtendedColorType::L8,
    )?;
    // Firma (8) + IHDR (4 lunghezza + 4 tipo + 13 dati + 4 CRC) = 33 byte.
    anyhow::ensure!(
        buf.len() > 33 && &buf[12..16] == b"IHDR",
        "PNG senza IHDR in testa"
    );
    let mut data = Vec::with_capacity(9);
    data.extend_from_slice(&ppm.to_be_bytes());
    data.extend_from_slice(&ppm.to_be_bytes());
    data.push(1);
    let mut chunk = Vec::with_capacity(21);
    chunk.extend_from_slice(&9u32.to_be_bytes());
    let mut body = b"pHYs".to_vec();
    body.extend_from_slice(&data);
    let mut crc = flate2::Crc::new();
    crc.update(&body);
    chunk.extend_from_slice(&body);
    chunk.extend_from_slice(&crc.sum().to_be_bytes());
    let mut out = Vec::with_capacity(buf.len() + chunk.len());
    out.extend_from_slice(&buf[..33]);
    out.extend_from_slice(&chunk);
    out.extend_from_slice(&buf[33..]);
    Ok(out)
}

// ---- tesseract ------------------------------------------------------------------------------

/// `VmPeak` e `VmHWM` (KiB) di un processo, se leggibili.
fn proc_mem(pid: u32) -> (Option<u64>, Option<u64>) {
    let path = format!("/proc/{pid}/status");
    match std::fs::read_to_string(path) {
        Ok(t) => (
            s02_parser::protocol::status_value_kb(&t, "VmPeak:"),
            s02_parser::protocol::status_value_kb(&t, "VmHWM:"),
        ),
        Err(_) => (None, None),
    }
}

fn opt_max(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

/// Esito del figlio e `vm_hwm_kb` da riportare (massimo fra figlio e discendente).
fn tesseract(image: &Path) -> (ChildOutcome, Option<u64>) {
    let t0 = Instant::now();
    let fail = |kind: Kind, d: String| (ChildOutcome::failed(kind, d), vm_hwm_kb());
    let bytes = match std::fs::read(image) {
        Ok(b) => b,
        Err(e) => return fail(Kind::Corrupt, format!("read: {e}")),
    };
    let dpi = match geom::read_phys_dpi(&bytes) {
        Ok(d) => d,
        Err(geom::DpiError::Missing) => return fail(Kind::Corrupt, "dpi_missing".into()),
        Err(geom::DpiError::NotPng) => return fail(Kind::Corrupt, "not_png".into()),
    };
    drop(bytes);
    // Percorso assoluto letterale: il padre mette in PATH solo la cartella dei bin ammessi, quindi
    // il discendente non si cerca nel PATH.
    let mut child = match std::process::Command::new("/usr/bin/tesseract")
        .args(tesseract_args(image, dpi))
        // Il config `tsv` dell'argv si cerca in `<tessdata-dir>/configs/`, che in
        // /opt/tessdata_best non c'è (solo i traineddata); Tesseract ripiega allora sul nome
        // relativo alla cartella corrente, e qui la cartella corrente è quella dei config del
        // pacchetto Debian, in sola lettura. Nessun file scritto nella cartella del job.
        .current_dir(TESS_CONFIG_DIR)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return fail(Kind::Crashed, format!("tesseract_spawn: {e}")),
    };
    let pid = child.id();
    let mut stdout = child.stdout.take().expect("stdout piped");
    let mut stderr = child.stderr.take().expect("stderr piped");
    let out_t = std::thread::spawn(move || {
        // Lettura limitata con io::copy sul Take (equivale a leggere fino in fondo il Take): oltre
        // il tetto si smette di leggere, il processo si ferma sulla pipe piena e lo uccide il
        // ciclo principale.
        let mut v = Vec::new();
        let _ = std::io::copy(&mut (&mut stdout).take(TSV_CAP + 1), &mut v);
        v
    });
    let err_t = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = std::io::copy(&mut (&mut stderr).take(4096), &mut v);
        v
    });
    let (mut peak, mut hwm) = (None, None);
    let status = loop {
        let (p, h) = proc_mem(pid);
        peak = opt_max(peak, p);
        hwm = opt_max(hwm, h);
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {
                if out_t.is_finished() {
                    // stdout chiuso o tetto raggiunto: si attende l'uscita con una scadenza.
                    let deadline = Instant::now() + Duration::from_secs(5);
                    loop {
                        if let Ok(Some(_)) = child.try_wait() {
                            break;
                        } else if Instant::now() > deadline {
                            let _ = child.kill();
                            break;
                        }
                        let (p, h) = proc_mem(pid);
                        peak = opt_max(peak, p);
                        hwm = opt_max(hwm, h);
                        std::thread::sleep(SAMPLE_EVERY);
                    }
                    match child.wait() {
                        Ok(st) => break st,
                        Err(e) => return fail(Kind::Crashed, format!("tesseract_wait: {e}")),
                    }
                }
                std::thread::sleep(SAMPLE_EVERY);
            }
            Err(e) => return fail(Kind::Crashed, format!("tesseract_wait: {e}")),
        }
    };
    let out = out_t.join().unwrap_or_default();
    let err = err_t.join().unwrap_or_default();
    let child_hwm = opt_max(vm_hwm_kb(), hwm);
    let child_peak = opt_max(vm_peak_kb(), peak);
    if out.len() as u64 > TSV_CAP {
        return (
            ChildOutcome::failed(Kind::OutputLimit, "tesseract_stdout_cap"),
            child_hwm,
        );
    }
    if !status.success() {
        let tail: String = String::from_utf8_lossy(&err)
            .chars()
            .filter(|c| !c.is_control())
            .take(200)
            .collect();
        return (
            ChildOutcome::failed(
                Kind::Corrupt,
                format!("tesseract_exit code={:?}: {tail}", status.code()),
            ),
            child_hwm,
        );
    }
    let text = String::from_utf8_lossy(&out);
    let words = match tsv::parse_tsv(&text) {
        Ok(w) => w,
        Err(e) => {
            return (
                ChildOutcome::failed(Kind::Corrupt, format!("tsv: {e}")),
                child_hwm,
            );
        }
    };
    let n = words.len();
    (
        ChildOutcome::Ok {
            pages: Some(1),
            output: None,
            data: Some(json!({
                "dpi": dpi,
                "words": words,
                "word_count": n,
                "tesseract_ms": t0.elapsed().as_millis() as u64,
                "tesseract_vm_hwm_kb": hwm,
                "tesseract_vm_peak_kb": peak,
                "vm_peak_kb": child_peak,
                "vm_hwm_kb": child_hwm,
            })),
        },
        child_hwm,
    )
}

// ---- map (stadio 1 e parole) ----------------------------------------------------------------

#[derive(clap::Parser, Debug)]
#[command(name = "ocr map")]
struct MapCli {
    /// Parole Tesseract (array JSON di parole in pixel) da riportare in punti PDF.
    #[arg(long, conflicts_with = "cases")]
    words: Option<PathBuf>,
    /// Trasformazione (`.render.json` o `.transform.json`) delle parole.
    #[arg(long, requires = "words")]
    transform: Option<PathBuf>,
    /// Stadio 1 del controllo analitico: `geom-cases.json` di `gen_geom.py`.
    #[arg(long)]
    cases: Option<PathBuf>,
    /// Mappa dei job tesseract (`tess-jobs`) da cui leggere le trasformazioni registrate.
    #[arg(long, requires = "cases")]
    map: Option<PathBuf>,
    #[arg(long)]
    results_raw: Option<PathBuf>,
}

fn run_map(args: Vec<OsString>) -> anyhow::Result<()> {
    use clap::Parser as _;
    let cli = MapCli::try_parse_from(std::iter::once("ocr-map".into()).chain(args))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if let (Some(w), Some(t)) = (&cli.words, &cli.transform) {
        let words: Vec<tsv::Word> = serde_json::from_value(score::read_json(w)?)?;
        let t = score::load_transform(t)?;
        let out: Vec<Value> = words
            .iter()
            .map(|w| json!({"text": w.text, "bbox_pt": geom::map_bbox(&t, score::word_px(w))}))
            .collect();
        println!("{}", serde_json::to_string(&out)?);
        return Ok(());
    }
    let (Some(cases), Some(map), Some(rr)) = (&cli.cases, &cli.map, &cli.results_raw) else {
        anyhow::bail!("map: --words/--transform oppure --cases/--map/--results-raw");
    };
    // Stadio 1: angoli attesi → px (diretta) → pt (inversa), errore massimo per asse in px.
    let v = score::read_json(cases)?;
    let mut by_doc = std::collections::BTreeMap::new();
    for p in v["pages"].as_array().context("pages")? {
        by_doc.insert(
            p["doc"].as_str().context("doc")?.to_owned(),
            p["words"].clone(),
        );
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut worst_all = 0.0f64;
    for it in score::load_map(map)?.iter().filter(|i| i.set == "geom") {
        if !seen.insert((it.doc.clone(), it.quality.clone())) {
            continue;
        }
        let t = score::load_transform(&score::host_path(&it.transform, rr))?;
        let mut worst = 0.0f64;
        let words = by_doc.get(&it.doc).context("caso assente")?;
        for w in words.as_array().context("words")? {
            let r: [f64; 4] = serde_json::from_value(w["rect"].clone())?;
            for c in [(r[0], r[1]), (r[2], r[1]), (r[0], r[3]), (r[2], r[3])] {
                let back = geom::to_pt(&t, geom::to_px(&t, c));
                let e = (back.0 - c.0).abs().max((back.1 - c.1).abs()) * t.dpi / 72.0;
                worst = worst.max(e);
            }
        }
        worst_all = worst_all.max(worst);
        println!(
            "stadio1 {} {} dpi {} rot {}: errore massimo {:.2e} px → {}",
            it.doc,
            it.quality,
            t.dpi,
            t.rotation_deg,
            worst,
            if worst <= 0.5 { "pass" } else { "fail" }
        );
    }
    println!(
        "stadio1 complessivo: errore massimo {worst_all:.2e} px → {}",
        if worst_all <= 0.5 { "pass" } else { "fail" }
    );
    Ok(())
}

// ---- tess-jobs ------------------------------------------------------------------------------

#[derive(clap::Parser, Debug)]
#[command(name = "ocr tess-jobs")]
struct TessJobsCli {
    /// Indice della run scritto da `make_scans.sh` (`ocr-index/<run_id>.json`).
    #[arg(long)]
    index: PathBuf,
    /// Cartella vista dal runner che nel worker è `/results/raw`.
    #[arg(long)]
    results_raw: PathBuf,
    #[arg(long, value_parser = ["good", "degraded"])]
    quality: String,
    /// Insiemi dell'indice (`oracle`, `geom`, `doc25`), separati da virgola.
    #[arg(long)]
    sets: String,
    #[arg(long)]
    rep: u32,
    #[arg(long)]
    out_jobs: PathBuf,
    /// Mappa `job_id` → pagina (si aggiunge alle voci già presenti).
    #[arg(long)]
    out_map: PathBuf,
}

/// Job `ocr_tesseract` delle pagine renderizzate (buone: `out_dir` del render a 300 DPI;
/// degradate: `ocr-degraded/<run_id>/<job_id del render a 200 DPI>/`).
fn run_tess_jobs(args: Vec<OsString>) -> anyhow::Result<()> {
    use clap::Parser as _;
    let cli = TessJobsCli::try_parse_from(std::iter::once("ocr-tess-jobs".into()).chain(args))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let idx = score::read_json(&cli.index)?;
    let run_id = idx["run_id"].as_str().context("run_id")?.to_owned();
    let sets: Vec<&str> = cli.sets.split(',').collect();
    let (want_dpi, q) = if cli.quality == "good" {
        (300, "g")
    } else {
        (200, "d")
    };
    let mut items: Vec<score::TessItem> = if cli.out_map.exists() {
        score::load_map(&cli.out_map)?
    } else {
        Vec::new()
    };
    let mut jobs = String::new();
    for r in idx["renders"].as_array().context("renders")? {
        let set = r["set"].as_str().unwrap_or_default();
        if !sets.contains(&set) || r["dpi"].as_u64() != Some(want_dpi) {
            continue;
        }
        let doc = r["doc"].as_str().context("doc")?;
        let rjob = r["job_id"].as_str().context("job_id")?;
        let dir_worker = if cli.quality == "good" {
            r["out_dir"].as_str().context("out_dir")?.to_owned()
        } else {
            format!("/results/raw/ocr-degraded/{run_id}/{rjob}")
        };
        let dir = score::host_path(&dir_worker, &cli.results_raw);
        let mut pngs: Vec<String> = std::fs::read_dir(&dir)
            .with_context(|| format!("{}", dir.display()))?
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.starts_with("page-") && n.ends_with(".png"))
            .collect();
        pngs.sort();
        for name in pngs {
            let page: u32 = name[5..9].parse().context("numero di pagina")?;
            let job_id = format!("t-{doc}-p{page}-{q}-r{}-{run_id}", cli.rep);
            anyhow::ensure!(
                s02_parser::job::valid_job_id(&job_id),
                "job_id non valido: {job_id}"
            );
            let png = format!("{dir_worker}/{name}");
            let side = if cli.quality == "good" {
                "render"
            } else {
                "transform"
            };
            let transform = format!("{dir_worker}/page-{page:04}.{side}.json");
            jobs.push_str(&serde_json::to_string(&json!({
                "job_id": job_id, "op": "ocr_tesseract", "input": png,
                "out_dir": null, "params": {},
            }))?);
            jobs.push('\n');
            items.push(score::TessItem {
                job_id,
                set: set.to_owned(),
                doc: doc.to_owned(),
                oracle_id: r["oracle_id"].as_str().map(str::to_owned),
                page,
                quality: cli.quality.clone(),
                rep: cli.rep,
                png,
                transform,
            });
        }
    }
    std::fs::write(&cli.out_jobs, jobs)?;
    std::fs::write(&cli.out_map, serde_json::to_string_pretty(&items)? + "\n")?;
    println!("tess-jobs: {} voci nella mappa", items.len());
    Ok(())
}

// ---- ttq-select e ttq-merge -----------------------------------------------------------------

#[derive(clap::Parser, Debug)]
#[command(name = "ocr ttq-select")]
struct SelectCli {
    /// `out_dir` di `extract_pdf` della run (`page-NNNN.json`).
    #[arg(long)]
    pages_dir: PathBuf,
    /// `table_heuristic_params.json` di 01-08 (parametri congelati).
    #[arg(long)]
    params: PathBuf,
}

/// Pagine da mandare a Docling (punteggio ≥ `uncertain_from`, le incerte contano come
/// selezionate) e pagine senza caratteri da mandare all'OCR. Il punteggio è la funzione
/// `page_score` di `src/bin/extract/tables.rs` (01-08), riportata in `ttq::page_score`.
fn run_ttq_select(args: Vec<OsString>) -> anyhow::Result<()> {
    use clap::Parser as _;
    let cli = SelectCli::try_parse_from(std::iter::once("ocr-ttq-select".into()).chain(args))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let params = score::read_json(&cli.params)?;
    let f = ttq::Features::from_params(&params)?;
    let uncertain = params["band"]["uncertain_from"]
        .as_f64()
        .context("band.uncertain_from")?;
    let mut names: Vec<String> = std::fs::read_dir(&cli.pages_dir)?
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with("page-") && n.ends_with(".json"))
        .collect();
    names.sort();
    let (mut tables, mut ocr) = (Vec::new(), Vec::new());
    for n in &names {
        let v = score::read_json(&cli.pages_dir.join(n))?;
        let page = v["page"].as_u64().context("page")? as u32;
        let words: Vec<(String, [f64; 4])> = v["words"]
            .as_array()
            .context("words")?
            .iter()
            .filter_map(|w| {
                Some((
                    w["text"].as_str()?.to_owned(),
                    serde_json::from_value(w["bbox"].clone()).ok()?,
                ))
            })
            .collect();
        if v["chars"].as_array().map_or(0, Vec::len) == 0 {
            ocr.push(page);
        } else if ttq::page_score(&words, &f) >= uncertain {
            tables.push(page);
        }
    }
    let join = |v: &[u32]| v.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    println!(
        "pages={}\ttable_pages={}\tocr_pages={}",
        names.len(),
        join(&tables),
        join(&ocr)
    );
    Ok(())
}

#[derive(clap::Parser, Debug)]
#[command(name = "ocr ttq-merge")]
struct MergeCli {
    #[arg(long)]
    run_dir: PathBuf,
    /// `ttq_parse.csv` in append (header scritto se il file è nuovo).
    #[arg(long)]
    csv: PathBuf,
}

fn run_ttq_merge(args: Vec<OsString>) -> anyhow::Result<()> {
    use clap::Parser as _;
    let cli = MergeCli::try_parse_from(std::iter::once("ocr-ttq-merge".into()).chain(args))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let (meta, comps) = ttq::read_run_dir(&cli.run_dir).map_err(anyhow::Error::msg)?;
    let row = ttq::merge(&meta, &comps).map_err(anyhow::Error::msg)?;
    let mut f = score::csv_append(&cli.csv, ttq::TTQ_HEADER)?;
    writeln!(f, "{row}")?;
    println!("{row}");
    Ok(())
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
            assert!(!code.contains(r#"Command::new("sh")"#), "{f}");
            assert!(!code.contains(r#"Command::new("bash")"#), "{f}");
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
