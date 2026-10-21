//! Istogramma di latenze in microsecondi (hdrhistogram, 3 cifre significative).

#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub n: u64,
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub max_us: u64,
}

use hdrhistogram::Histogram;

/// Cifre significative dell'istogramma: errore relativo di quantizzazione
/// massimo 0,1 %.
const SIGFIG: u8 = 3;

impl Summary {
    /// Campi nell'ordine `n, p50_us, p95_us, p99_us, max_us`.
    pub fn csv_fields(&self) -> [String; 5] {
        [
            self.n.to_string(),
            self.p50_us.to_string(),
            self.p95_us.to_string(),
            self.p99_us.to_string(),
            self.max_us.to_string(),
        ]
    }
}

pub struct Recorder {
    hist: Histogram<u64>,
}

impl Recorder {
    pub fn new() -> Self {
        // `Histogram::new(sigfig)` fallisce solo con sigfig > 5
        // (`CreationError::SigFigExceedsMax`); con la costante 3 l'errore è
        // irraggiungibile. Non esiste un costruttore infallibile, quindi il
        // ramo d'errore è esplicito e documentato invece di un `unwrap`.
        let hist = match Histogram::<u64>::new(SIGFIG) {
            Ok(h) => h,
            Err(e) => unreachable!("hdrhistogram con {SIGFIG} cifre significative: {e:?}"),
        };
        Recorder { hist }
    }

    /// Registra una latenza in microsecondi. L'istogramma si ridimensiona da
    /// solo; `saturating_record` non può fallire (un valore fuori scala viene
    /// limitato al massimo rappresentabile).
    pub fn record_us(&mut self, v: u64) {
        self.hist.saturating_record(v);
    }

    pub fn summary(&self) -> Summary {
        Summary {
            n: self.hist.len(),
            p50_us: self.hist.value_at_quantile(0.50),
            p95_us: self.hist.value_at_quantile(0.95),
            p99_us: self.hist.value_at_quantile(0.99),
            max_us: self.hist.max(),
        }
    }
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hist_summary_1_to_100_percentiles() {
        let mut r = Recorder::new();
        for v in 1..=100u64 {
            r.record_us(v);
        }
        let s = r.summary();
        assert_eq!(s.n, 100);
        assert!((50..=51).contains(&s.p50_us), "p50 = {}", s.p50_us);
        assert!((95..=96).contains(&s.p95_us), "p95 = {}", s.p95_us);
        assert!((99..=100).contains(&s.p99_us), "p99 = {}", s.p99_us);
        assert_eq!(s.max_us, 100);
    }

    #[test]
    fn hist_csv_fields_order_and_empty_recorder() {
        let s = Summary {
            n: 1,
            p50_us: 2,
            p95_us: 3,
            p99_us: 4,
            max_us: 5,
        };
        assert_eq!(s.csv_fields(), ["1", "2", "3", "4", "5"].map(String::from));
        let empty = Recorder::default().summary();
        assert_eq!(empty.n, 0);
        assert_eq!(empty.max_us, 0);
    }

    /// Criterio intero dello 0,1 %: `|got - exp| * 1000 <= exp`. Non va in
    /// overflow per i valori usati qui (al massimo 60_000_000 × 1000).
    fn assert_within_0_1_pct(label: &str, got: u64, exp: u64) {
        assert!(
            got.abs_diff(exp) * 1000 <= exp,
            "{label}: got = {got}, exp = {exp}, oltre lo 0,1 %"
        );
    }

    #[test]
    fn hist_single_values_above_initial_range() {
        // 1 ms sta sotto il massimo iniziale dell'istogramma (2047) ed è il
        // controllo; 10 ms, 1 s e 60 s richiedono il ridimensionamento.
        for v in [1_000u64, 10_000, 1_000_000, 60_000_000] {
            let mut r = Recorder::new();
            r.record_us(v);
            let s = r.summary();
            assert_eq!(s.n, 1, "v = {v}: n = {}", s.n);
            assert_within_0_1_pct(&format!("v = {v}: p50_us"), s.p50_us, v);
            assert_within_0_1_pct(&format!("v = {v}: p99_us"), s.p99_us, v);
            assert_within_0_1_pct(&format!("v = {v}: max_us"), s.max_us, v);
        }
    }

    #[test]
    fn hist_mixed_latencies_percentiles() {
        // Ranghi su 1000 campioni: p50 = 500° (gruppo 401-900, 10_000),
        // p95 = 950°/951° (gruppo 901-980, 1_000_000), p99 = 990°/991°
        // (gruppo 981-999, 5_000_000), max = 1000° (60_000_000). Ogni
        // percentile cade lontano dai confini dei gruppi.
        let mut r = Recorder::new();
        for (count, v) in [
            (400u64, 1_000u64),
            (500, 10_000),
            (80, 1_000_000),
            (19, 5_000_000),
            (1, 60_000_000),
        ] {
            for _ in 0..count {
                r.record_us(v);
            }
        }
        let s = r.summary();
        assert_eq!(s.n, 1000, "n = {}", s.n);
        assert_within_0_1_pct("p50_us", s.p50_us, 10_000);
        assert_within_0_1_pct("p95_us", s.p95_us, 1_000_000);
        assert_within_0_1_pct("p99_us", s.p99_us, 5_000_000);
        assert_within_0_1_pct("max_us", s.max_us, 60_000_000);
    }

    #[test]
    fn hist_extreme_value_u64_max() {
        // Con auto-resize nessun valore u64 viene rifiutato su target a 64 bit
        // (test upstream hdrhistogram `tests/auto_resize.rs:6-19`). Confronto
        // esatto: `highest_equivalent(u64::MAX)` restituisce `u64::MAX`, e il
        // criterio dello 0,1 % andrebbe in overflow.
        let mut r = Recorder::new();
        r.record_us(1_000);
        r.record_us(u64::MAX);
        let s = r.summary();
        assert_eq!(s.n, 2, "n = {}", s.n);
        assert_eq!(s.max_us, u64::MAX, "max_us = {}", s.max_us);
    }
}
