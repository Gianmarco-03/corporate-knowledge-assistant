//! Istogramma di latenze in microsecondi (hdrhistogram, 3 cifre significative).

#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub n: u64,
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub max_us: u64,
}

impl Summary {
    pub fn csv_fields(&self) -> [String; 5] {
        todo!()
    }
}

pub struct Recorder {
    _private: (),
}

impl Recorder {
    pub fn new() -> Self {
        todo!()
    }

    pub fn record_us(&mut self, _v: u64) {
        todo!()
    }

    pub fn summary(&self) -> Summary {
        todo!()
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
}
