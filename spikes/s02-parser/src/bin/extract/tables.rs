//! `extract select-tables --params <json>` (piano 01-08): euristica PDFium per le
//! pagine-tabella, banda di incertezza, split taratura/holdout e `table_pages.csv`.
//!
//! STUB di `test-write`: tipi e firme minime; i corpi sono dell'esecutore. I parametri
//! dell'euristica vivono solo nel file `--params` (nessuna costante nascosta, nessun default).
#![allow(dead_code)] // stub di test-write: l'esecutore lo rimuove con l'implementazione

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Parametri dell'euristica letti da `--params` (campi decisi dall'esecutore).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Params {
    pub band: Band,
}

/// Banda di decisione sul punteggio di pagina: `score ≥ select_from` → tabella;
/// `uncertain_from ≤ score < select_from` → incerta (mandata comunque a Docling); sotto → no.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Band {
    pub uncertain_from: f64,
    pub select_from: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Selection {
    Table,
    Uncertain,
    Skip,
}

impl Selection {
    /// Valore di `method` in `table_pages.csv` e verso Docling: `heuristic`,
    /// `heuristic+uncertain`; `None` per le pagine non selezionate.
    pub(crate) fn method(self) -> Option<&'static str> {
        todo!("01-08: method della selezione")
    }

    /// `true` se la pagina va a Docling (tabella o incerta).
    pub(crate) fn sent_to_docling(self) -> bool {
        todo!("01-08: invio a Docling")
    }
}

/// Riga di `table_pages.csv` (`hits` = pagine-tabella vere selezionate; non è una colonna).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TablePagesRow {
    pub doc_id: String,
    pub split: String,
    pub method: String,
    pub pages_true: usize,
    pub pages_selected: usize,
    pub hits: usize,
    pub recall: f64,
    pub precision: f64,
}

/// Parametri dal file; file assente o non valido → errore (mai un default).
pub(crate) fn load_params(_path: &Path) -> anyhow::Result<Params> {
    todo!("01-08: parametri dell'euristica")
}

/// Classe della pagina dato il punteggio e la banda.
pub(crate) fn classify(_score: f64, _band: &Band) -> Selection {
    todo!("01-08: banda di incertezza")
}

/// Recall e precisione delle pagine selezionate (`selected` = tabella o incerta) contro le
/// pagine-tabella vere; recall 1 con zero vere, precisione 1 con zero selezionate.
pub(crate) fn recall_precision(_truth: &BTreeSet<u32>, _selected: &BTreeSet<u32>) -> (f64, f64) {
    todo!("01-08: recall e precisione per documento")
}

/// Recall cumulata (Σ hits / Σ pages_true) delle sole righe `split = holdout`; `None` se
/// non ce ne sono. È l'unico numero confrontato con la soglia.
pub(crate) fn holdout_recall(_rows: &[TablePagesRow]) -> Option<f64> {
    todo!("01-08: recall di holdout")
}

/// Split disgiunti per item e per documento (`doc_of`: item → documento); errore se un item
/// o un documento compare in entrambi, o se un item non ha documento.
pub(crate) fn validate_splits(
    _tune: &[String],
    _holdout: &[String],
    _doc_of: &BTreeMap<String, String>,
) -> Result<(), String> {
    todo!("01-08: split disgiunti")
}

/// `table_pages.csv`: `doc_id,split,method,pages_true,pages_selected,recall,precision`.
pub(crate) fn write_table_pages(_path: &Path, _rows: &[TablePagesRow]) -> anyhow::Result<()> {
    todo!("01-08: table_pages.csv")
}

#[cfg(test)]
mod tests {
    //! TEST-6 (taratura sul tune, esito solo sull'holdout, parametri solo dal file, banda di
    //! incertezza verso Docling) e TEST-7 (header di `table_pages.csv`).
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::var_os("S02_TEST_BASE").map_or_else(std::env::temp_dir, PathBuf::from);
        let d = root.join(format!("s02-tables-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn set(v: &[u32]) -> BTreeSet<u32> {
        v.iter().copied().collect()
    }

    fn row(doc: &str, split: &str, pages_true: usize, hits: usize) -> TablePagesRow {
        TablePagesRow {
            doc_id: doc.into(),
            split: split.into(),
            method: "heuristic".into(),
            pages_true,
            pages_selected: hits,
            hits,
            recall: if pages_true == 0 {
                1.0
            } else {
                hits as f64 / pages_true as f64
            },
            precision: 1.0,
        }
    }

    #[test]
    fn params_never_fall_back_to_defaults() {
        let d = scratch("params");
        assert!(load_params(&d.join("assente.json")).is_err());
        let bad = d.join("bad.json");
        fs::write(&bad, b"{ non json").unwrap();
        assert!(load_params(&bad).is_err());
        let empty = d.join("empty.json");
        fs::write(&empty, b"{}").unwrap();
        assert!(
            load_params(&empty).is_err(),
            "parametri mancanti riempiti da default"
        );
    }

    #[test]
    fn band_boundaries_and_uncertain_pages_go_to_docling() {
        let b = Band {
            uncertain_from: 0.3,
            select_from: 0.6,
        };
        assert_eq!(classify(0.6, &b), Selection::Table);
        assert_eq!(classify(0.9, &b), Selection::Table);
        assert_eq!(classify(0.3, &b), Selection::Uncertain);
        assert_eq!(classify(0.59, &b), Selection::Uncertain);
        assert_eq!(classify(0.29, &b), Selection::Skip);
        assert_eq!(Selection::Table.method(), Some("heuristic"));
        assert_eq!(Selection::Uncertain.method(), Some("heuristic+uncertain"));
        assert_eq!(Selection::Skip.method(), None);
        assert!(Selection::Table.sent_to_docling());
        assert!(Selection::Uncertain.sent_to_docling());
        assert!(!Selection::Skip.sent_to_docling());
    }

    #[test]
    fn recall_precision_per_document() {
        let (r, p) = recall_precision(&set(&[3, 7, 9, 12]), &set(&[3, 7, 8, 9]));
        assert!(
            (r - 0.75).abs() < 1e-9 && (p - 0.75).abs() < 1e-9,
            "{r} {p}"
        );
        let (r, p) = recall_precision(&set(&[]), &set(&[]));
        assert_eq!((r, p), (1.0, 1.0));
        let (r, _) = recall_precision(&set(&[1]), &set(&[]));
        assert_eq!(r, 0.0);
    }

    #[test]
    fn verdict_uses_only_holdout_rows_pooled() {
        // Tune pessimo, holdout perfetto: l'esito è quello dell'holdout.
        let rows = [
            row("t1", "tune", 4, 2),
            row("h1", "holdout", 3, 3),
            row("it-25", "holdout", 5, 5),
        ];
        assert_eq!(holdout_recall(&rows), Some(1.0));
        // Tune perfetto, holdout 9/10 cumulato (non la media delle recall per documento).
        let rows = [
            row("t1", "tune", 10, 10),
            row("h1", "holdout", 1, 0),
            row("it-25", "holdout", 9, 9),
        ];
        let r = holdout_recall(&rows).unwrap();
        assert!((r - 0.9).abs() < 1e-9, "{r}");
        assert_eq!(holdout_recall(&[row("t1", "tune", 3, 3)]), None);
    }

    #[test]
    fn splits_are_disjoint_by_item_and_document() {
        let doc_of: BTreeMap<String, String> =
            [("a-p1", "A"), ("a-p2", "A"), ("b-p1", "B"), ("c-p1", "C")]
                .iter()
                .map(|(i, d)| ((*i).to_owned(), (*d).to_owned()))
                .collect();
        let v = |s: &[&str]| s.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
        assert!(validate_splits(&v(&["a-p1", "a-p2"]), &v(&["b-p1", "c-p1"]), &doc_of).is_ok());
        assert!(
            validate_splits(&v(&["a-p1"]), &v(&["a-p1"]), &doc_of).is_err(),
            "item in entrambi"
        );
        assert!(
            validate_splits(&v(&["a-p1"]), &v(&["a-p2"]), &doc_of).is_err(),
            "documento A in entrambi"
        );
        assert!(
            validate_splits(&v(&["x-p9"]), &v(&["b-p1"]), &doc_of).is_err(),
            "item senza documento"
        );
    }

    #[test]
    fn table_pages_csv_has_fixed_header() {
        let p = scratch("csv").join("table_pages.csv");
        let mut h = row("it-25", "holdout", 5, 4);
        h.method = "heuristic+uncertain".into();
        h.pages_selected = 6;
        h.precision = 4.0 / 6.0;
        write_table_pages(&p, &[row("t1", "tune", 2, 2), h]).unwrap();
        let text = fs::read_to_string(&p).unwrap();
        let l: Vec<Vec<&str>> = text.lines().map(|l| l.split(',').collect()).collect();
        assert_eq!(
            l[0].join(","),
            "doc_id,split,method,pages_true,pages_selected,recall,precision"
        );
        assert_eq!(l.len(), 3);
        assert_eq!(&l[1][..5], ["t1", "tune", "heuristic", "2", "2"]);
        assert_eq!(
            &l[2][..5],
            ["it-25", "holdout", "heuristic+uncertain", "5", "6"]
        );
        assert!((l[2][5].parse::<f64>().unwrap() - 0.8).abs() < 1e-9);
        assert_eq!(l[2].len(), 7, "hits non è una colonna");
    }
}
