//! Prescan anti-bomb di PDF e DOCX, nel figlio prima di PDFium (contratto W17, punto 5).
// stub del test-write: da implementare in 01-07
use std::path::Path;

use crate::protocol::Kind;

/// Tetti del prescan (Soglie del lanciatore, Task 1 di 01-07).
#[derive(Debug, Clone)]
pub struct Caps {
    /// Tetto per stream, anche al caricamento (`max_decompressed_size`): 64 MiB.
    pub per_stream_bytes: u64,
    /// Totale decompresso, caricamento compreso: 512 MiB.
    pub total_bytes: u64,
    /// Rapporto massimo di espansione: 100.
    pub max_ratio: u64,
    /// Il rapporto vale oltre questa soglia di byte espansi: 16 MiB.
    pub ratio_min_bytes: u64,
    /// Object stream ammessi (2 × massimo del corpus benigno).
    pub max_object_streams: u32,
    /// Lunghezza massima della catena di filtri: 2.
    pub max_filter_chain: usize,
    /// Limite di pagine: 2.000.
    pub max_pages: u32,
    /// Tetto `Width × Height × componenti` dei filtri immagine: 64 MiB.
    pub max_image_bytes: u64,
    /// Voci DOCX ammesse: 1.000.
    pub docx_max_entries: usize,
    /// Somma dichiarata delle voci DOCX: 200 MiB.
    pub docx_max_total_bytes: u64,
}

impl Default for Caps {
    fn default() -> Self {
        // stub del test-write: da implementare in 01-07
        todo!()
    }
}

/// Esito positivo del prescan.
#[derive(Debug, Clone)]
pub struct PrescanReport {
    // stub del test-write: da implementare in 01-07
    _stub: (),
}

/// Rifiuto tipizzato del prescan.
#[derive(Debug, Clone, PartialEq)]
pub enum Rejection {
    Failed(Kind, String),
    Quarantined(Vec<String>),
}

pub fn check_pdf(_bytes: &[u8], _caps: &Caps) -> Result<PrescanReport, Rejection> {
    // stub del test-write: da implementare in 01-07
    todo!()
}

pub fn check_docx(_path: &Path, _caps: &Caps) -> Result<PrescanReport, Rejection> {
    // stub del test-write: da implementare in 01-07
    todo!()
}

#[cfg(test)]
mod tests {
    //! TEST-7 e TEST-8: rifiuti del prescan prima di PDFium, con PDF e DOCX costruiti a mano.
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    const MIB: u64 = 1024 * 1024;
    const KIB: u64 = 1024;

    // ---- costruzione dei PDF ----------------------------------------------------------

    /// PDF con tabella xref classica; l'oggetto 1 è il Catalog.
    fn build(objs: &[(u32, Vec<u8>)], trailer_extra: &str) -> Vec<u8> {
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
            format!("trailer\n<< /Size {size} /Root 1 0 R {trailer_extra} >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
        );
        out
    }

    fn dict(s: &str) -> Vec<u8> {
        s.as_bytes().to_vec()
    }

    fn stream(dict_extra: &str, data: &[u8]) -> Vec<u8> {
        let mut v = format!("<< /Length {} {dict_extra} >>\nstream\n", data.len()).into_bytes();
        v.extend_from_slice(data);
        v.extend_from_slice(b"\nendstream");
        v
    }

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    /// `n` byte a zero compressi in streaming (nessun buffer da `n` byte).
    fn zlib_zeros(n: u64) -> Vec<u8> {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        let chunk = vec![0u8; 64 * 1024];
        let mut left = n;
        while left > 0 {
            let k = left.min(chunk.len() as u64) as usize;
            e.write_all(&chunk[..k]).unwrap();
            left -= k as u64;
        }
        e.finish().unwrap()
    }

    /// Byte pseudo-casuali, incomprimibili (xorshift).
    fn noise(n: usize, seed: u64) -> Vec<u8> {
        let mut x = seed | 1;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x >> 24) as u8
            })
            .collect()
    }

    /// Codifica ASCIIHex con il marcatore di fine `>`.
    fn hex(data: &[u8]) -> Vec<u8> {
        let mut v: Vec<u8> = data
            .iter()
            .flat_map(|b| format!("{b:02X}").into_bytes())
            .collect();
        v.push(b'>');
        v
    }

    /// Documento di `pages` pagine (oggetti 10..), contenuti 100.., più gli oggetti `extra`.
    fn doc(
        pages: u32,
        catalog_extra: &str,
        extra: Vec<(u32, Vec<u8>)>,
        trailer_extra: &str,
    ) -> Vec<u8> {
        let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", 10 + i)).collect();
        let mut objs = vec![
            (
                1,
                dict(&format!(
                    "<< /Type /Catalog /Pages 2 0 R {catalog_extra} >>"
                )),
            ),
            (
                2,
                dict(&format!(
                    "<< /Type /Pages /Kids [{}] /Count {pages} >>",
                    kids.join(" ")
                )),
            ),
        ];
        for i in 0..pages {
            objs.push((
                10 + i,
                dict(&format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {} 0 R >>",
                    100 + i
                )),
            ));
            objs.push((100 + i, stream("", b"BT /F1 12 Tf 72 712 Td (Ciao) Tj ET")));
        }
        objs.extend(extra);
        build(&objs, trailer_extra)
    }

    /// Una pagina con un solo stream di contenuto, filtri e dati scelti dal test.
    fn doc_with_content(filter: &str, data: &[u8]) -> Vec<u8> {
        build(
            &[
                (1, dict("<< /Type /Catalog /Pages 2 0 R >>")),
                (2, dict("<< /Type /Pages /Kids [3 0 R] /Count 1 >>")),
                (
                    3,
                    dict("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R >>"),
                ),
                (4, stream(filter, data)),
            ],
            "",
        )
    }

    /// Object stream Flate che contiene gli oggetti `(numero, corpo)`.
    fn objstm(objects: &[(u32, &[u8])]) -> Vec<u8> {
        let mut header = String::new();
        let mut body: Vec<u8> = Vec::new();
        for (n, b) in objects {
            header.push_str(&format!("{n} {} ", body.len()));
            body.extend_from_slice(b);
            body.push(b' ');
        }
        let first = header.len();
        let mut plain = header.into_bytes();
        plain.extend_from_slice(&body);
        stream(
            &format!(
                "/Type /ObjStm /N {} /First {first} /Filter /FlateDecode",
                objects.len()
            ),
            &zlib(&plain),
        )
    }

    /// Tetti ridotti per i test; i campi non toccati sono le Soglie (`Caps::default`).
    fn caps() -> Caps {
        Caps {
            per_stream_bytes: MIB,
            total_bytes: 4 * MIB,
            max_object_streams: 4,
            max_pages: 10,
            ..Caps::default()
        }
    }

    fn kind_of(r: Result<PrescanReport, Rejection>) -> Kind {
        match r {
            Err(Rejection::Failed(k, _)) => k,
            other => panic!("atteso Failed, ottenuto {other:?}"),
        }
    }

    fn reasons_of(r: Result<PrescanReport, Rejection>) -> Vec<String> {
        match r {
            Err(Rejection::Quarantined(r)) => r,
            other => panic!("atteso Quarantined, ottenuto {other:?}"),
        }
    }

    fn assert_ok(r: Result<PrescanReport, Rejection>) {
        if let Err(e) = r {
            panic!("atteso Ok, ottenuto {e:?}");
        }
    }

    // ---- soglie (Task 1) ---------------------------------------------------------------

    #[test]
    fn default_caps_are_the_launcher_thresholds() {
        let c = Caps::default();
        assert_eq!(c.per_stream_bytes, 64 * MIB);
        assert_eq!(c.total_bytes, 512 * MIB);
        assert_eq!(c.max_ratio, 100);
        assert_eq!(c.ratio_min_bytes, 16 * MIB);
        assert_eq!(c.max_filter_chain, 2);
        assert_eq!(c.max_pages, 2000);
        assert_eq!(c.max_image_bytes, 64 * MIB);
        assert_eq!(c.docx_max_entries, 1000);
        assert_eq!(c.docx_max_total_bytes, 200 * MIB);
    }

    // ---- PDF benigno -------------------------------------------------------------------

    #[test]
    fn benign_pdf_passes() {
        assert_ok(check_pdf(&doc(1, "", vec![], ""), &caps()));
    }

    // ---- PDF: cifratura, prima del caricamento -----------------------------------------

    fn encrypt_obj() -> (u32, Vec<u8>) {
        (
            50,
            dict("<< /Filter /Standard /V 1 /R 2 /O <00> /U <00> /P -4 >>"),
        )
    }

    #[test]
    fn encrypt_in_trailer_is_encrypted() {
        let pdf = doc(
            1,
            "",
            vec![encrypt_obj()],
            "/Encrypt 50 0 R /ID [<01> <01>]",
        );
        assert_eq!(kind_of(check_pdf(&pdf, &caps())), Kind::Encrypted);
    }

    #[test]
    fn encrypt_with_escaped_name_is_encrypted() {
        let pdf = doc(
            1,
            "",
            vec![encrypt_obj()],
            "/Encr#79pt 50 0 R /ID [<01> <01>]",
        );
        assert_eq!(kind_of(check_pdf(&pdf, &caps())), Kind::Encrypted);
    }

    #[test]
    fn encrypt_is_detected_even_if_body_is_corrupt() {
        let pdf =
            b"%PDF-1.7\nnot an object at all\ntrailer\n<< /Root 1 0 R /Encrypt 5 0 R >>\n%%EOF\n";
        assert_eq!(kind_of(check_pdf(pdf, &caps())), Kind::Encrypted);
    }

    #[test]
    fn garbage_pdf_is_corrupt() {
        assert_eq!(
            kind_of(check_pdf(b"%PDF-1.7\nthis is not a pdf\n", &caps())),
            Kind::Corrupt
        );
    }

    // ---- PDF: object stream e pagine (TEST-8) ------------------------------------------

    fn garbage_objstm(name: &str) -> Vec<u8> {
        stream(
            &format!("/Type /{name} /N 1 /First 4 /Filter /FlateDecode"),
            b"\x00\x01garbage",
        )
    }

    #[test]
    fn objstm_count_over_cap_is_rejected_before_load() {
        let extra = (0..3).map(|i| (60 + i, garbage_objstm("ObjStm"))).collect();
        let c = Caps {
            max_object_streams: 2,
            ..caps()
        };
        assert_eq!(
            kind_of(check_pdf(&doc(1, "", extra, ""), &c)),
            Kind::DecompressionLimit
        );
    }

    #[test]
    fn objstm_count_uses_normalized_names() {
        let extra = vec![
            (60, garbage_objstm("ObjStm")),
            (61, garbage_objstm("Obj#53tm")),
            (62, garbage_objstm("#4FbjStm")),
        ];
        let c = Caps {
            max_object_streams: 2,
            ..caps()
        };
        assert_eq!(
            kind_of(check_pdf(&doc(1, "", extra, ""), &c)),
            Kind::DecompressionLimit
        );
    }

    #[test]
    fn objstm_within_cap_passes() {
        let extra = vec![(60, objstm(&[(70, b"<< /Foo 1 >>")]))];
        let c = Caps {
            max_object_streams: 2,
            ..caps()
        };
        assert_ok(check_pdf(&doc(1, "", extra, ""), &c));
    }

    #[test]
    fn objstm_expanding_over_per_stream_cap_is_decompression_limit() {
        // Il tetto per stream deve agire già al caricamento (max_decompressed_size).
        let mut big = b"<< /Pad (".to_vec();
        big.extend(std::iter::repeat_n(b'A', 2 * MIB as usize));
        big.extend_from_slice(b") >>");
        let extra = vec![(60, objstm(&[(70, &big)]))];
        assert_eq!(
            kind_of(check_pdf(&doc(1, "", extra, ""), &caps())),
            Kind::DecompressionLimit
        );
    }

    #[test]
    fn pages_over_limit_is_resource_limit() {
        let c = Caps {
            max_pages: 3,
            ..caps()
        };
        assert_eq!(
            kind_of(check_pdf(&doc(4, "", vec![], ""), &c)),
            Kind::ResourceLimit
        );
    }

    #[test]
    fn pages_at_limit_pass() {
        let c = Caps {
            max_pages: 3,
            ..caps()
        };
        assert_ok(check_pdf(&doc(3, "", vec![], ""), &c));
    }

    // ---- PDF: contenuti attivi (TEST-7) ------------------------------------------------

    #[test]
    fn javascript_hidden_in_compressed_objstm_is_quarantined() {
        let js: &[u8] = b"<< /S /JavaScript /JS (app.alert\\(1\\)) >>";
        let pdf = doc(1, "/OpenAction 70 0 R", vec![(60, objstm(&[(70, js)]))], "");
        assert!(
            !pdf.windows(10).any(|w| w == b"JavaScript"),
            "il JS deve stare solo dentro l'object stream compresso"
        );
        let r = reasons_of(check_pdf(&pdf, &caps()));
        assert!(r.contains(&"javascript".to_owned()), "motivi {r:?}");
    }

    #[test]
    fn open_action_javascript_is_quarantined_with_both_reasons() {
        let pdf = doc(
            1,
            "/OpenAction 50 0 R",
            vec![(50, dict("<< /S /JavaScript /JS (app.alert\\(1\\)) >>"))],
            "",
        );
        let r = reasons_of(check_pdf(&pdf, &caps()));
        assert!(r.contains(&"javascript".to_owned()), "motivi {r:?}");
        assert!(r.contains(&"open_action_js".to_owned()), "motivi {r:?}");
    }

    #[test]
    fn bare_js_key_is_javascript() {
        let pdf = doc(1, "", vec![(50, dict("<< /Type /Action /JS (x) >>"))], "");
        let r = reasons_of(check_pdf(&pdf, &caps()));
        assert!(r.contains(&"javascript".to_owned()), "motivi {r:?}");
    }

    #[test]
    fn launch_action_is_quarantined() {
        let pdf = doc(1, "", vec![(50, dict("<< /S /Launch /F (cmd.exe) >>"))], "");
        let r = reasons_of(check_pdf(&pdf, &caps()));
        assert!(r.contains(&"launch".to_owned()), "motivi {r:?}");
    }

    #[test]
    fn launch_with_escaped_name_is_quarantined() {
        let pdf = doc(
            1,
            "",
            vec![(50, dict("<< /S /Laun#63h /F (cmd.exe) >>"))],
            "",
        );
        let r = reasons_of(check_pdf(&pdf, &caps()));
        assert!(r.contains(&"launch".to_owned()), "motivi {r:?}");
    }

    #[test]
    fn embedded_file_is_quarantined() {
        let pdf = doc(
            1,
            "",
            vec![
                (
                    50,
                    dict("<< /Type /Filespec /F (a.exe) /EF << /F 51 0 R >> >>"),
                ),
                (51, stream("/Type /EmbeddedFile", b"MZ\x90\x00")),
            ],
            "",
        );
        let r = reasons_of(check_pdf(&pdf, &caps()));
        assert!(r.contains(&"embedded_file".to_owned()), "motivi {r:?}");
    }

    // ---- PDF: catene di filtri (TEST-7) ------------------------------------------------

    #[test]
    fn filter_chain_longer_than_two_is_rejected() {
        let data = hex(&hex(&hex(b"BT ET")));
        let pdf = doc_with_content(
            "/Filter [/ASCIIHexDecode /ASCIIHexDecode /ASCIIHexDecode]",
            &data,
        );
        let k = kind_of(check_pdf(&pdf, &caps()));
        assert!(
            matches!(k, Kind::DecompressionLimit | Kind::Unsupported),
            "kind {k:?}"
        );
    }

    #[test]
    fn filter_chain_of_two_allowed_filters_passes() {
        let data = hex(&zlib(b"BT /F1 12 Tf 72 712 Td (Ciao) Tj ET"));
        let pdf = doc_with_content("/Filter [/ASCIIHexDecode /FlateDecode]", &data);
        assert_ok(check_pdf(&pdf, &caps()));
    }

    #[test]
    fn lzw_runlength_and_unknown_filters_are_unsupported() {
        for f in ["/LZWDecode", "/RunLengthDecode", "/FooDecode"] {
            let pdf = doc_with_content(&format!("/Filter {f}"), b"\x80\x0b\x60\x50");
            assert_eq!(
                kind_of(check_pdf(&pdf, &caps())),
                Kind::Unsupported,
                "filtro {f}"
            );
        }
    }

    #[test]
    fn image_filter_on_non_image_stream_is_rejected() {
        let pdf = doc_with_content("/Filter /DCTDecode", b"\xFF\xD8\xFF\xE0junk");
        let k = kind_of(check_pdf(&pdf, &caps()));
        assert!(
            matches!(k, Kind::Unsupported | Kind::DecompressionLimit),
            "kind {k:?}"
        );
    }

    fn image_doc(w: u32, h: u32) -> Vec<u8> {
        build(
            &[
                (1, dict("<< /Type /Catalog /Pages 2 0 R >>")),
                (2, dict("<< /Type /Pages /Kids [3 0 R] /Count 1 >>")),
                (
                    3,
                    dict(
                        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
                         /Resources << /XObject << /Im1 5 0 R >> >> /Contents 4 0 R >>",
                    ),
                ),
                (4, stream("", b"q 100 0 0 100 0 0 cm /Im1 Do Q")),
                (
                    5,
                    stream(
                        &format!(
                            "/Type /XObject /Subtype /Image /Width {w} /Height {h} \
                             /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode"
                        ),
                        b"\xFF\xD8\xFF\xE0junk",
                    ),
                ),
            ],
            "",
        )
    }

    #[test]
    fn image_filter_on_image_within_cap_passes() {
        assert_ok(check_pdf(&image_doc(100, 100), &caps()));
    }

    #[test]
    fn image_filter_on_image_over_cap_is_rejected() {
        // 10.000 × 10.000 × 3 byte ≈ 286 MiB, oltre i 64 MiB.
        let k = kind_of(check_pdf(&image_doc(10_000, 10_000), &caps()));
        assert!(
            matches!(
                k,
                Kind::DecompressionLimit | Kind::ResourceLimit | Kind::Unsupported
            ),
            "kind {k:?}"
        );
    }

    // ---- PDF: tetti di decompressione Flate (TEST-7) -----------------------------------

    #[test]
    fn flate_stream_at_per_stream_cap_passes() {
        let pdf = doc_with_content("/Filter /FlateDecode", &zlib_zeros(MIB));
        assert_ok(check_pdf(&pdf, &caps()));
    }

    #[test]
    fn flate_stream_over_per_stream_cap_is_decompression_limit() {
        let pdf = doc_with_content("/Filter /FlateDecode", &zlib_zeros(MIB + 1));
        assert_eq!(kind_of(check_pdf(&pdf, &caps())), Kind::DecompressionLimit);
    }

    #[test]
    fn flate_ratio_over_limit_is_decompression_limit() {
        let c = Caps {
            ratio_min_bytes: 64 * KIB,
            ..caps()
        };
        let pdf = doc_with_content("/Filter /FlateDecode", &zlib_zeros(512 * KIB));
        assert_eq!(kind_of(check_pdf(&pdf, &c)), Kind::DecompressionLimit);
    }

    #[test]
    fn incompressible_flate_under_ratio_passes() {
        let c = Caps {
            ratio_min_bytes: 64 * KIB,
            ..caps()
        };
        let pdf = doc_with_content("/Filter /FlateDecode", &zlib(&noise(512 * KIB as usize, 7)));
        assert_ok(check_pdf(&pdf, &c));
    }

    #[test]
    fn flate_total_over_cap_is_decompression_limit() {
        let c = Caps {
            total_bytes: MIB,
            ..caps()
        };
        let objs = vec![
            (1, dict("<< /Type /Catalog /Pages 2 0 R >>")),
            (2, dict("<< /Type /Pages /Kids [3 0 R] /Count 1 >>")),
            (
                3,
                dict(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents [4 0 R 5 0 R] >>",
                ),
            ),
            (
                4,
                stream("/Filter /FlateDecode", &zlib(&noise(600 * KIB as usize, 1))),
            ),
            (
                5,
                stream("/Filter /FlateDecode", &zlib(&noise(600 * KIB as usize, 2))),
            ),
        ];
        assert_eq!(
            kind_of(check_pdf(&build(&objs, ""), &c)),
            Kind::DecompressionLimit
        );
    }

    // ---- DOCX (TEST-7) -----------------------------------------------------------------
    // Namespace OOXML sostituiti da URN segnaposto: il prescan non li valida e lo scanner
    // rifiuta gli URL in chiaro.

    const CT_DOCX: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
    const CT_MACRO: &str = "application/vnd.ms-word.document.macroEnabled.main+xml";

    fn content_types(main: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="urn:test:ooxml/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="{main}"/></Types>"#
        )
    }

    const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="urn:test:ooxml/package/2006/relationships"><Relationship Id="rId1" Type="urn:test:ooxml/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

    const XML_DECL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>"#;
    const DOCUMENT_BODY: &str = r#"<w:document xmlns:w="urn:test:ooxml/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Ciao</w:t></w:r></w:p></w:body></w:document>"#;

    fn base_docx() -> Vec<(String, Vec<u8>)> {
        vec![
            (
                "[Content_Types].xml".into(),
                content_types(CT_DOCX).into_bytes(),
            ),
            ("_rels/.rels".into(), RELS.as_bytes().to_vec()),
            (
                "word/document.xml".into(),
                format!("{XML_DECL}{DOCUMENT_BODY}").into_bytes(),
            ),
        ]
    }

    fn scratch(ext: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!("s02-prescan-tw-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("d{}.{ext}", N.fetch_add(1, Ordering::SeqCst)))
    }

    fn write_docx(entries: &[(String, Vec<u8>)]) -> PathBuf {
        let path = scratch("docx");
        let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (name, data) in entries {
            z.start_file(name.as_str(), zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
        path
    }

    fn with(mut base: Vec<(String, Vec<u8>)>, name: &str, data: &[u8]) -> Vec<(String, Vec<u8>)> {
        base.retain(|(n, _)| n != name);
        base.push((name.to_owned(), data.to_vec()));
        base
    }

    #[test]
    fn benign_docx_passes() {
        assert_ok(check_docx(&write_docx(&base_docx()), &caps()));
    }

    #[test]
    fn non_zip_docx_is_corrupt() {
        let p = scratch("docx");
        std::fs::write(&p, noise(4096, 3)).unwrap();
        assert_eq!(kind_of(check_docx(&p, &caps())), Kind::Corrupt);
    }

    #[test]
    fn docx_with_too_many_entries_is_rejected() {
        let mut e = base_docx();
        for i in 0..3 {
            e.push((format!("word/media/p{i}.bin"), b"x".to_vec()));
        }
        let c = Caps {
            docx_max_entries: 5,
            ..caps()
        };
        let k = kind_of(check_docx(&write_docx(&e), &c));
        assert!(
            matches!(k, Kind::DecompressionLimit | Kind::ResourceLimit),
            "kind {k:?}"
        );
    }

    #[test]
    fn docx_with_declared_total_over_cap_is_rejected() {
        let e = with(
            base_docx(),
            "word/media/pad.bin",
            &vec![0u8; 8 * KIB as usize],
        );
        let c = Caps {
            docx_max_total_bytes: 4 * KIB,
            ..caps()
        };
        let k = kind_of(check_docx(&write_docx(&e), &c));
        assert!(
            matches!(k, Kind::DecompressionLimit | Kind::ResourceLimit),
            "kind {k:?}"
        );
    }

    #[test]
    fn docx_with_vba_project_is_unsupported() {
        let e = with(base_docx(), "word/vbaProject.bin", b"\xD0\xCF\x11\xE0");
        assert_eq!(
            kind_of(check_docx(&write_docx(&e), &caps())),
            Kind::Unsupported
        );
    }

    #[test]
    fn docx_with_macro_content_type_is_unsupported() {
        let e = with(
            base_docx(),
            "[Content_Types].xml",
            content_types(CT_MACRO).as_bytes(),
        );
        assert_eq!(
            kind_of(check_docx(&write_docx(&e), &caps())),
            Kind::Unsupported
        );
    }

    #[test]
    fn docx_with_doctype_is_corrupt() {
        let doc = format!("{XML_DECL}<!DOCTYPE w:document [<!ELEMENT a ANY>]>{DOCUMENT_BODY}");
        let e = with(base_docx(), "word/document.xml", doc.as_bytes());
        assert_eq!(kind_of(check_docx(&write_docx(&e), &caps())), Kind::Corrupt);
    }

    #[test]
    fn docx_with_entity_in_any_part_is_corrupt() {
        let styles = format!(r#"{XML_DECL}<!ENTITY lol "lol"><w:styles xmlns:w="x"/>"#);
        let e = with(base_docx(), "word/styles.xml", styles.as_bytes());
        assert_eq!(kind_of(check_docx(&write_docx(&e), &caps())), Kind::Corrupt);
    }

    #[test]
    fn docx_with_external_relationship_is_quarantined() {
        let rels = format!(
            r#"{XML_DECL}<Relationships xmlns="urn:test:ooxml/package/2006/relationships"><Relationship Id="rId9" Type="urn:test:ooxml/officeDocument/2006/relationships/attachedTemplate" Target="https://attacker.invalid/t.dotm" TargetMode="External"/></Relationships>"#
        );
        let e = with(base_docx(), "word/_rels/document.xml.rels", rels.as_bytes());
        let r = reasons_of(check_docx(&write_docx(&e), &caps()));
        assert!(r.contains(&"external_link".to_owned()), "motivi {r:?}");
    }
}
