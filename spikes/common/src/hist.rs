//! Istogramma di latenze in microsecondi (hdrhistogram, 3 cifre significative).

// Modifica: Claude (gsd-executor 01.2-01) · 2026-10-21 · record_us registra con record (auto-resize) invece di saturating_record; contatore rejected (gap-closure 1.2)

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
    // Valori che `Histogram::record` ha rifiutato; ciascuno è stato registrato
    // comunque, saturato.
    rejected: u64,
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
        Recorder { hist, rejected: 0 }
    }

    /// Registra una latenza in microsecondi con `Histogram::record`: con un
    /// istogramma creato da `Histogram::new` il ridimensionamento è automatico
    /// e copre qualunque valore, anche `u64::MAX`. `saturating_record`, al
    /// contrario, non ridimensiona e limitava ogni valore sopra 2047 µs al
    /// massimo tracciabile iniziale (difetto corretto nella gap-closure 1.2).
    ///
    /// Un errore di `record` è irraggiungibile con auto-resize attivo e 3 cifre
    /// (al più 56320 contatori, entro `usize` a 32 e 64 bit). Se capitasse, il
    /// valore viene registrato saturato, `rejected()` lo conta e il primo
    /// rifiuto viene segnalato su stderr.
    pub fn record_us(&mut self, v: u64) {
        match self.hist.record(v) {
            Ok(()) => {}
            Err(e) => {
                self.rejected = self.rejected.saturating_add(1);
                self.hist.saturating_record(v);
                if self.rejected == 1 {
                    eprintln!(
                        "common::hist: valore {v} us rifiutato da record ({e:?}), registrato saturato; rejected() conta i rifiuti"
                    );
                }
            }
        }
    }

    /// Numero di valori rifiutati da `Histogram::record`. Ciascuno è stato
    /// registrato saturato: se è diverso da 0 i percentili alti e il massimo
    /// non sono affidabili.
    pub fn rejected(&self) -> u64 {
        self.rejected
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

    #[test]
    fn hist_rejected_counter_stays_zero() {
        // Il ramo `Err` è irraggiungibile su target a 32/64 bit con auto-resize
        // attivo e 3 cifre (al più 56320 contatori, test upstream
        // `tests/auto_resize.rs:6-19`): qui si verifica che i valori estremi
        // non lo attivino.
        let mut r = Recorder::new();
        for v in [1_000u64, 60_000_000, u64::MAX] {
            r.record_us(v);
        }
        assert_eq!(r.rejected(), 0);
        assert_eq!(r.summary().n, 3);
        assert_eq!(Recorder::default().rejected(), 0);
    }

    #[test]
    fn hist_rejected_branch_counts_and_saturates() {
        // Istogramma senza auto-resize (massimo 2047): `record(10_000)` dà
        // `Err(ValueOutOfRangeResizeDisabled)`. Il modulo figlio vede i campi
        // privati, quindi si costruisce il Recorder a mano.
        let hist = match Histogram::<u64>::new_with_max(2047, SIGFIG) {
            Ok(h) => h,
            Err(e) => panic!("istogramma di prova non costruibile: {e:?}"),
        };
        let mut r = Recorder { hist, rejected: 0 };
        for _ in 0..3 {
            r.record_us(10_000);
        }
        r.record_us(1_000);
        let s = r.summary();
        assert_eq!(r.rejected(), 3);
        assert_eq!(s.n, 4, "n = {}", s.n);
        assert!(s.max_us <= 2047, "max_us = {}", s.max_us);
    }
}
