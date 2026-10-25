//! `score`, `hostile-verdict`, `throughput-row`, `paddle-check` e regole di giudizio del piano
//! 01-13 (TEST-2..6, 9, 11, 13, 15).
//!
//! Firme e tipi delle regole sono il contratto dei test del test-write (2026-10-23); i corpi
//! e i sottocomandi li scrive l'esecutore del Task 2. Le regole sono quelle delle «Soglie» di
//! `RISULTATI-ocr.md`:
//! - CER solo sugli oracoli `parser-it.json` con i tre assi verdi (`claude` valorizzato,
//!   `gpt` = `CONFIRMED`, `user` = `ratified`), normalizzazione di 01-08;
//! - fedeltà bbox: `reference=analytic` sempre giudicata, `reference=pdfium` `non_valutabile`
//!   se 01-08 non ha superato la propria soglia IoU;
//! - prova ostile sul render (D-34): regola di `hostile_verdict` di 01-07 + contenimento;
//! - conformità di un batch OCR col metodo di M10 di 01-18, prenotazione di 01-07 v6;
//! - controlli PaddleOCR: selezione v6 esplicita, famiglia `PP-OCRv6_`, manifest dei pesi,
//!   confronto sessione/build.
//!
//! I sottocomandi girano nel runner su dati derivati (mai come figli isolati) e leggono i
//! percorsi del worker (`/results/raw/...`) rimappati con `--results-raw`.
//!
//! **Modifica:** Claude (gsd-executor 01-13) · 2026-10-24 · corpi delle regole e sottocomandi (Task 2)
#![allow(dead_code)]
// Il test a tabella di `m10_pass` usa `Vec<(&str, Box<dyn Fn(&mut BatchObs)>)>`: il blocco di test
// è del test-write e non si tocca, quindi la regola si spegne a livello di modulo.
#![allow(clippy::type_complexity)]

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

use crate::geom::{self, Transform};
use crate::tsv::Word;

// ---- CER ---------------------------------------------------------------------------------

/// NFC, spazi bianchi collassati in uno, bordi rimossi (come `normalize_text` di 01-08).
pub(crate) fn normalize_text(s: &str) -> String {
    let nfc: String = s.nfc().collect();
    nfc.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Distanza di Levenshtein sui caratteri (due righe di memoria).
pub(crate) fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur[j + 1] = sub.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// CER = distanza di edit sui caratteri normalizzati / caratteri attesi.
pub(crate) fn cer(expected: &str, predicted: &str) -> f64 {
    let e: Vec<char> = normalize_text(expected).chars().collect();
    let p: Vec<char> = normalize_text(predicted).chars().collect();
    if e.is_empty() {
        return if p.is_empty() { 0.0 } else { 1.0 };
    }
    levenshtein(&e, &p) as f64 / e.len() as f64
}

/// Voce dell'oracolo con i tre assi verdi.
pub(crate) fn three_green(entry: &Value) -> bool {
    let g = &entry["guardrail"];
    g["claude"].as_str().is_some_and(|c| !c.trim().is_empty())
        && g["gpt"].as_str() == Some("CONFIRMED")
        && g["user"].as_str() == Some("ratified")
}

/// Voci utilizzabili e numero di voci pendenti escluse e contate.
pub(crate) fn usable_oracles(entries: &[Value]) -> (Vec<&Value>, usize) {
    let ok: Vec<&Value> = entries.iter().filter(|e| three_green(e)).collect();
    let excluded = entries.len() - ok.len();
    (ok, excluded)
}

/// Testo di una pagina dall'output JSON di PaddleOCR (`rec_texts`, nell'ordine).
pub(crate) fn paddle_text(out: &Value) -> Result<String, String> {
    let texts = out["res"]["rec_texts"]
        .as_array()
        .or_else(|| out["rec_texts"].as_array())
        .ok_or("rec_texts assente dall'output di PaddleOCR")?;
    let parts: Vec<&str> = texts.iter().filter_map(Value::as_str).collect();
    if parts.len() != texts.len() {
        return Err("rec_texts con voci non testuali".into());
    }
    Ok(parts.join(" "))
}

/// Righe di testo di PaddleOCR con il loro rettangolo in pixel (`rec_texts` accoppiati a
/// `rec_boxes` `[x_min, y_min, x_max, y_max]`, nell'ordine). Lunghezze diverse, box non a
/// quattro numeri o testi non testuali → `Err`: nessun accoppiamento indovinato.
pub(crate) fn paddle_boxes(out: &Value) -> Result<Vec<(String, [f64; 4])>, String> {
    let res = if out["res"].is_object() {
        &out["res"]
    } else {
        out
    };
    let texts = res["rec_texts"]
        .as_array()
        .ok_or("rec_texts assente dall'output di PaddleOCR")?;
    let boxes = res["rec_boxes"]
        .as_array()
        .ok_or("rec_boxes assente dall'output di PaddleOCR")?;
    if texts.len() != boxes.len() {
        return Err(format!(
            "rec_texts ({}) e rec_boxes ({}) di lunghezza diversa",
            texts.len(),
            boxes.len()
        ));
    }
    let mut out = Vec::with_capacity(texts.len());
    for (t, b) in texts.iter().zip(boxes) {
        let t = t.as_str().ok_or("rec_texts con voci non testuali")?;
        let b: Vec<f64> = b
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_default();
        let [x0, y0, x1, y1] = b[..] else {
            return Err("rec_boxes con voci non a quattro numeri".into());
        };
        out.push((t.to_owned(), [x0, y0, x1, y1]));
    }
    Ok(out)
}

/// Una pagina PaddleOCR salvata dal comando dello sfidante: l'unico `*_res.json` di
/// `<dir>/<job_id>/` (`--save_path` dell'invocazione) e il tempo di parete dell'invocazione
/// singola in `<dir>/<job_id>.wall_ms` (millisecondi interi, preso dentro il container).
#[derive(Debug, Clone)]
pub(crate) struct PaddlePage {
    pub text: String,
    pub boxes_px: Vec<(String, [f64; 4])>,
    pub wall_ms: u64,
}

pub(crate) fn load_paddle_page(dir: &Path, job_id: &str) -> anyhow::Result<PaddlePage> {
    let out_dir = dir.join(job_id);
    let mut res: Vec<PathBuf> = std::fs::read_dir(&out_dir)
        .with_context(|| format!("uscita PaddleOCR assente: {}", out_dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("_res.json"))
        })
        .collect();
    if res.len() != 1 {
        bail!(
            "{}: attesi 1 file *_res.json, trovati {}",
            out_dir.display(),
            res.len()
        );
    }
    let v = read_json(&res.remove(0))?;
    let text = paddle_text(&v).map_err(anyhow::Error::msg)?;
    let boxes_px = paddle_boxes(&v).map_err(anyhow::Error::msg)?;
    let wp = dir.join(format!("{job_id}.wall_ms"));
    let wall_ms = std::fs::read_to_string(&wp)
        .with_context(|| format!("{}", wp.display()))?
        .trim()
        .parse::<u64>()
        .with_context(|| format!("{}: millisecondi non interi", wp.display()))?;
    Ok(PaddlePage {
        text,
        boxes_px,
        wall_ms,
    })
}

/// Origine delle parole OCR di una voce della mappa: `RunReport` di `ocr_tesseract` oppure
/// uscite salvate di PaddleOCR (righe di testo, non parole: una riga si abbina a una parola
/// attesa solo se il testo normalizzato è uguale).
pub(crate) enum Source<'a> {
    Tesseract(&'a BTreeMap<String, Value>),
    Paddle(&'a Path),
}

impl Source<'_> {
    /// Testo della pagina e secondi dell'invocazione.
    fn text_secs(&self, it: &TessItem) -> anyhow::Result<(String, f64)> {
        match self {
            Source::Tesseract(reports) => {
                let r = reports
                    .get(&it.job_id)
                    .with_context(|| format!("RunReport assente per {}", it.job_id))?;
                let secs = r["wall_ms"].as_u64().unwrap_or(0) as f64 / 1000.0;
                Ok((words_text(&report_words(r)?), secs))
            }
            Source::Paddle(dir) => {
                let p = load_paddle_page(dir, &it.job_id)?;
                Ok((p.text, p.wall_ms as f64 / 1000.0))
            }
        }
    }

    /// Testi e rettangoli in pixel dell'immagine d'ingresso.
    fn boxes_px(&self, it: &TessItem) -> anyhow::Result<Vec<(String, [f64; 4])>> {
        match self {
            Source::Tesseract(reports) => {
                let r = reports
                    .get(&it.job_id)
                    .with_context(|| format!("RunReport assente per {}", it.job_id))?;
                Ok(report_words(r)?
                    .iter()
                    .map(|w| (w.text.clone(), word_px(w)))
                    .collect())
            }
            Source::Paddle(dir) => Ok(load_paddle_page(dir, &it.job_id)?.boxes_px),
        }
    }
}

/// Testo di una pagina OCR Tesseract: le parole nell'ordine del TSV, unite da uno spazio.
pub(crate) fn words_text(words: &[Word]) -> String {
    words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

// ---- Fedeltà bbox ------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Reference {
    Analytic,
    Pdfium,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Quality {
    Good,
    Degraded,
}

impl Quality {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Quality::Good => "good",
            Quality::Degraded => "degraded",
        }
    }

    pub(crate) fn parse(s: &str) -> anyhow::Result<Quality> {
        match s {
            "good" => Ok(Quality::Good),
            "degraded" => Ok(Quality::Degraded),
            _ => bail!("qualità sconosciuta: {s}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Verdict {
    Pass,
    Fail,
    NonValutabile,
}

impl Verdict {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Fail => "fail",
            Verdict::NonValutabile => "non_valutabile",
        }
    }
}

/// Metriche di una pagina su coppie (atteso, ottenuto) già abbinate, in punti PDF.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BboxMetrics {
    pub words_matched: usize,
    pub iou_median: f64,
    pub share_iou_ge_05: f64,
    /// p95 nearest-rank, per parola, della massima distanza fra angoli corrispondenti.
    pub corner_err_p95_pt: f64,
}

pub(crate) fn bbox_metrics(pairs: &[([f64; 4], [f64; 4])]) -> BboxMetrics {
    if pairs.is_empty() {
        return BboxMetrics {
            words_matched: 0,
            iou_median: 0.0,
            share_iou_ge_05: 0.0,
            corner_err_p95_pt: 0.0,
        };
    }
    let mut ious: Vec<f64> = pairs.iter().map(|(a, b)| iou(a, b)).collect();
    let mut errs: Vec<f64> = pairs.iter().map(|(a, b)| corner_err(a, b)).collect();
    ious.sort_by(f64::total_cmp);
    errs.sort_by(f64::total_cmp);
    let n = pairs.len();
    let median = if n % 2 == 1 {
        ious[n / 2]
    } else {
        (ious[n / 2 - 1] + ious[n / 2]) / 2.0
    };
    BboxMetrics {
        words_matched: n,
        iou_median: median,
        share_iou_ge_05: ious.iter().filter(|&&v| v >= 0.5).count() as f64 / n as f64,
        corner_err_p95_pt: nearest_rank(&errs, 0.95),
    }
}

/// Percentile nearest-rank su valori ordinati (`rank = ⌈p·n⌉`).
pub(crate) fn nearest_rank(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = ((p * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    sorted[rank - 1]
}

/// IoU di due rettangoli `[x0, y0, x1, y1]`.
pub(crate) fn iou(a: &[f64; 4], b: &[f64; 4]) -> f64 {
    let ix = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
    let iy = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    let inter = ix * iy;
    let area = |r: &[f64; 4]| (r[2] - r[0]).max(0.0) * (r[3] - r[1]).max(0.0);
    let union = area(a) + area(b) - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}

/// Massima distanza fra i quattro angoli corrispondenti di due rettangoli.
pub(crate) fn corner_err(a: &[f64; 4], b: &[f64; 4]) -> f64 {
    [(0, 1), (2, 1), (0, 3), (2, 3)]
        .iter()
        .map(|&(x, y)| ((a[x] - b[x]).powi(2) + (a[y] - b[y]).powi(2)).sqrt())
        .fold(0.0, f64::max)
}

/// Abbinamento uno a uno per testo normalizzato uguale e IoU decrescente (IoU > 0), come le
/// parole di 01-08: coppie (atteso, ottenuto).
pub(crate) fn match_words(
    expected: &[(String, [f64; 4])],
    got: &[(String, [f64; 4])],
) -> Vec<([f64; 4], [f64; 4])> {
    let mut cands: Vec<(f64, usize, usize)> = Vec::new();
    for (i, (te, re)) in expected.iter().enumerate() {
        let te = normalize_text(te);
        for (j, (tg, rg)) in got.iter().enumerate() {
            if normalize_text(tg) == te {
                let v = iou(re, rg);
                if v > 0.0 {
                    cands.push((v, i, j));
                }
            }
        }
    }
    cands.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let (mut used_e, mut used_g) = (vec![false; expected.len()], vec![false; got.len()]);
    let mut out = Vec::new();
    for (_, i, j) in cands {
        if !used_e[i] && !used_g[j] {
            used_e[i] = true;
            used_g[j] = true;
            out.push((expected[i].1, got[j].1));
        }
    }
    out
}

/// Verdetto di una riga di `bbox_map.csv`. `pdfium_gate` = esito della soglia IoU di 01-08
/// (`None` se non disponibile).
pub(crate) fn bbox_verdict(
    reference: Reference,
    quality: Quality,
    m: &BboxMetrics,
    pdfium_gate: Option<bool>,
) -> Verdict {
    match reference {
        Reference::Analytic => {
            let tol = match quality {
                Quality::Good => 1.0,
                Quality::Degraded => 3.0,
            };
            if m.words_matched > 0 && m.corner_err_p95_pt <= tol {
                Verdict::Pass
            } else {
                Verdict::Fail
            }
        }
        Reference::Pdfium => {
            if pdfium_gate != Some(true) {
                Verdict::NonValutabile
            } else if m.words_matched > 0 && m.iou_median >= 0.5 && m.corner_err_p95_pt <= 3.0 {
                Verdict::Pass
            } else {
                Verdict::Fail
            }
        }
    }
}

// ---- Prova ostile sul render (D-34) --------------------------------------------------------

/// Ciò che `run_hostile_render.sh` salva per un job.
#[derive(Debug, Clone)]
pub(crate) struct HostileObs {
    /// Riga `RunReport` di `parse-parent run` (assente se il padre non l'ha scritta).
    pub report: Option<Value>,
    /// Exit di `parse-parent`; `None` se terminato da un segnale.
    pub parent_exit: Option<i32>,
    /// `session.sh ps --assert-only worker` uscito 0 dopo il job.
    pub worker_alive: bool,
}

/// Esito di una riga di `hostile-render.csv`: `(pass, parent_alive)`.
pub(crate) fn hostile_row(
    expected_status: &str,
    expected_kinds: &[String],
    obs: &HostileObs,
) -> (bool, bool) {
    let parent_alive = obs.report.is_some() && obs.parent_exit.is_some() && obs.worker_alive;
    let Some(r) = obs.report.as_ref() else {
        return (false, parent_alive);
    };
    let outcome = &r["outcome"];
    let status = outcome["status"].as_str().unwrap_or_default();
    // Regola di `hostile_verdict` di 01-07: status uguale, kind fra le alternative (failed),
    // motivi attesi inclusi negli ottenuti (quarantined), array vuoto (ok); più oom_adj_set.
    let verdict = status == expected_status
        && match status {
            "failed" => outcome["kind"]
                .as_str()
                .is_some_and(|k| expected_kinds.iter().any(|e| e == k)),
            "quarantined" => {
                let got: Vec<&str> = outcome["reasons"]
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                expected_kinds.iter().all(|e| got.contains(&e.as_str()))
            }
            "ok" => expected_kinds.is_empty(),
            _ => false,
        };
    let contained = r["leftover_procs"].as_u64() == Some(0);
    let adj = r["oom_adj_set"].as_bool() == Some(true);
    (parent_alive && verdict && adj && contained, parent_alive)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum T09 {
    Chiuso,
    Aperto,
    ContenimentoFallito,
}

impl T09 {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            T09::Chiuso => "chiuso",
            T09::Aperto => "aperto",
            T09::ContenimentoFallito => "contenimento fallito",
        }
    }
}

/// Residuo T-09 JBIG2 dalle righe di `jbig2-malformed`: `(pass, parent_alive, leftover_procs)`.
pub(crate) fn t09_status(rows: &[(bool, bool, u32)]) -> T09 {
    if rows.iter().any(|&(_, alive, left)| !alive || left > 0) {
        T09::ContenimentoFallito
    } else if rows.len() == 3 && rows.iter().all(|r| r.0) {
        T09::Chiuso
    } else {
        T09::Aperto
    }
}

// ---- Conformità di un batch OCR (metodo di M10 di 01-18) ------------------------------------

/// Prenotazione per job di `ocr_tesseract` (01-07 v6): `memory_reservation_bytes` × (1 + 1
/// discendente) + `out_quota_bytes`, dal profilo `ocr_tesseract` se c'è, altrimenti `default`.
pub(crate) fn ocr_tesseract_reservation_bytes(limits: &Value) -> u64 {
    let p = limits
        .get("ocr_tesseract")
        .filter(|v| v.is_object())
        .unwrap_or(&limits["default"]);
    let mrb = p["memory_reservation_bytes"].as_u64().unwrap_or(u64::MAX);
    let oq = p["out_quota_bytes"].as_u64().unwrap_or(u64::MAX);
    mrb.saturating_mul(2).saturating_add(oq)
}

/// Osservazioni di un job ammesso.
#[derive(Debug, Clone)]
pub(crate) struct JobObs {
    pub peak_rss_kb: Option<u64>,
    pub leftover_procs: u32,
}

/// Osservazioni di un batch `parse-parent batch --concurrency N`.
#[derive(Debug, Clone)]
pub(crate) struct BatchObs {
    pub parent_alive: bool,
    pub parent_exit: i32,
    pub batch_summary: bool,
    /// `memory.max` del cgroup del worker; `None` = illimitato.
    pub cgroup_memory_max_bytes: Option<u64>,
    pub oom_kill_delta: Option<u64>,
    pub job_errors: u32,
    /// Prenotazione registrata dal batch, in KiB (`bytes / 1024`, come 01-18).
    pub reservation_kb: u64,
    pub jobs: Vec<JobObs>,
}

/// Memoria del worker su cui vale la conformità (D-28).
pub(crate) const WORKER_MEMORY_MAX: u64 = 2 * 1024 * 1024 * 1024;

/// `m10_pass` di una riga di `ocr_throughput.csv`, con la prenotazione ricalcolata dai limiti
/// correnti (`limits-ocr_tesseract.json`).
pub(crate) fn m10_pass(b: &BatchObs, limits: &Value) -> bool {
    let want_kb = ocr_tesseract_reservation_bytes(limits) / 1024;
    b.parent_alive
        && b.parent_exit == 0
        && b.batch_summary
        && b.cgroup_memory_max_bytes == Some(WORKER_MEMORY_MAX)
        && b.oom_kill_delta == Some(0)
        && b.job_errors == 0
        && b.reservation_kb == want_kb
        && !b.jobs.is_empty()
        && b.jobs.iter().all(|j| {
            j.leftover_procs == 0
                && j.peak_rss_kb
                    .is_some_and(|p| p > 0 && b.reservation_kb >= p)
        })
}

/// Concorrenza OCR pubblicata: il più grande N con batch conforme, limitato dal `max_admitted`
/// misurato. Righe `(requested_concurrency, max_admitted, m10_pass)`.
pub(crate) fn published_concurrency(rows: &[(u32, u32, bool)]) -> Option<u32> {
    let mut by_n: BTreeMap<u32, (bool, u32)> = BTreeMap::new();
    for &(n, admitted, pass) in rows {
        let e = by_n.entry(n).or_insert((true, u32::MAX));
        e.0 &= pass;
        e.1 = e.1.min(admitted);
    }
    by_n.into_iter()
        .filter(|(_, (ok, _))| *ok)
        .map(|(n, (_, admitted))| n.min(admitted))
        .max()
}

// ---- PaddleOCR: selezione v6, famiglia, pesi, sessione ------------------------------------

/// Argomenti della CLI dello sfidante da `v6.selection.json`: `--ocr_version <v>`, il flag di
/// variante/tier se presente (`tier: {"flag","value"}`), `--lang <l>`. `selectable=false`,
/// valori assenti o non v6 → `Err` con il motivo (nessun ripiego su v5).
pub(crate) fn v6_cli_args(selection: &Value) -> Result<Vec<String>, String> {
    if selection["selectable"].as_bool() != Some(true) {
        return Err(format!(
            "v6 non selezionabile: {}",
            selection["reason"]
                .as_str()
                .unwrap_or("selectable assente o false")
        ));
    }
    let ver = selection["ocr_version"]
        .as_str()
        .filter(|v| v.starts_with("PP-OCRv6"))
        .ok_or("ocr_version assente o non PP-OCRv6: nessun ripiego")?;
    let lang = selection["lang"]
        .as_str()
        .filter(|l| !l.trim().is_empty())
        .ok_or("lang assente: italiano non coperto da un --lang esplicito")?;
    let mut out = vec!["--ocr_version".to_owned(), ver.to_owned()];
    match &selection["tier"] {
        Value::Null => {}
        t => match (
            t["flag"].as_str().filter(|f| f.starts_with("--")),
            t["value"].as_str().filter(|v| !v.is_empty()),
        ) {
            (Some(f), Some(v)) => {
                out.push(f.to_owned());
                out.push(v.to_owned());
            }
            _ => return Err("tier presente ma senza flag e valore letterali".into()),
        },
    }
    out.push("--lang".to_owned());
    out.push(lang.to_owned());
    Ok(out)
}

/// Argomenti fissi del comando dello sfidante scritti nel piano 01-13 (Task 3), dopo quelli
/// di `v6_cli_args`: niente orientamento né unwarping, CPU.
pub(crate) const V6_PLAN_FIXED_ARGS: [&str; 8] = [
    "--use_doc_orientation_classify",
    "False",
    "--use_doc_unwarping",
    "False",
    "--use_textline_orientation",
    "False",
    "--device",
    "cpu",
];

/// Detection ridotta dello sfidante (decisione utente 2026-10-25, opzione a, un solo
/// tentativo): oneDNN spento per il difetto PIR di paddlepaddle 3.3.1 e lato lungo
/// dell'ingresso della detection portato a 960 px (`max`), per stare nei 2 GiB del servizio
/// `paddle`. Svantaggio dichiarato per lo sfidante sul testo piccolo; fissato prima della misura.
pub(crate) const V6_REDUCED_DET_ARGS: [&str; 6] = [
    "--enable_mkldnn",
    "False",
    "--text_det_limit_type",
    "max",
    "--text_det_limit_side_len",
    "960",
];

/// Argomenti completi della misura v6 (senza `-i` e `--save_path`, che dipendono dalla pagina):
/// selezione letterale, poi gli argomenti fissi del piano, poi la detection ridotta.
pub(crate) fn v6_measure_args(selection: &Value) -> Result<Vec<String>, String> {
    let mut out = v6_cli_args(selection)?;
    out.extend(
        V6_PLAN_FIXED_ARGS
            .iter()
            .chain(V6_REDUCED_DET_ARGS.iter())
            .map(|s| (*s).to_owned()),
    );
    Ok(out)
}

/// Famiglia di `v6.resolved.json`: `det.model_name` e `rec.model_name` con prefisso `PP-OCRv6_`.
pub(crate) fn v6_family_check(resolved: &Value) -> Result<(), String> {
    for part in ["det", "rec"] {
        match resolved[part]["model_name"].as_str() {
            Some(n) if n.starts_with("PP-OCRv6_") => {}
            Some(n) => {
                return Err(format!(
                    "{part}: model_name {n} fuori dalla famiglia PP-OCRv6_"
                ));
            }
            None => return Err(format!("{part}: model_name non esposto")),
        }
    }
    Ok(())
}

/// Righe `<64 hex>  <percorso>` di un manifest SHA-256, per percorso.
fn parse_manifest(text: &str, what: &str) -> Result<BTreeMap<String, String>, String> {
    let mut m = BTreeMap::new();
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let (h, p) = l
            .split_once("  ")
            .ok_or(format!("{what}: riga senza separatore"))?;
        if h.len() != 64 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("{what}: hash non sha256 per {p}"));
        }
        if m.insert(p.to_owned(), h.to_ascii_lowercase()).is_some() {
            return Err(format!("{what}: percorso ripetuto {p}"));
        }
    }
    if m.is_empty() {
        return Err(format!("{what}: manifest vuoto"));
    }
    Ok(m)
}

/// Manifest dei pesi nell'immagine contro `paddle-weights.sha256` committato: righe
/// `<64 hex>  <percorso>` ordinate per percorso, insiemi e hash identici.
pub(crate) fn weights_manifest_check(image: &str, committed: &str) -> Result<(), String> {
    let (a, b) = (
        parse_manifest(image, "immagine")?,
        parse_manifest(committed, "committato")?,
    );
    for (p, h) in &b {
        match a.get(p) {
            None => return Err(format!("file mancante nell'immagine: {p}")),
            Some(x) if x != h => return Err(format!("hash diverso: {p}")),
            _ => {}
        }
    }
    if let Some(p) = a.keys().find(|p| !b.contains_key(*p)) {
        return Err(format!("file in più nell'immagine: {p}"));
    }
    Ok(())
}

/// Configurazione risolta della sessione contro quella di build: stessi `model_name`, stesse
/// directory e stessi file di det e rec, famiglia `PP-OCRv6_`; modelli non esposti → `Err`.
pub(crate) fn session_matches_build(build: &Value, session: &Value) -> Result<(), String> {
    v6_family_check(build).map_err(|e| format!("build: {e}"))?;
    v6_family_check(session).map_err(|e| format!("sessione: {e}"))?;
    for part in ["det", "rec"] {
        for k in ["model_name", "model_dir", "files"] {
            let (a, b) = (&build[part][k], &session[part][k]);
            if a.is_null() || a != b {
                return Err(format!("{part}.{k} diverso fra build e sessione"));
            }
        }
    }
    Ok(())
}

// ---- I/O comune dei sottocomandi -------------------------------------------------------------

/// Voce della mappa dei job `ocr_tesseract` scritta da `tess-jobs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TessItem {
    pub job_id: String,
    pub set: String,
    pub doc: String,
    pub oracle_id: Option<String>,
    pub page: u32,
    pub quality: String,
    pub rep: u32,
    /// PNG d'ingresso (percorso del worker).
    pub png: String,
    /// Trasformazione registrata (percorso del worker): `.render.json` o `.transform.json`.
    pub transform: String,
}

/// Percorso del worker (`/results/raw/...`) rimappato sulla cartella vista dal runner.
pub(crate) fn host_path(worker: &str, results_raw: &Path) -> PathBuf {
    match worker.strip_prefix("/results/raw/") {
        Some(rest) => results_raw.join(rest),
        None => PathBuf::from(worker),
    }
}

pub(crate) fn read_json(p: &Path) -> anyhow::Result<Value> {
    let t = std::fs::read_to_string(p).with_context(|| format!("{}", p.display()))?;
    serde_json::from_str(&t).with_context(|| format!("JSON {}", p.display()))
}

/// `RunReport` dei file JSONL di `parse-parent batch`/`run`, per `job_id` (righe non
/// `RunReport` ignorate; `job_error` contate a parte dal chiamante).
pub(crate) fn read_reports(paths: &[PathBuf]) -> anyhow::Result<BTreeMap<String, Value>> {
    let mut out = BTreeMap::new();
    for p in paths {
        let t = std::fs::read_to_string(p).with_context(|| format!("{}", p.display()))?;
        for l in t.lines().filter(|l| l.trim_start().starts_with('{')) {
            let v: Value =
                serde_json::from_str(l).with_context(|| format!("riga di {}", p.display()))?;
            if v.get("outcome").is_some()
                && let Some(id) = v["job_id"].as_str()
            {
                out.insert(id.to_owned(), v);
            }
        }
    }
    Ok(out)
}

/// Trasformazione di una voce: `.transform.json` (degradata) o `.render.json` (buona, rotazione 0).
pub(crate) fn load_transform(path: &Path) -> anyhow::Result<Transform> {
    let v = read_json(path)?;
    if v.get("rotation_deg").is_some() {
        return serde_json::from_value(v).context("transform.json");
    }
    let (w, h) = (
        v["width_px"].as_u64().context("width_px")? as u32,
        v["height_px"].as_u64().context("height_px")? as u32,
    );
    Ok(Transform {
        dpi: v["dpi"].as_f64().context("dpi")?,
        width_px: w,
        height_px: h,
        page_h_pt: v["page_h_pt"].as_f64().context("page_h_pt")?,
        rotation_deg: 0.0,
        center_px: (f64::from(w) / 2.0, f64::from(h) / 2.0),
    })
}

/// Parole di un `RunReport` `ok` di `ocr_tesseract` (`data.words`).
pub(crate) fn report_words(r: &Value) -> anyhow::Result<Vec<Word>> {
    if r["outcome"]["status"].as_str() != Some("ok") {
        bail!("job {} non ok: {}", r["job_id"], r["outcome"]);
    }
    serde_json::from_value(r["outcome"]["data"]["words"].clone()).context("data.words")
}

/// Bbox in pixel `[left, top, right, bottom]` di una parola.
pub(crate) fn word_px(w: &Word) -> [f64; 4] {
    [
        f64::from(w.left),
        f64::from(w.top),
        f64::from(w.left + w.width),
        f64::from(w.top + w.height),
    ]
}

/// Apre un CSV in append; scrive l'header se il file è nuovo o vuoto, rifiuta un header diverso.
pub(crate) fn csv_append(path: &Path, header: &str) -> anyhow::Result<std::fs::File> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    if let Some(first) = existing.lines().next()
        && first != header
    {
        bail!("{}: header diverso da {header}", path.display());
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("{}", path.display()))?;
    if existing.is_empty() {
        writeln!(f, "{header}")?;
    }
    Ok(f)
}

/// Campo CSV senza virgole né a capo (gli header fissi del piano non usano virgolette).
pub(crate) fn csv_field(s: &str) -> String {
    s.replace([',', '\n', '\r'], ";")
}

fn f4(v: f64) -> String {
    format!("{v:.4}")
}

// ---- `score cer` e `score bbox` -------------------------------------------------------------

pub(crate) const OCR_HEADER: &str =
    "engine,version,doc_id,page,scan_quality,run,chars_expected,cer,seconds";
pub(crate) const BBOX_HEADER: &str = "engine,reference,doc_id,page,scan_quality,words_matched,iou_median,share_iou_ge_05,corner_err_p95_pt,verdict";

#[derive(clap::Parser, Debug)]
#[command(name = "ocr score")]
struct ScoreCli {
    #[command(subcommand)]
    cmd: ScoreCmd,
}

#[derive(clap::Subcommand, Debug)]
enum ScoreCmd {
    /// CER delle pagine dell'oracolo (righe di `ocr.csv`).
    Cer {
        #[arg(long)]
        map: PathBuf,
        /// JSONL dei `RunReport` di `ocr_tesseract` (motore Tesseract).
        #[arg(
            long,
            required_unless_present = "paddle_dir",
            conflicts_with = "paddle_dir"
        )]
        runs: Vec<PathBuf>,
        /// Cartella delle uscite di PaddleOCR per `job_id` della mappa (`<job_id>/*_res.json`
        /// e `<job_id>.wall_ms`), vista dal runner: sostituisce `--runs`.
        #[arg(long)]
        paddle_dir: Option<PathBuf>,
        #[arg(long)]
        oracle: PathBuf,
        #[arg(long, default_value = "tesseract")]
        engine: String,
        #[arg(long)]
        version: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Fedeltà delle bbox (righe di `bbox_map.csv`).
    Bbox {
        #[arg(long, value_parser = ["analytic", "pdfium"])]
        reference: String,
        #[arg(long)]
        map: PathBuf,
        #[arg(
            long,
            required_unless_present = "paddle_dir",
            conflicts_with = "paddle_dir"
        )]
        runs: Vec<PathBuf>,
        /// Uscite di PaddleOCR come in `cer`: sostituisce `--runs`.
        #[arg(long)]
        paddle_dir: Option<PathBuf>,
        #[arg(long)]
        results_raw: PathBuf,
        /// Ripetizione usata per le bbox (una sola, uguale per ogni pagina).
        #[arg(long)]
        rep: u32,
        /// `geom-cases.json` di `gen_geom.py` (riferimento analitico).
        #[arg(long)]
        cases: Option<PathBuf>,
        /// Indice della run (`ocr-index/<run_id>.json`) con l'`out_dir` di `extract_pdf` per documento.
        #[arg(long)]
        index: Option<PathBuf>,
        /// Esito della soglia IoU di 01-08: `pass`, `fail` o `absent`.
        #[arg(long, value_parser = ["pass", "fail", "absent"])]
        pdfium_gate: Option<String>,
        /// `bbox_iou.csv` di 01-08: plafond riportato accanto.
        #[arg(long)]
        plafond: Option<PathBuf>,
        #[arg(long, default_value = "tesseract")]
        engine: String,
        #[arg(long)]
        out: PathBuf,
    },
}

/// `ocr score …`
pub(crate) fn run_score(args: Vec<std::ffi::OsString>) -> anyhow::Result<()> {
    use clap::Parser as _;
    let cli = ScoreCli::try_parse_from(std::iter::once("ocr-score".into()).chain(args))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        ScoreCmd::Cer {
            map,
            runs,
            paddle_dir,
            oracle,
            engine,
            version,
            out,
        } => {
            let reports = read_reports(&runs)?;
            let src = match &paddle_dir {
                Some(d) => Source::Paddle(d),
                None => Source::Tesseract(&reports),
            };
            score_cer(&map, &src, &oracle, &engine, &version, &out)
        }
        ScoreCmd::Bbox {
            reference,
            map,
            runs,
            paddle_dir,
            results_raw,
            rep,
            cases,
            index,
            pdfium_gate,
            plafond,
            engine,
            out,
        } => {
            let items = load_map(&map)?;
            let reports = read_reports(&runs)?;
            let src = match &paddle_dir {
                Some(d) => Source::Paddle(d),
                None => Source::Tesseract(&reports),
            };
            let mut f = csv_append(&out, BBOX_HEADER)?;
            let rows = if reference == "analytic" {
                bbox_analytic(
                    &items,
                    &src,
                    &results_raw,
                    rep,
                    &cases.context("--cases richiesto da analytic")?,
                )?
            } else {
                let gate = match pdfium_gate.as_deref() {
                    Some("pass") => Some(true),
                    Some("fail") => Some(false),
                    _ => None,
                };
                if let Some(p) = &plafond {
                    println!("{}", plafond_line(p)?);
                }
                bbox_pdfium(
                    &items,
                    &src,
                    &results_raw,
                    rep,
                    &index.context("--index richiesto da pdfium")?,
                    gate,
                )?
            };
            for (doc, page, q, m, v) in rows {
                writeln!(
                    f,
                    "{engine},{reference},{doc},{page},{},{},{},{},{},{}",
                    q.as_str(),
                    m.words_matched,
                    f4(m.iou_median),
                    f4(m.share_iou_ge_05),
                    f4(m.corner_err_p95_pt),
                    v.as_str()
                )?;
                println!(
                    "bbox {reference} {doc} p{page} {}: parole {} iou_med {:.3} p95 {:.2} pt → {}",
                    q.as_str(),
                    m.words_matched,
                    m.iou_median,
                    m.corner_err_p95_pt,
                    v.as_str()
                );
            }
            Ok(())
        }
    }
}

pub(crate) fn load_map(p: &Path) -> anyhow::Result<Vec<TessItem>> {
    serde_json::from_value(read_json(p)?).context("mappa dei job tesseract")
}

fn score_cer(
    map: &Path,
    src: &Source<'_>,
    oracle: &Path,
    engine: &str,
    version: &str,
    out: &Path,
) -> anyhow::Result<()> {
    let items = load_map(map)?;
    let entries: Vec<Value> = serde_json::from_value(read_json(oracle)?).context("oracolo")?;
    let (usable, excluded) = usable_oracles(&entries);
    let by_id: BTreeMap<&str, &Value> = usable
        .iter()
        .filter_map(|e| e["oracle_id"].as_str().map(|id| (id, *e)))
        .collect();
    let mut f = csv_append(out, OCR_HEADER)?;
    let mut n = 0;
    for it in items.iter().filter(|i| i.set == "oracle") {
        let Some(oid) = it.oracle_id.as_deref() else {
            continue;
        };
        let Some(entry) = by_id.get(oid) else {
            continue;
        };
        let expected = entry["expected_value"]
            .as_str()
            .context("expected_value non testuale")?;
        let (text, secs) = src.text_secs(it)?;
        let c = cer(expected, &text);
        writeln!(
            f,
            "{engine},{version},{},{},{},{},{},{},{:.3}",
            it.doc,
            it.page,
            it.quality,
            it.rep,
            normalize_text(expected).chars().count(),
            f4(c),
            secs
        )?;
        n += 1;
    }
    println!("righe ocr.csv: {n}; oracoli parser-it pendenti esclusi e contati: {excluded}");
    Ok(())
}

/// Casi di `gen_geom.py`: per documento, parole con rettangolo d'inchiostro atteso in pt.
fn geom_cases(p: &Path) -> anyhow::Result<BTreeMap<String, Vec<(String, [f64; 4])>>> {
    let v = read_json(p)?;
    let mut out = BTreeMap::new();
    for page in v["pages"].as_array().context("pages")? {
        let doc = page["doc"].as_str().context("doc")?.to_owned();
        let mut words = Vec::new();
        for w in page["words"].as_array().context("words")? {
            let r: [f64; 4] = serde_json::from_value(w["rect"].clone()).context("rect")?;
            words.push((w["text"].as_str().context("text")?.to_owned(), r));
        }
        out.insert(doc, words);
    }
    Ok(out)
}

type BboxRow = (String, u32, Quality, BboxMetrics, Verdict);

fn ocr_words_pt(
    it: &TessItem,
    src: &Source<'_>,
    results_raw: &Path,
) -> anyhow::Result<Vec<(String, [f64; 4])>> {
    let t = load_transform(&host_path(&it.transform, results_raw))?;
    Ok(src
        .boxes_px(it)?
        .into_iter()
        .map(|(text, px)| (text, geom::map_bbox(&t, px)))
        .collect())
}

fn bbox_analytic(
    items: &[TessItem],
    reports: &Source<'_>,
    results_raw: &Path,
    rep: u32,
    cases: &Path,
) -> anyhow::Result<Vec<BboxRow>> {
    let cases = geom_cases(cases)?;
    let mut rows = Vec::new();
    for it in items.iter().filter(|i| i.set == "geom" && i.rep == rep) {
        let expected = cases
            .get(&it.doc)
            .with_context(|| format!("caso geometrico assente: {}", it.doc))?;
        let got = ocr_words_pt(it, reports, results_raw)?;
        let q = Quality::parse(&it.quality)?;
        let m = bbox_metrics(&match_words(expected, &got));
        let v = bbox_verdict(Reference::Analytic, q, &m, None);
        rows.push((it.doc.clone(), it.page, q, m, v));
    }
    Ok(rows)
}

fn bbox_pdfium(
    items: &[TessItem],
    reports: &Source<'_>,
    results_raw: &Path,
    rep: u32,
    index: &Path,
    gate: Option<bool>,
) -> anyhow::Result<Vec<BboxRow>> {
    let idx = read_json(index)?;
    let mut extract_dirs: BTreeMap<String, String> = BTreeMap::new();
    for e in idx["extract"].as_array().context("indice senza extract")? {
        extract_dirs.insert(
            e["doc"].as_str().context("doc")?.to_owned(),
            e["out_dir"].as_str().context("out_dir")?.to_owned(),
        );
    }
    let mut rows = Vec::new();
    for it in items
        .iter()
        .filter(|i| i.set != "geom" && i.rep == rep && extract_dirs.contains_key(&i.doc))
    {
        let dir = host_path(&extract_dirs[&it.doc], results_raw);
        let page = read_json(&dir.join(format!("page-{:04}.json", it.page)))?;
        let mut expected = Vec::new();
        for w in page["words"].as_array().context("words di extract_pdf")? {
            let r: [f64; 4] = serde_json::from_value(w["bbox"].clone()).context("bbox")?;
            expected.push((w["text"].as_str().unwrap_or_default().to_owned(), r));
        }
        let got = ocr_words_pt(it, reports, results_raw)?;
        let q = Quality::parse(&it.quality)?;
        let m = bbox_metrics(&match_words(&expected, &got));
        let v = bbox_verdict(Reference::Pdfium, q, &m, gate);
        rows.push((it.doc.clone(), it.page, q, m, v));
    }
    Ok(rows)
}

/// Plafond di 01-08: media di `iou_mean` e di `share_iou_ge_05` su `bbox_iou.csv`.
fn plafond_line(p: &Path) -> anyhow::Result<String> {
    let t = std::fs::read_to_string(p).with_context(|| format!("{}", p.display()))?;
    let mut lines = t.lines();
    let header: Vec<&str> = lines
        .next()
        .context("bbox_iou.csv vuoto")?
        .split(',')
        .collect();
    let col = |k: &str| header.iter().position(|h| *h == k);
    let (Some(a), Some(b)) = (col("iou_mean"), col("share_iou_ge_05")) else {
        bail!("bbox_iou.csv senza iou_mean o share_iou_ge_05");
    };
    let (mut sa, mut sb, mut n) = (0.0, 0.0, 0u32);
    for l in lines.filter(|l| !l.is_empty()) {
        let f: Vec<&str> = l.split(',').collect();
        if let (Some(x), Some(y)) = (
            f.get(a).and_then(|v| v.parse::<f64>().ok()),
            f.get(b).and_then(|v| v.parse::<f64>().ok()),
        ) {
            sa += x;
            sb += y;
            n += 1;
        }
    }
    Ok(format!(
        "plafond 01-08 (bbox_iou.csv, {n} pagine): iou_mean medio {:.3}, share_iou_ge_05 medio {:.3}",
        sa / f64::from(n.max(1)),
        sb / f64::from(n.max(1))
    ))
}

// ---- `hostile-verdict` ----------------------------------------------------------------------

pub(crate) const HOSTILE_HEADER: &str = "case_id,run,expected,outcome,kind,detail,peak_rss_kb,wall_ms,parent_alive,leftover_procs,oom_kill_delta,oom_adj_set,pass";

#[derive(clap::Parser, Debug)]
#[command(name = "ocr hostile-verdict")]
struct HostileCli {
    /// Manifest dei casi (01-18 e 1.5): campi `case_id`, `expected_status`,
    /// `expected_kind_or_reasons` e, se c'è, `expected_detail`.
    #[arg(long, required = true)]
    manifest: Vec<PathBuf>,
    /// Cartella con `<job_id>.stdout` e `<job_id>.meta` di `run_hostile_render.sh`.
    #[arg(long)]
    reports: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

/// Atteso di un caso dai manifest.
#[derive(Debug, Clone)]
struct Expect {
    status: String,
    kinds: Vec<String>,
    detail_prefix: Option<String>,
}

/// `ocr hostile-verdict …`
pub(crate) fn run_hostile_verdict(args: Vec<std::ffi::OsString>) -> anyhow::Result<()> {
    use clap::Parser as _;
    let cli = HostileCli::try_parse_from(std::iter::once("ocr-hostile-verdict".into()).chain(args))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut expect: BTreeMap<String, Expect> = BTreeMap::new();
    for m in &cli.manifest {
        let cases: Vec<Value> = serde_json::from_value(read_json(m)?).context("manifest")?;
        for c in cases {
            let id = c["case_id"].as_str().context("case_id")?.to_owned();
            expect.insert(
                id,
                Expect {
                    status: c["expected_status"]
                        .as_str()
                        .context("expected_status")?
                        .to_owned(),
                    kinds: serde_json::from_value(c["expected_kind_or_reasons"].clone())
                        .context("expected_kind_or_reasons")?,
                    detail_prefix: c["expected_detail"].as_str().map(str::to_owned),
                },
            );
        }
    }
    let mut metas: Vec<PathBuf> = std::fs::read_dir(&cli.reports)
        .with_context(|| format!("{}", cli.reports.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "meta"))
        .collect();
    metas.sort();
    let mut rows: Vec<(String, u32, Vec<String>, (bool, bool, u32))> = Vec::new();
    for meta in &metas {
        // `.meta`: righe `chiave<TAB>valore` (`case_id`, `run`, `parent_exit`, `worker_alive`).
        let text = std::fs::read_to_string(meta)?;
        let kv: BTreeMap<&str, &str> = text
            .lines()
            .filter_map(|l| l.trim_end_matches('\r').split_once('\t'))
            .collect();
        let case = kv.get("case_id").context("meta senza case_id")?.to_string();
        let run: u32 = kv.get("run").context("meta senza run")?.parse()?;
        let exit: i32 = kv
            .get("parent_exit")
            .context("meta senza parent_exit")?
            .parse()?;
        let worker_alive = kv.get("worker_alive") == Some(&"true");
        let e = expect
            .get(&case)
            .with_context(|| format!("caso {case} assente dai manifest"))?;
        let stdout = std::fs::read_to_string(meta.with_extension("stdout")).unwrap_or_default();
        let report = stdout
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .find(|v| v.get("outcome").is_some());
        let obs = HostileObs {
            report: report.clone(),
            // Un exit ≥ 128 è un processo terminato da un segnale (convenzione della shell).
            parent_exit: (0..128).contains(&exit).then_some(exit),
            worker_alive,
        };
        let (mut pass, alive) = hostile_row(&e.status, &e.kinds, &obs);
        let r = report.unwrap_or(Value::Null);
        let o = &r["outcome"];
        let detail = o["detail"].as_str().unwrap_or_default();
        if let Some(prefix) = &e.detail_prefix {
            pass &= detail.starts_with(prefix.as_str());
        }
        let kind = match o["status"].as_str() {
            Some("quarantined") => o["reasons"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("|")
                })
                .unwrap_or_default(),
            _ => o["kind"].as_str().unwrap_or_default().to_owned(),
        };
        let leftover = r["leftover_procs"].as_u64().map(|v| v as u32);
        let opt = |v: &Value| v.as_u64().map(|x| x.to_string()).unwrap_or_default();
        rows.push((
            case.clone(),
            run,
            vec![
                case.clone(),
                run.to_string(),
                format!("{}/{}", e.status, e.kinds.join("|")),
                o["status"].as_str().unwrap_or("absent").to_owned(),
                kind,
                csv_field(detail),
                opt(&r["peak_rss_kb"]),
                opt(&r["wall_ms"]),
                alive.to_string(),
                leftover.map(|v| v.to_string()).unwrap_or_default(),
                opt(&r["oom_kill_delta"]),
                r["oom_adj_set"].as_bool().unwrap_or(false).to_string(),
                pass.to_string(),
            ],
            (pass, alive, leftover.unwrap_or(u32::MAX)),
        ));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut f =
        std::fs::File::create(&cli.out).with_context(|| format!("{}", cli.out.display()))?;
    writeln!(f, "{HOSTILE_HEADER}")?;
    let mut by_case: BTreeMap<String, Vec<(bool, bool, u32)>> = BTreeMap::new();
    for (case, _, fields, t) in &rows {
        writeln!(f, "{}", fields.join(","))?;
        by_case.entry(case.clone()).or_default().push(*t);
    }
    for (case, t) in &by_case {
        let passed = t.iter().filter(|r| r.0).count();
        println!("{case}: {passed}/{} pass", t.len());
    }
    let jb = by_case.get("jbig2-malformed").cloned().unwrap_or_default();
    println!("residuo T-09 JBIG2: {}", t09_status(&jb).as_str());
    Ok(())
}

// ---- `throughput-row` -----------------------------------------------------------------------

pub(crate) const THROUGHPUT_HEADER: &str = "engine,version,pages,requested_concurrency,max_admitted,run,wall_s,s_per_page,failed_jobs,oom_kill_delta,job_errors,leftover_procs_max,reservation_kb,peak_rss_kb_max,reserve_peak_ratio_min,parent_exit,m10_pass";

#[derive(clap::Parser, Debug)]
#[command(name = "ocr throughput-row")]
struct ThroughputCli {
    /// Stdout di `parse-parent batch` (righe `RunReport`, `job_error`, `batch_summary`).
    #[arg(long)]
    runs: PathBuf,
    /// `limits-ocr_tesseract.json` passato al batch.
    #[arg(long)]
    limits: PathBuf,
    /// Contenuto di `memory.max` del cgroup del worker (`max` = illimitato).
    #[arg(long)]
    memory_max: String,
    /// `oom_kill` di `memory.events` prima e dopo il batch.
    #[arg(long)]
    oom_before: u64,
    #[arg(long)]
    oom_after: u64,
    #[arg(long)]
    parent_exit: i32,
    #[arg(long)]
    concurrency: u32,
    #[arg(long)]
    run: u32,
    /// Parete esterna del batch (ms).
    #[arg(long)]
    wall_ms: u64,
    #[arg(long, default_value = "tesseract")]
    engine: String,
    #[arg(long)]
    version: String,
    #[arg(long)]
    out: PathBuf,
}

/// `ocr throughput-row …`: una riga di `ocr_throughput.csv` con `m10_pass`.
pub(crate) fn run_throughput_row(args: Vec<std::ffi::OsString>) -> anyhow::Result<()> {
    use clap::Parser as _;
    let cli =
        ThroughputCli::try_parse_from(std::iter::once("ocr-throughput-row".into()).chain(args))
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    let limits = read_json(&cli.limits)?;
    let text =
        std::fs::read_to_string(&cli.runs).with_context(|| format!("{}", cli.runs.display()))?;
    let (mut jobs, mut job_errors, mut failed, mut summary) = (Vec::new(), 0u32, 0u32, None);
    for l in text.lines().filter(|l| l.trim_start().starts_with('{')) {
        let v: Value = serde_json::from_str(l)?;
        match v["status"].as_str() {
            Some("batch_summary") => summary = Some(v),
            Some("job_error") => job_errors += 1,
            _ if v.get("outcome").is_some() => {
                if v["outcome"]["status"].as_str() != Some("ok") {
                    failed += 1;
                }
                jobs.push(JobObs {
                    peak_rss_kb: v["peak_rss_kb"].as_u64(),
                    leftover_procs: v["leftover_procs"].as_u64().unwrap_or(u64::from(u32::MAX))
                        as u32,
                });
            }
            _ => {}
        }
    }
    let reservation_kb = ocr_tesseract_reservation_bytes(&limits) / 1024;
    let mem = cli.memory_max.trim();
    let b = BatchObs {
        parent_alive: summary.is_some() && (0..128).contains(&cli.parent_exit),
        parent_exit: cli.parent_exit,
        batch_summary: summary.is_some(),
        cgroup_memory_max_bytes: if mem == "max" { None } else { mem.parse().ok() },
        oom_kill_delta: cli.oom_after.checked_sub(cli.oom_before),
        job_errors: job_errors
            + summary
                .as_ref()
                .and_then(|s| s["job_errors"].as_u64())
                .map_or(0, |v| v as u32)
                .saturating_sub(job_errors),
        reservation_kb,
        jobs: jobs.clone(),
    };
    let pass = m10_pass(&b, &limits);
    let pages = jobs.len() as u64;
    let peak_max = jobs.iter().filter_map(|j| j.peak_rss_kb).max();
    let ratio_min = jobs
        .iter()
        .filter_map(|j| j.peak_rss_kb)
        .filter(|&p| p > 0)
        .map(|p| reservation_kb as f64 / p as f64)
        .fold(f64::INFINITY, f64::min);
    let admitted = summary
        .as_ref()
        .and_then(|s| s["max_admitted"].as_u64())
        .map(|v| v.to_string())
        .unwrap_or_default();
    let wall_s = cli.wall_ms as f64 / 1000.0;
    let row = [
        cli.engine.clone(),
        cli.version.clone(),
        pages.to_string(),
        cli.concurrency.to_string(),
        admitted,
        cli.run.to_string(),
        format!("{wall_s:.3}"),
        format!("{:.3}", wall_s / pages.max(1) as f64),
        failed.to_string(),
        b.oom_kill_delta.map(|v| v.to_string()).unwrap_or_default(),
        b.job_errors.to_string(),
        jobs.iter()
            .map(|j| j.leftover_procs)
            .max()
            .unwrap_or(0)
            .to_string(),
        reservation_kb.to_string(),
        peak_max.map(|v| v.to_string()).unwrap_or_default(),
        if ratio_min.is_finite() {
            format!("{ratio_min:.3}")
        } else {
            String::new()
        },
        cli.parent_exit.to_string(),
        pass.to_string(),
    ]
    .join(",");
    let mut f = csv_append(&cli.out, THROUGHPUT_HEADER)?;
    writeln!(f, "{row}")?;
    println!("{row}");
    Ok(())
}

// ---- `limits-from-runs` ---------------------------------------------------------------------

#[derive(clap::Parser, Debug)]
#[command(name = "ocr limits-from-runs")]
struct LimitsCli {
    /// Op del profilo (in allowlist, con discendenti: `ocr_tesseract`).
    #[arg(long)]
    op: String,
    /// `RunReport` dei job di calibrazione (stdout di `parse-parent batch`).
    #[arg(long)]
    runs: PathBuf,
    /// Totale del budget (`memory.max` del worker) e riserva del padre, come `calibrate`.
    #[arg(long)]
    budget_total_bytes: u64,
    #[arg(long, default_value_t = 256 * 1024 * 1024)]
    parent_reserve_bytes: u64,
    /// Massimo di byte scritti nell'`out_dir` dai job (0 per un'op con `out_dir` null).
    #[arg(long, default_value_t = 0)]
    max_out_bytes: u64,
    /// Descrizione degli input (cartella del worker), riportata in `_meta_<op>`.
    #[arg(long)]
    inputs: String,
    #[arg(long)]
    out: PathBuf,
}

/// Profilo di `limits.json` per un'op con discendenti, con la formula di 01-07 v6 applicata
/// ai `RunReport` di job benigni: `memory_reservation_bytes` = 3 × max `VmHWM`,
/// `address_space_bytes` = min(3 × max `VmPeak`, totale − riserva), `out_quota_bytes` = 2 × max
/// byte scritti, `file_size_bytes` = max(quota, 1 MiB), CPU 60 s, parete 120 s, 64 file aperti.
/// Serve perché `parse-parent calibrate` (01-07, file chiuso) prenota per i job di calibrazione
/// `(totale − riserva − 256 MiB) × (1 + discendenti) + 256 MiB`, che con un discendente supera
/// sempre il budget (`BudgetExceeded`): difetto registrato in RISULTATI-ocr.md, non corretto qui.
pub(crate) fn run_limits_from_runs(args: Vec<std::ffi::OsString>) -> anyhow::Result<()> {
    use clap::Parser as _;
    const MIB: u64 = 1024 * 1024;
    let cli = LimitsCli::try_parse_from(std::iter::once("ocr-limits-from-runs".into()).chain(args))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let text =
        std::fs::read_to_string(&cli.runs).with_context(|| format!("{}", cli.runs.display()))?;
    let (mut max_hwm, mut max_peak, mut ok_docs, mut docs) = (0u64, 0u64, 0u32, 0u32);
    let mut anomalies: Vec<String> = Vec::new();
    for l in text.lines().filter(|l| l.trim_start().starts_with('{')) {
        let v: Value = serde_json::from_str(l)?;
        if v["status"].as_str() == Some("job_error") {
            bail!("job_error nei job di calibrazione: {l}");
        }
        if v.get("outcome").is_none() {
            continue;
        }
        docs += 1;
        let id = v["job_id"].as_str().unwrap_or_default();
        if v["oom_adj_set"].as_bool() != Some(true) {
            anomalies.push(format!("{id}: oom_adj_set=false"));
        }
        if v["outcome"]["status"].as_str() != Some("ok") {
            anomalies.push(format!("{id}: {}", v["outcome"]));
            continue;
        }
        let d = &v["outcome"]["data"];
        let hwm = v["peak_rss_kb"]
            .as_u64()
            .or_else(|| d["vm_hwm_kb"].as_u64());
        let peak = d["vm_peak_kb"].as_u64();
        max_hwm = max_hwm.max(hwm.unwrap_or(0));
        max_peak = max_peak.max(peak.unwrap_or(0));
        ok_docs += 1;
    }
    if ok_docs == 0 || max_hwm == 0 || max_peak == 0 {
        bail!("nessun job riuscito con VmPeak e VmHWM: limiti non calcolabili");
    }
    let capacity = (cli
        .budget_total_bytes
        .saturating_sub(cli.parent_reserve_bytes)
        / MIB)
        * MIB;
    let quota = cli.max_out_bytes.saturating_mul(2);
    let profile = serde_json::json!({
        "address_space_bytes": max_peak.saturating_mul(1024).saturating_mul(3).min(capacity),
        "cpu_seconds": 60,
        "file_size_bytes": quota.max(MIB),
        "max_open_files": 64,
        "memory_reservation_bytes": max_hwm.saturating_mul(1024).saturating_mul(3),
        "out_quota_bytes": quota,
        "wall_timeout": {"nanos": 0, "secs": 120},
    });
    let doc = serde_json::json!({
        "default": profile,
        cli.op.clone(): profile,
        format!("_meta_{}", cli.op): {
            "formula": "memory_reservation_bytes = 3 × max VmHWM; address_space_bytes = min(3 × max VmPeak, totale − riserva del padre); out_quota_bytes = 2 × max byte scritti nell'out_dir; file_size_bytes = max(quota, 1 MiB); cpu_seconds 60, wall_timeout 120 s, max_open_files 64 fissi",
            "calibrated_by": "ocr limits-from-runs (formula di 01-07 v6 applicata ai RunReport; parse-parent calibrate rifiuta le op con discendenti con BudgetExceeded)",
            "inputs": cli.inputs,
            "docs": docs,
            "ok_docs": ok_docs,
            "max_vm_peak_kb": max_peak,
            "max_vm_hwm_kb": max_hwm,
            "budget_total_bytes": cli.budget_total_bytes,
            "parent_reserve_bytes": cli.parent_reserve_bytes,
            "max_out_bytes": cli.max_out_bytes,
            "anomalies": anomalies,
        },
    });
    std::fs::write(&cli.out, serde_json::to_string_pretty(&doc)? + "\n")
        .with_context(|| format!("{}", cli.out.display()))?;
    println!(
        "limits-from-runs {}: {ok_docs}/{docs} ok, max VmHWM {max_hwm} kB, max VmPeak {max_peak} kB, prenotazione per job {} B",
        cli.op,
        ocr_tesseract_reservation_bytes(&doc)
    );
    Ok(())
}

// ---- `paddle-check` -------------------------------------------------------------------------

#[derive(clap::Parser, Debug)]
#[command(name = "ocr paddle-check")]
struct PaddleCli {
    #[arg(long)]
    selection: PathBuf,
    #[arg(long)]
    resolved: PathBuf,
    /// Configurazione risolta della sessione (`resolved-session.json`).
    #[arg(long)]
    session: Option<PathBuf>,
    /// Manifest dei pesi letto dall'immagine e quello committato.
    #[arg(long)]
    manifest_image: Option<PathBuf>,
    #[arg(long)]
    manifest_committed: Option<PathBuf>,
    /// Stampa gli argomenti completi della misura (`v6_measure_args`) invece della sola selezione.
    #[arg(long)]
    measure: bool,
}

/// `ocr paddle-check …`: stampa gli argomenti v6 della CLI o fallisce con il motivo.
pub(crate) fn run_paddle_check(args: Vec<std::ffi::OsString>) -> anyhow::Result<()> {
    use clap::Parser as _;
    let cli = PaddleCli::try_parse_from(std::iter::once("ocr-paddle-check".into()).chain(args))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let sel = read_json(&cli.selection)?;
    let argv = if cli.measure {
        v6_measure_args(&sel)
    } else {
        v6_cli_args(&sel)
    }
    .map_err(anyhow::Error::msg)?;
    let build = read_json(&cli.resolved)?;
    v6_family_check(&build).map_err(anyhow::Error::msg)?;
    if let Some(s) = &cli.session {
        session_matches_build(&build, &read_json(s)?).map_err(anyhow::Error::msg)?;
    }
    if let (Some(a), Some(b)) = (&cli.manifest_image, &cli.manifest_committed) {
        weights_manifest_check(&std::fs::read_to_string(a)?, &std::fs::read_to_string(b)?)
            .map_err(anyhow::Error::msg)?;
    }
    println!("{}", argv.join(" "));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ---- TEST-9: CER, oracoli, bbox ----------------------------------------------------------

    #[test]
    fn test9_cer_on_normalized_text() {
        assert_eq!(cer("abc", "abc"), 0.0);
        assert!((cer("abcd", "abxd") - 0.25).abs() < 1e-12);
        assert!((cer("Totale  attivo\n2026", " Totale attivo 2026 ") - 0.0).abs() < 1e-12);
        // NFC: «è» composto e decomposto sono lo stesso carattere.
        assert_eq!(cer("perché", "perche\u{301}"), 0.0);
        assert!((cer("ab", "") - 1.0).abs() < 1e-12);
    }

    fn oracle(gpt: &str, user: &str) -> Value {
        json!({"oracle_id":"P-x","expected_value":"t","guardrail":{"claude":"concordo","gpt":gpt,"user":user}})
    }

    #[test]
    fn test9_only_three_green_oracles_are_scored_and_others_counted() {
        assert!(three_green(&oracle("CONFIRMED", "ratified")));
        assert!(!three_green(&oracle("PENDING", "ratified")));
        assert!(!three_green(&oracle("CONFIRMED", "pending")));
        assert!(!three_green(
            &json!({"guardrail":{"claude":null,"gpt":"CONFIRMED","user":"ratified"}})
        ));
        let e = vec![
            oracle("CONFIRMED", "ratified"),
            oracle("REJECTED", "ratified"),
            oracle("CONFIRMED", ""),
        ];
        let (ok, excluded) = usable_oracles(&e);
        assert_eq!((ok.len(), excluded), (1, 2));
    }

    #[test]
    fn test9_paddle_text_from_rec_texts() {
        let out =
            json!({"res":{"rec_texts":["Totale","attivo"],"rec_boxes":[[0,0,1,1],[2,0,3,1]]}});
        assert_eq!(paddle_text(&out).unwrap(), "Totale attivo");
        assert!(paddle_text(&json!({"res":{}})).is_err());
    }

    // Percorso PP-OCRv6 di `score` (continuazione paddle, 2026-10-25): righe e rettangoli
    // dall'output salvato, tempo dell'invocazione singola dal file `.wall_ms`.
    #[test]
    fn test9_paddle_boxes_pair_texts_and_boxes_or_fail() {
        let top = json!({"rec_texts":["ATTIVO","2026"],"rec_boxes":[[10,20,110,44],[5,6,7,8]]});
        assert_eq!(
            paddle_boxes(&top).unwrap(),
            vec![
                ("ATTIVO".to_owned(), [10.0, 20.0, 110.0, 44.0]),
                ("2026".to_owned(), [5.0, 6.0, 7.0, 8.0])
            ]
        );
        let nested = json!({"res":{"rec_texts":["A"],"rec_boxes":[[0,0,1,1]]}});
        assert_eq!(paddle_boxes(&nested).unwrap().len(), 1);
        assert!(paddle_boxes(&json!({"rec_texts":["A","B"],"rec_boxes":[[0,0,1,1]]})).is_err());
        assert!(paddle_boxes(&json!({"rec_texts":["A"],"rec_boxes":[[0,0,1]]})).is_err());
        assert!(paddle_boxes(&json!({"rec_texts":["A"]})).is_err());
    }

    fn paddle_fixture(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "cka-ocr-paddle-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("job-1")).unwrap();
        d
    }

    #[test]
    fn test9_paddle_page_reads_single_res_json_and_wall_ms() {
        let d = paddle_fixture("ok");
        std::fs::write(
            d.join("job-1/page-0001_res.json"),
            r#"{"rec_texts":["Totale","attivo"],"rec_boxes":[[0,0,1,1],[2,0,3,1]]}"#,
        )
        .unwrap();
        std::fs::write(d.join("job-1.wall_ms"), "4321\n").unwrap();
        let p = load_paddle_page(&d, "job-1").unwrap();
        assert_eq!(p.text, "Totale attivo");
        assert_eq!(p.wall_ms, 4321);
        assert_eq!(p.boxes_px.len(), 2);
        let it = TessItem {
            job_id: "job-1".into(),
            set: "oracle".into(),
            doc: "p-001".into(),
            oracle_id: Some("P-001".into()),
            page: 1,
            quality: "good".into(),
            rep: 2,
            png: String::new(),
            transform: String::new(),
        };
        let (text, secs) = Source::Paddle(&d).text_secs(&it).unwrap();
        assert_eq!(text, "Totale attivo");
        assert!((secs - 4.321).abs() < 1e-9);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn test9_paddle_page_rejects_missing_time_or_ambiguous_output() {
        let d = paddle_fixture("bad");
        // Nessun *_res.json: invocazione fallita, mai una pagina vuota.
        assert!(load_paddle_page(&d, "job-1").is_err());
        std::fs::write(
            d.join("job-1/a_res.json"),
            r#"{"rec_texts":[],"rec_boxes":[]}"#,
        )
        .unwrap();
        // Tempo assente: la riga non si scrive.
        assert!(load_paddle_page(&d, "job-1").is_err());
        std::fs::write(d.join("job-1.wall_ms"), "abc").unwrap();
        assert!(load_paddle_page(&d, "job-1").is_err());
        std::fs::write(d.join("job-1.wall_ms"), "10").unwrap();
        assert!(load_paddle_page(&d, "job-1").is_ok());
        // Due uscite nella stessa cartella: ambiguità, errore.
        std::fs::write(
            d.join("job-1/b_res.json"),
            r#"{"rec_texts":[],"rec_boxes":[]}"#,
        )
        .unwrap();
        assert!(load_paddle_page(&d, "job-1").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn test9_bbox_metrics_iou_and_corner_error() {
        let a = [10.0, 10.0, 20.0, 20.0];
        let m = bbox_metrics(&[(a, a), (a, [11.0, 10.0, 21.0, 20.0])]);
        assert_eq!(m.words_matched, 2);
        assert!((m.corner_err_p95_pt - 1.0).abs() < 1e-9);
        // IoU: 1 e 9/11; mediana di due valori = media.
        assert!((m.iou_median - (1.0 + 9.0 / 11.0) / 2.0).abs() < 1e-9);
        assert!((m.share_iou_ge_05 - 1.0).abs() < 1e-12);
    }

    fn metrics(iou: f64, p95: f64) -> BboxMetrics {
        BboxMetrics {
            words_matched: 10,
            iou_median: iou,
            share_iou_ge_05: 1.0,
            corner_err_p95_pt: p95,
        }
    }

    #[test]
    fn test9_analytic_thresholds_by_quality() {
        use {Quality::*, Reference::*, Verdict::*};
        assert_eq!(bbox_verdict(Analytic, Good, &metrics(0.9, 1.0), None), Pass);
        assert_eq!(
            bbox_verdict(Analytic, Good, &metrics(0.9, 1.01), None),
            Fail
        );
        assert_eq!(
            bbox_verdict(Analytic, Degraded, &metrics(0.5, 3.0), None),
            Pass
        );
        assert_eq!(
            bbox_verdict(Analytic, Degraded, &metrics(0.5, 3.2), None),
            Fail
        );
        // Il controllo analitico non dipende da 01-08.
        assert_eq!(
            bbox_verdict(Analytic, Good, &metrics(0.9, 0.5), Some(false)),
            Pass
        );
    }

    #[test]
    fn test9_pdfium_reference_is_non_valutabile_without_0108_gate() {
        use {Quality::*, Reference::*, Verdict::*};
        assert_eq!(
            bbox_verdict(Pdfium, Good, &metrics(0.9, 0.5), None),
            NonValutabile
        );
        assert_eq!(
            bbox_verdict(Pdfium, Good, &metrics(0.9, 0.5), Some(false)),
            NonValutabile
        );
        assert_eq!(
            bbox_verdict(Pdfium, Good, &metrics(0.5, 3.0), Some(true)),
            Pass
        );
        assert_eq!(
            bbox_verdict(Pdfium, Degraded, &metrics(0.49, 1.0), Some(true)),
            Fail
        );
        assert_eq!(
            bbox_verdict(Pdfium, Degraded, &metrics(0.8, 3.5), Some(true)),
            Fail
        );
    }

    // ---- TEST-13: prova ostile sul render ----------------------------------------------------

    fn report(status: &str, kind: Option<&str>, adj: bool, leftover: u32) -> Value {
        let outcome = match kind {
            Some(k) => json!({"status":status,"kind":k,"detail":"pdfium_render"}),
            None => json!({"status":status,"pages":1,"output":null,"data":null}),
        };
        json!({"job_id":"hr-jbig2-malformed-1-x","outcome":outcome,"exit_signal":null,
               "peak_rss_kb":40000,"wall_ms":120,"oom_kill_delta":0,
               "leftover_procs":leftover,"oom_adj_set":adj})
    }

    fn obs(r: Option<Value>) -> HostileObs {
        HostileObs {
            report: r,
            parent_exit: Some(0),
            worker_alive: true,
        }
    }

    fn kinds() -> Vec<String> {
        vec!["Corrupt".into(), "Crashed".into()]
    }

    #[test]
    fn test13_hostile_rows_table() {
        let k = kinds();
        assert_eq!(
            hostile_row(
                "failed",
                &k,
                &obs(Some(report("failed", Some("Corrupt"), true, 0)))
            ),
            (true, true)
        );
        assert_eq!(
            hostile_row(
                "failed",
                &k,
                &obs(Some(report("failed", Some("Crashed"), true, 0)))
            ),
            (true, true)
        );
        assert_eq!(
            hostile_row("failed", &k, &obs(Some(report("ok", None, true, 0)))),
            (false, true)
        );
        assert_eq!(
            hostile_row(
                "failed",
                &k,
                &obs(Some(report("failed", Some("Corrupt"), false, 0)))
            ),
            (false, true)
        );
        assert_eq!(
            hostile_row(
                "failed",
                &k,
                &obs(Some(report("failed", Some("Corrupt"), true, 1)))
            ),
            (false, true)
        );
        assert_eq!(
            hostile_row(
                "failed",
                &k,
                &obs(Some(report("failed", Some("Timeout"), true, 0)))
            ),
            (false, true)
        );
        assert_eq!(hostile_row("failed", &k, &obs(None)), (false, false));
    }

    #[test]
    fn test13_parent_alive_needs_self_exit_and_live_worker() {
        let k = kinds();
        let r = Some(report("failed", Some("Corrupt"), true, 0));
        let killed = HostileObs {
            report: r.clone(),
            parent_exit: None,
            worker_alive: true,
        };
        assert_eq!(hostile_row("failed", &k, &killed), (false, false));
        let dead_worker = HostileObs {
            report: r,
            parent_exit: Some(0),
            worker_alive: false,
        };
        assert_eq!(hostile_row("failed", &k, &dead_worker), (false, false));
    }

    #[test]
    fn test13_t09_closed_only_with_three_of_three() {
        assert_eq!(t09_status(&[(true, true, 0); 3]), T09::Chiuso);
        assert_eq!(
            t09_status(&[(true, true, 0), (false, true, 0), (true, true, 0)]),
            T09::Aperto
        );
        assert_eq!(
            t09_status(&[(true, true, 0), (true, true, 0)]),
            T09::Aperto,
            "meno di 3 righe"
        );
        assert_eq!(
            t09_status(&[(true, true, 0), (false, false, 0), (true, true, 0)]),
            T09::ContenimentoFallito
        );
        assert_eq!(
            t09_status(&[(false, true, 2), (true, true, 0), (true, true, 0)]),
            T09::ContenimentoFallito
        );
        assert_eq!(T09::ContenimentoFallito.as_str(), "contenimento fallito");
    }

    // ---- TEST-11, TEST-15: conformità M10 e freschezza dei limiti ------------------------------

    fn limits(mrb: u64, oq: u64) -> Value {
        json!({"default":{"memory_reservation_bytes":1,"out_quota_bytes":1},
               "ocr_tesseract":{"memory_reservation_bytes":mrb,"out_quota_bytes":oq,"address_space_bytes":999}})
    }

    const GIB2: u64 = 2 * 1024 * 1024 * 1024;

    fn batch(l: &Value, peaks: &[u64]) -> BatchObs {
        BatchObs {
            parent_alive: true,
            parent_exit: 0,
            batch_summary: true,
            cgroup_memory_max_bytes: Some(GIB2),
            oom_kill_delta: Some(0),
            job_errors: 0,
            reservation_kb: ocr_tesseract_reservation_bytes(l) / 1024,
            jobs: peaks
                .iter()
                .map(|&p| JobObs {
                    peak_rss_kb: Some(p),
                    leftover_procs: 0,
                })
                .collect(),
        }
    }

    #[test]
    fn test11_reservation_is_v6_formula_with_one_descendant() {
        let l = limits(100 * 1024 * 1024, 2 * 1024 * 1024);
        assert_eq!(
            ocr_tesseract_reservation_bytes(&l),
            2 * 100 * 1024 * 1024 + 2 * 1024 * 1024
        );
        // Senza profilo per op vale default; address_space_bytes non entra mai.
        let d = json!({"default":{"memory_reservation_bytes":10,"out_quota_bytes":3,"address_space_bytes":1_000_000}});
        assert_eq!(ocr_tesseract_reservation_bytes(&d), 23);
    }

    #[test]
    fn test11_m10_pass_and_each_failure() {
        let l = limits(100 * 1024 * 1024, 2 * 1024 * 1024);
        let ok = batch(&l, &[50_000, 120_000]);
        assert!(m10_pass(&ok, &l));
        let cases: Vec<(&str, Box<dyn Fn(&mut BatchObs)>)> = vec![
            ("padre morto", Box::new(|b| b.parent_alive = false)),
            ("exit != 0", Box::new(|b| b.parent_exit = 1)),
            ("senza batch_summary", Box::new(|b| b.batch_summary = false)),
            (
                "memory.max illimitato",
                Box::new(|b| b.cgroup_memory_max_bytes = None),
            ),
            ("oom_kill_delta 1", Box::new(|b| b.oom_kill_delta = Some(1))),
            (
                "oom_kill_delta ignoto",
                Box::new(|b| b.oom_kill_delta = None),
            ),
            ("job_error", Box::new(|b| b.job_errors = 1)),
            ("leftover", Box::new(|b| b.jobs[1].leftover_procs = 1)),
            ("picco assente", Box::new(|b| b.jobs[0].peak_rss_kb = None)),
            (
                "rapporto < 1",
                Box::new(|b| b.jobs[0].peak_rss_kb = Some(b.reservation_kb + 1)),
            ),
            ("nessun job", Box::new(|b| b.jobs.clear())),
        ];
        for (name, f) in cases {
            let mut b = ok.clone();
            f(&mut b);
            assert!(!m10_pass(&b, &l), "{name}");
        }
    }

    #[test]
    fn test11_published_concurrency_is_largest_conforming_n_capped_by_admitted() {
        assert_eq!(
            published_concurrency(&[(1, 1, true), (4, 3, true), (8, 3, true), (16, 3, false)]),
            Some(3)
        );
        assert_eq!(
            published_concurrency(&[(1, 1, true), (4, 4, true), (8, 6, false), (16, 6, false)]),
            Some(4)
        );
        assert_eq!(published_concurrency(&[(1, 1, false)]), None);
        // Basta un run non conforme per N per escluderlo.
        assert_eq!(
            published_concurrency(&[(1, 1, true), (4, 4, true), (4, 4, false)]),
            Some(1)
        );
    }

    #[test]
    fn test15_stale_reservation_or_other_cgroup_is_not_conforming() {
        let old = limits(100 * 1024 * 1024, 2 * 1024 * 1024);
        let now = limits(120 * 1024 * 1024, 2 * 1024 * 1024);
        // Batch misurato con i limiti vecchi, giudicato con quelli correnti: non conforme.
        let b = batch(&old, &[50_000]);
        assert!(m10_pass(&b, &old));
        assert!(
            !m10_pass(&b, &now),
            "prenotazione stantia rispetto ai limiti correnti"
        );
        // D-28: vale per il worker da 2 GiB; un cgroup diverso non si estrapola.
        let mut b4 = batch(&now, &[50_000]);
        b4.cgroup_memory_max_bytes = Some(2 * GIB2);
        assert!(!m10_pass(&b4, &now));
    }

    // ---- TEST-2..6: PaddleOCR -----------------------------------------------------------------

    #[test]
    fn test2_v6_cli_args_are_explicit_literals() {
        let sel = json!({"selectable":true,"ocr_version":"PP-OCRv6","tier":null,"lang":"it","source":"paddleocr ocr --help"});
        assert_eq!(
            v6_cli_args(&sel).unwrap(),
            ["--ocr_version", "PP-OCRv6", "--lang", "it"]
        );
        let sel_t = json!({"selectable":true,"ocr_version":"PP-OCRv6","tier":{"flag":"--text_recognition_model_name","value":"PP-OCRv6_medium_rec"},"lang":"it"});
        assert_eq!(
            v6_cli_args(&sel_t).unwrap(),
            [
                "--ocr_version",
                "PP-OCRv6",
                "--text_recognition_model_name",
                "PP-OCRv6_medium_rec",
                "--lang",
                "it"
            ]
        );
    }

    // Detection ridotta (decisione utente 2026-10-25, opzione a): argomenti fissati prima
    // della misura, in coda alla selezione letterale e agli argomenti fissi del piano.
    #[test]
    fn test2_v6_measure_args_add_plan_flags_and_reduced_detection() {
        let sel = json!({"selectable":true,"ocr_version":"PP-OCRv6","tier":null,"lang":"it"});
        assert_eq!(
            v6_measure_args(&sel).unwrap(),
            [
                "--ocr_version",
                "PP-OCRv6",
                "--lang",
                "it",
                "--use_doc_orientation_classify",
                "False",
                "--use_doc_unwarping",
                "False",
                "--use_textline_orientation",
                "False",
                "--device",
                "cpu",
                "--enable_mkldnn",
                "False",
                "--text_det_limit_type",
                "max",
                "--text_det_limit_side_len",
                "960"
            ]
        );
        assert!(
            v6_measure_args(&json!({"selectable":false,"reason":"x"})).is_err(),
            "la detection ridotta non apre la porta a uno sfidante non selezionabile"
        );
    }

    #[test]
    fn test6_unselectable_v6_or_missing_italian_stops() {
        assert!(
            v6_cli_args(&json!({"selectable":false,"reason":"ocr_version senza PP-OCRv6"}))
                .is_err()
        );
        assert!(
            v6_cli_args(&json!({"selectable":true,"ocr_version":"PP-OCRv6","lang":null})).is_err()
        );
        assert!(v6_cli_args(&json!({"selectable":true,"ocr_version":"PP-OCRv6"})).is_err());
        assert!(
            v6_cli_args(&json!({"selectable":true,"lang":"it"})).is_err(),
            "ocr_version mai implicito"
        );
        assert!(
            v6_cli_args(&json!({"selectable":true,"ocr_version":"PP-OCRv5","lang":"it"})).is_err(),
            "nessun ripiego su v5"
        );
        assert!(
            v6_cli_args(&json!({"ocr_version":"PP-OCRv6","lang":"it"})).is_err(),
            "selectable assente"
        );
    }

    fn resolved(det: &str, rec: &str) -> Value {
        json!({"det":{"model_name":det,"model_dir":"/opt/paddle-models/det","files":["inference.pdiparams"]},
               "rec":{"model_name":rec,"model_dir":"/opt/paddle-models/rec","files":["inference.pdiparams"]}})
    }

    #[test]
    fn test3_family_check_on_resolved_det_and_rec() {
        assert!(v6_family_check(&resolved("PP-OCRv6_medium_det", "PP-OCRv6_medium_rec")).is_ok());
        assert!(v6_family_check(&resolved("PP-OCRv6_medium_det", "PP-OCRv5_server_rec")).is_err());
        assert!(
            v6_family_check(&resolved(
                "PP-OCRv6_medium_det",
                "latin_PP-OCRv5_mobile_rec"
            ))
            .is_err()
        );
        assert!(v6_family_check(&resolved("PP-OCRv5_server_det", "PP-OCRv6_medium_rec")).is_err());
        assert!(
            v6_family_check(&resolved("PP-OCRv6", "PP-OCRv6_medium_rec")).is_err(),
            "prefisso con _"
        );
        assert!(
            v6_family_check(&json!({"det":{},"rec":{"model_name":"PP-OCRv6_medium_rec"}})).is_err()
        );
    }

    const H1: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const H2: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    const H3: &str = "3333333333333333333333333333333333333333333333333333333333333333";

    fn manifest(rows: &[(&str, &str)]) -> String {
        rows.iter().map(|(h, p)| format!("{h}  {p}\n")).collect()
    }

    #[test]
    fn test4_weights_manifest_detects_change_omission_and_substitution() {
        let base = manifest(&[
            (H1, "/opt/paddle-models/det/a"),
            (H2, "/opt/paddle-models/rec/b"),
        ]);
        assert!(weights_manifest_check(&base, &base).is_ok());
        let changed = manifest(&[
            (H1, "/opt/paddle-models/det/a"),
            (H3, "/opt/paddle-models/rec/b"),
        ]);
        assert!(weights_manifest_check(&changed, &base).is_err(), "modifica");
        let omitted = manifest(&[(H1, "/opt/paddle-models/det/a")]);
        assert!(
            weights_manifest_check(&omitted, &base).is_err(),
            "omissione"
        );
        let added = manifest(&[
            (H1, "/opt/paddle-models/det/a"),
            (H2, "/opt/paddle-models/rec/b"),
            (H3, "/opt/paddle-models/rec/c"),
        ]);
        assert!(weights_manifest_check(&added, &base).is_err(), "aggiunta");
        let swapped = manifest(&[
            (H2, "/opt/paddle-models/det/a"),
            (H1, "/opt/paddle-models/rec/b"),
        ]);
        assert!(
            weights_manifest_check(&swapped, &base).is_err(),
            "sostituzione"
        );
        assert!(weights_manifest_check("", "").is_err(), "manifest vuoto");
        assert!(
            weights_manifest_check("zz  /x\n", "zz  /x\n").is_err(),
            "hash non sha256"
        );
    }

    #[test]
    fn test5_session_must_match_build_configuration() {
        let b = resolved("PP-OCRv6_medium_det", "PP-OCRv6_medium_rec");
        assert!(session_matches_build(&b, &b).is_ok());
        let other_rec = resolved("PP-OCRv6_medium_det", "PP-OCRv6_server_rec");
        assert!(session_matches_build(&b, &other_rec).is_err());
        let mut other_dir = b.clone();
        other_dir["rec"]["model_dir"] = json!("/home/u/.paddlex/official_models/rec");
        assert!(
            session_matches_build(&b, &other_dir).is_err(),
            "pesi ri-scaricati altrove"
        );
        assert!(
            session_matches_build(&b, &json!({})).is_err(),
            "modelli non esposti: mai confronto saltato"
        );
        let v5 = resolved("PP-OCRv5_server_det", "PP-OCRv5_server_rec");
        assert!(
            session_matches_build(&v5, &v5).is_err(),
            "famiglia v6 richiesta anche se uguali"
        );
    }
}
