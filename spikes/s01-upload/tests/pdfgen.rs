//! Generatore di PDF (RISK-005) e prefiltro di validazione.
//!
//! - TEST-2 (supporto): PDF di dimensione esatta ai confini 25 MiB - 1, 25 MiB, 25 MiB + 1
//!   e alle dimensioni di misura 1, 10, 24 MiB; validi per `qpdf --check` (validatore
//!   indipendente) e `accepted` dal prefiltro (nessun falso positivo).
//! - TEST-3: file cifrati, XML/ZIP ostili e polyglot rifiutati o in quarantena da
//!   `validate::check`, che nel flusso gira prima di storage e DB.
//! - TEST-4 (parte non interrompibile): `validate::check` rispetta deadline e
//!   cancellazione (422 `validation_timeout`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use s01_upload::pdfgen;
use s01_upload::validate::{self, Rejection, Verdict};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;

const MIB: u64 = 1024 * 1024;
const SIZES: [u64; 6] = [MIB, 10 * MIB, 24 * MIB, 26_214_399, 26_214_400, 26_214_401];

fn chk(path: &Path) -> Result<Verdict, Rejection> {
    validate::check(
        path,
        Instant::now() + Duration::from_secs(10),
        &AtomicBool::new(false),
    )
}

fn write_tmp(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).expect("write fixture");
    p
}

fn sha_file(path: &Path) -> String {
    hex::encode(Sha256::digest(std::fs::read(path).expect("read")))
}

const CONTENT_TYPES: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;

/// DOCX sintetico: `[Content_Types].xml`, `word/document.xml` e voci extra.
fn docx(document_xml: &str, extra: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opt = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    w.start_file("[Content_Types].xml", opt).expect("zip");
    w.write_all(CONTENT_TYPES).expect("zip");
    w.start_file("word/document.xml", opt).expect("zip");
    w.write_all(document_xml.as_bytes()).expect("zip");
    for (name, data) in extra {
        w.start_file(*name, opt).expect("zip");
        w.write_all(data).expect("zip");
    }
    w.finish().expect("zip").into_inner()
}

const DOC_OK: &str = r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Relazione</w:t></w:r></w:p></w:body></w:document>"#;

fn mini_pdf(extra: &str) -> Vec<u8> {
    format!(
        "%PDF-1.7\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R {extra} >>\nendobj\n\
         2 0 obj\n<< /Type /Pages /Kids [] /Count 0 >>\nendobj\n\
         trailer\n<< /Root 1 0 R >>\n%%EOF\n"
    )
    .into_bytes()
}

// ---------------------------------------------------------------- generatore (TEST-2)

#[test]
fn test2_pdfgen_dimensione_esatta_e_prefiltro_accepted() {
    let dir = tempfile::tempdir().expect("tempdir");
    for size in SIZES {
        let p = dir.path().join(format!("gen-{size}.pdf"));
        pdfgen::generate(&p, size, 7).expect("generate");
        let len = std::fs::metadata(&p).expect("metadata").len();
        assert_eq!(len, size, "dimensione richiesta {size}, ottenuta {len}");
        assert_eq!(
            chk(&p),
            Ok(Verdict::Accepted),
            "il prefiltro non deve segnalare il PDF generato da {size} B"
        );
        std::fs::remove_file(&p).expect("rm");
    }
}

#[test]
fn test2_pdfgen_deterministico_per_size_e_seed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = dir.path().join("a.pdf");
    let b = dir.path().join("b.pdf");
    let c = dir.path().join("c.pdf");
    pdfgen::generate(&a, MIB, 3).expect("generate");
    pdfgen::generate(&b, MIB, 3).expect("generate");
    pdfgen::generate(&c, MIB, 4).expect("generate");
    assert_eq!(
        sha_file(&a),
        sha_file(&b),
        "stesso (size, seed) → stessi byte"
    );
    assert_ne!(sha_file(&a), sha_file(&c), "seed diverso → corpo diverso");
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "qpdf è nel runner Linux: eseguire via session.sh oneshot"
)]
fn test2_pdfgen_qpdf_check_verde() {
    let dir = tempfile::tempdir().expect("tempdir");
    for size in SIZES {
        let p = dir.path().join(format!("q-{size}.pdf"));
        pdfgen::generate(&p, size, 11).expect("generate");
        let out = std::process::Command::new("qpdf")
            .arg("--check")
            .arg(&p)
            .output()
            .expect("qpdf nel PATH");
        assert!(
            out.status.success(),
            "qpdf --check su {size} B: {}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stdout)
        );
        std::fs::remove_file(&p).expect("rm");
    }
}

// ---------------------------------------------------------------- prefiltro (TEST-3)

#[test]
fn test3_pdf_cifrato_rifiutato_422() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut pdf = mini_pdf("");
    pdf.extend_from_slice(b"trailer\n<< /Root 1 0 R /Encrypt 3 0 R >>\n%%EOF\n");
    let p = write_tmp(dir.path(), "cifrato.pdf", &pdf);
    let r = chk(&p).expect_err("PDF con /Encrypt va rifiutato");
    assert_eq!(r.http_status, 422, "{r:?}");

    // Nome offuscato con `#hh` (`#79` = `y`): il prefiltro decodifica i nomi.
    let mut pdf = mini_pdf("");
    pdf.extend_from_slice(b"trailer\n<< /Root 1 0 R /Encr#79pt 3 0 R >>\n%%EOF\n");
    let p = write_tmp(dir.path(), "cifrato-offuscato.pdf", &pdf);
    let r = chk(&p).expect_err("PDF con /Encr#79pt va rifiutato");
    assert_eq!(r.http_status, 422, "{r:?}");
}

#[test]
fn test3_pdf_attivi_in_quarantena() {
    let dir = tempfile::tempdir().expect("tempdir");
    let casi = [
        (
            "javascript",
            "/OpenAction << /S /JavaScript /JS (app.alert(1)) >>",
        ),
        ("js", "/AA << /O << /JS (x) >> >>"),
        (
            "offuscato",
            "/OpenAction << /S /J#61vaScript /JS (app.alert(1)) >>",
        ),
        ("launch", "/OpenAction << /S /Launch /F (calc.exe) >>"),
        (
            "embedded",
            "/Names << /EmbeddedFiles << /Names [(a) << /Type /EmbeddedFile >>] >> >>",
        ),
    ];
    for (nome, extra) in casi {
        let p = write_tmp(dir.path(), &format!("{nome}.pdf"), &mini_pdf(extra));
        assert_eq!(chk(&p), Ok(Verdict::Quarantined), "caso {nome}");
    }
}

#[test]
fn test3_magic_mismatch_e_polyglot_415() {
    let dir = tempfile::tempdir().expect("tempdir");
    // ZIP con nome .pdf ma senza le parti DOCX: magic ZIP, nessun formato ammesso.
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    w.start_file("doc.pdf", SimpleFileOptions::default())
        .expect("zip");
    w.write_all(&mini_pdf("")).expect("zip");
    let poly = w.finish().expect("zip").into_inner();
    let casi: [(&str, Vec<u8>); 3] = [
        ("polyglot.pdf", poly),
        (
            "prefisso.pdf",
            [b"MZ\x90\x00".as_slice(), &mini_pdf("")].concat(),
        ),
        ("binario.md", vec![0x23, 0x20, 0x00, 0xff, 0xfe, 0x00]),
    ];
    for (nome, bytes) in casi {
        let p = write_tmp(dir.path(), nome, &bytes);
        let r = chk(&p).expect_err(nome);
        assert_eq!(r.http_status, 415, "caso {nome}: {r:?}");
    }
}

#[test]
fn test3_markdown_e_docx_validi_accepted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let md = write_tmp(
        dir.path(),
        "nota.md",
        "# Relazione\n\nUtile netto in crescita.\n".as_bytes(),
    );
    assert_eq!(chk(&md), Ok(Verdict::Accepted));
    let d = write_tmp(dir.path(), "ok.docx", &docx(DOC_OK, &[]));
    assert_eq!(chk(&d), Ok(Verdict::Accepted));
}

#[test]
fn test3_docx_con_dtd_o_entita_422() {
    let dir = tempfile::tempdir().expect("tempdir");
    let xml = r#"<?xml version="1.0"?><!DOCTYPE w [<!ENTITY a "aaaaaaaaaa">]><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>&a;</w:body></w:document>"#;
    let p = write_tmp(dir.path(), "dtd.docx", &docx(xml, &[]));
    let r = chk(&p).expect_err("DOCX con DTD/entità va rifiutato");
    assert_eq!(r.http_status, 422, "{r:?}");
}

#[test]
fn test3_docx_oltre_1000_voci_422() {
    let dir = tempfile::tempdir().expect("tempdir");
    let extra: Vec<(String, Vec<u8>)> = (0..1000)
        .map(|i| (format!("word/media/x{i}.bin"), vec![b'a'; 8]))
        .collect();
    let extra_ref: Vec<(&str, Vec<u8>)> =
        extra.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
    // 2 parti obbligatorie + 1000 = 1002 voci > 1000.
    let p = write_tmp(dir.path(), "voci.docx", &docx(DOC_OK, &extra_ref));
    let r = chk(&p).expect_err("DOCX con più di 1000 voci va rifiutato");
    assert_eq!(r.http_status, 422, "{r:?}");
}

#[test]
fn test3_docx_rapporto_di_compressione_oltre_100_sopra_16mib_422() {
    let dir = tempfile::tempdir().expect("tempdir");
    let zeros = vec![0u8; (17 * MIB) as usize];
    let p = write_tmp(
        dir.path(),
        "bomba.docx",
        &docx(DOC_OK, &[("word/media/zeri.bin", zeros)]),
    );
    let r = chk(&p).expect_err("voce oltre 16 MiB con rapporto > 100 va rifiutata");
    assert_eq!(r.http_status, 422, "{r:?}");
}

#[test]
fn test3_docx_somma_dichiarata_oltre_200mib_422() {
    let dir = tempfile::tempdir().expect("tempdir");
    // 13 voci da 16 MiB (nessuna oltre 16 MiB, quindi la regola del rapporto non
    // scatta): somma dei `size()` dichiarati 208 MiB > 200 MiB.
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opt = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(1));
    w.start_file("[Content_Types].xml", opt).expect("zip");
    w.write_all(CONTENT_TYPES).expect("zip");
    w.start_file("word/document.xml", opt).expect("zip");
    w.write_all(DOC_OK.as_bytes()).expect("zip");
    let zeros = vec![0u8; (16 * MIB) as usize];
    for i in 0..13 {
        w.start_file(format!("word/media/s{i}.bin"), opt)
            .expect("zip");
        w.write_all(&zeros).expect("zip");
    }
    let bytes = w.finish().expect("zip").into_inner();
    let p = write_tmp(dir.path(), "somma.docx", &bytes);
    let r = chk(&p).expect_err("somma dichiarata oltre 200 MiB va rifiutata");
    assert_eq!(r.http_status, 422, "{r:?}");
}

#[test]
fn test3_docx_con_macro_rifiutato() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = write_tmp(
        dir.path(),
        "macro.docx",
        &docx(
            DOC_OK,
            &[("word/vbaProject.bin", vec![0xd0, 0xcf, 0x11, 0xe0])],
        ),
    );
    let r = chk(&p).expect_err("vbaProject.bin va rifiutato");
    assert!((400..500).contains(&r.http_status), "{r:?}");
}

// ---------------------------------------------------------------- deadline (TEST-4)

#[test]
fn test4_validazione_oltre_deadline_422_validation_timeout() {
    let dir = tempfile::tempdir().expect("tempdir");
    let extra: Vec<(String, Vec<u8>)> = (0..990)
        .map(|i| (format!("word/media/x{i}.bin"), vec![b'a'; 4096]))
        .collect();
    let extra_ref: Vec<(&str, Vec<u8>)> =
        extra.iter().map(|(n, d)| (n.as_str(), d.clone())).collect();
    let p = write_tmp(dir.path(), "lento.docx", &docx(DOC_OK, &extra_ref));
    // Deadline già scaduta: il controllo per voce deve interrompere la validazione.
    let r =
        validate::check(&p, Instant::now(), &AtomicBool::new(false)).expect_err("deadline scaduta");
    assert_eq!(r.http_status, 422, "{r:?}");
    assert_eq!(r.code, "validation_timeout", "{r:?}");
}

#[test]
fn test4_validazione_cancellata_si_interrompe() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path().join("grande.pdf");
    std::fs::write(&p, mini_pdf("")).expect("write");
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&p)
        .expect("open");
    f.write_all(&vec![b'0'; (4 * MIB) as usize]).expect("pad");
    drop(f);
    let r = validate::check(
        &p,
        Instant::now() + Duration::from_secs(10),
        &AtomicBool::new(true),
    )
    .expect_err("cancellazione richiesta prima della scansione");
    assert_eq!(r.http_status, 422, "{r:?}");
}
