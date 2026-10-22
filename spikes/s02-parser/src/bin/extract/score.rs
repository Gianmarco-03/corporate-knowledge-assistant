//! Punteggi di `extract score` secondo i «Contratti di abbinamento» di RISULTATI-estrazione.md
//! (piano 01-08): CER, IoU delle parole con abbinamento uno a uno, F1 di cella, recall delle
//! regioni tabella, conversione delle coordinate TAT-DQA, CSV con header fissi.
//!
//! Convenzione dei bbox: `[x0, y0, x1, y1]` in punti PDF, origine in basso a sinistra.
//! Le funzioni pure stanno qui; la lettura dei file e i sottocomandi stanno in `cli`.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use anyhow::Context;
use unicode_normalization::UnicodeNormalization;

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

impl MatchKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            MatchKind::Matched => "matched",
            MatchKind::Merged => "merged",
            MatchKind::Missed => "missed",
            MatchKind::Spurious => "spurious",
            MatchKind::Unscorable => "unscorable",
        }
    }
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

impl PageScore {
    /// Attese valutabili (denominatore di `iou_mean` e `share_iou_ge_05`).
    pub(crate) fn scorable(&self) -> usize {
        self.words - self.unscorable
    }

    /// Attese con match (`matched` o `merged`) e IoU ≥ 0,5.
    pub(crate) fn good(&self) -> usize {
        self.alignments
            .iter()
            .filter(|a| matches!(a.kind, MatchKind::Matched | MatchKind::Merged) && a.iou >= 0.5)
            .count()
    }
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
pub(crate) fn normalize_token(s: &str) -> String {
    s.nfc().collect::<String>().trim().to_owned()
}

/// Testo per il CER: NFC, spazi bianchi (a capo compresi) collassati in uno spazio, bordi
/// rimossi.
pub(crate) fn normalize_text(s: &str) -> String {
    let nfc: String = s.nfc().collect();
    nfc.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Testo di cella: token normalizzato, spazi interni collassati, meno U+2212 → `-`.
pub(crate) fn normalize_cell(s: &str) -> String {
    normalize_text(&s.replace('\u{2212}', "-"))
}

/// Operazioni di modifica di un allineamento di Levenshtein ottimo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct EditOps {
    pub substitutions: u64,
    pub deletions: u64,
    pub insertions: u64,
}

impl EditOps {
    pub(crate) fn distance(&self) -> u64 {
        self.substitutions + self.deletions + self.insertions
    }
}

/// Levenshtein sui caratteri con la scomposizione delle operazioni lungo un cammino ottimo
/// (due righe di memoria: ogni cella porta le operazioni del predecessore scelto).
pub(crate) fn edit_ops(expected: &[char], predicted: &[char]) -> EditOps {
    let m = predicted.len();
    let mut prev: Vec<EditOps> = (0..=m)
        .map(|j| EditOps {
            insertions: j as u64,
            ..EditOps::default()
        })
        .collect();
    let mut cur = vec![EditOps::default(); m + 1];
    for (i, e) in expected.iter().enumerate() {
        cur[0] = EditOps {
            deletions: i as u64 + 1,
            ..EditOps::default()
        };
        for j in 1..=m {
            let mut diag = prev[j - 1];
            if *e != predicted[j - 1] {
                diag.substitutions += 1;
            }
            let mut del = prev[j];
            del.deletions += 1;
            let mut ins = cur[j - 1];
            ins.insertions += 1;
            let mut best = diag;
            if del.distance() < best.distance() {
                best = del;
            }
            if ins.distance() < best.distance() {
                best = ins;
            }
            cur[j] = best;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[m]
}

/// CER e operazioni sul testo normalizzato con [`normalize_text`]; con atteso vuoto il CER
/// vale 0 se anche il predetto è vuoto, altrimenti 1.
pub(crate) fn cer_ops(expected: &str, predicted: &str) -> (f64, usize, EditOps) {
    let e: Vec<char> = normalize_text(expected).chars().collect();
    let p: Vec<char> = normalize_text(predicted).chars().collect();
    let ops = edit_ops(&e, &p);
    let cer = if e.is_empty() {
        if p.is_empty() { 0.0 } else { 1.0 }
    } else {
        ops.distance() as f64 / e.len() as f64
    };
    (cer, e.len(), ops)
}

/// CER = distanza di Levenshtein sui caratteri / caratteri attesi, sul testo normalizzato
/// come in `parser-it.json` (NFC, spazi e a capo collassati in uno spazio, bordi rimossi).
pub(crate) fn cer(expected: &str, predicted: &str) -> f64 {
    cer_ops(expected, predicted).0
}

/// Regole meccaniche dell'oracolo IT sul lato predetto (Contratti): trattino morbido tolto,
/// sillabazione a fine riga unita, legature scritte come lettere, meno U+2212 → `-`.
pub(crate) fn oracle_it_prediction(lines: &[String]) -> String {
    let joined = lines.join("\n");
    let chars: Vec<char> = joined.chars().filter(|c| *c != '\u{ad}').collect();
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '-'
            && i > 0
            && chars[i - 1].is_alphabetic()
            && chars.get(i + 1) == Some(&'\n')
            && chars.get(i + 2).is_some_and(|n| n.is_lowercase())
        {
            i += 2;
            continue;
        }
        match c {
            '\u{fb00}' => out.push_str("ff"),
            '\u{fb01}' => out.push_str("fi"),
            '\u{fb02}' => out.push_str("fl"),
            '\u{fb03}' => out.push_str("ffi"),
            '\u{fb04}' => out.push_str("ffl"),
            '\u{fb05}' | '\u{fb06}' => out.push_str("st"),
            '\u{2212}' => out.push('-'),
            other => out.push(other),
        }
        i += 1;
    }
    out
}

fn degenerate(b: &[f64; 4]) -> bool {
    !(b[2] > b[0] && b[3] > b[1]) || b.iter().any(|v| !v.is_finite())
}

fn area(b: &[f64; 4]) -> f64 {
    (b[2] - b[0]) * (b[3] - b[1])
}

/// IoU di due bbox; 0 se disgiunti o degeneri.
pub(crate) fn iou(a: &[f64; 4], b: &[f64; 4]) -> f64 {
    if degenerate(a) || degenerate(b) {
        return 0.0;
    }
    let w = a[2].min(b[2]) - a[0].max(b[0]);
    let h = a[3].min(b[3]) - a[1].max(b[1]);
    if w <= 0.0 || h <= 0.0 {
        return 0.0;
    }
    let inter = w * h;
    inter / (area(a) + area(b) - inter)
}

fn union(boxes: impl IntoIterator<Item = [f64; 4]>) -> Option<[f64; 4]> {
    boxes.into_iter().fold(None, |acc, b| {
        Some(match acc {
            None => b,
            Some(a) => [
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[2].max(b[2]),
                a[3].max(b[3]),
            ],
        })
    })
}

fn center_distance(a: &[f64; 4], b: &[f64; 4]) -> f64 {
    let (ax, ay) = ((a[0] + a[2]) / 2.0, (a[1] + a[3]) / 2.0);
    let (bx, by) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
    ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt()
}

/// Bbox TAT-DQA `[x0, y0, x1, y1]` in pixel, origine in alto a sinistra, sull'immagine
/// `img_w × img_h` → punti PDF, origine in basso a sinistra, sulla pagina `page_w × page_h`.
pub(crate) fn tatdqa_to_pdf(
    bbox_px: [f64; 4],
    img_w: f64,
    img_h: f64,
    page_w: f64,
    page_h: f64,
) -> [f64; 4] {
    let sx = page_w / img_w;
    let sy = page_h / img_h;
    [
        bbox_px[0] * sx,
        page_h - bbox_px[3] * sy,
        bbox_px[2] * sx,
        page_h - bbox_px[1] * sy,
    ]
}

/// Due parole sulla stessa riga: intervalli verticali sovrapposti per almeno metà
/// dell'altezza minore.
fn same_line(a: &[f64; 4], b: &[f64; 4]) -> bool {
    let overlap = a[3].min(b[3]) - a[1].max(b[1]);
    let min_h = (a[3] - a[1]).min(b[3] - b[1]);
    min_h > 0.0 && overlap >= min_h / 2.0
}

fn inside(b: &[f64; 4], page: &[f64; 4]) -> bool {
    const EPS: f64 = 1e-6;
    b[0] >= page[0] - EPS && b[1] >= page[1] - EPS && b[2] <= page[2] + EPS && b[3] <= page[3] + EPS
}

/// Abbinamento delle parole di una pagina (`page` = box della pagina, per gli `unscorable`).
/// `predicted` è in ordine di lettura (le unioni `merged` usano predette consecutive).
pub(crate) fn score_words(expected: &[Word], predicted: &[Word], page: [f64; 4]) -> PageScore {
    let et: Vec<String> = expected.iter().map(|w| normalize_token(&w.text)).collect();
    let pt: Vec<String> = predicted.iter().map(|w| normalize_token(&w.text)).collect();
    let scorable: Vec<bool> = expected
        .iter()
        .zip(&et)
        .map(|(w, t)| !t.is_empty() && !degenerate(&w.bbox) && inside(&w.bbox, &page))
        .collect();

    // Prima passata: uno a uno greedy per IoU decrescente sulle coppie con testo uguale.
    let mut by_text: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (j, t) in pt.iter().enumerate() {
        by_text.entry(t.as_str()).or_default().push(j);
    }
    let mut cands: Vec<(f64, f64, usize, usize)> = Vec::new();
    for (i, w) in expected.iter().enumerate() {
        if !scorable[i] {
            continue;
        }
        for &j in by_text
            .get(et[i].as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            let v = iou(&w.bbox, &predicted[j].bbox);
            if v > 0.0 {
                cands.push((v, center_distance(&w.bbox, &predicted[j].bbox), i, j));
            }
        }
    }
    cands.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then(a.1.total_cmp(&b.1))
            .then(a.2.cmp(&b.2))
            .then(a.3.cmp(&b.3))
    });
    let mut exp_match: Vec<Option<(MatchKind, Vec<usize>, f64)>> = vec![None; expected.len()];
    let mut used = vec![false; predicted.len()];
    for (v, _, i, j) in cands {
        if exp_match[i].is_none() && !used[j] {
            exp_match[i] = Some((MatchKind::Matched, vec![j], v));
            used[j] = true;
        }
    }

    // Seconda passata: unioni di 2-3 predette libere, consecutive e sulla stessa riga.
    for i in 0..expected.len() {
        if !scorable[i] || exp_match[i].is_some() {
            continue;
        }
        'k: for k in 2..=3usize {
            let mut best: Option<(f64, usize)> = None;
            for s in 0..predicted.len().saturating_sub(k - 1) {
                let win = s..s + k;
                if win.clone().any(|j| used[j]) {
                    continue;
                }
                if !win
                    .clone()
                    .skip(1)
                    .all(|j| same_line(&predicted[j - 1].bbox, &predicted[j].bbox))
                {
                    continue;
                }
                let concat: String = win.clone().map(|j| pt[j].as_str()).collect();
                if concat != et[i] {
                    continue;
                }
                let Some(u) = union(win.clone().map(|j| predicted[j].bbox)) else {
                    continue;
                };
                let v = iou(&expected[i].bbox, &u);
                if v > 0.0 && best.is_none_or(|(bv, _)| v > bv) {
                    best = Some((v, s));
                }
            }
            if let Some((v, s)) = best {
                let idx: Vec<usize> = (s..s + k).collect();
                for &j in &idx {
                    used[j] = true;
                }
                exp_match[i] = Some((MatchKind::Merged, idx, v));
                break 'k;
            }
        }
    }

    let mut alignments = Vec::with_capacity(expected.len() + predicted.len());
    let (mut matched, mut merged, mut missed, mut unscorable) = (0, 0, 0, 0);
    let (mut iou_sum, mut good) = (0.0, 0usize);
    for i in 0..expected.len() {
        if !scorable[i] {
            unscorable += 1;
            alignments.push(Alignment {
                expected: Some(i),
                predicted: vec![],
                kind: MatchKind::Unscorable,
                iou: 0.0,
            });
            continue;
        }
        match exp_match[i].take() {
            Some((kind, predicted, v)) => {
                if kind == MatchKind::Matched {
                    matched += 1;
                } else {
                    merged += 1;
                }
                iou_sum += v;
                if v >= 0.5 {
                    good += 1;
                }
                alignments.push(Alignment {
                    expected: Some(i),
                    predicted,
                    kind,
                    iou: v,
                });
            }
            None => {
                missed += 1;
                alignments.push(Alignment {
                    expected: Some(i),
                    predicted: vec![],
                    kind: MatchKind::Missed,
                    iou: 0.0,
                });
            }
        }
    }
    let mut spurious = 0;
    for (j, u) in used.iter().enumerate() {
        if !u {
            spurious += 1;
            alignments.push(Alignment {
                expected: None,
                predicted: vec![j],
                kind: MatchKind::Spurious,
                iou: 0.0,
            });
        }
    }
    let n = expected.len() - unscorable;
    let (iou_mean, share_iou_ge_05) = if n == 0 {
        (0.0, 0.0)
    } else {
        (iou_sum / n as f64, good as f64 / n as f64)
    };
    PageScore {
        words: expected.len(),
        matched,
        merged,
        missed,
        spurious,
        unscorable,
        iou_mean,
        share_iou_ge_05,
        alignments,
    }
}

/// Celle abbinate uno a uno per uguaglianza della tripla (multinsieme), con testo
/// normalizzato come cella.
pub(crate) fn matched_cells(expected: &[Cell], predicted: &[Cell]) -> usize {
    let mut counts: BTreeMap<(u32, u32, String), usize> = BTreeMap::new();
    for c in predicted {
        *counts
            .entry((c.row, c.col, normalize_cell(&c.text)))
            .or_default() += 1;
    }
    let mut n = 0;
    for c in expected {
        if let Some(k) = counts.get_mut(&(c.row, c.col, normalize_cell(&c.text)))
            && *k > 0
        {
            *k -= 1;
            n += 1;
        }
    }
    n
}

/// Punteggio di una tabella attesa contro la predetta abbinata (o nessuna).
pub(crate) fn table_score(expected: &Table, predicted: Option<&Table>) -> TableScore {
    let cells_expected = expected.cells.len();
    let (cells_found, matched) = match predicted {
        Some(p) => (p.cells.len(), matched_cells(&expected.cells, &p.cells)),
        None => (0, 0),
    };
    let precision = if cells_found == 0 {
        0.0
    } else {
        matched as f64 / cells_found as f64
    };
    let recall = if cells_expected == 0 {
        0.0
    } else {
        matched as f64 / cells_expected as f64
    };
    let f1 = if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    };
    TableScore {
        table_id: expected.id.clone(),
        cells_expected,
        cells_found,
        matched,
        precision,
        recall,
        f1,
    }
}

/// F1 di cella con abbinamento uno a uno sulle tabelle della stessa pagina (IoU massimo);
/// un risultato per tabella attesa, nell'ordine di `expected`.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "contratto con bbox (TEST-5): gli oracoli tabella di questo piano non hanno rettangoli"
    )
)]
pub(crate) fn score_tables(expected: &[Table], predicted: &[Table]) -> Vec<TableScore> {
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for (i, e) in expected.iter().enumerate() {
        for (j, p) in predicted.iter().enumerate() {
            if e.page == p.page {
                let v = iou(&e.bbox, &p.bbox);
                if v > 0.0 {
                    pairs.push((v, i, j));
                }
            }
        }
    }
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut of_expected: Vec<Option<usize>> = vec![None; expected.len()];
    let mut taken = vec![false; predicted.len()];
    for (_, i, j) in pairs {
        if of_expected[i].is_none() && !taken[j] {
            of_expected[i] = Some(j);
            taken[j] = true;
        }
    }
    expected
        .iter()
        .zip(of_expected)
        .map(|(e, j)| table_score(e, j.map(|j| &predicted[j])))
        .collect()
}

/// Oracolo senza bbox: la tabella attesa si abbina, fra le predette della stessa pagina,
/// a quella con più celle abbinate (a parità la prima).
pub(crate) fn score_table_without_bbox(expected: &Table, predicted: &[Table]) -> TableScore {
    let mut best: Option<(usize, &Table)> = None;
    for p in predicted.iter().filter(|p| p.page == expected.page) {
        let m = matched_cells(&expected.cells, &p.cells);
        if best.is_none_or(|(bm, _)| m > bm) {
            best = Some((m, p));
        }
    }
    table_score(expected, best.map(|(_, p)| p))
}

/// Regioni attese abbinate uno a uno, greedy per IoU decrescente, con IoU ≥ 0,5.
pub(crate) fn region_matches(expected: &[[f64; 4]], predicted: &[[f64; 4]]) -> usize {
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for (i, e) in expected.iter().enumerate() {
        for (j, p) in predicted.iter().enumerate() {
            let v = iou(e, p);
            if v >= 0.5 {
                pairs.push((v, i, j));
            }
        }
    }
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut e_used = vec![false; expected.len()];
    let mut p_used = vec![false; predicted.len()];
    let mut n = 0;
    for (_, i, j) in pairs {
        if !e_used[i] && !p_used[j] {
            e_used[i] = true;
            p_used[j] = true;
            n += 1;
        }
    }
    n
}

/// Recall delle regioni tabella: uno a uno greedy per IoU decrescente, match se IoU ≥ 0.5.
/// Senza regioni attese vale 1.
pub(crate) fn region_recall(expected: &[[f64; 4]], predicted: &[[f64; 4]]) -> f64 {
    if expected.is_empty() {
        return 1.0;
    }
    region_matches(expected, predicted) as f64 / expected.len() as f64
}

/// Campo CSV senza virgole né a capo (gli identificatori del piano non ne hanno).
fn field(s: &str) -> anyhow::Result<&str> {
    anyhow::ensure!(
        !s.contains([',', '\n', '\r', '"']),
        "campo CSV non ammesso: {s:?}"
    );
    Ok(s)
}

fn write_csv(path: &Path, header: &str, body: &str) -> anyhow::Result<()> {
    let mut text = String::with_capacity(header.len() + body.len() + 1);
    text.push_str(header);
    text.push('\n');
    text.push_str(body);
    std::fs::write(path, text).with_context(|| format!("scrittura di {}", path.display()))
}

pub(crate) const TEXT_CER_HEADER: &str = "source,doc_id,page,chars_expected,cer,two_column";
pub(crate) const BBOX_IOU_HEADER: &str =
    "doc_id,page,words,matched,merged,missed,spurious,unscorable,iou_mean,share_iou_ge_05";
pub(crate) const TABLES_HEADER: &str =
    "engine,device,table_id,cells_expected,cells_found,precision,recall,f1";

/// `text_cer.csv`: `source,doc_id,page,chars_expected,cer,two_column`.
pub(crate) fn write_text_cer(path: &Path, rows: &[TextCerRow]) -> anyhow::Result<()> {
    let mut body = String::new();
    for r in rows {
        writeln!(
            body,
            "{},{},{},{},{},{}",
            field(&r.source)?,
            field(&r.doc_id)?,
            r.page,
            r.chars_expected,
            r.cer,
            r.two_column
        )?;
    }
    write_csv(path, TEXT_CER_HEADER, &body)
}

/// `bbox_iou.csv`: `doc_id,page,words,matched,merged,missed,spurious,unscorable,iou_mean,share_iou_ge_05`.
pub(crate) fn write_bbox_iou(path: &Path, rows: &[(String, u32, PageScore)]) -> anyhow::Result<()> {
    let mut body = String::new();
    for (doc, page, s) in rows {
        writeln!(
            body,
            "{},{},{},{},{},{},{},{},{},{}",
            field(doc)?,
            page,
            s.words,
            s.matched,
            s.merged,
            s.missed,
            s.spurious,
            s.unscorable,
            s.iou_mean,
            s.share_iou_ge_05
        )?;
    }
    write_csv(path, BBOX_IOU_HEADER, &body)
}

fn tables_body(engine: &str, device: &str, rows: &[TableScore]) -> anyhow::Result<String> {
    let mut body = String::new();
    for r in rows {
        writeln!(
            body,
            "{},{},{},{},{},{},{},{}",
            field(engine)?,
            field(device)?,
            field(&r.table_id)?,
            r.cells_expected,
            r.cells_found,
            r.precision,
            r.recall,
            r.f1
        )?;
    }
    Ok(body)
}

/// `tables.csv`: `engine,device,table_id,cells_expected,cells_found,precision,recall,f1`.
pub(crate) fn write_tables(
    path: &Path,
    engine: &str,
    device: &str,
    rows: &[TableScore],
) -> anyhow::Result<()> {
    write_csv(path, TABLES_HEADER, &tables_body(engine, device, rows)?)
}

/// Più blocchi `(engine, device, righe)` nello stesso `tables.csv`.
pub(crate) fn write_tables_multi(
    path: &Path,
    blocks: &[(String, String, Vec<TableScore>)],
) -> anyhow::Result<()> {
    let Some(((e, d, rows), rest)) = blocks.split_first() else {
        return write_csv(path, TABLES_HEADER, "");
    };
    write_tables(path, e, d, rows)?;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .with_context(|| format!("{}", path.display()))?;
    for (e, d, rows) in rest {
        std::io::Write::write_all(&mut f, tables_body(e, d, rows)?.as_bytes())?;
    }
    Ok(())
}

/// Pagina valutata, per gli esempi.
pub(crate) struct ScoredPage {
    pub doc_id: String,
    pub page: u32,
    pub expected: Vec<Word>,
    pub predicted: Vec<Word>,
    pub score: PageScore,
}

/// Esempi di allineamento per `iou_examples.json`: fino a `per_kind` per tipo, nell'ordine
/// dato delle pagine, `total` al più.
pub(crate) fn examples(pages: &[ScoredPage], per_kind: usize, total: usize) -> serde_json::Value {
    let kinds = [
        MatchKind::Matched,
        MatchKind::Merged,
        MatchKind::Missed,
        MatchKind::Spurious,
        MatchKind::Unscorable,
    ];
    let mut taken: BTreeMap<&str, usize> = BTreeMap::new();
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for kind in kinds {
        for sp in pages {
            let (doc, page, exp, pred) = (&sp.doc_id, &sp.page, &sp.expected, &sp.predicted);
            for a in &sp.score.alignments {
                if a.kind != kind || out.len() >= total {
                    continue;
                }
                let n = taken.entry(kind.as_str()).or_default();
                if *n >= per_kind {
                    continue;
                }
                let key = (doc.clone(), *page, a.expected, a.predicted.clone());
                if !seen.insert(key) {
                    continue;
                }
                *n += 1;
                out.push(serde_json::json!({
                    "doc_id": doc,
                    "page": page,
                    "kind": kind.as_str(),
                    "iou": a.iou,
                    "expected": a.expected.map(|i| serde_json::json!({"text": exp[i].text, "bbox": exp[i].bbox})),
                    "predicted": a.predicted.iter().map(|&j| serde_json::json!({"text": pred[j].text, "bbox": pred[j].bbox})).collect::<Vec<_>>(),
                }));
            }
        }
    }
    serde_json::Value::Array(out)
}

// ---- file di ingresso e sottocomando `score` ------------------------------------------------

/// Parola, carattere o riga del JSON di pagina scritto da `extract pdf`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub(crate) struct PageItem {
    pub text: String,
    pub bbox: [f64; 4],
}

/// JSON di pagina scritto da `extract pdf` (contratto in `pdf.rs`).
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct PageDoc {
    pub page: u32,
    pub width: f64,
    pub height: f64,
    #[allow(dead_code)]
    pub rotation: u32,
    pub text: String,
    pub words: Vec<PageItem>,
    pub lines: Vec<PageItem>,
}

impl PageDoc {
    pub(crate) fn words(&self) -> Vec<Word> {
        self.words
            .iter()
            .map(|w| Word {
                text: w.text.clone(),
                bbox: w.bbox,
            })
            .collect()
    }
}

/// Voce di `spikes/data/s02/manifest.json`.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ManifestItem {
    pub dataset: String,
    pub item_id: String,
    pub doc_id: String,
    pub role: String,
    pub file: String,
    #[serde(default)]
    pub coco_image_id: Option<u64>,
}

pub(crate) fn load_manifest(path: &Path) -> anyhow::Result<Vec<ManifestItem>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("manifest {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("manifest {} non valido", path.display()))
}

/// Mappa i percorsi del worker (`/data`, `/results/raw`) su quelli del runner.
#[derive(Debug, Clone)]
pub(crate) struct Roots {
    pub data: std::path::PathBuf,
    pub results_raw: std::path::PathBuf,
}

impl Roots {
    pub(crate) fn map(&self, worker_path: &str) -> std::path::PathBuf {
        if let Some(rest) = worker_path.strip_prefix("/data/") {
            self.data.join(rest)
        } else if let Some(rest) = worker_path.strip_prefix("/results/raw/") {
            self.results_raw.join(rest)
        } else {
            std::path::PathBuf::from(worker_path)
        }
    }
}

/// Job `extract_pdf` con il suo esito: `input` del job → (`out_dir`, esito).
#[derive(Debug, Clone)]
pub(crate) struct ExtractRun {
    pub out_dir: String,
    pub status: String,
}

/// Job `extract_pdf` di `jobs.jsonl` uniti ai `RunReport` / `job_error` di `runs.jsonl`
/// (il `RunReport` di 01-07 non porta `input`). Chiave: `input` del job. Un input con più job
/// tiene l'ultimo `ok`, altrimenti l'ultimo visto.
pub(crate) fn extract_runs(jobs_jsonl: &str, runs_jsonl: &str) -> BTreeMap<String, ExtractRun> {
    let mut status: BTreeMap<String, String> = BTreeMap::new();
    for line in runs_jsonl.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(id) = v["job_id"].as_str() else {
            continue;
        };
        let st = v["outcome"]["status"]
            .as_str()
            .or_else(|| v["status"].as_str())
            .unwrap_or("unknown");
        status.insert(id.to_owned(), st.to_owned());
    }
    let mut out: BTreeMap<String, ExtractRun> = BTreeMap::new();
    for line in jobs_jsonl.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v["op"] != "extract_pdf" {
            continue;
        }
        let (Some(id), Some(input), Some(out_dir)) = (
            v["job_id"].as_str(),
            v["input"].as_str(),
            v["out_dir"].as_str(),
        ) else {
            continue;
        };
        let st = status
            .get(id)
            .cloned()
            .unwrap_or_else(|| "missing".to_owned());
        let keep_old = out
            .get(input)
            .is_some_and(|o| o.status == "ok" && st != "ok");
        if !keep_old {
            out.insert(
                input.to_owned(),
                ExtractRun {
                    out_dir: out_dir.to_owned(),
                    status: st,
                },
            );
        }
    }
    out
}

/// Pagine scritte da `extract pdf` in `dir`, per numero di pagina.
pub(crate) fn load_pages(dir: &Path) -> anyhow::Result<BTreeMap<u32, PageDoc>> {
    let mut out = BTreeMap::new();
    for e in std::fs::read_dir(dir).with_context(|| format!("pagine {}", dir.display()))? {
        let p = e?.path();
        if p.extension().is_some_and(|x| x == "json") {
            let d: PageDoc = serde_json::from_slice(&std::fs::read(&p)?)
                .with_context(|| format!("pagina {}", p.display()))?;
            out.insert(d.page, d);
        }
    }
    Ok(out)
}

/// Pagine del job `ok` di `input`, o `None` se il job manca o non è `ok`.
pub(crate) fn pages_of(
    runs: &BTreeMap<String, ExtractRun>,
    roots: &Roots,
    input: &str,
) -> anyhow::Result<Option<BTreeMap<u32, PageDoc>>> {
    match runs.get(input) {
        Some(r) if r.status == "ok" => Ok(Some(load_pages(&roots.map(&r.out_dir))?)),
        _ => Ok(None),
    }
}

/// Voce dell'oracolo IT ratificato (solo i campi usati).
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct OracleEntry {
    pub oracle_id: String,
    pub parser_kind: String,
    pub source_ref: serde_json::Value,
    pub expected_value: serde_json::Value,
    pub guardrail: serde_json::Value,
}

impl OracleEntry {
    /// Tre assi verdi: Claude concorde, GPT `CONFIRMED`, utente `ratified`.
    pub(crate) fn green(&self) -> bool {
        let g = &self.guardrail;
        g["claude"]
            .as_str()
            .is_some_and(|s| s.starts_with("concordo"))
            && g["gpt"] == "CONFIRMED"
            && g["user"] == "ratified"
    }

    pub(crate) fn document(&self) -> &str {
        self.source_ref["document_id"].as_str().unwrap_or_default()
    }

    pub(crate) fn page(&self) -> u32 {
        self.source_ref["page"].as_u64().unwrap_or(0) as u32
    }
}

pub(crate) fn load_oracle(path: &Path) -> anyhow::Result<Vec<OracleEntry>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("oracolo {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("oracolo {} non valido", path.display()))
}

/// Tabella attesa da una voce `table` dell'oracolo IT (solo celle del corpo, senza bbox).
pub(crate) fn oracle_table(o: &OracleEntry) -> Table {
    let cells = o.expected_value["cells"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|c| Cell {
                    row: c["row"].as_u64().unwrap_or(0) as u32,
                    col: c["col"].as_u64().unwrap_or(0) as u32,
                    text: c["text"].as_str().unwrap_or_default().to_owned(),
                })
                .filter(|c| c.row >= 1)
                .collect()
        })
        .unwrap_or_default();
    Table {
        id: o.oracle_id.clone(),
        page: o.page(),
        bbox: [0.0; 4],
        cells,
    }
}

/// Parole attese di una pagina TAT-DQA, in punti PDF, nell'ordine dei blocchi (`order`), e
/// il testo atteso per il CER.
pub(crate) fn tatdqa_page(
    ann: &serde_json::Value,
    page_index: usize,
    page_w: f64,
    page_h: f64,
) -> anyhow::Result<(Vec<Word>, String)> {
    let p = &ann["pages"][page_index];
    let pb = p["bbox"].as_array().context("pages[].bbox assente")?;
    let img_w = pb.get(2).and_then(|v| v.as_f64()).context("bbox")?
        - pb.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
    let img_h = pb.get(3).and_then(|v| v.as_f64()).context("bbox")?
        - pb.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let mut blocks: Vec<&serde_json::Value> = p["blocks"]
        .as_array()
        .context("blocks assenti")?
        .iter()
        .collect();
    blocks.sort_by_key(|b| b["order"].as_u64().unwrap_or(u64::MAX));
    let mut words = Vec::new();
    for b in blocks {
        let wl = b["words"]["word_list"].as_array();
        let bl = b["words"]["bbox_list"].as_array();
        let (Some(wl), Some(bl)) = (wl, bl) else {
            continue;
        };
        for (t, bb) in wl.iter().zip(bl) {
            let v: Vec<f64> = bb
                .as_array()
                .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
                .unwrap_or_default();
            let bbox = if v.len() == 4 {
                tatdqa_to_pdf([v[0], v[1], v[2], v[3]], img_w, img_h, page_w, page_h)
            } else {
                [0.0; 4]
            };
            words.push(Word {
                text: t.as_str().unwrap_or_default().to_owned(),
                bbox,
            });
        }
    }
    let text = words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    Ok((words, text))
}

/// Righe dentro la pagina utile per l'oracolo IT: esclusa la banda del 7% in alto e in basso.
pub(crate) fn it_body_lines(page: &PageDoc) -> Vec<String> {
    let band = page.height * 0.07;
    page.lines
        .iter()
        .filter(|l| {
            let mid = (l.bbox[1] + l.bbox[3]) / 2.0;
            mid > band && mid < page.height - band
        })
        .map(|l| l.text.clone())
        .collect()
}

#[derive(clap::Parser, Debug)]
#[command(name = "extract score")]
struct ScoreCli {
    #[command(subcommand)]
    cmd: ScoreCmd,
}

#[derive(clap::Args, Debug)]
struct Common {
    /// JSONL dei job `extract_pdf` (percorsi del worker).
    #[arg(long)]
    jobs: std::path::PathBuf,
    /// `RunReport` di `parse-parent batch`.
    #[arg(long)]
    runs: std::path::PathBuf,
    /// Cartella che nel worker è `/data`.
    #[arg(long)]
    data_root: std::path::PathBuf,
    /// Cartella che nel worker è `/results/raw`.
    #[arg(long)]
    results_raw: std::path::PathBuf,
    #[arg(long)]
    manifest: std::path::PathBuf,
    #[arg(long)]
    oracle: std::path::PathBuf,
    /// Cartella dei risultati (con `hw.json`).
    #[arg(long)]
    out_dir: std::path::PathBuf,
}

#[derive(clap::Subcommand, Debug)]
enum ScoreCmd {
    /// CER (TAT-DQA, oracolo IT), IoU delle parole TAT-DQA, esempi.
    Text {
        #[command(flatten)]
        common: Common,
        /// Documento dell'oracolo IT impaginato a due colonne (ripetibile).
        #[arg(long)]
        two_column_doc: Vec<String>,
    },
    /// F1 di cella (Docling e baseline PDFium) e recall delle regioni DocLayNet.
    Tables {
        #[command(flatten)]
        common: Common,
        /// `<device>=<cartella>` delle tabelle normalizzate di `docling-client` (ripetibile).
        #[arg(long)]
        docling: Vec<String>,
        /// Parametri dell'euristica (per `cell_gap_pt` della baseline).
        #[arg(long)]
        params: std::path::PathBuf,
        /// Annotazioni COCO DocLayNet (test.json).
        #[arg(long)]
        doclaynet_coco: std::path::PathBuf,
        /// Ripetizione Docling da valutare (le tabelle non cambiano fra ripetizioni).
        #[arg(long, default_value_t = 3)]
        run: u32,
    },
}

/// `extract score …`
pub(crate) fn run(args: Vec<std::ffi::OsString>) -> anyhow::Result<()> {
    use clap::Parser as _;
    let cli = ScoreCli::try_parse_from(
        std::iter::once(std::ffi::OsString::from("extract-score")).chain(args),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        ScoreCmd::Text {
            common,
            two_column_doc,
        } => score_text(&common, &two_column_doc),
        ScoreCmd::Tables {
            common,
            docling,
            params,
            doclaynet_coco,
            run,
        } => score_tables_cmd(&common, &docling, &params, &doclaynet_coco, run),
    }
}

struct Loaded {
    roots: Roots,
    runs: BTreeMap<String, ExtractRun>,
    manifest: Vec<ManifestItem>,
    oracle: Vec<OracleEntry>,
}

fn load_common(c: &Common) -> anyhow::Result<Loaded> {
    common::manifest::ensure_present(&c.out_dir)
        .with_context(|| format!("hw.json assente in {}", c.out_dir.display()))?;
    let jobs = std::fs::read_to_string(&c.jobs)?;
    let runs = std::fs::read_to_string(&c.runs)?;
    Ok(Loaded {
        roots: Roots {
            data: c.data_root.clone(),
            results_raw: c.results_raw.clone(),
        },
        runs: extract_runs(&jobs, &runs),
        manifest: load_manifest(&c.manifest)?,
        oracle: load_oracle(&c.oracle)?,
    })
}

fn s02_input(file: &str) -> String {
    format!("/data/s02/raw/{file}")
}

fn corpus_input(doc_id: &str) -> String {
    format!("/data/corpus/raw/{doc_id}.pdf")
}

fn score_text(c: &Common, two_column: &[String]) -> anyhow::Result<()> {
    let l = load_common(c)?;
    let mut cer_rows = Vec::new();
    let mut iou_rows: Vec<(String, u32, PageScore)> = Vec::new();
    let mut ex_pages: Vec<ScoredPage> = Vec::new();
    let mut not_ok = Vec::new();
    let (mut en_dist, mut en_chars) = (0.0f64, 0u64);
    let (mut good, mut scorable, mut unscorable) = (0usize, 0usize, 0usize);

    for item in l
        .manifest
        .iter()
        .filter(|i| i.dataset == "TAT-DQA" && i.role == "pdf")
    {
        let ann_item = l
            .manifest
            .iter()
            .find(|a| a.dataset == "TAT-DQA" && a.role == "annotation" && a.doc_id == item.doc_id)
            .with_context(|| format!("annotazione TAT-DQA di {}", item.item_id))?;
        let ann: serde_json::Value = serde_json::from_slice(&std::fs::read(
            l.roots.data.join("s02/raw").join(&ann_item.file),
        )?)?;
        let n_pages = ann["pages"].as_array().map_or(0, Vec::len);
        let pages = pages_of(&l.runs, &l.roots, &s02_input(&item.file))?;
        for idx in 0..n_pages {
            let page_no = idx as u32 + 1;
            let pd = pages.as_ref().and_then(|p| p.get(&page_no));
            let (w, h) = pd.map_or((612.0, 792.0), |p| (p.width, p.height));
            let (exp, exp_text) = tatdqa_page(&ann, idx, w, h)?;
            let (pred, pred_text) = match pd {
                Some(p) => (p.words(), p.text.clone()),
                None => {
                    not_ok.push(format!("{}#{}", item.item_id, page_no));
                    (Vec::new(), String::new())
                }
            };
            let cer_v = if pd.is_some() {
                cer(&exp_text, &pred_text)
            } else {
                1.0
            };
            let n = normalize_text(&exp_text).chars().count();
            en_dist += cer_v * n as f64;
            en_chars += n as u64;
            cer_rows.push(TextCerRow {
                source: "tat-dqa".into(),
                doc_id: item.item_id.clone(),
                page: page_no,
                chars_expected: n,
                cer: cer_v,
                two_column: false,
            });
            let page_box = [0.0, 0.0, w, h];
            let s = score_words(&exp, &pred, page_box);
            good += s.good();
            scorable += s.scorable();
            unscorable += s.unscorable;
            iou_rows.push((item.item_id.clone(), page_no, s.clone()));
            ex_pages.push(ScoredPage {
                doc_id: item.item_id.clone(),
                page: page_no,
                expected: exp,
                predicted: pred,
                score: s,
            });
        }
    }

    let (mut it_dist, mut it_chars) = (0u64, 0u64);
    let mut it_ops = Vec::new();
    let mut excluded = Vec::new();
    for o in l.oracle.iter().filter(|o| o.parser_kind == "page-text") {
        if !o.green() {
            excluded.push(o.oracle_id.clone());
            continue;
        }
        let expected = o.expected_value.as_str().unwrap_or_default();
        let pages = pages_of(&l.runs, &l.roots, &corpus_input(o.document()))?;
        let pd = pages.as_ref().and_then(|p| p.get(&o.page()));
        let predicted = match pd {
            Some(p) => oracle_it_prediction(&it_body_lines(p)),
            None => {
                not_ok.push(format!("{}#{}", o.document(), o.page()));
                String::new()
            }
        };
        let (cer_v, n, ops) = cer_ops(expected, &predicted);
        it_dist += ops.distance();
        it_chars += n as u64;
        it_ops.push(serde_json::json!({
            "oracle_id": o.oracle_id, "doc_id": o.document(), "page": o.page(),
            "chars_expected": n, "cer": cer_v, "substitutions": ops.substitutions,
            "deletions": ops.deletions, "insertions": ops.insertions,
            "s_plus_d_over_n": if n == 0 { 0.0 } else { (ops.substitutions + ops.deletions) as f64 / n as f64 },
        }));
        cer_rows.push(TextCerRow {
            source: "oracle-it".into(),
            doc_id: o.document().to_owned(),
            page: o.page(),
            chars_expected: n,
            cer: cer_v,
            two_column: two_column.iter().any(|d| d == o.document()),
        });
    }

    write_text_cer(&c.out_dir.join("text_cer.csv"), &cer_rows)?;
    write_bbox_iou(&c.out_dir.join("bbox_iou.csv"), &iou_rows)?;
    let ex = examples(&ex_pages, 2, 10);
    std::fs::write(
        c.out_dir.join("iou_examples.json"),
        serde_json::to_string_pretty(&ex)? + "\n",
    )?;
    let ratio = |a: u64, b: u64| if b == 0 { 0.0 } else { a as f64 / b as f64 };
    println!(
        "{}",
        serde_json::json!({
            "status": "score_text",
            "e1_cer_tatdqa": if en_chars == 0 { 0.0 } else { en_dist / en_chars as f64 },
            "e1_chars": en_chars,
            "e2_cer_oracle_it": ratio(it_dist, it_chars),
            "e2_chars": it_chars,
            "e2_pages": it_ops,
            "e3_share_iou_ge_05": ratio(good as u64, scorable as u64),
            "e3_scorable": scorable,
            "e3_good": good,
            "unscorable": unscorable,
            "oracles_excluded": excluded,
            "pages_without_ok_job": not_ok,
        })
    );
    Ok(())
}

/// Tabelle normalizzate scritte da `docling-client` (`*.tables.json`).
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct DoclingTables {
    pub tables: Vec<DoclingTable>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct DoclingTable {
    pub page: u32,
    pub bbox: Option<[f64; 4]>,
    pub cells: Vec<DoclingCell>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct DoclingCell {
    pub row: u32,
    pub col: u32,
    pub text: String,
}

impl DoclingTable {
    pub(crate) fn to_table(&self, id: String) -> Table {
        Table {
            id,
            page: self.page,
            bbox: self.bbox.unwrap_or([0.0; 4]),
            cells: self
                .cells
                .iter()
                .map(|c| Cell {
                    row: c.row,
                    col: c.col,
                    text: c.text.clone(),
                })
                .collect(),
        }
    }
}

/// Nome del file delle tabelle di `docling-client` per documento, pagina e ripetizione.
pub(crate) fn docling_tables_file(
    dir: &Path,
    doc_id: &str,
    page: u32,
    run: u32,
) -> std::path::PathBuf {
    dir.join(format!("{doc_id}__p{page}__r{run}.tables.json"))
}

fn load_docling(
    dir: &Path,
    doc_id: &str,
    page: u32,
    run: u32,
) -> anyhow::Result<Option<Vec<Table>>> {
    let f = docling_tables_file(dir, doc_id, page, run);
    if !f.is_file() {
        return Ok(None);
    }
    let d: DoclingTables = serde_json::from_slice(&std::fs::read(&f)?)
        .with_context(|| format!("tabelle {}", f.display()))?;
    Ok(Some(
        d.tables
            .iter()
            .enumerate()
            .map(|(i, t)| t.to_table(format!("{doc_id}-p{page}-t{i}")))
            .collect(),
    ))
}

/// Regioni `Table` (id 9, `precedence` 0) di un'immagine COCO DocLayNet, in punti PDF.
pub(crate) fn doclaynet_regions(
    coco: &serde_json::Value,
    image_id: u64,
    page_w: f64,
    page_h: f64,
) -> Vec<[f64; 4]> {
    let img = coco["images"]
        .as_array()
        .and_then(|a| a.iter().find(|i| i["id"].as_u64() == Some(image_id)));
    let (cw, ch) = img.map_or((1025.0, 1025.0), |i| {
        (
            i["width"].as_f64().unwrap_or(1025.0),
            i["height"].as_f64().unwrap_or(1025.0),
        )
    });
    coco["annotations"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|x| {
                    x["image_id"].as_u64() == Some(image_id)
                        && x["category_id"].as_u64() == Some(9)
                        && x["precedence"].as_u64().unwrap_or(0) == 0
                })
                .filter_map(|x| {
                    let b: Vec<f64> = x["bbox"]
                        .as_array()?
                        .iter()
                        .filter_map(|v| v.as_f64())
                        .collect();
                    (b.len() == 4).then(|| {
                        tatdqa_to_pdf(
                            [b[0], b[1], b[0] + b[2], b[1] + b[3]],
                            cw,
                            ch,
                            page_w,
                            page_h,
                        )
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn score_tables_cmd(
    c: &Common,
    docling: &[String],
    params_path: &Path,
    coco_path: &Path,
    run: u32,
) -> anyhow::Result<()> {
    let l = load_common(c)?;
    let params = crate::tables::load_params(params_path)?;
    let coco: serde_json::Value = serde_json::from_slice(&std::fs::read(coco_path)?)
        .with_context(|| format!("COCO {}", coco_path.display()))?;
    let mut dirs: Vec<(String, std::path::PathBuf)> = Vec::new();
    for d in docling {
        let (dev, dir) = d
            .split_once('=')
            .with_context(|| format!("--docling atteso <device>=<cartella>: {d}"))?;
        dirs.push((dev.to_owned(), std::path::PathBuf::from(dir)));
    }
    let tables: Vec<&OracleEntry> = l
        .oracle
        .iter()
        .filter(|o| o.parser_kind == "table" && o.green())
        .collect();
    let excluded: Vec<&str> = l
        .oracle
        .iter()
        .filter(|o| o.parser_kind == "table" && !o.green())
        .map(|o| o.oracle_id.as_str())
        .collect();
    let mut blocks: Vec<(String, String, Vec<TableScore>)> = Vec::new();
    let mut summary = serde_json::Map::new();
    for (dev, dir) in &dirs {
        let mut rows = Vec::new();
        for o in &tables {
            let exp = oracle_table(o);
            let pred = load_docling(dir, o.document(), o.page(), run)?.unwrap_or_default();
            rows.push(score_table_without_bbox(&exp, &pred));
        }
        summary.insert(format!("e4_docling_{dev}"), micro(&rows));
        blocks.push(("docling".into(), dev.clone(), rows));
    }
    let mut base = Vec::new();
    for o in &tables {
        let exp = oracle_table(o);
        let pages = pages_of(&l.runs, &l.roots, &corpus_input(o.document()))?;
        let pred: Vec<Table> = pages
            .as_ref()
            .and_then(|p| p.get(&o.page()))
            .map(|p| {
                vec![crate::tables::baseline_table(
                    p,
                    &params,
                    format!("pdfium-{}", o.oracle_id),
                )]
            })
            .unwrap_or_default();
        base.push(score_table_without_bbox(&exp, &pred));
    }
    summary.insert("e5_pdfium_baseline".into(), micro(&base));
    blocks.push(("pdfium".into(), "cpu".into(), base));
    write_tables_multi(&c.out_dir.join("tables.csv"), &blocks)?;

    // E-6: regioni tabella DocLayNet sulle pagine con almeno una tabella.
    for (dev, dir) in &dirs {
        let (mut exp_n, mut hit_n, mut pred_n, mut pages_n, mut missing) = (0, 0, 0, 0, Vec::new());
        let mut per_page = Vec::new();
        for item in l
            .manifest
            .iter()
            .filter(|i| i.dataset == "DocLayNet" && i.role == "pdf")
        {
            let Some(image_id) = item.coco_image_id else {
                continue;
            };
            let pages = pages_of(&l.runs, &l.roots, &s02_input(&item.file))?;
            let Some(pd) = pages.as_ref().and_then(|p| p.get(&1)) else {
                continue;
            };
            let exp = doclaynet_regions(&coco, image_id, pd.width, pd.height);
            if exp.is_empty() {
                continue;
            }
            pages_n += 1;
            let Some(pred) = load_docling(dir, &item.item_id, 1, run)? else {
                missing.push(item.item_id.clone());
                exp_n += exp.len();
                continue;
            };
            let pb: Vec<[f64; 4]> = pred.iter().map(|t| t.bbox).collect();
            let m = region_matches(&exp, &pb);
            exp_n += exp.len();
            hit_n += m;
            pred_n += pb.len();
            per_page.push(serde_json::json!({"item_id": item.item_id, "expected": exp.len(), "predicted": pb.len(), "matched": m, "recall": region_recall(&exp, &pb)}));
        }
        summary.insert(
            format!("e6_regions_{dev}"),
            serde_json::json!({
                "pages": pages_n, "regions_expected": exp_n, "regions_matched": hit_n,
                "regions_predicted": pred_n,
                "recall": if exp_n == 0 { 1.0 } else { hit_n as f64 / exp_n as f64 },
                "pages_without_docling": missing, "per_page": per_page,
            }),
        );
    }
    summary.insert("status".into(), "score_tables".into());
    summary.insert("oracles_excluded".into(), serde_json::json!(excluded));
    println!("{}", serde_json::Value::Object(summary));
    Ok(())
}

fn micro(rows: &[TableScore]) -> serde_json::Value {
    let m: usize = rows.iter().map(|r| r.matched).sum();
    let e: usize = rows.iter().map(|r| r.cells_expected).sum();
    let f: usize = rows.iter().map(|r| r.cells_found).sum();
    let p = if f == 0 { 0.0 } else { m as f64 / f as f64 };
    let r = if e == 0 { 0.0 } else { m as f64 / e as f64 };
    let f1 = if p + r == 0.0 {
        0.0
    } else {
        2.0 * p * r / (p + r)
    };
    serde_json::json!({"matched": m, "expected": e, "found": f, "precision": p, "recall": r, "f1": f1})
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
