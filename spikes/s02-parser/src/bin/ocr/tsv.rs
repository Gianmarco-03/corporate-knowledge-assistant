//! Parser del TSV di Tesseract (piano 01-13, TEST-8).
//!
//! STUB del test-write (2026-10-23): corpo `todo!()` scritto dall'esecutore del Task 2.
//! Colonne attese ([NV] RESEARCH §11.6): `level page_num block_num par_num line_num word_num
//! left top width height conf text`; si tengono le sole righe di livello 5 (parole), coordinate
//! in pixel con origine in alto a sinistra.

/// Parola OCR in pixel.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Word {
    pub page: u32,
    pub block: u32,
    pub par: u32,
    pub line: u32,
    pub word: u32,
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
    pub conf: f64,
    pub text: String,
}

/// Parole di livello 5 con testo non vuoto; header diverso da quello atteso → `Err`.
pub(crate) fn parse_tsv(_text: &str) -> Result<Vec<Word>, String> {
    todo!("01-13 Task 2: parse_tsv")
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext";

    #[test]
    fn test8_tsv_keeps_level5_words_with_pixel_bbox() {
        let tsv = format!(
            "{HEADER}\n\
             1\t1\t0\t0\t0\t0\t0\t0\t2480\t3508\t-1\t\n\
             4\t1\t1\t1\t1\t0\t300\t300\t500\t40\t-1\t\n\
             5\t1\t1\t1\t1\t1\t300\t301\t210\t38\t96.5\tATTIVO\n\
             5\t1\t1\t1\t1\t2\t530\t300\t180\t39\t91.0\tTOTALE\n\
             5\t1\t1\t1\t1\t3\t720\t300\t5\t39\t95.0\t \n"
        );
        let w = parse_tsv(&tsv).unwrap();
        assert_eq!(w.len(), 2, "solo livello 5 con testo");
        assert_eq!(
            w[0],
            Word {
                page: 1,
                block: 1,
                par: 1,
                line: 1,
                word: 1,
                left: 300,
                top: 301,
                width: 210,
                height: 38,
                conf: 96.5,
                text: "ATTIVO".into()
            }
        );
        assert_eq!(w[1].text, "TOTALE");
    }

    #[test]
    fn test8_tsv_keeps_utf8_and_preserved_spaces_in_text() {
        let tsv = format!("{HEADER}\n5\t1\t1\t1\t1\t1\t10\t10\t50\t20\t88\tL’attività\n");
        assert_eq!(parse_tsv(&tsv).unwrap()[0].text, "L’attività");
    }

    #[test]
    fn test8_tsv_rejects_unexpected_header_or_short_rows() {
        assert!(parse_tsv("level\tpage_num\tleft\ttop\ttext\n5\t1\t1\t1\tX\n").is_err());
        assert!(parse_tsv(&format!("{HEADER}\n5\t1\t1\t1\t1\t1\t10\t10\n")).is_err());
        assert!(parse_tsv("").is_err());
    }
}
