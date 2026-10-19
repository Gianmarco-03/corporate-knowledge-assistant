//! Cronometro monotono per le misure degli spike.

pub struct Stopwatch {
    _private: (),
}

impl Stopwatch {
    pub fn start() -> Self {
        todo!()
    }

    pub fn elapsed_us(&self) -> u64 {
        todo!()
    }

    pub fn elapsed_ms_f64(&self) -> f64 {
        todo!()
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
        assert!(b >= 5_000, "dopo 5 ms devono essere trascorsi almeno 5000 us: {b}");
        assert!(sw.elapsed_ms_f64() >= 5.0);
    }
}
