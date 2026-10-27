//! `ttq-merge`: una riga di `ttq_parse.csv` dai log di una run (piano 01-13, TEST-10).
//!
//! Componenti con `run_id` o `doc_sha256` diversi, o insiemi di pagine incoerenti, si
//! rifiutano con errore; il tempo totale è il `wall_ms` esterno, mai la somma dei componenti.
//! Più componenti dello stesso passo (per esempio una richiesta Docling per intervallo di
//! pagine) si sommano nei millisecondi e si uniscono nelle pagine; una pagina ripetuta nello
//! stesso passo è un errore.
//!
//! La lettura della cartella esige `reports.jsonl` e conta i rapporti ok come insieme di
//! `job_id` (`check_ok_reports`): una run con rapporti assenti, vuoti, mancanti, duplicati o
//! estranei si rifiuta.
//!
//! *Storia:* fino alla gap-closure 1.6 `reports.jsonl` era facoltativo e i rapporti non si
//! contavano, quindi una run senza rapporti passava come riuscita (01-REVIEW §B.4, D-10).
//!
//! **Modifica:** Claude (gsd-executor 01-13) · 2026-10-24 · corpo di `merge` e lettura della cartella della run (Task 2)
//! **Modifica:** Claude (gsd-executor 01.6-05) · 2026-10-27 · `reports.jsonl` obbligatorio e `check_ok_reports` (gap-closure 1.6, D-10)
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
pub(crate) fn merge(meta: &RunMeta, components: &[Component]) -> Result<String, String> {
    use std::collections::BTreeSet;
    let mut steps: [(u64, BTreeSet<u32>, usize); 4] = Default::default();
    for c in components {
        if c.run_id != meta.run_id {
            return Err(format!("run_id misto: {} contro {}", c.run_id, meta.run_id));
        }
        if c.doc_sha256 != meta.doc_sha256 {
            return Err(format!("doc_sha256 misto nel passo {:?}", c.step));
        }
        let slot = &mut steps[c.step as usize];
        slot.0 += c.ms;
        slot.2 += 1;
        for &p in &c.pages {
            if p == 0 || p > meta.pages {
                return Err(format!(
                    "pagina {p} fuori dal documento di {} pagine",
                    meta.pages
                ));
            }
            if !slot.1.insert(p) {
                return Err(format!("pagina {p} ripetuta nel passo {:?}", c.step));
            }
        }
    }
    let [extract, docling, render, tess] = &steps;
    let all: BTreeSet<u32> = (1..=meta.pages).collect();
    if extract.2 == 0 {
        return Err("componente extract mancante".into());
    }
    if extract.1 != all {
        return Err("extract senza tutte le pagine del documento".into());
    }
    if render.1 != tess.1 || (render.2 == 0) != (tess.2 == 0) {
        return Err("pagine di render e tesseract incoerenti".into());
    }
    let sum = extract.0 + docling.0 + render.0 + tess.0;
    Ok([
        meta.run_id.clone(),
        meta.doc_id.clone(),
        meta.doc_sha256.clone(),
        meta.pages.to_string(),
        meta.run.to_string(),
        meta.warm.to_string(),
        extract.0.to_string(),
        docling.1.len().to_string(),
        meta.docling_device.clone(),
        docling.0.to_string(),
        tess.1.len().to_string(),
        render.0.to_string(),
        tess.0.to_string(),
        sum.to_string(),
        meta.wall_ms.to_string(),
    ]
    .join(","))
}

impl Step {
    fn parse(s: &str) -> Result<Step, String> {
        match s {
            "extract" => Ok(Step::Extract),
            "docling" => Ok(Step::Docling),
            "render" => Ok(Step::Render),
            "tesseract" => Ok(Step::Tesseract),
            _ => Err(format!("passo sconosciuto: {s}")),
        }
    }
}

/// Pagine `1,2,5` o vuoto.
fn parse_pages(s: &str) -> Result<Vec<u32>, String> {
    s.split(',')
        .filter(|t| !t.is_empty())
        .map(|t| t.parse::<u32>().map_err(|e| format!("pagina {t:?}: {e}")))
        .collect()
}

/// Legge la cartella dei log di una run (`run_ttq.sh`): `meta.tsv` con righe `chiave<TAB>valore`
/// (`run_id`, `doc_id`, `doc_sha256`, `pages`, `run`, `warm`, `docling_device`, `t_start_ns`,
/// `t_end_ns`) e `steps.tsv` con righe `passo<TAB>run_id<TAB>doc_sha256<TAB>pagine<TAB>t0_ns<TAB>t1_ns`
/// (timestamp esterni `date +%s%N`). `reports.jsonl` è obbligatorio; ogni `RunReport` salvato
/// (righe `batch_summary` escluse) deve avere `job_id` che inizia con il `run_id` e
/// `outcome.status` = `ok`, e gli ok devono essere esattamente quelli attesi
/// ([`check_ok_reports`]).
pub(crate) fn read_run_dir(dir: &std::path::Path) -> Result<(RunMeta, Vec<Component>), String> {
    use std::collections::BTreeMap;
    let read = |n: &str| {
        std::fs::read_to_string(dir.join(n)).map_err(|e| format!("{}: {e}", dir.join(n).display()))
    };
    let meta_text = read("meta.tsv")?;
    let kv: BTreeMap<&str, &str> = meta_text
        .lines()
        .filter_map(|l| l.trim_end_matches('\r').split_once('\t'))
        .collect();
    let get = |k: &str| kv.get(k).copied().ok_or(format!("meta.tsv senza {k}"));
    let num = |k: &str| -> Result<u64, String> {
        get(k)?.parse::<u64>().map_err(|e| format!("meta {k}: {e}"))
    };
    let (t0, t1) = (num("t_start_ns")?, num("t_end_ns")?);
    if t1 < t0 {
        return Err("t_end_ns prima di t_start_ns".into());
    }
    let meta = RunMeta {
        run_id: get("run_id")?.to_owned(),
        doc_id: get("doc_id")?.to_owned(),
        doc_sha256: get("doc_sha256")?.to_owned(),
        pages: u32::try_from(num("pages")?).map_err(|e| e.to_string())?,
        run: u32::try_from(num("run")?).map_err(|e| e.to_string())?,
        warm: get("warm")? == "true",
        docling_device: get("docling_device")?.to_owned(),
        wall_ms: (t1 - t0) / 1_000_000,
    };
    let mut comps = Vec::new();
    for l in read("steps.tsv")?.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = l.trim_end_matches('\r').split('\t').collect();
        if f.len() != 6 {
            return Err(format!("steps.tsv: riga con {} campi: {l:?}", f.len()));
        }
        let (a, b) = (
            f[4].parse::<u64>().map_err(|e| e.to_string())?,
            f[5].parse::<u64>().map_err(|e| e.to_string())?,
        );
        if b < a || a < t0 || b > t1 {
            return Err(format!("passo {} fuori dalla finestra della run", f[0]));
        }
        comps.push(Component {
            step: Step::parse(f[0])?,
            run_id: f[1].to_owned(),
            doc_sha256: f[2].to_owned(),
            pages: parse_pages(f[3])?,
            ms: (b - a) / 1_000_000,
        });
    }
    // `reports.jsonl` è obbligatorio: un file assente è un errore, non una run senza rapporti.
    let reports = read("reports.jsonl")?;
    let mut ok_ids = Vec::new();
    for l in reports.lines().filter(|l| !l.trim().is_empty()) {
        let v: serde_json::Value =
            serde_json::from_str(l).map_err(|e| format!("reports.jsonl: {e}"))?;
        if v["status"].as_str() == Some("batch_summary") {
            continue;
        }
        let Some(id) = v["job_id"].as_str() else {
            return Err(format!("reports.jsonl: rapporto senza job_id: {l}"));
        };
        if !id.starts_with(&meta.run_id) {
            return Err(format!("RunReport di un'altra run: {id}"));
        }
        if v["status"].as_str() == Some("job_error")
            || v["outcome"]["status"].as_str() != Some("ok")
        {
            return Err(format!("job {id} non ok: {}", v["outcome"]));
        }
        ok_ids.push(id.to_owned());
    }
    check_ok_reports(&meta.run_id, &comps, &ok_ids)?;
    Ok((meta, comps))
}

/// Confronta i `job_id` dei rapporti ok (nell'ordine del file) con l'insieme atteso dai
/// componenti della run: `<run_id>-extract`, più `<run_id>-render` se c'è il passo `render`,
/// più `<run_id>-tess-<p>` per ogni pagina `p` del passo `tesseract`. `docling` non ha
/// rapporti. Un duplicato, un mancante o un id in più sono errori: gli esiti si contano come
/// insieme, non come numero (gap-closure 1.6, D-10). Nessun file system: l'I/O resta in
/// [`read_run_dir`].
pub(crate) fn check_ok_reports(
    run_id: &str,
    components: &[Component],
    ok_job_ids: &[String],
) -> Result<(), String> {
    use std::collections::BTreeSet;
    if ok_job_ids.is_empty() {
        return Err("reports.jsonl vuoto: nessun rapporto ok".into());
    }
    let mut found = BTreeSet::new();
    for id in ok_job_ids {
        if !found.insert(id.as_str()) {
            return Err(format!("rapporto ok duplicato: {id}"));
        }
    }
    let mut expected = BTreeSet::new();
    expected.insert(format!("{run_id}-extract"));
    for c in components {
        match c.step {
            Step::Render => {
                expected.insert(format!("{run_id}-render"));
            }
            Step::Tesseract => {
                for p in &c.pages {
                    expected.insert(format!("{run_id}-tess-{p}"));
                }
            }
            Step::Extract | Step::Docling => {}
        }
    }
    let missing: Vec<&str> = expected
        .iter()
        .map(String::as_str)
        .filter(|id| !found.contains(id))
        .take(5)
        .collect();
    let extra: Vec<&str> = found
        .iter()
        .copied()
        .filter(|id| !expected.contains(*id))
        .take(5)
        .collect();
    if missing.is_empty() && extra.is_empty() {
        return Ok(());
    }
    Err(format!(
        "rapporti ok attesi {}, trovati {}; mancanti (al più 5): [{}]; in eccesso (al più 5): [{}]",
        expected.len(),
        found.len(),
        missing.join(", "),
        extra.join(", ")
    ))
}

// ---- euristica delle pagine-tabella di 01-08 ------------------------------------------------

/// Campi di `features` di `table_heuristic_params.json` usati dal punteggio di pagina
/// (`Features` di `src/bin/extract/tables.rs`, 01-08, sola lettura).
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct Features {
    pub strip_chars: String,
    pub inner_chars: String,
    pub min_digits: u32,
    pub min_tokens: u32,
    pub density_saturation: f64,
    pub align_tol_pt: f64,
    pub min_column_rows: u32,
    pub line_tol_pt: f64,
    pub min_numeric: u32,
    pub w_density: f64,
    pub w_align: f64,
}

impl Features {
    pub(crate) fn from_params(params: &serde_json::Value) -> anyhow::Result<Features> {
        use anyhow::Context as _;
        serde_json::from_value(params["features"].clone()).context("features dei parametri")
    }
}

/// `is_numeric` di 01-08: bordi tolti, solo cifre e separatori ammessi, cifre minime.
pub(crate) fn is_numeric(token: &str, f: &Features) -> bool {
    let t = token.trim_matches(|c: char| f.strip_chars.contains(c) || c.is_whitespace());
    let t = t.strip_prefix('-').unwrap_or(t);
    if t.is_empty() {
        return false;
    }
    let mut digits = 0;
    for c in t.chars() {
        if c.is_ascii_digit() {
            digits += 1;
        } else if !f.inner_chars.contains(c) {
            return false;
        }
    }
    digits >= f.min_digits
}

/// `page_score` di 01-08 (copia riga per riga di `tables.rs`): densità di token numerici e
/// quota di numeri allineati in colonna sul bordo destro, in [0, 1].
pub(crate) fn page_score(words: &[(String, [f64; 4])], f: &Features) -> f64 {
    let n = words.len();
    if n < f.min_tokens as usize || n == 0 {
        return 0.0;
    }
    let nums: Vec<(f64, f64)> = words
        .iter()
        .filter(|w| is_numeric(&w.0, f))
        .map(|w| (w.1[2], (w.1[1] + w.1[3]) / 2.0))
        .collect();
    let density = nums.len() as f64 / n as f64;
    let align = if nums.len() < f.min_numeric as usize {
        0.0
    } else {
        let aligned = nums
            .iter()
            .filter(|(x, y)| {
                nums.iter()
                    .filter(|(x2, y2)| {
                        (x - x2).abs() <= f.align_tol_pt && (y - y2).abs() > f.line_tol_pt
                    })
                    .count()
                    + 1
                    >= f.min_column_rows as usize
            })
            .count();
        aligned as f64 / nums.len() as f64
    };
    let s = f.w_density * (density / f.density_saturation).min(1.0) + f.w_align * align;
    s.clamp(0.0, 1.0)
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

    // ---- read_run_dir con reports.jsonl contato (gap-closure 1.6, D-10) ------------------
    //
    // **Modifica:** Claude (test-write 01.6-05) · 2026-10-27 · test di read_run_dir con rapporti
    // assenti, vuoti, mancanti o duplicati (D-10)

    /// Cartella di una run sintetica in `temp_dir()/s02-ttq-<pid>-<n>`, coerente con `meta()`.
    /// Il `Drop` la rimuove su ogni uscita del test, panico con unwind compreso, come
    /// `SelftestInput` di `parse-parent.rs`.
    struct RunDirFixture {
        path: std::path::PathBuf,
    }

    impl RunDirFixture {
        /// Scrive `meta.tsv` e `steps.tsv` per i componenti dati; `reports.jsonl` non c'è.
        fn new(components: &[Component]) -> RunDirFixture {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("s02-ttq-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            let fixture = RunDirFixture { path };
            let m = meta();
            let t0: u64 = 1_000_000_000;
            let t1 = t0 + m.wall_ms * 1_000_000;
            let meta_tsv = format!(
                "run_id\t{}\ndoc_id\t{}\ndoc_sha256\t{}\npages\t{}\nrun\t{}\nwarm\t{}\n\
                 docling_device\t{}\nt_start_ns\t{t0}\nt_end_ns\t{t1}\n",
                m.run_id, m.doc_id, m.doc_sha256, m.pages, m.run, m.warm, m.docling_device
            );
            std::fs::write(fixture.path.join("meta.tsv"), meta_tsv).unwrap();
            let mut steps = String::new();
            let mut at = t0;
            for c in components {
                let name = match c.step {
                    Step::Extract => "extract",
                    Step::Docling => "docling",
                    Step::Render => "render",
                    Step::Tesseract => "tesseract",
                };
                let pages: Vec<String> = c.pages.iter().map(|p| p.to_string()).collect();
                let end = at + c.ms * 1_000_000;
                steps.push_str(&format!(
                    "{name}\t{}\t{}\t{}\t{at}\t{end}\n",
                    c.run_id,
                    c.doc_sha256,
                    pages.join(",")
                ));
                at = end;
            }
            assert!(at <= t1, "fixture: passi oltre la finestra della run");
            std::fs::write(fixture.path.join("steps.tsv"), steps).unwrap();
            fixture
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }

        /// `reports.jsonl` con il testo dato, così com'è.
        fn write_reports_raw(&self, text: &str) {
            std::fs::write(self.path.join("reports.jsonl"), text).unwrap();
        }

        /// `reports.jsonl` con un `RunReport` ok per ogni suffisso (`extract`, `tess-4`, …).
        fn write_ok_reports(&self, suffixes: &[&str]) {
            self.write_reports_raw(&ok_reports(suffixes));
        }
    }

    impl Drop for RunDirFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    /// Righe JSON di `RunReport` ok con `job_id` `<run_id>-<suffisso>`.
    fn ok_reports(suffixes: &[&str]) -> String {
        let rid = meta().run_id;
        suffixes
            .iter()
            .map(|s| format!("{{\"job_id\":\"{rid}-{s}\",\"outcome\":{{\"status\":\"ok\"}}}}\n"))
            .collect()
    }

    const BATCH_SUMMARY: &str = "{\"status\":\"batch_summary\",\"launch_errors\":0}\n";

    /// Rapporti attesi per `ok_components()`: extract, render e una pagina tess per pagina OCR.
    const FULL: [&str; 4] = ["extract", "render", "tess-4", "tess-5"];

    #[test]
    fn test10_missing_reports_is_rejected() {
        let fx = RunDirFixture::new(&ok_components());
        let r = read_run_dir(fx.path());
        assert!(r.is_err(), "reports.jsonl assente accettato");
    }

    #[test]
    fn test10_empty_reports_is_rejected() {
        for text in ["", "\n  \n", BATCH_SUMMARY] {
            let fx = RunDirFixture::new(&ok_components());
            fx.write_reports_raw(text);
            match read_run_dir(fx.path()) {
                Ok(_) => panic!("reports.jsonl senza rapporti ok accettato: {text:?}"),
                Err(e) => assert!(
                    e.contains("reports.jsonl vuoto"),
                    "errore inatteso per {text:?}: {e}"
                ),
            }
        }
    }

    #[test]
    fn test10_missing_tess_report_is_rejected() {
        let fx = RunDirFixture::new(&ok_components());
        fx.write_ok_reports(&["extract", "render", "tess-4"]);
        match read_run_dir(fx.path()) {
            Ok(_) => panic!("rapporto tess-5 mancante accettato"),
            Err(e) => {
                assert!(e.contains("rapporti ok attesi 4, trovati 3"), "errore: {e}");
                assert!(e.contains("tess-5"), "il mancante non è nominato: {e}");
            }
        }
    }

    #[test]
    fn test10_duplicate_ok_report_is_rejected() {
        let fx = RunDirFixture::new(&ok_components());
        fx.write_ok_reports(&["extract", "render", "tess-4", "tess-4", "tess-5"]);
        assert!(
            read_run_dir(fx.path()).is_err(),
            "rapporto ok duplicato accettato"
        );
    }

    #[test]
    fn test10_missing_job_id_is_rejected() {
        let fx = RunDirFixture::new(&ok_components());
        fx.write_reports_raw(&format!(
            "{}{{\"outcome\":{{\"status\":\"ok\"}}}}\n",
            ok_reports(&FULL)
        ));
        assert!(
            read_run_dir(fx.path()).is_err(),
            "rapporto ok senza job_id accettato"
        );
    }

    #[test]
    fn test10_full_reports_are_accepted() {
        let fx = RunDirFixture::new(&ok_components());
        fx.write_ok_reports(&FULL);
        let (m, comps) = read_run_dir(fx.path()).unwrap();
        assert_eq!(m.run_id, meta().run_id);
        assert_eq!(comps.len(), ok_components().len());
    }

    #[test]
    fn test10_digital_run_needs_only_extract() {
        let v = vec![
            c(Step::Extract, &[1, 2, 3, 4, 5], 700),
            c(Step::Docling, &[2, 3], 900),
        ];
        let fx = RunDirFixture::new(&v);
        fx.write_ok_reports(&["extract"]);
        read_run_dir(fx.path()).unwrap();
    }

    #[test]
    fn test10_batch_summary_is_ignored() {
        let fx = RunDirFixture::new(&ok_components());
        fx.write_reports_raw(&format!("{}{BATCH_SUMMARY}", ok_reports(&FULL)));
        read_run_dir(fx.path()).unwrap();
    }

    #[test]
    fn run_dir_fixture_cleans_on_panic() {
        let saved = std::sync::Mutex::new(None);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let fx = RunDirFixture::new(&ok_components());
            *saved.lock().unwrap() = Some(fx.path().to_path_buf());
            panic!("panico voluto");
        }));
        assert!(r.is_err());
        let p = saved.lock().unwrap().take().unwrap();
        assert!(
            !p.exists(),
            "cartella della fixture rimasta: {}",
            p.display()
        );
        assert_ne!(p, std::env::temp_dir());
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with(&format!("s02-ttq-{}-", std::process::id())),
            "nome della fixture: {name}"
        );
    }

    // ---- check_ok_reports, helper puro (Task 2 passo 0, D-10) ----------------------------
    //
    // **Modifica:** Claude (gsd-executor 01.6-05) · 2026-10-27 · test dell'helper puro con
    // vettori in memoria, scritti prima del GREEN (il test-write li ha lasciati a questo passo)

    /// `job_id` `<run_id di meta()>-<suffisso>`.
    fn ids(suffixes: &[&str]) -> Vec<String> {
        let rid = meta().run_id;
        suffixes.iter().map(|s| format!("{rid}-{s}")).collect()
    }

    #[test]
    fn test10_check_ok_reports_accepts_expected_set() {
        let rid = meta().run_id;
        check_ok_reports(&rid, &ok_components(), &ids(&FULL)).unwrap();
        // L'ordine del file non conta.
        check_ok_reports(
            &rid,
            &ok_components(),
            &ids(&["tess-5", "render", "extract", "tess-4"]),
        )
        .unwrap();
    }

    #[test]
    fn test10_check_ok_reports_rejects_missing_tess() {
        let e = check_ok_reports(
            &meta().run_id,
            &ok_components(),
            &ids(&["extract", "render", "tess-4"]),
        )
        .unwrap_err();
        assert!(e.contains("rapporti ok attesi 4, trovati 3"), "errore: {e}");
        assert!(e.contains("tess-5"), "il mancante non è nominato: {e}");
    }

    #[test]
    fn test10_check_ok_reports_rejects_duplicate() {
        let rid = meta().run_id;
        let e = check_ok_reports(
            &rid,
            &ok_components(),
            &ids(&["extract", "render", "tess-4", "tess-4", "tess-5"]),
        )
        .unwrap_err();
        assert!(e.contains("duplicato"), "errore: {e}");
        // Un duplicato che compensa una pagina mancante: il numero torna, l'insieme no.
        assert!(
            check_ok_reports(
                &rid,
                &ok_components(),
                &ids(&["extract", "render", "tess-4", "tess-4"]),
            )
            .is_err()
        );
    }

    #[test]
    fn test10_check_ok_reports_rejects_unexpected_id() {
        // Prefisso di un'altra run: `R1-extract` nella run `R`.
        let ids_r: Vec<String> = ["R1-extract", "R-render", "R-tess-4", "R-tess-5"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let e = check_ok_reports("R", &ok_components(), &ids_r).unwrap_err();
        assert!(e.contains("R1-extract"), "l'eccesso non è nominato: {e}");
        assert!(e.contains("R-extract"), "il mancante non è nominato: {e}");
        // Pagina tess non elencata, in più all'insieme atteso.
        let e = check_ok_reports(
            &meta().run_id,
            &ok_components(),
            &ids(&["extract", "render", "tess-4", "tess-5", "tess-9"]),
        )
        .unwrap_err();
        assert!(e.contains("tess-9"), "l'eccesso non è nominato: {e}");
        // Al più 5 id nominati per lista (in ordine lessicografico).
        let many: Vec<String> = (10..30).map(|p| format!("x-{p}")).collect();
        let e = check_ok_reports(&meta().run_id, &ok_components(), &many).unwrap_err();
        assert!(e.contains("x-14") && !e.contains("x-15"), "errore: {e}");
    }

    #[test]
    fn test10_check_ok_reports_rejects_empty() {
        let e = check_ok_reports(&meta().run_id, &ok_components(), &[]).unwrap_err();
        assert!(e.contains("reports.jsonl vuoto"), "errore: {e}");
    }

    #[test]
    fn test10_check_ok_reports_render_without_tesseract() {
        let rid = meta().run_id;
        let v = vec![
            c(Step::Extract, &[1, 2, 3, 4, 5], 700),
            c(Step::Render, &[4, 5], 300),
        ];
        check_ok_reports(&rid, &v, &ids(&["extract", "render"])).unwrap();
        assert!(check_ok_reports(&rid, &v, &ids(&["extract"])).is_err());
    }

    #[test]
    fn test10_check_ok_reports_tess_uses_page_number() {
        let rid = meta().run_id;
        let v = vec![
            c(Step::Extract, &[1, 2, 3, 4, 5], 700),
            c(Step::Render, &[3, 5], 300),
            c(Step::Tesseract, &[3, 5], 1200),
        ];
        check_ok_reports(&rid, &v, &ids(&["extract", "render", "tess-3", "tess-5"])).unwrap();
        assert!(
            check_ok_reports(&rid, &v, &ids(&["extract", "render", "tess-1", "tess-2"])).is_err(),
            "tess numerati per posizione accettati"
        );
    }

    #[test]
    fn test10_check_ok_reports_digital_needs_only_extract() {
        let rid = meta().run_id;
        let v = vec![
            c(Step::Extract, &[1, 2, 3, 4, 5], 700),
            c(Step::Docling, &[2, 3], 900),
        ];
        check_ok_reports(&rid, &v, &ids(&["extract"])).unwrap();
        assert!(check_ok_reports(&rid, &v, &ids(&["extract", "render"])).is_err());
    }
}
