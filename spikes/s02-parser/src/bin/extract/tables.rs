//! `extract select-tables` (piano 01-08): euristica PDFium per le pagine-tabella, banda di
//! incertezza, split taratura/holdout, `table_pages.csv` e baseline PDFium a celle.
//!
//! I parametri dell'euristica vivono solo nel file `--params` (nessuna costante nascosta,
//! nessun default): `load_params` rifiuta un file senza tutti i campi.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::score::{self, Cell, PageDoc, Table};

/// Parametri dell'euristica letti da `--params`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Params {
    pub band: Band,
    pub features: Features,
    /// Come si è ottenuta la banda (regola, pagine di taratura, punteggi): solo documentazione.
    pub tuning: Option<serde_json::Value>,
}

/// Banda di decisione sul punteggio di pagina: `score ≥ select_from` → tabella;
/// `uncertain_from ≤ score < select_from` → incerta (mandata comunque a Docling); sotto → no.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Band {
    pub uncertain_from: f64,
    pub select_from: f64,
}

/// Pesi e soglie del punteggio di pagina e della baseline a celle, fissati prima della
/// taratura della banda.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Features {
    /// Caratteri tolti ai bordi di un token prima del test numerico (`(`, `$`, `%`, …).
    pub strip_chars: String,
    /// Caratteri ammessi dentro un numero oltre alle cifre (separatori).
    pub inner_chars: String,
    /// Cifre minime di un token numerico.
    pub min_digits: u32,
    /// Sotto questo numero di parole la pagina vale 0.
    pub min_tokens: u32,
    /// Densità di token numerici che satura la componente di densità.
    pub density_saturation: f64,
    /// Tolleranza sul bordo destro per l'allineamento di colonna (pt).
    pub align_tol_pt: f64,
    /// Righe minime di una colonna allineata (token compreso).
    pub min_column_rows: u32,
    /// Due token stanno su righe diverse se i centri verticali distano più di così (pt).
    pub line_tol_pt: f64,
    /// Token numerici minimi perché l'allineamento conti.
    pub min_numeric: u32,
    pub w_density: f64,
    pub w_align: f64,
    /// Baseline a celle: spazio orizzontale sotto cui due parole stanno nella stessa cella (pt).
    pub cell_gap_pt: f64,
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
        match self {
            Selection::Table => Some("heuristic"),
            Selection::Uncertain => Some("heuristic+uncertain"),
            Selection::Skip => None,
        }
    }

    /// `true` se la pagina va a Docling (tabella o incerta).
    pub(crate) fn sent_to_docling(self) -> bool {
        !matches!(self, Selection::Skip)
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
pub(crate) fn load_params(path: &Path) -> anyhow::Result<Params> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("parametri {}", path.display()))?;
    let p: Params = serde_json::from_str(&text)
        .with_context(|| format!("parametri {} non validi", path.display()))?;
    anyhow::ensure!(
        p.band.uncertain_from <= p.band.select_from,
        "banda non ordinata: uncertain_from > select_from"
    );
    Ok(p)
}

/// Classe della pagina dato il punteggio e la banda.
pub(crate) fn classify(score: f64, band: &Band) -> Selection {
    if score >= band.select_from {
        Selection::Table
    } else if score >= band.uncertain_from {
        Selection::Uncertain
    } else {
        Selection::Skip
    }
}

/// Recall e precisione delle pagine selezionate (`selected` = tabella o incerta) contro le
/// pagine-tabella vere; recall 1 con zero vere, precisione 1 con zero selezionate.
pub(crate) fn recall_precision(truth: &BTreeSet<u32>, selected: &BTreeSet<u32>) -> (f64, f64) {
    let hits = truth.intersection(selected).count();
    let recall = if truth.is_empty() {
        1.0
    } else {
        hits as f64 / truth.len() as f64
    };
    let precision = if selected.is_empty() {
        1.0
    } else {
        hits as f64 / selected.len() as f64
    };
    (recall, precision)
}

/// Recall cumulata (Σ hits / Σ pages_true) delle sole righe `split = holdout`; `None` se
/// non ce ne sono. È l'unico numero confrontato con la soglia.
pub(crate) fn holdout_recall(rows: &[TablePagesRow]) -> Option<f64> {
    let h: Vec<&TablePagesRow> = rows.iter().filter(|r| r.split == "holdout").collect();
    if h.is_empty() {
        return None;
    }
    let hits: usize = h.iter().map(|r| r.hits).sum();
    let truth: usize = h.iter().map(|r| r.pages_true).sum();
    Some(if truth == 0 {
        1.0
    } else {
        hits as f64 / truth as f64
    })
}

/// Split disgiunti per item e per documento (`doc_of`: item → documento); errore se un item
/// o un documento compare in entrambi, o se un item non ha documento.
pub(crate) fn validate_splits(
    tune: &[String],
    holdout: &[String],
    doc_of: &BTreeMap<String, String>,
) -> Result<(), String> {
    let t: BTreeSet<&String> = tune.iter().collect();
    if let Some(x) = holdout.iter().find(|x| t.contains(x)) {
        return Err(format!("item in entrambi gli split: {x}"));
    }
    let docs = |items: &[String]| -> Result<BTreeSet<String>, String> {
        items
            .iter()
            .map(|i| {
                doc_of
                    .get(i)
                    .cloned()
                    .ok_or_else(|| format!("item senza documento: {i}"))
            })
            .collect()
    };
    let (dt, dh) = (docs(tune)?, docs(holdout)?);
    if let Some(d) = dt.intersection(&dh).next() {
        return Err(format!("documento in entrambi gli split: {d}"));
    }
    Ok(())
}

/// `table_pages.csv`: `doc_id,split,method,pages_true,pages_selected,recall,precision`.
pub(crate) fn write_table_pages(path: &Path, rows: &[TablePagesRow]) -> anyhow::Result<()> {
    let mut text = String::from("doc_id,split,method,pages_true,pages_selected,recall,precision\n");
    for r in rows {
        for f in [&r.doc_id, &r.split, &r.method] {
            anyhow::ensure!(
                !f.contains([',', '\n', '"']),
                "campo CSV non ammesso: {f:?}"
            );
        }
        writeln!(
            text,
            "{},{},{},{},{},{},{}",
            r.doc_id, r.split, r.method, r.pages_true, r.pages_selected, r.recall, r.precision
        )?;
    }
    std::fs::write(path, text).with_context(|| format!("scrittura di {}", path.display()))
}

// ---- punteggio di pagina e baseline ---------------------------------------------------------

/// Token numerico secondo i parametri (bordi tolti, solo cifre e separatori ammessi).
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

/// Punteggio di pagina in [0, 1]: densità di token numerici e quota di numeri allineati in
/// colonna sul bordo destro.
pub(crate) fn page_score(page: &PageDoc, f: &Features) -> f64 {
    let n = page.words.len();
    if n < f.min_tokens as usize || n == 0 {
        return 0.0;
    }
    let nums: Vec<(f64, f64)> = page
        .words
        .iter()
        .filter(|w| is_numeric(&w.text, f))
        .map(|w| (w.bbox[2], (w.bbox[1] + w.bbox[3]) / 2.0))
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

/// Baseline PDFium a celle (Contratti): righe di testo con almeno 3 gruppi di parole; riga =
/// ordine dall'alto fra queste, colonna = ordine del gruppo nella riga.
pub(crate) fn baseline_table(page: &PageDoc, params: &Params, id: String) -> Table {
    let f = &params.features;
    let mut words: Vec<&score::PageItem> = page.words.iter().collect();
    words.sort_by(|a, b| {
        let (ya, yb) = ((a.bbox[1] + a.bbox[3]) / 2.0, (b.bbox[1] + b.bbox[3]) / 2.0);
        yb.total_cmp(&ya).then(a.bbox[0].total_cmp(&b.bbox[0]))
    });
    let mut lines: Vec<Vec<&score::PageItem>> = Vec::new();
    for w in words {
        let yc = (w.bbox[1] + w.bbox[3]) / 2.0;
        match lines.last_mut() {
            Some(l)
                if {
                    let y0 = (l[0].bbox[1] + l[0].bbox[3]) / 2.0;
                    (y0 - yc).abs() <= f.line_tol_pt
                } =>
            {
                l.push(w)
            }
            _ => lines.push(vec![w]),
        }
    }
    let mut cells = Vec::new();
    let mut bbox: Option<[f64; 4]> = None;
    let mut row = 0;
    for mut l in lines {
        l.sort_by(|a, b| a.bbox[0].total_cmp(&b.bbox[0]));
        let mut groups: Vec<Vec<&score::PageItem>> = Vec::new();
        for w in l {
            match groups.last_mut() {
                Some(g) if w.bbox[0] - g[g.len() - 1].bbox[2] < f.cell_gap_pt => g.push(w),
                _ => groups.push(vec![w]),
            }
        }
        if groups.len() < 3 {
            continue;
        }
        row += 1;
        for (c, g) in groups.iter().enumerate() {
            let text = g
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            cells.push(Cell {
                row,
                col: c as u32 + 1,
                text,
            });
            for w in g {
                bbox = Some(match bbox {
                    None => w.bbox,
                    Some(b) => [
                        b[0].min(w.bbox[0]),
                        b[1].min(w.bbox[1]),
                        b[2].max(w.bbox[2]),
                        b[3].max(w.bbox[3]),
                    ],
                });
            }
        }
    }
    Table {
        id,
        page: page.page,
        bbox: bbox.unwrap_or([0.0; 4]),
        cells,
    }
}

// ---- sottocomando `select-tables` -----------------------------------------------------------

#[derive(clap::Parser, Debug)]
#[command(name = "extract select-tables")]
struct SelectCli {
    #[command(subcommand)]
    cmd: SelectCmd,
}

#[derive(clap::Args, Debug)]
struct Inputs {
    #[arg(long)]
    jobs: PathBuf,
    #[arg(long)]
    runs: PathBuf,
    #[arg(long)]
    data_root: PathBuf,
    #[arg(long)]
    results_raw: PathBuf,
    #[arg(long)]
    manifest: PathBuf,
    #[arg(long)]
    splits: PathBuf,
    #[arg(long)]
    doclaynet_coco: PathBuf,
}

#[derive(clap::Subcommand, Debug)]
enum SelectCmd {
    /// Taratura della banda sulle sole pagine `tune` (regola dei Contratti).
    Tune {
        #[command(flatten)]
        inputs: Inputs,
        /// `Features` in JSON, fissati prima della taratura.
        #[arg(long)]
        features_json: String,
        /// File dei parametri da scrivere (`table_heuristic_params.json`).
        #[arg(long)]
        out: PathBuf,
    },
    /// Misura con i parametri congelati su tune e holdout (DocLayNet + documento IT).
    Measure {
        #[command(flatten)]
        inputs: Inputs,
        #[arg(long)]
        params: PathBuf,
        /// Oracolo IT (voce `table-pages` del documento da 25 pagine).
        #[arg(long)]
        oracle: PathBuf,
        /// `doc_id` del corpus del documento IT da 25 pagine (holdout per costruzione).
        #[arg(long)]
        it_doc: String,
        /// `table_pages.csv` (cartella con `hw.json`).
        #[arg(long)]
        out_csv: PathBuf,
        /// Pagine da mandare a Docling: `split<TAB>doc_id<TAB>input<TAB>pagina<TAB>method<TAB>score`.
        #[arg(long)]
        selected_out: PathBuf,
    },
}

/// `extract select-tables …`
pub(crate) fn run(args: Vec<std::ffi::OsString>) -> anyhow::Result<()> {
    use clap::Parser as _;
    let cli = SelectCli::try_parse_from(
        std::iter::once(std::ffi::OsString::from("extract-select-tables")).chain(args),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        SelectCmd::Tune {
            inputs,
            features_json,
            out,
        } => tune(&inputs, &features_json, &out),
        SelectCmd::Measure {
            inputs,
            params,
            oracle,
            it_doc,
            out_csv,
            selected_out,
        } => measure(&inputs, &params, &oracle, &it_doc, &out_csv, &selected_out),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Splits {
    tune: Vec<String>,
    holdout: Vec<String>,
}

struct Ctx {
    roots: score::Roots,
    runs: BTreeMap<String, score::ExtractRun>,
    manifest: Vec<score::ManifestItem>,
    splits: Splits,
    coco: serde_json::Value,
}

fn load(i: &Inputs) -> anyhow::Result<Ctx> {
    let manifest = score::load_manifest(&i.manifest)?;
    let splits: Splits = serde_json::from_slice(&std::fs::read(&i.splits)?)
        .with_context(|| format!("splits {}", i.splits.display()))?;
    let doc_of: BTreeMap<String, String> = manifest
        .iter()
        .filter(|m| m.role == "pdf")
        .map(|m| (m.item_id.clone(), m.doc_id.clone()))
        .collect();
    validate_splits(&splits.tune, &splits.holdout, &doc_of).map_err(anyhow::Error::msg)?;
    Ok(Ctx {
        roots: score::Roots {
            data: i.data_root.clone(),
            results_raw: i.results_raw.clone(),
        },
        runs: score::extract_runs(
            &std::fs::read_to_string(&i.jobs)?,
            &std::fs::read_to_string(&i.runs)?,
        ),
        manifest,
        splits,
        coco: serde_json::from_slice(&std::fs::read(&i.doclaynet_coco)?)?,
    })
}

/// Pagina DocLayNet di un item: (documento, input, pagina estratta, è pagina-tabella).
fn doclaynet_page(
    c: &Ctx,
    item_id: &str,
) -> anyhow::Result<(String, String, Option<PageDoc>, bool)> {
    let m = c
        .manifest
        .iter()
        .find(|m| m.item_id == item_id && m.role == "pdf")
        .with_context(|| format!("item {item_id} assente dal manifest"))?;
    let input = format!("/data/s02/raw/{}", m.file);
    let page = score::pages_of(&c.runs, &c.roots, &input)?.and_then(|mut p| p.remove(&1));
    let image_id = m.coco_image_id.context("coco_image_id assente")?;
    let (w, h) = page
        .as_ref()
        .map_or((612.0, 792.0), |p| (p.width, p.height));
    let is_table = !score::doclaynet_regions(&c.coco, image_id, w, h).is_empty();
    Ok((m.doc_id.clone(), input, page, is_table))
}

fn tune(i: &Inputs, features_json: &str, out: &Path) -> anyhow::Result<()> {
    let c = load(i)?;
    let f: Features = serde_json::from_str(features_json).context("--features-json")?;
    let mut scores = Vec::new();
    for item in &c.splits.tune {
        let (doc, _, page, is_table) = doclaynet_page(&c, item)?;
        let s = page.as_ref().map_or(0.0, |p| page_score(p, &f));
        scores.push(serde_json::json!({"item_id": item, "doc_id": doc, "score": s, "table": is_table, "extracted": page.is_some()}));
    }
    // Le pagine senza job extract_pdf ok non hanno punteggio: restano nei punteggi di
    // taratura (extracted=false) ma non fissano la banda.
    let min_true = scores
        .iter()
        .filter(|s| s["table"] == true && s["extracted"] == true)
        .filter_map(|s| s["score"].as_f64())
        .fold(f64::INFINITY, f64::min);
    anyhow::ensure!(min_true.is_finite(), "nessuna pagina-tabella di taratura");
    let params = Params {
        band: Band {
            uncertain_from: 0.5 * min_true,
            select_from: min_true,
        },
        features: f,
        tuning: Some(serde_json::json!({
            "rule": "select_from = punteggio minimo fra le pagine-tabella vere di taratura; uncertain_from = 0,5 × select_from (Contratti di RISULTATI-estrazione.md)",
            "split": "tune",
            "pages": c.splits.tune.len(),
            "scores": scores,
        })),
    };
    std::fs::write(out, serde_json::to_string_pretty(&params)? + "\n")?;
    println!(
        "{}",
        serde_json::json!({"status": "tuned", "band": params.band})
    );
    Ok(())
}

fn measure(
    i: &Inputs,
    params_path: &Path,
    oracle_path: &Path,
    it_doc: &str,
    out_csv: &Path,
    selected_out: &Path,
) -> anyhow::Result<()> {
    let c = load(i)?;
    let out_dir = out_csv.parent().unwrap_or(Path::new("."));
    common::manifest::ensure_present(out_dir)
        .with_context(|| format!("hw.json assente in {}", out_dir.display()))?;
    let params = load_params(params_path)?;
    let mut rows = Vec::new();
    let mut selected = String::new();
    for (split, items) in [("tune", &c.splits.tune), ("holdout", &c.splits.holdout)] {
        // Per documento: pagine vere e selezionate (indice = posizione dell'item nel documento).
        let mut per_doc: BTreeMap<String, (BTreeSet<u32>, BTreeSet<u32>)> = BTreeMap::new();
        for (k, item) in items.iter().enumerate() {
            let (doc, input, page, is_table) = doclaynet_page(&c, item)?;
            let s = page
                .as_ref()
                .map_or(0.0, |p| page_score(p, &params.features));
            let sel = classify(s, &params.band);
            let e = per_doc.entry(doc.clone()).or_default();
            let key = k as u32;
            if is_table {
                e.0.insert(key);
            }
            if sel.sent_to_docling() {
                e.1.insert(key);
                writeln!(
                    selected,
                    "{split}\t{item}\t{input}\t1\t{}\t{s}",
                    sel.method().unwrap_or_default()
                )?;
            }
        }
        for (doc, (truth, sel)) in per_doc {
            rows.push(row(doc, split, &truth, &sel));
        }
    }
    // Documento IT da 25 pagine: holdout per costruzione, verità dall'oracolo `table-pages`.
    let oracle = score::load_oracle(oracle_path)?;
    let o = oracle
        .iter()
        .find(|o| o.parser_kind == "table-pages" && o.document() == it_doc)
        .with_context(|| format!("oracolo table-pages di {it_doc}"))?;
    anyhow::ensure!(
        o.green(),
        "oracolo {} non ratificato sui tre assi",
        o.oracle_id
    );
    let truth: BTreeSet<u32> = o.expected_value["table_pages"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| {
                    v.as_str()
                        .and_then(|s| s.parse().ok())
                        .or(v.as_u64().map(|n| n as u32))
                })
                .collect()
        })
        .unwrap_or_default();
    let input = format!("/data/corpus/raw/{it_doc}.pdf");
    let pages = score::pages_of(&c.runs, &c.roots, &input)?
        .with_context(|| format!("nessun job extract_pdf ok per {it_doc}"))?;
    let mut sel = BTreeSet::new();
    for (n, p) in &pages {
        let s = page_score(p, &params.features);
        let cl = classify(s, &params.band);
        if cl.sent_to_docling() {
            sel.insert(*n);
            writeln!(
                selected,
                "holdout\t{it_doc}\t{input}\t{n}\t{}\t{s}",
                cl.method().unwrap_or_default()
            )?;
        }
    }
    rows.push(row(it_doc.to_owned(), "holdout", &truth, &sel));
    write_table_pages(out_csv, &rows)?;
    std::fs::write(selected_out, selected)?;
    let tune_rows: Vec<TablePagesRow> = rows
        .iter()
        .filter(|r| r.split == "tune")
        .cloned()
        .map(|mut r| {
            r.split = "holdout".into();
            r
        })
        .collect();
    let pooled_precision = |split: &str| {
        let s: usize = rows
            .iter()
            .filter(|r| r.split == split)
            .map(|r| r.pages_selected)
            .sum();
        let h: usize = rows
            .iter()
            .filter(|r| r.split == split)
            .map(|r| r.hits)
            .sum();
        if s == 0 { 1.0 } else { h as f64 / s as f64 }
    };
    println!(
        "{}",
        serde_json::json!({
            "status": "table_pages",
            "e7_holdout_recall": holdout_recall(&rows),
            "e8_holdout_precision": pooled_precision("holdout"),
            "e8_tune_recall": holdout_recall(&tune_rows),
            "e8_tune_precision": pooled_precision("tune"),
            "band": params.band,
        })
    );
    Ok(())
}

fn row(doc: String, split: &str, truth: &BTreeSet<u32>, sel: &BTreeSet<u32>) -> TablePagesRow {
    let (recall, precision) = recall_precision(truth, sel);
    TablePagesRow {
        doc_id: doc,
        split: split.to_owned(),
        method: "heuristic+uncertain".to_owned(),
        pages_true: truth.len(),
        pages_selected: sel.len(),
        hits: truth.intersection(sel).count(),
        recall,
        precision,
    }
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
