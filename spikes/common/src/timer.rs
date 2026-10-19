//! Cronometro monotono per le misure degli spike.

use std::time::Instant;

/// Basato su `Instant`: monotono, immune ai cambi dell'orologio di sistema.
pub struct Stopwatch {
    start: Instant,
}

impl Stopwatch {
    pub fn start() -> Self {
        Stopwatch {
            start: Instant::now(),
        }
    }

    /// Microsecondi trascorsi; satura a `u64::MAX` (oltre 584 000 anni).
    pub fn elapsed_us(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_micros()).unwrap_or(u64::MAX)
    }

    pub fn elapsed_ms_f64(&self) -> f64 {
        self.start.elapsed().as_secs_f64() * 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_elapsed_is_monotonic_and_consistent() {
        let sw = Stopwatch::start();
        let a = sw.elapsed_us();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let b = sw.elapsed_us();
        assert!(b >= a);
        assert!(
            b >= 5_000,
            "dopo 5 ms devono essere trascorsi almeno 5000 us: {b}"
        );
        assert!(sw.elapsed_ms_f64() >= 5.0);
    }
}
