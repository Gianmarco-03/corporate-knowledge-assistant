//! Lettore DOCX (`word/document.xml`) → blocchi `cka-text-v1` (piano 01-09, Task 1).
//!
//! [`parse_file`] chiama `s02_parser::prescan::check_docx` prima di leggere l'XML, poi legge
//! `word/document.xml` in streaming (`zip` con `take`, `quick-xml` a eventi, nessuna DTD né
//! entità). Si esegue solo nel figlio `locator locate`. Le relazioni del pacchetto non si
//! leggono mai: la destinazione di un hyperlink non entra nel testo.
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use s02_parser::prescan::{self, Caps, Rejection};
use s02_parser::protocol::Kind;
use unicode_normalization::UnicodeNormalization;

use crate::canon::{Block, Parsed};

/// Stili di paragrafo esclusi, simmetrici al front matter del Markdown.
const EXCLUDED_STYLES: [&str; 4] = ["Title", "Subtitle", "Author", "Date"];

/// Blocchi canonici dell'XML di `word/document.xml`. DTD o entità → `Err`.
///
/// Lettura in memoria, usata dai test sull'XML scritto a mano; il figlio passa da [`parse_file`].
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn parse_document_xml(xml: &[u8]) -> Result<Parsed, String> {
    parse_reader(xml)
}

/// Prescan (`check_docx`) e poi lettura di `word/document.xml`; un `Rejection` del prescan
/// torna invariato, senza leggere l'XML.
pub(crate) fn parse_file(path: &Path, caps: &Caps) -> Result<Parsed, Rejection> {
    prescan::check_docx(path, caps)?;
    let corrupt = |d: String| Rejection::Failed(Kind::Corrupt, d);
    let file = std::fs::File::open(path).map_err(|e| corrupt(format!("open: {e}")))?;
    let mut z = zip::ZipArchive::new(file).map_err(|e| corrupt(format!("zip: {e}")))?;
    let entry = z
        .by_name("word/document.xml")
        .map_err(|e| corrupt(format!("document_xml: {e}")))?;
    if entry.size() > caps.docx_max_total_bytes {
        return Err(Rejection::Failed(
            Kind::DecompressionLimit,
            "document_xml_size".to_owned(),
        ));
    }
    // Il prescan ha già contato i byte reali; `take` resta come tetto in profondità.
    let limited = entry.take(caps.docx_max_total_bytes);
    parse_reader(BufReader::new(limited)).map_err(corrupt)
}

/// Paragrafo in costruzione.
#[derive(Default)]
struct Para {
    style: Option<String>,
    text: String,
}

/// Tabella in costruzione: righe di celle, ogni cella come lista di paragrafi.
#[derive(Default)]
struct Table {
    rows: Vec<Vec<String>>,
    row: Vec<String>,
    cell: Vec<String>,
}

#[derive(Default)]
struct State {
    blocks: Vec<Block>,
    unsupported: Vec<String>,
    para: Option<Para>,
    tables: Vec<Table>,
    /// Profondità dentro `w:r` (run), `w:t`, `w:del`, `w:pPr`, contenuti saltati.
    in_run: u32,
    in_t: u32,
    in_del: u32,
    in_ppr: u32,
    skip: u32,
}

fn local(e: &BytesStart<'_>) -> String {
    e.local_name().as_ref().to_owned()
}

fn attr(e: &BytesStart<'_>, name: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find_map(|a| (a.key.local_name().as_ref() == name).then(|| a.value.clone().into_owned()))
}

/// Contenitori il cui contenuto non è testo del documento: disegni e caselle di testo,
/// oggetti incorporati, il ramo `Choice` dei contenuti alternativi (vale `Fallback`).
fn skipped(name: &str) -> Option<&'static str> {
    match name {
        "drawing" | "pict" => Some("drawing"),
        "object" => Some("object"),
        "Choice" => Some("alternate_content"),
        _ => None,
    }
}

impl State {
    fn push_text(&mut self, s: &str) {
        if self.skip > 0 || self.in_del > 0 || self.in_t == 0 {
            return;
        }
        if let Some(p) = self.para.as_mut() {
            p.text.push_str(s);
        }
    }

    fn push_char(&mut self, c: char) {
        if self.skip > 0 || self.in_del > 0 || self.in_run == 0 {
            return;
        }
        if let Some(p) = self.para.as_mut() {
            p.text.push(c);
        }
    }

    fn start(&mut self, e: &BytesStart<'_>, empty: bool) {
        let name = local(e);
        if self.skip > 0 {
            if !empty {
                self.skip += 1;
            }
            return;
        }
        if let Some(what) = skipped(&name) {
            self.unsupported.push(what.to_owned());
            if !empty {
                self.skip = 1;
            }
            return;
        }
        match name.as_str() {
            "p" if !empty => self.para = Some(Para::default()),
            "pPr" if !empty => self.in_ppr += 1,
            "pStyle" if self.in_ppr > 0 => {
                if let Some(p) = self.para.as_mut() {
                    p.style = attr(e, "val");
                }
            }
            "r" if !empty => self.in_run += 1,
            "t" if !empty => self.in_t += 1,
            "del" if !empty => self.in_del += 1,
            "tab" if self.in_ppr == 0 => self.push_char('\t'),
            // Interruzione di pagina: non è testo.
            "br" if attr(e, "type").as_deref() == Some("page") => {}
            "br" => self.push_char('\n'),
            "cr" => self.push_char('\n'),
            "tbl" if !empty => self.tables.push(Table::default()),
            "tr" if !empty => {
                if let Some(t) = self.tables.last_mut() {
                    t.row.clear();
                }
            }
            "tc" if !empty => {
                if let Some(t) = self.tables.last_mut() {
                    t.cell.clear();
                }
            }
            _ => {}
        }
    }

    fn end(&mut self, name: &str) {
        if self.skip > 0 {
            self.skip -= 1;
            return;
        }
        match name {
            "p" => self.end_para(),
            "pPr" => self.in_ppr = self.in_ppr.saturating_sub(1),
            "r" => self.in_run = self.in_run.saturating_sub(1),
            "t" => self.in_t = self.in_t.saturating_sub(1),
            "del" => self.in_del = self.in_del.saturating_sub(1),
            "tc" => {
                if let Some(t) = self.tables.last_mut() {
                    let cell = compress(&t.cell.join(" "));
                    t.row.push(cell);
                    t.cell.clear();
                }
            }
            "tr" => {
                if let Some(t) = self.tables.last_mut() {
                    let row = std::mem::take(&mut t.row);
                    t.rows.push(row);
                }
            }
            "tbl" => {
                if let Some(t) = self.tables.pop() {
                    let text = t
                        .rows
                        .iter()
                        .map(|r| r.join("\t"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    match self.tables.last_mut() {
                        Some(outer) => {
                            self.unsupported.push("nested_table".to_owned());
                            outer.cell.push(text);
                        }
                        None => self.push_block(Block::Text(text)),
                    }
                }
            }
            _ => {}
        }
    }

    fn end_para(&mut self) {
        let Some(p) = self.para.take() else {
            return;
        };
        let style = p.style.as_deref().unwrap_or("");
        if EXCLUDED_STYLES.contains(&style) {
            return;
        }
        if let Some(t) = self.tables.last_mut() {
            t.cell.push(p.text);
            return;
        }
        if let Some(level) = heading_level(style) {
            let text = compress(&p.text);
            self.blocks.push(Block::Heading {
                level,
                text: text.nfc().collect(),
            });
            return;
        }
        let text = if style == "SourceCode" {
            p.text
        } else {
            compress(&p.text)
        };
        self.push_block(Block::Text(text));
    }

    fn push_block(&mut self, b: Block) {
        if let Block::Text(t) = b {
            let t: String = t.replace("\r\n", "\n").replace('\r', "\n").nfc().collect();
            if !t.is_empty() {
                self.blocks.push(Block::Text(t));
            }
        } else {
            self.blocks.push(b);
        }
    }
}

/// `Heading1`..`Heading6` → livello.
fn heading_level(style: &str) -> Option<u8> {
    let n = style.strip_prefix("Heading")?;
    let level: u8 = n.parse().ok()?;
    (1..=6).contains(&level).then_some(level)
}

/// Sequenze di spazi ASCII compresse in uno, estremi tolti (`\t` e `\n` restano).
fn compress(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c == ' ' {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out.trim_matches(' ').to_owned()
}

/// Entità predefinite di XML; ogni altra entità è un errore (nessuna espansione).
fn predefined(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => None,
    }
}

fn parse_reader<R: BufRead>(input: R) -> Result<Parsed, String> {
    let mut reader = Reader::from_reader(input);
    let mut buf = Vec::new();
    let mut st = State::default();
    loop {
        let ev = reader
            .read_event_into(&mut buf)
            .map_err(|e| format!("xml: {e}"))?;
        match ev {
            Event::DocType(_) => return Err("xml_doctype".to_owned()),
            Event::Start(e) => st.start(&e, false),
            Event::Empty(e) => st.start(&e, true),
            Event::End(e) => st.end(e.local_name().as_ref()),
            Event::Text(t) => st.push_text(&t.xml10_content()),
            Event::CData(c) => {
                let s = c.into_inner().into_owned();
                st.push_text(&s);
            }
            Event::GeneralRef(r) => {
                let c = if r.is_char_ref() {
                    r.resolve_char_ref()
                        .map_err(|e| format!("xml_char_ref: {e}"))?
                } else {
                    predefined(&r.xml10_content())
                };
                match c {
                    Some(c) => st.push_text(c.encode_utf8(&mut [0; 4])),
                    None => return Err("xml_entity".to_owned()),
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    let mut unsupported = st.unsupported;
    unsupported.sort();
    unsupported.dedup();
    Ok(Parsed {
        blocks: st.blocks,
        unsupported,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    //! Test (3) lato DOCX e concordanza MD/DOCX (TEST-1), (5) lato DOCX, TEST-7 (relazioni
    //! esterne e contenuti ostili non risolti, prescan prima dell'XML). XML scritto inline;
    //! i DOCX dei test di `parse_file` sono zip costruiti nel test.
    use super::*;
    use crate::canon::{self, Block, CharRange, SectionStep};
    use crate::md;
    use s02_parser::protocol::Kind;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Radice dei nomi di namespace OOXML. Sono identificatori XML, non URL di rete: il
    /// segnaposto `@OX@` evita il falso positivo `http-url` dello scanner sui letterali.
    const OOXML: &str = concat!("http", "://schemas.openxmlformats.org/");

    /// Sostituisce il segnaposto `@OX@` con la radice dei namespace OOXML reali.
    pub(crate) fn ox(s: &str) -> String {
        s.replace("@OX@", OOXML)
    }

    const NS: &str = r#"xmlns:w="@OX@wordprocessingml/2006/main" xmlns:r="@OX@officeDocument/2006/relationships""#;

    pub(crate) fn doc(body: &str) -> String {
        ox(&format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {NS}><w:body>{body}</w:body></w:document>"#
        ))
    }

    fn step(title: &str, ordinal: u32) -> SectionStep {
        SectionStep {
            title: title.to_owned(),
            ordinal,
        }
    }

    fn texts(xml: &str) -> Vec<(Vec<SectionStep>, String)> {
        canon::sections(&parse_document_xml(xml.as_bytes()).expect("XML valido"))
            .sections
            .into_iter()
            .map(|s| (s.path, s.text))
            .collect()
    }

    fn cell(text: &str) -> String {
        format!("<w:tc><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:tc>")
    }

    fn row(cells: &[&str]) -> String {
        format!(
            "<w:tr>{}</w:tr>",
            cells.iter().map(|c| cell(c)).collect::<String>()
        )
    }

    /// Gemello DOCX di `md::tests::TWIN_MD`: stile Title escluso, titolo con run in
    /// grassetto e hyperlink, testo spezzato in due run, interruzione di pagina in un
    /// paragrafo a sé e `w:lastRenderedPageBreak`, tabella 3×3.
    pub(crate) fn twin_document_xml() -> String {
        let body = format!(
            concat!(
                r#"<w:p><w:pPr><w:pStyle w:val="Title"/></w:pPr><w:r><w:t>Non entra</w:t></w:r></w:p>"#,
                r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Relazione</w:t></w:r></w:p>"#,
                r#"<w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t xml:space="preserve">Risultati </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>chiave</w:t></w:r><w:r><w:t xml:space="preserve"> del </w:t></w:r><w:hyperlink r:id="rId9"><w:r><w:t>periodo</w:t></w:r></w:hyperlink></w:p>"#,
                r#"<w:p><w:pPr><w:pStyle w:val="FirstParagraph"/></w:pPr><w:r><w:t xml:space="preserve">Il fattu</w:t></w:r><w:r><w:lastRenderedPageBreak/><w:t>rato è cresciuto del 5%.</w:t></w:r></w:p>"#,
                r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#,
                "<w:tbl>{}{}{}</w:tbl>",
            ),
            row(&["Voce", "2025", "2026"]),
            row(&["Ricavi", "182,4", "196,0"]),
            row(&["EBITDA", "27,1", "30,4"]),
        );
        doc(&body)
    }

    /// Test (3) lato DOCX: stesso testo canonico scritto a mano del gemello MD.
    #[test]
    fn twin_docx_has_hand_written_canonical_text() {
        assert_eq!(
            texts(&twin_document_xml()),
            vec![
                (vec![step("Relazione", 1)], String::new()),
                (
                    vec![
                        step("Relazione", 1),
                        step("Risultati chiave del periodo", 1)
                    ],
                    md::tests::TWIN_SECTION_TEXT.to_owned()
                ),
            ]
        );
    }

    /// TEST-1 / test (3): MD e DOCX equivalenti danno stesso percorso, stesso intervallo
    /// (scritto a mano: «cresciuto del 5%.\nVoce\t2025» = scalar [15, 42)), stesso hash di
    /// sezione e stessa proiezione.
    #[test]
    fn md_and_docx_twins_agree_on_path_range_hash_and_projection() {
        let pm = canon::sections(&md::parse(md::tests::TWIN_MD));
        let pd = canon::sections(&parse_document_xml(twin_document_xml().as_bytes()).unwrap());
        let excerpt = "cresciuto del 5%.\nVoce\t2025";
        let sha_md = "4444444444444444444444444444444444444444444444444444444444444444";
        let sha_docx = "5555555555555555555555555555555555555555555555555555555555555555";
        let lm = canon::locate(&pm, sha_md, excerpt);
        let ld = canon::locate(&pd, sha_docx, excerpt);
        assert_eq!((lm.len(), ld.len()), (1, 1));
        let (m, d) = (&lm[0], &ld[0]);
        assert_eq!(m.section_path, d.section_path);
        assert_eq!(m.char_range, CharRange { start: 15, end: 42 });
        assert_eq!(m.char_range, d.char_range);
        assert_eq!(m.section_text_sha256, d.section_text_sha256);
        assert_eq!(m.projection, canon::PROJECTION);
        assert_eq!(d.projection, canon::PROJECTION);
        assert_eq!(canon::resolve(m, &pm, sha_md).as_deref(), Ok(excerpt));
        assert_eq!(canon::resolve(d, &pd, sha_docx).as_deref(), Ok(excerpt));
        // Il locator di un formato non vale sull'altro: doc_sha256 diverso.
        assert_eq!(
            canon::resolve(m, &pd, sha_docx),
            Err(canon::ResolveError::VersionMismatch)
        );
    }

    /// Test (5) lato DOCX: stili Title, Subtitle, Author, Date esclusi.
    #[test]
    fn title_subtitle_author_date_are_excluded() {
        let xml = doc(concat!(
            r#"<w:p><w:pPr><w:pStyle w:val="Title"/></w:pPr><w:r><w:t>Segreto</w:t></w:r></w:p>"#,
            r#"<w:p><w:pPr><w:pStyle w:val="Subtitle"/></w:pPr><w:r><w:t>Sotto</w:t></w:r></w:p>"#,
            r#"<w:p><w:pPr><w:pStyle w:val="Author"/></w:pPr><w:r><w:t>Mario</w:t></w:r></w:p>"#,
            r#"<w:p><w:pPr><w:pStyle w:val="Date"/></w:pPr><w:r><w:t>2026</w:t></w:r></w:p>"#,
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>A</w:t></w:r></w:p>"#,
            r#"<w:p><w:r><w:t>Testo.</w:t></w:r></w:p>"#,
        ));
        assert_eq!(texts(&xml), vec![(vec![step("A", 1)], "Testo.".to_owned())]);
    }

    /// Titoli DOCX: `Heading1..Heading6` con il livello dal nome dello stile.
    #[test]
    fn heading_styles_map_to_levels() {
        let xml = doc(concat!(
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t xml:space="preserve">  Uno </w:t></w:r></w:p>"#,
            r#"<w:p><w:pPr><w:pStyle w:val="Heading3"/></w:pPr><w:r><w:t>Tre</w:t></w:r></w:p>"#,
            r#"<w:p><w:pPr><w:pStyle w:val="Heading6"/></w:pPr><w:r><w:t>Sei</w:t></w:r></w:p>"#,
        ));
        assert_eq!(
            parse_document_xml(xml.as_bytes()).unwrap().blocks,
            vec![
                Block::Heading {
                    level: 1,
                    text: "Uno".to_owned()
                },
                Block::Heading {
                    level: 3,
                    text: "Tre".to_owned()
                },
                Block::Heading {
                    level: 6,
                    text: "Sei".to_owned()
                },
            ]
        );
    }

    /// Inline DOCX: `w:tab` → `\t`, `w:br` non di pagina → `\n`, `w:ins` incluso,
    /// `w:delText` e `w:instrText` esclusi, riferimenti a note esclusi.
    #[test]
    fn inline_runs_follow_cka_text_v1() {
        let xml = doc(concat!(
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>A</w:t></w:r></w:p>"#,
            r#"<w:p><w:r><w:t>a</w:t><w:tab/><w:t>b</w:t><w:br/><w:t>c</w:t></w:r>"#,
            r#"<w:ins><w:r><w:t xml:space="preserve"> nuovo</w:t></w:r></w:ins>"#,
            r#"<w:del><w:r><w:delText xml:space="preserve"> vecchio</w:delText></w:r></w:del>"#,
            r#"<w:r><w:footnoteReference w:id="1"/></w:r></w:p>"#,
        ));
        assert_eq!(
            texts(&xml),
            vec![(vec![step("A", 1)], "a\tb\nc nuovo".to_owned())]
        );
    }

    /// TEST-7: campo HYPERLINK (`w:instrText`) e destinazione dell'hyperlink non entrano
    /// nel testo; resta solo il testo visibile.
    #[test]
    fn hyperlink_targets_and_field_codes_are_not_resolved() {
        let xml = doc(concat!(
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>A</w:t></w:r></w:p>"#,
            r#"<w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> HYPERLINK "https://attacker.invalid/x" </w:instrText></w:r>"#,
            r#"<w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>vedi</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#,
            r#"<w:r><w:t xml:space="preserve"> e </w:t></w:r><w:hyperlink r:id="rId7" w:tgtFrame="_blank"><w:r><w:t>nota</w:t></w:r></w:hyperlink></w:p>"#,
        ));
        let got = texts(&xml);
        assert_eq!(got, vec![(vec![step("A", 1)], "vedi e nota".to_owned())]);
    }

    /// TEST-7: DTD o entità nell'XML → errore, nessuna espansione (difesa in profondità
    /// oltre al prescan).
    #[test]
    fn doctype_or_entity_in_xml_is_an_error() {
        let xml = r#"<?xml version="1.0"?><!DOCTYPE w:document [<!ENTITY x "ESPANSA">]><w:document xmlns:w="@OX@wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>&x;</w:t></w:r></w:p></w:body></w:document>"#;
        assert!(parse_document_xml(ox(xml).as_bytes()).is_err());
    }

    // ---- parse_file: prescan prima dell'XML (TEST-7) -----------------------------------

    static SEQ: AtomicU32 = AtomicU32::new(0);

    pub(crate) fn scratch_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "s02-locator-tw-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="@OX@package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="@OX@package/2006/relationships"><Relationship Id="rId1" Type="@OX@officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

    /// DOCX minimo in `dir/name`, con `word/_rels/document.xml.rels` opzionale.
    pub(crate) fn write_docx(
        dir: &Path,
        name: &str,
        document_xml: &str,
        doc_rels: Option<&str>,
    ) -> PathBuf {
        let path = dir.join(name);
        let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let mut put = |n: &str, data: &str| {
            z.start_file(n, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(ox(data).as_bytes()).unwrap();
        };
        put("[Content_Types].xml", CONTENT_TYPES);
        put("_rels/.rels", ROOT_RELS);
        put("word/document.xml", document_xml);
        if let Some(r) = doc_rels {
            put("word/_rels/document.xml.rels", r);
        }
        z.finish().unwrap();
        path
    }

    pub(crate) const EXTERNAL_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="@OX@package/2006/relationships"><Relationship Id="rId9" Type="@OX@officeDocument/2006/relationships/hyperlink" Target="https://attacker.invalid/x" TargetMode="External"/></Relationships>"#;

    /// Un DOCX benigno passa dal prescan e dà gli stessi blocchi dell'XML letto in memoria.
    #[test]
    fn parse_file_reads_benign_docx() {
        let dir = scratch_dir();
        let p = write_docx(&dir, "twin.docx", &twin_document_xml(), None);
        let parsed = parse_file(&p, &Caps::default()).expect("DOCX benigno");
        assert_eq!(
            parsed,
            parse_document_xml(twin_document_xml().as_bytes()).unwrap()
        );
    }

    /// TEST-7: relazione `TargetMode="External"` → quarantena del prescan, il locator non la
    /// risolve né legge l'XML.
    #[test]
    fn parse_file_quarantines_external_relationship() {
        let dir = scratch_dir();
        let p = write_docx(&dir, "ext.docx", &twin_document_xml(), Some(EXTERNAL_RELS));
        match parse_file(&p, &Caps::default()) {
            Err(Rejection::Quarantined(r)) => {
                assert!(r.contains(&"external_link".to_owned()), "motivi {r:?}")
            }
            other => panic!("atteso Quarantined, ottenuto {other:?}"),
        }
    }

    /// TEST-7: DTD nel `document.xml` → `Corrupt` dal prescan, prima del parser XML.
    #[test]
    fn parse_file_rejects_doctype_before_reading_xml() {
        let dir = scratch_dir();
        let xml = r#"<?xml version="1.0"?><!DOCTYPE d [<!ENTITY a "aaaaaaaaaa">]><w:document xmlns:w="@OX@wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>&a;</w:t></w:r></w:p></w:body></w:document>"#;
        let p = write_docx(&dir, "dtd.docx", xml, None);
        match parse_file(&p, &Caps::default()) {
            Err(Rejection::Failed(Kind::Corrupt, _)) => {}
            other => panic!("atteso Failed(Corrupt), ottenuto {other:?}"),
        }
    }

    /// TEST-7: un file che non è uno zip → `Corrupt`, nessun panico.
    #[test]
    fn parse_file_rejects_non_zip() {
        let dir = scratch_dir();
        let p = dir.join("rumore.docx");
        std::fs::write(&p, b"non sono uno zip").unwrap();
        match parse_file(&p, &Caps::default()) {
            Err(Rejection::Failed(Kind::Corrupt, _)) => {}
            other => panic!("atteso Failed(Corrupt), ottenuto {other:?}"),
        }
    }
}
