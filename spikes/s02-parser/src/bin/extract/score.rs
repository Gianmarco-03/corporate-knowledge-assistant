//! Punteggi di `extract score` secondo i «Contratti di abbinamento» di RISULTATI-estrazione.md
//! (piano 01-08): CER, IoU delle parole con abbinamento uno a uno, F1 di cella, recall delle
//! regioni tabella, conversione delle coordinate TAT-DQA, CSV con header fissi.
//!
//! STUB di `test-write`: tipi e firme minime; i corpi sono dell'esecutore.
//! Convenzione dei bbox: `[x0, y0, x1, y1]` in punti PDF, origine in basso a sinistra.
#![allow(dead_code)] // stub di test-write: l'esecutore lo rimuove con l'implementazione

use std::path::Path;

/// Parola con testo e bbox.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Word {
    pub text: String,
    pub bbox: [f64; 4],
}

/// Esito di una parola attesa (o predetta, per `Spurious`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MatchKind {
    Matched,
    Merged,
    Missed,
    Spurious,
    Unscorable,
}

/// Un allineamento: indice dell'attesa (assente per `Spurious`), indici delle predette usate
/// (vuoto per `Missed`/`Unscorable`), IoU (0 per `Missed`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Alignment {
    pub expected: Option<usize>,
    pub predicted: Vec<usize>,
    pub kind: MatchKind,
    pub iou: f64,
}

/// Punteggio di una pagina (una riga di `bbox_iou.csv`). `words` = tutte le attese,
/// valutabili e no; `matched` non include `merged`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PageScore {
    pub words: usize,
    pub matched: usize,
    pub merged: usize,
    pub missed: usize,
    pub spurious: usize,
    pub unscorable: usize,
    pub iou_mean: f64,
    pub share_iou_ge_05: f64,
    pub alignments: Vec<Alignment>,
}

/// Cella `(riga, colonna, testo)`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Cell {
    pub row: u32,
    pub col: u32,
    pub text: String,
}

/// Tabella con pagina, bbox e celle.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Table {
    pub id: String,
    pub page: u32,
    pub bbox: [f64; 4],
    pub cells: Vec<Cell>,
}

/// Punteggio di cella di una tabella attesa (una riga di `tables.csv` senza engine/device).
/// `cells_found` = celle della tabella predetta abbinata (0 se nessuna); con 0 predette
/// `precision` = 0.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TableScore {
    pub table_id: String,
    pub cells_expected: usize,
    pub cells_found: usize,
    pub matched: usize,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
}

/// Riga di `text_cer.csv`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TextCerRow {
    pub source: String,
    pub doc_id: String,
    pub page: u32,
    pub chars_expected: usize,
    pub cer: f64,
    pub two_column: bool,
}

/// NFC e spazi rimossi ai bordi; nient'altro (la punteggiatura resta).
pub(crate) fn normalize_token(_s: &str) -> String {
    todo!("01-08: normalizzazione dei token")
}

/// CER = distanza di Levenshtein sui caratteri / caratteri attesi, sul testo normalizzato
/// come in `parser-it.json` (NFC, spazi e a capo collassati in uno spazio, bordi rimossi).
pub(crate) fn cer(_expected: &str, _predicted: &str) -> f64 {
    todo!("01-08: CER")
}

/// IoU di due bbox; 0 se disgiunti o degeneri.
pub(crate) fn iou(_a: &[f64; 4], _b: &[f64; 4]) -> f64 {
    todo!("01-08: IoU")
}

/// Bbox TAT-DQA `[x0, y0, x1, y1]` in pixel, origine in alto a sinistra, sull'immagine
/// `img_w × img_h` → punti PDF, origine in basso a sinistra, sulla pagina `page_w × page_h`.
pub(crate) fn tatdqa_to_pdf(
    _bbox_px: [f64; 4],
    _img_w: f64,
    _img_h: f64,
    _page_w: f64,
    _page_h: f64,
) -> [f64; 4] {
    todo!("01-08: conversione coordinate TAT-DQA")
}

/// Abbinamento delle parole di una pagina (`page` = box della pagina, per gli `unscorable`).
/// `predicted` è in ordine di lettura (le unioni `merged` usano predette consecutive).
pub(crate) fn score_words(_expected: &[Word], _predicted: &[Word], _page: [f64; 4]) -> PageScore {
    todo!("01-08: abbinamento uno a uno delle parole")
}

/// F1 di cella con abbinamento uno a uno sulle tabelle della stessa pagina (IoU massimo);
/// un risultato per tabella attesa, nell'ordine di `expected`.
pub(crate) fn score_tables(_expected: &[Table], _predicted: &[Table]) -> Vec<TableScore> {
    todo!("01-08: F1 di cella")
}

/// Recall delle regioni tabella: uno a uno greedy per IoU decrescente, match se IoU ≥ 0.5.
pub(crate) fn region_recall(_expected: &[[f64; 4]], _predicted: &[[f64; 4]]) -> f64 {
    todo!("01-08: recall delle regioni")
}

/// `text_cer.csv`: `source,doc_id,page,chars_expected,cer,two_column`.
pub(crate) fn write_text_cer(_path: &Path, _rows: &[TextCerRow]) -> anyhow::Result<()> {
    todo!("01-08: text_cer.csv")
}

/// `bbox_iou.csv`: `doc_id,page,words,matched,merged,missed,spurious,unscorable,iou_mean,share_iou_ge_05`.
pub(crate) fn write_bbox_iou(
    _path: &Path,
    _rows: &[(String, u32, PageScore)],
) -> anyhow::Result<()> {
    todo!("01-08: bbox_iou.csv")
}

/// `tables.csv`: `engine,device,table_id,cells_expected,cells_found,precision,recall,f1`.
pub(crate) fn write_tables(
    _path: &Path,
    _engine: &str,
    _device: &str,
    _rows: &[TableScore],
) -> anyhow::Result<()> {
    todo!("01-08: tables.csv")
}

#[cfg(test)]
mod tests {
    //! TEST-5 (abbinamento uno a uno, merged/missed/spurious/unscorable, casi (a)-(d) del
    //! piano) e TEST-7 (header e coerenza dei CSV scritti da `score`).
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    const PAGE: [f64; 4] = [0.0, 0.0, 612.0, 792.0];

    fn w(text: &str, bbox: [f64; 4]) -> Word {
        Word {
            text: text.to_owned(),
            bbox,
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn assert_close(a: f64, b: f64) {
        assert!(close(a, b), "{a} != {b}");
    }

    /// Invarianti comuni: conteggi coerenti, nessuna predetta riusata, spurie = non usate.
    fn assert_invariants(s: &PageScore, expected: usize, predicted: usize) {
        assert_eq!(s.words, expected);
        assert_eq!(
            s.matched + s.merged + s.missed + s.unscorable,
            s.words,
            "{s:?}"
        );
        let mut used = BTreeSet::new();
        for a in &s.alignments {
            if a.kind == MatchKind::Spurious {
                continue;
            }
            for &p in &a.predicted {
                assert!(used.insert(p), "predetta {p} usata due volte: {s:?}");
            }
        }
        assert_eq!(s.spurious, predicted - used.len(), "{s:?}");
        assert!((0.0..=1.0).contains(&s.share_iou_ge_05), "{s:?}");
        assert!((0.0..=1.0).contains(&s.iou_mean), "{s:?}");
        let kinds = |k| s.alignments.iter().filter(|a| a.kind == k).count();
        assert_eq!(kinds(MatchKind::Matched), s.matched);
        assert_eq!(kinds(MatchKind::Merged), s.merged);
        assert_eq!(kinds(MatchKind::Missed), s.missed);
        assert_eq!(kinds(MatchKind::Spurious), s.spurious);
        assert_eq!(kinds(MatchKind::Unscorable), s.unscorable);
    }

    // ---- normalizzazione, CER, IoU -------------------------------------------------------

    #[test]
    fn token_normalization_is_nfc_and_trim_only() {
        assert_eq!(normalize_token("  12 "), "12");
        assert_eq!(normalize_token("e\u{300}"), "\u{e8}");
        assert_eq!(normalize_token("1.234"), "1.234");
        assert_ne!(normalize_token("1.234"), normalize_token("1234"));
        assert_eq!(normalize_token("Ricavi,"), "Ricavi,");
    }

    #[test]
    fn cer_is_levenshtein_over_expected_chars() {
        assert_close(cer("abcd", "abcd"), 0.0);
        assert_close(cer("abcd", "abed"), 0.25);
        assert_close(cer("abcd", "abc"), 0.25);
        assert_close(cer("1.234", "1234"), 0.2);
        assert_close(cer("a  b\nc", "a b c"), 0.0);
        assert_close(cer("perch\u{e9}", "perche\u{301}"), 0.0);
    }

    #[test]
    fn iou_basics() {
        let a = [0.0, 0.0, 10.0, 10.0];
        assert_close(iou(&a, &a), 1.0);
        assert_close(iou(&a, &[5.0, 0.0, 15.0, 10.0]), 50.0 / 150.0);
        assert_close(iou(&a, &[20.0, 0.0, 30.0, 10.0]), 0.0);
        assert_close(iou(&a, &[5.0, 5.0, 5.0, 8.0]), 0.0);
    }

    // ---- (d) coordinate TAT-DQA -----------------------------------------------------------

    #[test]
    fn tatdqa_pixels_top_left_to_pdf_points_bottom_left() {
        // Immagine 1700 × 2200 px, pagina 612 × 792 pt: 0,36 pt/px su entrambi gli assi.
        // x: 170·0,36 = 61,2; 340·0,36 = 122,4. y: 792 − 330·0,36 = 673,2; 792 − 220·0,36 = 712,8.
        let b = tatdqa_to_pdf([170.0, 220.0, 340.0, 330.0], 1700.0, 2200.0, 612.0, 792.0);
        for (got, want) in b.iter().zip([61.2, 673.2, 122.4, 712.8]) {
            assert!((got - want).abs() < 1e-6, "{b:?}");
        }
        // Scala diversa sugli assi: immagine 1000 × 1000, pagina 500 × 800.
        let b = tatdqa_to_pdf([0.0, 0.0, 1000.0, 500.0], 1000.0, 1000.0, 500.0, 800.0);
        for (got, want) in b.iter().zip([0.0, 400.0, 500.0, 800.0]) {
            assert!((got - want).abs() < 1e-6, "{b:?}");
        }
    }

    // ---- (a) colonna intera mancante -----------------------------------------------------

    #[test]
    fn missing_column_counts_as_missed_and_lowers_share() {
        let mut expected = Vec::new();
        for r in 0..4 {
            for c in 0..3 {
                let y = 700.0 - 20.0 * r as f64;
                let x = 100.0 + 120.0 * c as f64;
                expected.push(w(&format!("r{r}c{c}"), [x, y, x + 40.0, y + 10.0]));
            }
        }
        let predicted: Vec<Word> = expected
            .iter()
            .filter(|e| !e.text.ends_with("c2"))
            .cloned()
            .collect();
        let s = score_words(&expected, &predicted, PAGE);
        assert_invariants(&s, 12, 8);
        assert_eq!((s.matched, s.merged, s.missed, s.spurious), (8, 0, 4, 0));
        assert_close(s.share_iou_ge_05, 8.0 / 12.0);
        assert_close(s.iou_mean, 8.0 / 12.0);
        for a in s.alignments.iter().filter(|a| a.kind == MatchKind::Missed) {
            let e = &expected[a.expected.unwrap()];
            assert!(e.text.ends_with("c2"), "miss fuori colonna: {e:?}");
            assert_close(a.iou, 0.0);
        }
    }

    // ---- (b) numeri identici: uno a uno ---------------------------------------------------

    #[test]
    fn identical_numbers_are_matched_one_to_one() {
        // Dieci "100" larghi 20 pt a passo 18: ognuno si sovrappone ai vicini (IoU > 0).
        let expected: Vec<Word> = (0..10)
            .map(|i| {
                let x = 50.0 + 18.0 * i as f64;
                w("100", [x, 500.0, x + 20.0, 510.0])
            })
            .collect();
        let predicted: Vec<Word> = expected
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 4)
            .map(|(_, e)| e.clone())
            .collect();
        let s = score_words(&expected, &predicted, PAGE);
        assert_invariants(&s, 10, 9);
        assert_eq!((s.matched, s.missed, s.spurious), (9, 1, 0));
        let missed: Vec<usize> = s
            .alignments
            .iter()
            .filter(|a| a.kind == MatchKind::Missed)
            .map(|a| a.expected.unwrap())
            .collect();
        assert_eq!(missed, [4], "la mancante è la quinta, non un vicino");
        assert_close(s.share_iou_ge_05, 0.9);
        assert_close(s.iou_mean, 0.9);
    }

    // ---- (c) token spezzato --------------------------------------------------------------

    #[test]
    fn split_token_is_merged_with_union_bbox() {
        let expected = [w("1.234", [100.0, 400.0, 140.0, 410.0])];
        let predicted = [
            w("1.", [100.0, 400.0, 112.0, 410.0]),
            w("234", [112.0, 400.0, 140.0, 410.0]),
        ];
        let s = score_words(&expected, &predicted, PAGE);
        assert_invariants(&s, 1, 2);
        assert_eq!((s.matched, s.merged, s.missed, s.spurious), (0, 1, 0, 0));
        let a = s
            .alignments
            .iter()
            .find(|a| a.kind == MatchKind::Merged)
            .unwrap();
        assert_eq!(a.predicted, [0, 1]);
        assert_close(a.iou, 1.0);
        assert_close(s.share_iou_ge_05, 1.0);
    }

    #[test]
    fn three_pieces_merge_but_other_lines_do_not() {
        let expected = [w("1.234", [100.0, 400.0, 140.0, 410.0])];
        let three = [
            w("1", [100.0, 400.0, 106.0, 410.0]),
            w(".", [106.0, 400.0, 112.0, 410.0]),
            w("234", [112.0, 400.0, 140.0, 410.0]),
        ];
        let s = score_words(&expected, &three, PAGE);
        assert_eq!((s.merged, s.missed, s.spurious), (1, 0, 0), "{s:?}");

        // Stessa concatenazione ma su due righe diverse: niente unione.
        let lines = [
            w("1.", [100.0, 400.0, 112.0, 410.0]),
            w("234", [100.0, 380.0, 128.0, 390.0]),
        ];
        let s = score_words(&expected, &lines, PAGE);
        assert_invariants(&s, 1, 2);
        assert_eq!((s.merged, s.missed, s.spurious), (0, 1, 2), "{s:?}");
    }

    // ---- testo, spurie, non valutabili ---------------------------------------------------

    #[test]
    fn text_must_match_after_normalization() {
        let b = [100.0, 400.0, 140.0, 410.0];
        let s = score_words(&[w("1.234", b)], &[w("1234", b)], PAGE);
        assert_eq!((s.matched, s.missed, s.spurious), (0, 1, 1), "{s:?}");
        let s = score_words(&[w("perch\u{e9}", b)], &[w(" perche\u{301} ", b)], PAGE);
        assert_eq!((s.matched, s.missed, s.spurious), (1, 0, 0), "{s:?}");
    }

    #[test]
    fn unscorable_words_are_counted_but_out_of_denominator() {
        let expected = [
            w("ok", [100.0, 400.0, 120.0, 410.0]),
            w("", [130.0, 400.0, 150.0, 410.0]),
            w("fuori", [700.0, 400.0, 740.0, 410.0]),
            w("piatta", [200.0, 400.0, 200.0, 410.0]),
        ];
        let predicted = [
            w("ok", [100.0, 400.0, 120.0, 410.0]),
            w("extra", [300.0, 300.0, 330.0, 310.0]),
        ];
        let s = score_words(&expected, &predicted, PAGE);
        assert_invariants(&s, 4, 2);
        assert_eq!(
            (s.matched, s.missed, s.spurious, s.unscorable),
            (1, 0, 1, 3)
        );
        assert_close(s.share_iou_ge_05, 1.0);
        assert_close(s.iou_mean, 1.0);
    }

    #[test]
    fn low_iou_match_is_counted_but_not_in_share() {
        // Stesso testo, IoU = 1/3 < 0,5: match uno a uno, ma fuori dalla quota ≥ 0,5.
        let s = score_words(
            &[w("Ricavi", [0.0, 0.0, 10.0, 10.0])],
            &[w("Ricavi", [5.0, 0.0, 15.0, 10.0])],
            PAGE,
        );
        assert_eq!(s.matched, 1);
        assert_close(s.share_iou_ge_05, 0.0);
        assert_close(s.iou_mean, 50.0 / 150.0);
    }

    /// Proprietà su pagine pseudo-casuali deterministiche: testi ripetuti, predette mancanti,
    /// spostate, spezzate o in più. Le invarianti di conteggio e unicità valgono sempre.
    #[test]
    fn property_counts_and_uniqueness_hold_on_random_pages() {
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = |m: u64| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x % m
        };
        let vocab = ["100", "1.234", "Ricavi", "2023", "-", "Totale"];
        for _ in 0..200 {
            let n = 1 + next(30) as usize;
            let mut expected = Vec::new();
            let mut predicted = Vec::new();
            for i in 0..n {
                let row = (i / 5) as f64;
                let col = (i % 5) as f64;
                let b = [
                    50.0 + 60.0 * col,
                    700.0 - 15.0 * row,
                    50.0 + 60.0 * col + 40.0,
                    710.0 - 15.0 * row,
                ];
                let t = vocab[next(vocab.len() as u64) as usize];
                expected.push(w(t, b));
                match next(5) {
                    0 => {}
                    1 => {
                        let d = next(30) as f64;
                        predicted.push(w(t, [b[0] + d, b[1], b[2] + d, b[3]]));
                    }
                    2 if t.len() > 1 => {
                        let mid = (b[0] + b[2]) / 2.0;
                        predicted.push(w(&t[..1], [b[0], b[1], mid, b[3]]));
                        predicted.push(w(&t[1..], [mid, b[1], b[2], b[3]]));
                    }
                    3 => {
                        predicted.push(w(t, b));
                        predicted.push(w(t, [b[0] + 3.0, b[1], b[2] + 3.0, b[3]]));
                    }
                    _ => predicted.push(w(t, b)),
                }
            }
            let s = score_words(&expected, &predicted, PAGE);
            assert_invariants(&s, expected.len(), predicted.len());
        }
    }

    // ---- celle e regioni -----------------------------------------------------------------

    fn cell(row: u32, col: u32, text: &str) -> Cell {
        Cell {
            row,
            col,
            text: text.to_owned(),
        }
    }

    fn table(id: &str, page: u32, bbox: [f64; 4], cells: Vec<Cell>) -> Table {
        Table {
            id: id.to_owned(),
            page,
            bbox,
            cells,
        }
    }

    #[test]
    fn cells_are_a_multiset_matched_once_each() {
        let b = [0.0, 0.0, 100.0, 100.0];
        let e = table(
            "t",
            1,
            b,
            vec![cell(0, 0, "a"), cell(0, 0, "a"), cell(0, 1, "b")],
        );
        let p = table(
            "p",
            1,
            b,
            vec![cell(0, 0, "a"), cell(0, 1, "b"), cell(0, 1, "b")],
        );
        let s = &score_tables(&[e], &[p])[0];
        assert_eq!((s.cells_expected, s.cells_found, s.matched), (3, 3, 2));
        assert_close(s.precision, 2.0 / 3.0);
        assert_close(s.recall, 2.0 / 3.0);
        assert_close(s.f1, 2.0 / 3.0);
    }

    #[test]
    fn tables_pair_by_page_and_max_iou_and_unmatched_expected_is_all_miss() {
        let left = [0.0, 0.0, 100.0, 100.0];
        let right = [300.0, 0.0, 400.0, 100.0];
        let e = [
            table("left", 1, left, vec![cell(0, 0, "L1"), cell(1, 0, "L2")]),
            table("right", 1, right, vec![cell(0, 0, "R1")]),
            table("p2", 2, left, vec![cell(0, 0, "X"), cell(0, 1, "Y")]),
        ];
        // Predette in ordine inverso; nessuna tabella a pagina 2 (una a pagina 3 con bbox uguale).
        let p = [
            table("pr", 1, right, vec![cell(0, 0, "R1")]),
            table("pl", 1, left, vec![cell(0, 0, "L1"), cell(1, 0, "L2")]),
            table("p3", 3, left, vec![cell(0, 0, "X"), cell(0, 1, "Y")]),
        ];
        let s = score_tables(&e, &p);
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].table_id, "left");
        assert_close(s[0].f1, 1.0);
        assert_eq!(s[1].table_id, "right");
        assert_close(s[1].f1, 1.0);
        assert_eq!(s[2].table_id, "p2");
        assert_eq!(
            (s[2].cells_expected, s[2].cells_found, s[2].matched),
            (2, 0, 0)
        );
        assert_close(s[2].recall, 0.0);
        assert_close(s[2].precision, 0.0);
        assert_close(s[2].f1, 0.0);
    }

    #[test]
    fn region_recall_is_one_to_one_with_iou_ge_05() {
        let a = [0.0, 0.0, 100.0, 100.0];
        let a2 = [0.0, 10.0, 100.0, 110.0];
        assert_close(region_recall(&[a, a2], &[a]), 0.5);
        assert_close(region_recall(&[a, a2], &[a, a2]), 1.0);
        // IoU = 1/3 sotto la soglia.
        assert_close(region_recall(&[a], &[[50.0, 0.0, 150.0, 100.0]]), 0.0);
        assert_close(region_recall(&[a], &[]), 0.0);
    }

    // ---- TEST-7: CSV ---------------------------------------------------------------------

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::var_os("S02_TEST_BASE").map_or_else(std::env::temp_dir, PathBuf::from);
        let d = root.join(format!("s02-score-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn lines(path: &Path) -> Vec<Vec<String>> {
        fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|l| l.split(',').map(str::to_owned).collect())
            .collect()
    }

    #[test]
    fn text_cer_csv_has_fixed_header_and_rows() {
        let p = scratch("cer").join("text_cer.csv");
        write_text_cer(
            &p,
            &[
                TextCerRow {
                    source: "tat-dqa".into(),
                    doc_id: "d1".into(),
                    page: 1,
                    chars_expected: 1200,
                    cer: 0.004,
                    two_column: false,
                },
                TextCerRow {
                    source: "oracle-it".into(),
                    doc_id: "it-2col".into(),
                    page: 3,
                    chars_expected: 900,
                    cer: 0.01,
                    two_column: true,
                },
            ],
        )
        .unwrap();
        let l = lines(&p);
        assert_eq!(
            l[0].join(","),
            "source,doc_id,page,chars_expected,cer,two_column"
        );
        assert_eq!(l.len(), 3);
        assert_eq!(&l[1][..4], ["tat-dqa", "d1", "1", "1200"]);
        assert!((l[1][4].parse::<f64>().unwrap() - 0.004).abs() < 1e-9);
        assert_eq!(l[1][5], "false");
        assert_eq!(l[2][0], "oracle-it");
        assert_eq!(l[2][5], "true");
    }

    #[test]
    fn bbox_iou_csv_has_fixed_header_and_consistent_counts() {
        let expected = [
            w("a", [10.0, 10.0, 20.0, 20.0]),
            w("b", [30.0, 10.0, 40.0, 20.0]),
            w("", [50.0, 10.0, 60.0, 20.0]),
        ];
        let predicted = [
            w("a", [10.0, 10.0, 20.0, 20.0]),
            w("z", [90.0, 10.0, 99.0, 20.0]),
        ];
        let s = score_words(&expected, &predicted, PAGE);
        let p = scratch("iou").join("bbox_iou.csv");
        write_bbox_iou(&p, &[("doc-1".into(), 2, s)]).unwrap();
        let l = lines(&p);
        assert_eq!(
            l[0].join(","),
            "doc_id,page,words,matched,merged,missed,spurious,unscorable,iou_mean,share_iou_ge_05"
        );
        assert_eq!(l.len(), 2);
        assert_eq!(l[1].len(), 10);
        assert_eq!(&l[1][..8], ["doc-1", "2", "3", "1", "0", "1", "1", "1"]);
        let n: Vec<usize> = l[1][2..8].iter().map(|v| v.parse().unwrap()).collect();
        assert_eq!(
            n[0],
            n[1] + n[2] + n[3] + n[5],
            "words = matched+merged+missed+unscorable"
        );
        assert!((l[1][8].parse::<f64>().unwrap() - 0.5).abs() < 1e-9);
        assert!((l[1][9].parse::<f64>().unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn tables_csv_has_fixed_header() {
        let p = scratch("tables").join("tables.csv");
        let rows = [TableScore {
            table_id: "it-t1".into(),
            cells_expected: 40,
            cells_found: 38,
            matched: 36,
            precision: 36.0 / 38.0,
            recall: 0.9,
            f1: 2.0 * (36.0 / 38.0) * 0.9 / (36.0 / 38.0 + 0.9),
        }];
        write_tables(&p, "docling", "gpu", &rows).unwrap();
        let l = lines(&p);
        assert_eq!(
            l[0].join(","),
            "engine,device,table_id,cells_expected,cells_found,precision,recall,f1"
        );
        assert_eq!(l.len(), 2);
        assert_eq!(&l[1][..5], ["docling", "gpu", "it-t1", "40", "38"]);
        assert!((l[1][6].parse::<f64>().unwrap() - 0.9).abs() < 1e-9);
    }
}
