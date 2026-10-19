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
}
