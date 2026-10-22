//! Sottocomando `pdf` del bin `extract`: figlio di `op: extract_pdf` (piano 01-08).
//!
//! Gira solo come figlio isolato lanciato da `parse-parent` con l'argv costruito da
//! `job::resolve` di 01-07. Contratto:
//! - `check_pdf(&bytes, caps)` prima di PDFium; un rifiuto è `failed`/`quarantined` e non
//!   scrive nulla in `out` (PDFium mai legato in quei casi);
//! - con `Ok(_)`: un JSON per pagina in `out` (`page-NNNN.json`) con `page` (1-based),
//!   `width`, `height` (pagina ruotata), `rotation` (gradi), `media_box`, `crop_box`,
//!   `crop_differs`, `text` (testo di PDFium), `chars`, `words` e `lines`, ciascuno
//!   `{"text": …, "bbox": [x0, y0, x1, y1]}` in punti PDF, origine in basso a sinistra, nello
//!   spazio della pagina dopo `/Rotate`; esito `ok` con `pages`, `output` = `out` e `data` con
//!   i conteggi `chars` e `words`, `vm_peak_kb`/`vm_hwm_kb` per `calibrate` e i campi del
//!   rapporto di prescan.
use std::io::Write as _;
use std::path::Path;
use std::time::Instant;

use pdfium_render::prelude::{
    PdfPageRenderRotation, PdfRect, Pdfium, PdfiumError, PdfiumInternalError,
};
use s02_parser::prescan::{self, Caps, PrescanReport, Rejection};
use s02_parser::protocol::{ChildOutcome, Kind, vm_hwm_kb, vm_peak_kb};
use serde_json::{Value, json};

/// Cifre decimali dei bbox nel JSON di pagina (0,01 pt).
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn rejection(r: Rejection) -> ChildOutcome {
    match r {
        Rejection::Failed(kind, detail) => ChildOutcome::Failed { kind, detail },
        Rejection::Quarantined(reasons) => ChildOutcome::Quarantined { reasons },
    }
}

/// Campi del rapporto di prescan in `data` (come `parse-child`), letti da `calibrate`.
fn stats(r: &PrescanReport) -> Value {
    json!({
        "encryption": r.encryption.as_str(),
        "embedded_files": r.embedded_files,
        "objstm_count": r.objstm_count,
        "objstm_decompressed_bytes": r.objstm_decompressed_bytes,
        "xref_streams": r.xref_streams,
        "decompress_work_bytes": r.decompress_work_bytes,
        "max_flate_ratio": r.max_flate_ratio,
        "max_stream_bytes": r.max_stream_bytes,
        "total_decompressed_bytes": r.total_decompressed_bytes,
        "entries": r.entries,
    })
}

/// Trasformazione dallo spazio utente allo spazio della pagina ruotata (Contratti).
#[derive(Debug, Clone, Copy)]
struct Frame {
    media: [f64; 4],
    rotation: u32,
}

impl Frame {
    fn point(&self, x: f64, y: f64) -> (f64, f64) {
        let [x0, y0, x1, y1] = self.media;
        match self.rotation {
            90 => (y - y0, x1 - x),
            180 => (x1 - x, y1 - y),
            270 => (y1 - y, x - x0),
            _ => (x - x0, y - y0),
        }
    }

    fn rect(&self, r: &PdfRect) -> [f64; 4] {
        let (ax, ay) = self.point(r.left().value as f64, r.bottom().value as f64);
        let (bx, by) = self.point(r.right().value as f64, r.top().value as f64);
        [
            round2(ax.min(bx)),
            round2(ay.min(by)),
            round2(ax.max(bx)),
            round2(ay.max(by)),
        ]
    }
}

fn rect4(r: &PdfRect) -> [f64; 4] {
    [
        r.left().value as f64,
        r.bottom().value as f64,
        r.right().value as f64,
        r.top().value as f64,
    ]
}

fn unite(acc: &mut Option<[f64; 4]>, b: [f64; 4]) {
    *acc = Some(match *acc {
        None => b,
        Some(a) => [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ],
    });
}

/// Accumulatore di una sequenza di caratteri (parola o riga).
#[derive(Default)]
struct Run {
    text: String,
    bbox: Option<[f64; 4]>,
}

impl Run {
    fn push(&mut self, s: &str, b: Option<[f64; 4]>) {
        self.text.push_str(s);
        if let Some(b) = b {
            unite(&mut self.bbox, b);
        }
    }

    /// Chiude la sequenza in `out` (solo se ha testo non bianco e un bbox).
    fn flush(&mut self, out: &mut Vec<Value>) {
        let r = std::mem::take(self);
        let text = r.text.trim();
        if let (false, Some(b)) = (text.is_empty(), r.bbox) {
            out.push(json!({"text": text, "bbox": b}));
        }
    }
}

fn page_json(page: &pdfium_render::prelude::PdfPage, number: u32) -> Result<Value, ChildOutcome> {
    let corrupt = |what: &str, e: PdfiumError| {
        ChildOutcome::failed(Kind::Corrupt, format!("pdfium_{what}: {e}"))
    };
    let rotation = match page.rotation().map_err(|e| corrupt("rotation", e))? {
        PdfPageRenderRotation::None => 0,
        PdfPageRenderRotation::Degrees90 => 90,
        PdfPageRenderRotation::Degrees180 => 180,
        PdfPageRenderRotation::Degrees270 => 270,
    };
    let media = page
        .boundaries()
        .media()
        .map(|b| rect4(&b.bounds))
        .unwrap_or([
            0.0,
            0.0,
            page.width().value as f64,
            page.height().value as f64,
        ]);
    let crop = page.boundaries().crop().map(|b| rect4(&b.bounds)).ok();
    let crop_differs = crop.is_some_and(|c| c.iter().zip(media).any(|(a, b)| (a - b).abs() > 0.01));
    let frame = Frame { media, rotation };
    // Larghezza e altezza della pagina ruotata nel MediaBox, lo stesso riferimento dei bbox
    // (PDFium misura la pagina sul CropBox, che può essere più piccolo).
    let (mw, mh) = (media[2] - media[0], media[3] - media[1]);
    let (width, height) = if matches!(rotation, 90 | 270) {
        (mh, mw)
    } else {
        (mw, mh)
    };
    let text = page.text().map_err(|e| corrupt("text", e))?;
    let all = text.all();

    let mut chars = Vec::new();
    let mut words = Vec::new();
    let mut lines = Vec::new();
    let (mut word, mut line) = (Run::default(), Run::default());
    for ch in text.chars().iter() {
        let s = ch.unicode_string().unwrap_or_default();
        if s == "\r" || s == "\n" {
            word.flush(&mut words);
            line.flush(&mut lines);
            continue;
        }
        if s.chars().all(char::is_whitespace) {
            word.flush(&mut words);
            line.push(" ", None);
            continue;
        }
        let b = ch.loose_bounds().ok().map(|r| frame.rect(&r));
        if let Some(b) = b {
            chars.push(json!({"text": s, "bbox": b}));
        }
        word.push(&s, b);
        line.push(&s, b);
    }
    word.flush(&mut words);
    line.flush(&mut lines);

    Ok(json!({
        "page": number,
        "width": round2(width),
        "height": round2(height),
        "rotation": rotation,
        "media_box": media,
        "crop_box": crop,
        "crop_differs": crop_differs,
        "text": all,
        "chars": chars,
        "words": words,
        "lines": lines,
    }))
}

/// Estrazione di testo e bbox di `input` in `out` (già creata dal padre).
pub(crate) fn run_pdf(input: &Path, out: &Path, caps: &Caps, pdfium_dir: &Path) -> ChildOutcome {
    let t0 = Instant::now();
    let bytes = match std::fs::read(input) {
        Ok(b) => b,
        Err(e) => return ChildOutcome::failed(Kind::Corrupt, format!("read: {e}")),
    };
    // Prescan prima di qualunque contatto con PDFium: un rifiuto non scrive nulla.
    let report = match prescan::check_pdf(&bytes, caps) {
        Ok(r) => r,
        Err(r) => return rejection(r),
    };
    let bindings =
        match Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(pdfium_dir)) {
            Ok(b) => b,
            Err(e) => return ChildOutcome::failed(Kind::Crashed, format!("pdfium_bind: {e}")),
        };
    let pdfium = Pdfium::new(bindings);
    let doc = match pdfium.load_pdf_from_byte_slice(&bytes, None) {
        Ok(d) => d,
        Err(PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError)) => {
            return ChildOutcome::failed(Kind::Encrypted, "pdfium_password");
        }
        Err(e) => return ChildOutcome::failed(Kind::Corrupt, format!("pdfium_load: {e}")),
    };
    let (mut pages, mut n_chars, mut n_words) = (0u64, 0u64, 0u64);
    for (i, page) in doc.pages().iter().enumerate() {
        let number = i as u32 + 1;
        let v = match page_json(&page, number) {
            Ok(v) => v,
            Err(o) => return o,
        };
        n_chars += v["chars"].as_array().map_or(0, Vec::len) as u64;
        n_words += v["words"].as_array().map_or(0, Vec::len) as u64;
        let path = out.join(format!("page-{number:04}.json"));
        // Scrittura bufferizzata: senza buffer serde_json fa una chiamata di sistema per
        // token, e sul bind mount di Docker Desktop ognuna costa millisecondi.
        let written = std::fs::File::create(&path).and_then(|f| {
            let mut w = std::io::BufWriter::new(f);
            serde_json::to_writer(&mut w, &v)?;
            w.flush()
        });
        if let Err(e) = written {
            return ChildOutcome::failed(Kind::ResourceLimit, format!("write: {e}"));
        }
        pages += 1;
    }
    let mut data = stats(&report);
    data["format"] = json!("pdf");
    data["pages"] = json!(pages);
    data["chars"] = json!(n_chars);
    data["words"] = json!(n_words);
    data["parse_ms"] = json!(t0.elapsed().as_millis() as u64);
    data["vm_peak_kb"] = json!(vm_peak_kb());
    data["vm_hwm_kb"] = json!(vm_hwm_kb());
    ChildOutcome::Ok {
        pages: Some(pages),
        output: Some(out.to_string_lossy().into_owned()),
        data: Some(data),
    }
}

#[cfg(test)]
mod tests {
    //! TEST-1 (prescan prima di PDFium, nessuna scrittura sui rifiuti) e contratto delle
    //! coordinate del JSON di pagina (PDFium vero, `chromium/8100`, come in `tests/isolation.rs`).
    use super::*;
    use s02_parser::protocol::Kind;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;

    // ---- fixture PDF (stesso schema dei test di prescan.rs, copiato: quelli sono privati) --

    fn build(objs: &[(u32, Vec<u8>)]) -> Vec<u8> {
        let mut out: Vec<u8> = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let size = objs.iter().map(|(n, _)| *n).max().unwrap_or(0) + 1;
        let mut offsets = vec![None; size as usize];
        for (n, body) in objs {
            offsets[*n as usize] = Some(out.len());
            out.extend_from_slice(format!("{n} 0 obj\n").as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {size}\n0000000000 65535 f \n").as_bytes());
        for off in offsets.iter().skip(1) {
            match off {
                Some(o) => out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes()),
                None => out.extend_from_slice(b"0000000000 65535 f \n"),
            }
        }
        out.extend_from_slice(
            format!("trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n")
                .as_bytes(),
        );
        out
    }

    fn stream(data: &str) -> Vec<u8> {
        format!("<< /Length {} >>\nstream\n{data}\nendstream", data.len()).into_bytes()
    }

    /// Una pagina per testo, Helvetica 12 pt a (72, 712) su 612 × 792; `catalog_extra` e
    /// `extra` per le azioni attive; `rotate` per `/Rotate`.
    fn doc(
        texts: &[&str],
        rotate: u32,
        catalog_extra: &str,
        extra: Vec<(u32, Vec<u8>)>,
    ) -> Vec<u8> {
        let kids: Vec<String> = (0..texts.len())
            .map(|i| format!("{} 0 R", 10 + i))
            .collect();
        let mut objs = vec![
            (
                1,
                format!("<< /Type /Catalog /Pages 2 0 R {catalog_extra} >>").into_bytes(),
            ),
            (
                2,
                format!(
                    "<< /Type /Pages /Kids [{}] /Count {} >>",
                    kids.join(" "),
                    texts.len()
                )
                .into_bytes(),
            ),
            (
                3,
                b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
            ),
        ];
        for (i, t) in texts.iter().enumerate() {
            let i = i as u32;
            objs.push((
                10 + i,
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Rotate {rotate} \
                     /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
                    100 + i
                )
                .into_bytes(),
            ));
            objs.push((
                100 + i,
                stream(&format!("BT /F1 12 Tf 72 712 Td ({t}) Tj ET")),
            ));
        }
        objs.extend(extra);
        build(&objs)
    }

    // ---- ambiente ---------------------------------------------------------------------------

    /// Cartella di prova vuota: `<S02_TEST_BASE | temp>/s02-extract-<pid>-<name>`.
    fn scratch(name: &str) -> PathBuf {
        let root = std::env::var_os("S02_TEST_BASE").map_or_else(std::env::temp_dir, PathBuf::from);
        let d = root.join(format!("s02-extract-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("out")).unwrap();
        d
    }

    /// `(input, out)` con il PDF scritto.
    fn case(name: &str, pdf: &[u8]) -> (PathBuf, PathBuf) {
        let d = scratch(name);
        let input = d.join("in.pdf");
        fs::write(&input, pdf).unwrap();
        (input, d.join("out"))
    }

    fn entries(dir: &Path) -> Vec<PathBuf> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect()
    }

    /// Cartella inesistente: se il figlio legasse PDFium prima del prescan l'esito sarebbe
    /// `Crashed`/`pdfium_bind`, non il rifiuto del prescan.
    fn no_pdfium() -> PathBuf {
        PathBuf::from("/nonexistent/pdfium-not-here")
    }

    /// Stessa ricerca di `tests/isolation.rs`: `S02_TEST_PDFIUM_DIR`, `PDFIUM_DIR`,
    /// `/opt/pdfium/lib`, `/src/target/pdfium/lib` (copia ignorata nel runner).
    fn pdfium_dir() -> PathBuf {
        let mut c: Vec<PathBuf> = ["S02_TEST_PDFIUM_DIR", "PDFIUM_DIR"]
            .iter()
            .filter_map(|k| std::env::var_os(k).filter(|v| !v.is_empty()))
            .map(PathBuf::from)
            .collect();
        c.push(PathBuf::from("/opt/pdfium/lib"));
        c.push(PathBuf::from("/src/target/pdfium/lib"));
        c.iter()
            .find(|d| d.join("libpdfium.so").is_file())
            .cloned()
            .unwrap_or_else(|| panic!("PDFium assente in {c:?}: copiarlo in spikes/target/pdfium"))
    }

    fn pages_json(out: &Path) -> Vec<Value> {
        let mut v: Vec<Value> = entries(out)
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .map(|p| serde_json::from_slice(&fs::read(&p).unwrap()).unwrap())
            .collect();
        v.sort_by_key(|p| p["page"].as_u64());
        v
    }

    fn bbox(v: &Value) -> [f64; 4] {
        let a = v["bbox"].as_array().expect("bbox array");
        assert_eq!(a.len(), 4, "bbox {v}");
        [0, 1, 2, 3].map(|i| a[i].as_f64().expect("bbox numerico"))
    }

    // ---- TEST-1: prescan prima di PDFium ----------------------------------------------------

    #[test]
    fn javascript_pdf_is_quarantined_before_pdfium_and_writes_nothing() {
        let pdf = doc(
            &["Ciao"],
            0,
            "/OpenAction 50 0 R",
            vec![(50, b"<< /S /JavaScript /JS (app.alert\\(1\\)) >>".to_vec())],
        );
        let (input, out) = case("js", &pdf);
        match run_pdf(&input, &out, &Caps::default(), &no_pdfium()) {
            ChildOutcome::Quarantined { reasons } => {
                assert!(reasons.contains(&"javascript".to_owned()), "{reasons:?}")
            }
            other => panic!("atteso quarantined dal prescan, ottenuto {other:?}"),
        }
        assert!(entries(&out).is_empty(), "scritto qualcosa in out");
    }

    #[test]
    fn garbage_is_failed_by_prescan_and_writes_nothing() {
        let (input, out) = case("garbage", b"questo non e' un PDF");
        match run_pdf(&input, &out, &Caps::default(), &no_pdfium()) {
            ChildOutcome::Failed { kind, detail } => {
                assert_ne!(
                    kind,
                    Kind::Crashed,
                    "PDFium legato prima del prescan: {detail}"
                );
                assert!(!detail.contains("pdfium"), "dettaglio da PDFium: {detail}");
            }
            other => panic!("atteso failed, ottenuto {other:?}"),
        }
        assert!(entries(&out).is_empty());
    }

    #[test]
    fn caps_from_caller_are_enforced_before_pdfium() {
        let (input, out) = case("pages-cap", &doc(&["uno", "due"], 0, "", vec![]));
        let caps = Caps {
            max_pages: 1,
            ..Caps::default()
        };
        match run_pdf(&input, &out, &caps, &no_pdfium()) {
            ChildOutcome::Failed { kind, .. } => assert_eq!(kind, Kind::ResourceLimit),
            other => panic!("atteso ResourceLimit dal prescan, ottenuto {other:?}"),
        }
        assert!(entries(&out).is_empty());
    }

    // ---- contratto del JSON di pagina (PDFium vero) -----------------------------------------

    #[test]
    fn benign_pdf_gives_one_json_per_page_in_pdf_points_bottom_left() {
        let pdf = doc(&["Ciao mondo", "Pagina due"], 0, "", vec![]);
        let (input, out) = case("benign", &pdf);
        let o = run_pdf(&input, &out, &Caps::default(), &pdfium_dir());
        let ChildOutcome::Ok {
            pages,
            output,
            data,
        } = o
        else {
            panic!("atteso ok, ottenuto {o:?}");
        };
        assert_eq!(pages, Some(2));
        assert_eq!(output.as_deref(), Some(out.to_str().unwrap()));
        let data = data.expect("data con i conteggi");
        assert!(data["chars"].as_u64().unwrap() >= 18, "{data}");
        assert_eq!(data["words"].as_u64(), Some(4), "{data}");

        let ps = pages_json(&out);
        assert_eq!(ps.len(), 2, "un JSON per pagina");
        assert_eq!(ps[0]["page"].as_u64(), Some(1), "pagine 1-based");
        assert_eq!(ps[1]["page"].as_u64(), Some(2));
        for p in &ps {
            assert!((p["width"].as_f64().unwrap() - 612.0).abs() < 0.5, "{p}");
            assert!((p["height"].as_f64().unwrap() - 792.0).abs() < 0.5, "{p}");
            assert_eq!(p["rotation"].as_u64(), Some(0), "{p}");
        }

        let words: Vec<&Value> = ps[0]["words"].as_array().unwrap().iter().collect();
        let texts: Vec<&str> = words.iter().map(|w| w["text"].as_str().unwrap()).collect();
        assert_eq!(texts, ["Ciao", "mondo"]);
        let (ciao, mondo) = (bbox(words[0]), bbox(words[1]));
        // Testo a (72, 712) in punti PDF con origine in basso a sinistra: la y resta vicina a
        // 712 (con origine in alto sarebbe ≈ 80).
        assert!((70.0..=76.0).contains(&ciao[0]), "x0 {ciao:?}");
        assert!((700.0..=715.0).contains(&ciao[1]), "y0 {ciao:?}");
        assert!((712.0..=730.0).contains(&ciao[3]), "y1 {ciao:?}");
        assert!(
            ciao[0] < ciao[2] && ciao[1] < ciao[3],
            "bbox [x0,y0,x1,y1] {ciao:?}"
        );
        assert!(mondo[0] > ciao[2], "parole in ordine: {ciao:?} {mondo:?}");

        let chars = ps[0]["chars"].as_array().unwrap();
        let first = chars
            .iter()
            .find(|c| c["text"] == "C")
            .expect("carattere C");
        let c = bbox(first);
        assert!(
            (70.0..=76.0).contains(&c[0]) && (700.0..=715.0).contains(&c[1]),
            "{c:?}"
        );
    }

    #[test]
    fn rotation_is_reported_in_degrees() {
        let (input, out) = case("rot", &doc(&["Ruotata"], 90, "", vec![]));
        let o = run_pdf(&input, &out, &Caps::default(), &pdfium_dir());
        assert_eq!(o.status(), "ok", "{o:?}");
        let ps = pages_json(&out);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0]["rotation"].as_u64(), Some(90), "{}", ps[0]);
    }
}
