//! `score`, `hostile-verdict` e regole di giudizio del piano 01-13 (TEST-2..6, 9, 11, 13, 15).
//!
//! STUB del test-write (2026-10-23): firme e tipi sono il contratto dei test; i corpi `todo!()`
//! li scrive l'esecutore del Task 2. Le regole sono quelle delle «Soglie» del piano:
//! - CER solo sugli oracoli `parser-it.json` con i tre assi verdi (`claude` valorizzato,
//!   `gpt` = `CONFIRMED`, `user` = `ratified`), normalizzazione di 01-08;
//! - fedeltà bbox: `reference=analytic` sempre giudicata, `reference=pdfium` `non_valutabile`
//!   se 01-08 non ha superato la propria soglia IoU;
//! - prova ostile sul render (D-34): regola di `hostile_verdict` di 01-07 + contenimento;
//! - conformità di un batch OCR col metodo di M10 di 01-18, prenotazione di 01-07 v6;
//! - controlli PaddleOCR: selezione v6 esplicita, famiglia `PP-OCRv6_`, manifest dei pesi,
//!   confronto sessione/build.
#![allow(dead_code)]

use serde_json::Value;

// ---- CER ---------------------------------------------------------------------------------

/// NFC, spazi bianchi collassati in uno, bordi rimossi (come `normalize_text` di 01-08).
pub(crate) fn normalize_text(_s: &str) -> String {
    todo!("01-13 Task 2: normalize_text")
}

/// CER = distanza di edit sui caratteri normalizzati / caratteri attesi.
pub(crate) fn cer(_expected: &str, _predicted: &str) -> f64 {
    todo!("01-13 Task 2: cer")
}

/// Voce dell'oracolo con i tre assi verdi.
pub(crate) fn three_green(_entry: &Value) -> bool {
    todo!("01-13 Task 2: three_green")
}

/// Voci utilizzabili e numero di voci pendenti escluse e contate.
pub(crate) fn usable_oracles(_entries: &[Value]) -> (Vec<&Value>, usize) {
    todo!("01-13 Task 2: usable_oracles")
}

/// Testo di una pagina dall'output JSON di PaddleOCR (`rec_texts`, nell'ordine).
pub(crate) fn paddle_text(_out: &Value) -> Result<String, String> {
    todo!("01-13 Task 2: paddle_text")
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

pub(crate) fn bbox_metrics(_pairs: &[([f64; 4], [f64; 4])]) -> BboxMetrics {
    todo!("01-13 Task 2: bbox_metrics")
}

/// Verdetto di una riga di `bbox_map.csv`. `pdfium_gate` = esito della soglia IoU di 01-08
/// (`None` se non disponibile).
pub(crate) fn bbox_verdict(
    _reference: Reference,
    _quality: Quality,
    _m: &BboxMetrics,
    _pdfium_gate: Option<bool>,
) -> Verdict {
    todo!("01-13 Task 2: bbox_verdict")
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
    _expected_status: &str,
    _expected_kinds: &[String],
    _obs: &HostileObs,
) -> (bool, bool) {
    todo!("01-13 Task 2: hostile_row")
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
pub(crate) fn t09_status(_rows: &[(bool, bool, u32)]) -> T09 {
    todo!("01-13 Task 2: t09_status")
}

// ---- Conformità di un batch OCR (metodo di M10 di 01-18) ------------------------------------

/// Prenotazione per job di `ocr_tesseract` (01-07 v6): `memory_reservation_bytes` × (1 + 1
/// discendente) + `out_quota_bytes`, dal profilo `ocr_tesseract` se c'è, altrimenti `default`.
pub(crate) fn ocr_tesseract_reservation_bytes(_limits: &Value) -> u64 {
    todo!("01-13 Task 2: ocr_tesseract_reservation_bytes")
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

/// `m10_pass` di una riga di `ocr_throughput.csv`, con la prenotazione ricalcolata dai limiti
/// correnti (`limits-ocr_tesseract.json`).
pub(crate) fn m10_pass(_b: &BatchObs, _limits: &Value) -> bool {
    todo!("01-13 Task 2: m10_pass")
}

/// Concorrenza OCR pubblicata: il più grande N con batch conforme, limitato dal `max_admitted`
/// misurato. Righe `(requested_concurrency, max_admitted, m10_pass)`.
pub(crate) fn published_concurrency(_rows: &[(u32, u32, bool)]) -> Option<u32> {
    todo!("01-13 Task 2: published_concurrency")
}

// ---- PaddleOCR: selezione v6, famiglia, pesi, sessione ------------------------------------

/// Argomenti della CLI dello sfidante da `v6.selection.json`: `--ocr_version <v>`, il flag di
/// variante/tier se presente (`tier: {"flag","value"}`), `--lang <l>`. `selectable=false`,
/// valori assenti o non v6 → `Err` con il motivo (nessun ripiego su v5).
pub(crate) fn v6_cli_args(_selection: &Value) -> Result<Vec<String>, String> {
    todo!("01-13 Task 2: v6_cli_args")
}

/// Famiglia di `v6.resolved.json`: `det.model_name` e `rec.model_name` con prefisso `PP-OCRv6_`.
pub(crate) fn v6_family_check(_resolved: &Value) -> Result<(), String> {
    todo!("01-13 Task 2: v6_family_check")
}

/// Manifest dei pesi nell'immagine contro `paddle-weights.sha256` committato: righe
/// `<64 hex>  <percorso>` ordinate per percorso, insiemi e hash identici.
pub(crate) fn weights_manifest_check(_image: &str, _committed: &str) -> Result<(), String> {
    todo!("01-13 Task 2: weights_manifest_check")
}

/// Configurazione risolta della sessione contro quella di build: stessi `model_name`, stesse
/// directory e stessi file di det e rec, famiglia `PP-OCRv6_`; modelli non esposti → `Err`.
pub(crate) fn session_matches_build(_build: &Value, _session: &Value) -> Result<(), String> {
    todo!("01-13 Task 2: session_matches_build")
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
