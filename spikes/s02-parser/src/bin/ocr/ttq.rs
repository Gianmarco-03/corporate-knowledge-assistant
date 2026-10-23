//! `ttq-merge`: una riga di `ttq_parse.csv` dai log di una run (piano 01-13, TEST-10).
//!
//! STUB del test-write (2026-10-23): corpo `todo!()` scritto dall'esecutore del Task 2.
//! Componenti con `run_id` o `doc_sha256` diversi, o insiemi di pagine incoerenti, si
//! rifiutano con errore; il tempo totale è il `wall_ms` esterno, mai la somma dei componenti.
#![allow(dead_code)]

/// Header fisso di `ttq_parse.csv`.
pub(crate) const TTQ_HEADER: &str = "run_id,doc_id,doc_sha256,pages,run,warm,extract_ms,table_pages,docling_device,docling_ms,ocr_pages,render_ms,tesseract_ms,sum_components_ms,wall_ms";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Step {
    Extract,
    Docling,
    Render,
    Tesseract,
}

/// Una componente della sequenza, letta dai log della run.
#[derive(Debug, Clone)]
pub(crate) struct Component {
    pub step: Step,
    pub run_id: String,
    pub doc_sha256: String,
    /// Pagine trattate dalla componente (1-based).
    pub pages: Vec<u32>,
    pub ms: u64,
}

/// Dati della run presi dallo script (cronometro esterno).
#[derive(Debug, Clone)]
pub(crate) struct RunMeta {
    pub run_id: String,
    pub doc_id: String,
    pub doc_sha256: String,
    pub pages: u32,
    pub run: u32,
    pub warm: bool,
    pub docling_device: String,
    pub wall_ms: u64,
}

/// Riga CSV (senza newline) nell'ordine di [`TTQ_HEADER`].
pub(crate) fn merge(_meta: &RunMeta, _components: &[Component]) -> Result<String, String> {
    todo!("01-13 Task 2: ttq merge")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn meta() -> RunMeta {
        RunMeta {
            run_id: "t1761200000000-ab12".into(),
            doc_id: "mixed".into(),
            doc_sha256: SHA.into(),
            pages: 5,
            run: 2,
            warm: true,
            docling_device: "cuda".into(),
            wall_ms: 4100,
        }
    }

    fn c(step: Step, pages: &[u32], ms: u64) -> Component {
        Component {
            step,
            run_id: "t1761200000000-ab12".into(),
            doc_sha256: SHA.into(),
            pages: pages.to_vec(),
            ms,
        }
    }

    fn ok_components() -> Vec<Component> {
        vec![
            c(Step::Extract, &[1, 2, 3, 4, 5], 700),
            c(Step::Docling, &[2], 1500),
            c(Step::Render, &[4, 5], 300),
            c(Step::Tesseract, &[4, 5], 1200),
        ]
    }

    #[test]
    fn test10_merge_writes_row_with_external_wall_and_component_sum() {
        let row = merge(&meta(), &ok_components()).unwrap();
        assert_eq!(
            row,
            format!(
                "t1761200000000-ab12,mixed,{SHA},5,2,true,700,1,cuda,1500,2,300,1200,3700,4100"
            )
        );
        assert_eq!(row.split(',').count(), TTQ_HEADER.split(',').count());
    }

    #[test]
    fn test10_mixed_run_id_is_rejected() {
        let mut v = ok_components();
        v[2].run_id = "t1761200009999-ffff".into();
        assert!(merge(&meta(), &v).is_err());
    }

    #[test]
    fn test10_mixed_doc_hash_is_rejected() {
        let mut v = ok_components();
        v[1].doc_sha256 = "b".repeat(64);
        assert!(merge(&meta(), &v).is_err());
    }

    #[test]
    fn test10_incoherent_page_sets_are_rejected() {
        let mut v = ok_components();
        v[3].pages = vec![4];
        assert!(
            merge(&meta(), &v).is_err(),
            "tesseract su pagine diverse dal render"
        );
        let mut v = ok_components();
        v[1].pages = vec![9];
        assert!(merge(&meta(), &v).is_err(), "pagina fuori dal documento");
        let mut v = ok_components();
        v[0].pages = vec![1, 2, 3];
        assert!(merge(&meta(), &v).is_err(), "extract senza tutte le pagine");
        let v: Vec<Component> = ok_components()
            .into_iter()
            .filter(|c| c.step != Step::Extract)
            .collect();
        assert!(merge(&meta(), &v).is_err(), "extract mancante");
    }

    #[test]
    fn test10_digital_doc_without_ocr_pages_is_valid() {
        let v = vec![
            c(Step::Extract, &[1, 2, 3, 4, 5], 700),
            c(Step::Docling, &[2, 3], 900),
        ];
        let row = merge(&meta(), &v).unwrap();
        let f: Vec<&str> = row.split(',').collect();
        assert_eq!((f[10], f[11], f[12], f[13]), ("0", "0", "0", "1600"));
    }
}
