//! Chunker di misura (piano 01-10): circa 1.600 caratteri con sovrapposizione di 200,
//! mai a cavallo di due pagine o due sezioni; offset in caratteri.
//!
//! STUB test-write 01-10: `chunk_pages` e `chunk_markdown` sono `todo!()`.

/// Lingua del chunk, nome della configurazione testuale di PostgreSQL (`regconfig`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Italian,
    English,
}

/// Lunghezza massima di un chunk, in caratteri.
pub const CHUNK_CHARS: usize = 1_600;
/// Sovrapposizione fra chunk consecutivi della stessa pagina o sezione, in caratteri.
pub const OVERLAP_CHARS: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub doc_key: String,
    /// Pagina fisica del PDF a partire da 1; `None` per i Markdown.
    pub page: Option<u32>,
    /// Intestazioni dalla più esterna alla più interna separate da " > "; `None` per i PDF.
    pub section_path: Option<String>,
    /// Offset in caratteri: relativi alla pagina per i PDF, al documento per i Markdown.
    pub char_start: usize,
    pub char_end: usize,
    pub lang: Lang,
    pub body: String,
}

/// Testo di un PDF con le pagine separate da form feed (`pdftotext -layout`).
pub fn chunk_pages(_doc_key: &str, _lang: Lang, _text: &str) -> Vec<Chunk> {
    todo!("chunk_pages: implementazione del piano 01-10")
}

/// Markdown spezzato per sezione (intestazioni `#`).
pub fn chunk_markdown(_doc_key: &str, _lang: Lang, _md: &str) -> Vec<Chunk> {
    todo!("chunk_markdown: implementazione del piano 01-10")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn slice(s: &str, start: usize, end: usize) -> String {
        chars(s)[start..end].iter().collect()
    }

    /// Pagina 2 di esattamente 4.000 caratteri con «à» e «€» (byte != caratteri).
    fn long_page() -> String {
        let unit = "Ricavi à 12,5 € ";
        let mut s: String = unit.repeat(4_000 / unit.chars().count() + 1);
        s = s.chars().take(4_000).collect();
        assert_eq!(s.chars().count(), 4_000);
        assert!(s.len() > 4_000, "il testo deve avere caratteri multibyte");
        s
    }

    // TEST-10 (RISK-6): chunk mai a cavallo di pagina, offset in caratteri, sovrapposizione 200.
    #[test]
    fn chunk_never_crosses_page_or_section() {
        let p1 = "Prima pagina: utile netto 3,4 € milioni.";
        let p2 = long_page();
        let p3 = "Terza pagina à fine.";
        let text = format!("{p1}\u{c}{p2}\u{c}{p3}");
        let pages = [p1.to_string(), p2.clone(), p3.to_string()];

        let chunks = chunk_pages("doc-x", Lang::Italian, &text);
        assert!(!chunks.is_empty());

        for c in &chunks {
            let page = c.page.expect("ogni chunk PDF ha una pagina") as usize;
            assert!((1..=3).contains(&page), "pagina fuori intervallo: {page}");
            assert_eq!(c.doc_key, "doc-x");
            assert_eq!(c.lang, Lang::Italian);
            assert!(c.section_path.is_none());
            let page_text = &pages[page - 1];
            assert!(c.char_end <= page_text.chars().count());
            assert_eq!(
                c.body,
                slice(page_text, c.char_start, c.char_end),
                "body = pagina[char_start..char_end] in caratteri"
            );
            assert!(
                c.body.chars().count() <= CHUNK_CHARS,
                "chunk oltre 1.600 caratteri"
            );
            assert!(!c.body.contains('\u{c}'), "chunk a cavallo di due pagine");
        }

        // Pagine 1 e 3: un chunk ciascuna, intero.
        for (n, p) in [(1u32, p1), (3u32, p3)] {
            let of_page: Vec<_> = chunks.iter().filter(|c| c.page == Some(n)).collect();
            assert_eq!(of_page.len(), 1, "pagina {n}");
            assert_eq!(of_page[0].body, p);
            assert_eq!(
                (of_page[0].char_start, of_page[0].char_end),
                (0, p.chars().count())
            );
        }

        // Pagina 2: coperta tutta, consecutivi sovrapposti di esattamente 200 caratteri.
        let mut p2c: Vec<_> = chunks.iter().filter(|c| c.page == Some(2)).collect();
        p2c.sort_by_key(|c| c.char_start);
        assert!(p2c.len() >= 3, "4.000 caratteri richiedono almeno 3 chunk");
        assert_eq!(p2c.first().unwrap().char_start, 0);
        assert_eq!(p2c.last().unwrap().char_end, 4_000);
        for w in p2c.windows(2) {
            assert_eq!(
                w[0].char_end - w[1].char_start,
                OVERLAP_CHARS,
                "sovrapposizione 200"
            );
        }
    }

    // TEST-10 (RISK-6), parte Markdown: sezioni adiacenti separate, section_path annidato.
    #[test]
    fn chunk_markdown_sections_are_separate_and_nested() {
        let md = "## A\n\nIntro della sezione A.\n\n### B\n\nTesto di B con 5 €.\n\n### C\n\nTesto di C à.\n\n## D\n\nTesto di D.\n";
        let chunks = chunk_markdown("syn-x", Lang::Italian, md);
        assert!(!chunks.is_empty());

        for c in &chunks {
            assert!(c.page.is_none());
            assert!(c.section_path.is_some());
            assert_eq!(
                c.body,
                slice(md, c.char_start, c.char_end),
                "offset Markdown in caratteri sul documento"
            );
            assert!(c.body.chars().count() <= CHUNK_CHARS);
            let markers = [
                "Testo di B",
                "Testo di C",
                "Testo di D",
                "Intro della sezione A",
            ];
            let found = markers.iter().filter(|m| c.body.contains(*m)).count();
            assert!(found <= 1, "due sezioni nello stesso chunk: {:?}", c.body);
        }

        let path_of = |marker: &str| {
            chunks
                .iter()
                .find(|c| c.body.contains(marker))
                .and_then(|c| c.section_path.clone())
                .unwrap_or_else(|| panic!("nessun chunk con {marker}"))
        };
        assert_eq!(path_of("Intro della sezione A"), "A");
        assert_eq!(path_of("Testo di B"), "A > B");
        assert_eq!(path_of("Testo di C"), "A > C");
        assert_eq!(path_of("Testo di D"), "D");
    }
}
