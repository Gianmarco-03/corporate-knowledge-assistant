//! Figlio PDFium/DOCX, op `parse` (piano 01-07): `parse-child --input <file> --out <dir>`.
//!
//! Gira solo come figlio isolato lanciato da `parse-parent` (argv costruito da
//! `job::resolve`). Magic bytes → prescan (`prescan::check_pdf` / `check_docx`) → per i PDF
//! PDFium (`page.text()` per pagina), per i DOCX `word/document.xml` letto con `take`. Il
//! testo va in `<out>/text.txt`; l'esito è una riga `protocol::emit`. I fallimenti del
//! documento sono esiti `failed` con exit 0: un exit diverso o un segnale li classifica il padre.
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use pdfium_render::prelude::{Pdfium, PdfiumError, PdfiumInternalError};
use s02_parser::prescan::{self, Caps, PrescanReport, Rejection};
use s02_parser::protocol::{ChildOutcome, Kind, emit, vm_peak_kb};
use serde_json::{Value, json};

/// Tetto di lettura di `word/document.xml` (somma dichiarata delle Soglie DOCX).
const DOCX_DOCUMENT_CAP: u64 = 200 * 1024 * 1024;

fn main() -> ExitCode {
    let outcome = match cli() {
        Ok((input, out)) => parse(&input, &out),
        Err(e) => ChildOutcome::failed(Kind::Unsupported, format!("cli: {e}")),
    };
    match emit(&outcome) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(1),
    }
}

/// Solo la CLI costruita da `job::resolve` per `op: parse`.
fn cli() -> anyhow::Result<(PathBuf, PathBuf)> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    match args.as_slice() {
        [a, input, b, out] if a == "--input" && b == "--out" => {
            Ok((PathBuf::from(input), PathBuf::from(out)))
        }
        _ => anyhow::bail!("attesi --input <file> --out <dir>"),
    }
}

fn rejection(r: Rejection) -> ChildOutcome {
    match r {
        Rejection::Failed(kind, detail) => ChildOutcome::Failed { kind, detail },
        Rejection::Quarantined(reasons) => ChildOutcome::Quarantined { reasons },
    }
}

fn stats(r: &PrescanReport) -> Value {
    json!({
        "objstm_count": r.objstm_count,
        "max_flate_ratio": r.max_flate_ratio,
        "max_stream_bytes": r.max_stream_bytes,
        "total_decompressed_bytes": r.total_decompressed_bytes,
        "entries": r.entries,
    })
}

fn parse(input: &Path, out: &Path) -> ChildOutcome {
    let t0 = Instant::now();
    let bytes = match std::fs::read(input) {
        Ok(b) => b,
        Err(e) => return ChildOutcome::failed(Kind::Corrupt, format!("read: {e}")),
    };
    let head = &bytes[..bytes.len().min(1024)];
    let result = if head.windows(5).any(|w| w == b"%PDF-") {
        parse_pdf(&bytes, out)
    } else if bytes.starts_with(b"PK\x03\x04") {
        drop(bytes);
        parse_docx(input, out)
    } else {
        return ChildOutcome::failed(Kind::Unsupported, "magic_bytes");
    };
    match result {
        Ok((pages, output, mut data)) => {
            data["parse_ms"] = json!(t0.elapsed().as_millis() as u64);
            data["vm_peak_kb"] = json!(vm_peak_kb());
            ChildOutcome::Ok {
                pages,
                output: Some(output.to_string_lossy().into_owned()),
                data: Some(data),
            }
        }
        Err(o) => o,
    }
}

type Parsed = Result<(Option<u64>, PathBuf, Value), ChildOutcome>;

fn write_text(out: &Path) -> Result<(PathBuf, std::fs::File), ChildOutcome> {
    let path = out.join("text.txt");
    std::fs::File::create(&path)
        .map(|f| (path, f))
        .map_err(|e| ChildOutcome::failed(Kind::ResourceLimit, format!("write: {e}")))
}

fn parse_pdf(bytes: &[u8], out: &Path) -> Parsed {
    let report = prescan::check_pdf(bytes, &Caps::default()).map_err(rejection)?;
    let dir = std::env::var_os("PDFIUM_DIR").unwrap_or_else(|| "/opt/pdfium/lib".into());
    let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&dir))
        .map_err(|e| ChildOutcome::failed(Kind::Crashed, format!("pdfium_bind: {e}")))?;
    let pdfium = Pdfium::new(bindings);
    let doc = pdfium
        .load_pdf_from_byte_slice(bytes, None)
        .map_err(|e| match e {
            PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => {
                ChildOutcome::failed(Kind::Encrypted, "pdfium_password")
            }
            other => ChildOutcome::failed(Kind::Corrupt, format!("pdfium_load: {other}")),
        })?;
    let (path, mut file) = write_text(out)?;
    let mut chars = 0u64;
    let mut pages = 0u64;
    for page in doc.pages().iter() {
        let text = page
            .text()
            .map_err(|e| ChildOutcome::failed(Kind::Corrupt, format!("pdfium_text: {e}")))?
            .all();
        chars += text.chars().count() as u64;
        pages += 1;
        file.write_all(text.as_bytes())
            .and_then(|()| file.write_all(b"\x0c"))
            .map_err(|e| ChildOutcome::failed(Kind::ResourceLimit, format!("write: {e}")))?;
    }
    let mut data = json!({"format": "pdf", "chars": chars});
    data["prescan"] = stats(&report);
    Ok((Some(pages), path, data))
}

fn parse_docx(input: &Path, out: &Path) -> Parsed {
    let report = prescan::check_docx(input, &Caps::default()).map_err(rejection)?;
    let file = std::fs::File::open(input)
        .map_err(|e| ChildOutcome::failed(Kind::Corrupt, format!("open: {e}")))?;
    let mut z = zip::ZipArchive::new(file)
        .map_err(|e| ChildOutcome::failed(Kind::Corrupt, format!("zip: {e}")))?;
    let entry = z
        .by_name("word/document.xml")
        .map_err(|e| ChildOutcome::failed(Kind::Corrupt, format!("document.xml: {e}")))?;
    let mut xml = Vec::new();
    std::io::copy(&mut entry.take(DOCX_DOCUMENT_CAP), &mut xml)
        .map_err(|e| ChildOutcome::failed(Kind::Corrupt, format!("document.xml: {e}")))?;
    let text = docx_text(&xml);
    let (path, mut f) = write_text(out)?;
    f.write_all(text.as_bytes())
        .map_err(|e| ChildOutcome::failed(Kind::ResourceLimit, format!("write: {e}")))?;
    let mut data = json!({"format": "docx", "chars": text.chars().count()});
    data["prescan"] = stats(&report);
    Ok((None, path, data))
}

/// Testo dei run `<w:t>` di `word/document.xml`, paragrafi separati da a capo.
fn docx_text(xml: &[u8]) -> String {
    let s = String::from_utf8_lossy(xml);
    let mut out = String::new();
    let mut rest: &str = &s;
    while let Some(i) = rest.find('<') {
        let after = &rest[i..];
        let Some(j) = after.find('>') else { break };
        let tag = &after[1..j];
        rest = &after[j + 1..];
        if tag == "w:t" || tag.starts_with("w:t ") {
            if let Some(end) = rest.find("</w:t>") {
                out.push_str(&unescape(&rest[..end]));
                rest = &rest[end..];
            }
        } else if tag == "/w:p" {
            out.push('\n');
        }
    }
    out
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docx_text_reads_runs_and_paragraphs() {
        let xml = br#"<w:document><w:body><w:p><w:r><w:t>Ciao</w:t></w:r><w:r><w:t xml:space="preserve"> a &amp; b</w:t></w:r></w:p><w:p><w:r><w:t>due</w:t></w:r></w:p></w:body></w:document>"#;
        assert_eq!(docx_text(xml), "Ciao a & b\ndue\n");
    }
}
