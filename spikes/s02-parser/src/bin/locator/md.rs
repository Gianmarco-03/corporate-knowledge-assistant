//! Lettore Markdown → blocchi `cka-text-v1` (piano 01-09, Task 1).
//!
//! STUB di `test-write`: firma minima perché i test rossi compilino. L'implementazione è
//! dell'esecutore; si esegue solo nel figlio `locator locate` (job `op: locate`).
use crate::canon::Parsed;

/// Blocchi canonici di un Markdown: front matter, `\newpage`, immagini e note esclusi; HTML
/// grezzo in `unsupported`; inline senza marcatori di enfasi, link ridotti al testo.
pub(crate) fn parse(_text: &str) -> Parsed {
    todo!("01-09: md::parse")
}

#[cfg(test)]
pub(crate) mod tests {
    //! Test (3) lato MD, (4), (5) del Task 1; TEST-7 lato Markdown (link e HTML grezzo non
    //! risolti); TEST-1 sulla proiezione stabile.
    use super::*;
    use crate::canon::{self, Block, CharRange, SectionStep};

    fn step(title: &str, ordinal: u32) -> SectionStep {
        SectionStep {
            title: title.to_owned(),
            ordinal,
        }
    }

    fn texts(md: &str) -> Vec<(Vec<SectionStep>, String)> {
        canon::sections(&parse(md))
            .sections
            .into_iter()
            .map(|s| (s.path, s.text))
            .collect()
    }

    /// Documento MD gemello di `docx::tests::TWIN_DOCUMENT_XML` (test (3)).
    pub(crate) const TWIN_MD: &str = "---\n\
doc_id: twin\n\
title: Non entra\n\
---\n\
\n\
# Relazione\n\
\n\
## Risultati **chiave** del [periodo](https://attacker.invalid/x)\n\
\n\
Il fatturato \u{e8} cresciuto del 5%.\n\
\n\
\\newpage\n\
\n\
| Voce | 2025 | 2026 |\n\
|---|---:|---:|\n\
| Ricavi | 182,4 | 196,0 |\n\
| EBITDA | 27,1 | 30,4 |\n";

    /// Testo canonico atteso della sezione «Risultati chiave del periodo», scritto a mano.
    pub(crate) const TWIN_SECTION_TEXT: &str = "Il fatturato \u{e8} cresciuto del 5%.\nVoce\t2025\t2026\nRicavi\t182,4\t196,0\nEBITDA\t27,1\t30,4";

    /// Test (3) lato MD: titolo con enfasi e link, `\newpage` escluso, tabella multi-cella.
    #[test]
    fn twin_markdown_has_hand_written_canonical_text() {
        assert_eq!(
            texts(TWIN_MD),
            vec![
                (vec![step("Relazione", 1)], String::new()),
                (
                    vec![
                        step("Relazione", 1),
                        step("Risultati chiave del periodo", 1)
                    ],
                    TWIN_SECTION_TEXT.to_owned()
                ),
            ]
        );
    }

    /// Test (4): `A😀e` + `q\u{301}` + CRLF + forma decomposta → `char_range` scritto a mano
    /// (A0 😀1 e2 ␠3 q4 ◌́5 x6 ␠7 p8 e9 r10 c11 h12 é13 ␠14 f15 i16 n17 e18), riestratto
    /// identico.
    #[test]
    fn astral_combining_and_crlf_offsets_are_hand_checked() {
        let md = "# Titolo\r\n\r\nA\u{1F600}e\r\nq\u{301}x perche\u{301} fine\r\n";
        let p = canon::sections(&parse(md));
        assert_eq!(p.sections.len(), 1);
        let text = &p.sections[0].text;
        assert_eq!(text, "A\u{1F600}e q\u{301}x perch\u{e9} fine");
        assert!(!text.contains('\r'));
        let sha = "3333333333333333333333333333333333333333333333333333333333333333";
        for (excerpt, start, end, again) in [
            ("q\u{301}x", 4, 7, "q\u{301}x"),
            ("perche\u{301}", 8, 14, "perch\u{e9}"),
            ("A\u{1F600}e q", 0, 5, "A\u{1F600}e q"),
        ] {
            let locs = canon::locate(&p, sha, excerpt);
            assert_eq!(locs.len(), 1, "{excerpt:?}");
            assert_eq!(locs[0].char_range, CharRange { start, end }, "{excerpt:?}");
            assert_eq!(canon::resolve(&locs[0], &p, sha).as_deref(), Ok(again));
        }
    }

    /// Test (5): il front matter YAML non entra nel testo.
    #[test]
    fn front_matter_is_excluded() {
        let md = "---\ndoc_id: x\ntitle: Segreto\n---\n\n# A\n\nTesto.\n";
        assert_eq!(texts(md), vec![(vec![step("A", 1)], "Testo.".to_owned())]);
    }

    /// Paragrafi: righe unite da uno spazio, spazi ASCII compressi, estremi rimossi.
    #[test]
    fn paragraph_lines_join_with_single_space() {
        let md = "# A\n\n  Prima   riga\nseconda    riga  \n\nAltro.\n";
        assert_eq!(
            texts(md),
            vec![(
                vec![step("A", 1)],
                "Prima riga seconda riga\nAltro.".to_owned()
            )]
        );
    }

    /// Elenchi, codice, escape ed enfasi secondo `cka-text-v1`.
    #[test]
    fn lists_code_escapes_and_emphasis() {
        let md = "# A\n\n- uno\n* due\n+ tre\n1. quattro\n\n```\n# non titolo\n  rientro\n```\n\n\\*letterale\\* __forte__ _corsivo_ ~~barrato~~ `codice`\n";
        let p = canon::sections(&parse(md));
        assert_eq!(p.sections.len(), 1, "il # nel codice non è un titolo");
        assert_eq!(
            p.sections[0].text,
            "uno\ndue\ntre\nquattro\n# non titolo\n  rientro\n*letterale* forte corsivo barrato codice"
        );
    }

    /// Titoli ATX: livello dal numero di `#`, testo inline canonico.
    #[test]
    fn atx_headings_are_canonical_blocks() {
        let p = parse("# Uno\n\n### Tre **b**\n\n###### Sei\n");
        assert_eq!(
            p.blocks,
            vec![
                Block::Heading {
                    level: 1,
                    text: "Uno".to_owned()
                },
                Block::Heading {
                    level: 3,
                    text: "Tre b".to_owned()
                },
                Block::Heading {
                    level: 6,
                    text: "Sei".to_owned()
                },
            ]
        );
    }

    /// TEST-7 lato MD: URL dei link e HTML grezzo non entrano nel testo; l'HTML è
    /// registrato in `unsupported`.
    #[test]
    fn links_and_raw_html_are_not_resolved() {
        let md = "# A\n\nVedi [la nota](javascript:alert(1)) e ![logo](https://attacker.invalid/p.png).\n\n<script>fetch('https://attacker.invalid')</script>\n\nFine.\n";
        let parsed = parse(md);
        let p = canon::sections(&parsed);
        let all: String = p.sections.iter().map(|s| s.text.as_str()).collect();
        assert!(!all.contains("javascript"), "{all:?}");
        assert!(!all.contains("attacker"), "{all:?}");
        assert!(!all.contains("script"), "{all:?}");
        assert!(all.contains("Vedi la nota e"), "{all:?}");
        assert!(all.contains("Fine."), "{all:?}");
        assert!(!parsed.unsupported.is_empty(), "HTML grezzo non registrato");
    }

    /// Note a piè di pagina (riferimenti e definizioni) escluse.
    #[test]
    fn footnotes_are_excluded() {
        let md = "# A\n\nTesto[^1] finale.\n\n[^1]: Nota nascosta.\n";
        assert_eq!(
            texts(md),
            vec![(vec![step("A", 1)], "Testo finale.".to_owned())]
        );
    }
}
