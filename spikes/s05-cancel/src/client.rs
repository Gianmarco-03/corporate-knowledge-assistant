//! Client reqwest verso i provider e fan-out con deadline globale (spike S-05).

use tokio::time::Instant;

/// Protocollo verso il mock.
// stub del test-write: da implementare in 01-05
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proto {
    H1,
    H2,
}

/// Esito di un fan-out con deadline.
// stub del test-write: da implementare in 01-05
#[derive(Debug, Clone)]
pub struct FanoutOutcome {
    /// Microsecondi fra la deadline e il ritorno del chiamante (negativo se prima).
    pub exit_after_deadline_us: i64,
    pub completed: usize,
    pub cancelled: usize,
}

/// `reqwest::Client` con `connect_timeout(2 s)`, `pool_idle_timeout(90 s)`;
/// `Proto::H2` usa `http2_prior_knowledge()`.
// stub del test-write: da implementare in 01-05
pub fn build_client(_proto: Proto) -> anyhow::Result<reqwest::Client> {
    todo!("stub del test-write: build_client")
}

/// Variante A: `timeout_at(deadline, iter.map(call).buffer_unordered(concurrency).collect())`.
// stub del test-write: da implementare in 01-05
pub async fn fanout_variant_a(
    _client: &reqwest::Client,
    _url: &str,
    _fanout: usize,
    _concurrency: usize,
    _deadline: Instant,
) -> FanoutOutcome {
    todo!("stub del test-write: fanout_variant_a")
}

/// Variante B: `JoinSet` + `Arc<Semaphore>(concurrency)`, abort allo scadere della deadline.
// stub del test-write: da implementare in 01-05
pub async fn fanout_variant_b(
    _client: &reqwest::Client,
    _url: &str,
    _fanout: usize,
    _concurrency: usize,
    _deadline: Instant,
) -> FanoutOutcome {
    todo!("stub del test-write: fanout_variant_b")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe;
    use crate::server::{Metrics, SLOW_PATH, spawn_slow_server};
    use std::time::Duration;

    /// URL loopback del mock lento (il server ascolta solo su 127.0.0.1).
    fn loopback_endpoint(addr: std::net::SocketAddr) -> String {
        format!("http://127.0.0.1:{}{SLOW_PATH}", addr.port())
    }

    /// Attende fino a `max` che `cond` sia vera; restituisce l'ultimo esito.
    async fn wait_until(max: Duration, mut cond: impl FnMut() -> bool) -> bool {
        let end = Instant::now() + max;
        loop {
            if cond() {
                return true;
            }
            if Instant::now() >= end {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    async fn cancel_one(proto: Proto) -> std::sync::Arc<Metrics> {
        let (addr, metrics) = spawn_slow_server(Duration::from_millis(2000))
            .await
            .expect("server avviato");
        let client = build_client(proto).expect("client");
        let endpoint = loopback_endpoint(addr);
        let res =
            tokio::time::timeout(Duration::from_millis(50), client.get(&endpoint).send()).await;
        assert!(res.is_err(), "la richiesta deve essere cancellata a 50 ms");
        metrics
    }

    // TEST-6 (RISK-3, RISK-9)
    #[tokio::test(flavor = "multi_thread")]
    async fn server_sees_client_cancel_h2() {
        let metrics = cancel_one(Proto::H2).await;
        let seen = wait_until(Duration::from_millis(500), || metrics.dropped() == 1).await;
        assert!(seen, "il server non ha visto il drop entro 500 ms");
        assert_eq!(metrics.completed(), 0);
    }

    // TEST-6, caso HTTP/1: registrato senza asserzione (nessuna soglia per h1).
    #[tokio::test(flavor = "multi_thread")]
    async fn server_sees_client_cancel_h1_recorded() {
        let metrics = cancel_one(Proto::H1).await;
        let seen = wait_until(Duration::from_millis(500), || metrics.dropped() == 1).await;
        eprintln!(
            "h1: drop visto entro 500 ms = {seen}, dropped = {}, completed = {}",
            metrics.dropped(),
            metrics.completed()
        );
    }

    // TEST-7 (RISK-4)
    #[tokio::test(flavor = "multi_thread")]
    async fn deadline_variant_a_exits_and_drops_inflight() {
        let (addr, metrics) = spawn_slow_server(Duration::from_millis(5000))
            .await
            .expect("server avviato");
        let client = build_client(Proto::H2).expect("client");
        let endpoint = loopback_endpoint(addr);
        let budget = Duration::from_millis(200);
        let start = Instant::now();
        let out = fanout_variant_a(&client, &endpoint, 8, 8, start + budget).await;
        let elapsed = start.elapsed();
        assert!(
            elapsed <= budget + Duration::from_millis(50),
            "ritorno dopo {elapsed:?}, budget {budget:?}"
        );
        assert_eq!(out.completed, 0, "nessuna richiesta completa a 5 s");
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(metrics.started(), 8, "tutte le 8 richieste avviate");
        assert_eq!(
            metrics.dropped(),
            8,
            "le richieste in volo devono essere cancellate, non abbandonate"
        );
        assert_eq!(metrics.completed(), 0);
    }

    // TEST-8 (RISK-4)
    #[tokio::test(flavor = "multi_thread")]
    async fn deadline_variant_b_aborts_joinset() {
        let (addr, metrics) = spawn_slow_server(Duration::from_millis(5000))
            .await
            .expect("server avviato");
        let client = build_client(Proto::H2).expect("client");
        let endpoint = loopback_endpoint(addr);

        // Riscaldamento: apre la connessione h2 (task di connessione stabili) prima della base.
        let _ = tokio::time::timeout(Duration::from_millis(50), client.get(&endpoint).send()).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        let base_tasks = probe::alive_tasks();
        let base_started = metrics.started();
        let base_dropped = metrics.dropped();

        let budget = Duration::from_millis(200);
        let start = Instant::now();
        let out = fanout_variant_b(&client, &endpoint, 16, 8, start + budget).await;
        let elapsed = start.elapsed();
        assert!(
            elapsed <= budget + Duration::from_millis(50),
            "ritorno dopo {elapsed:?}, budget {budget:?}"
        );
        assert_eq!(out.completed, 0, "nessuna richiesta completa a 5 s");

        tokio::time::sleep(Duration::from_millis(500)).await;
        let alive = probe::alive_tasks();
        assert!(
            alive <= base_tasks + 2 && alive + 2 >= base_tasks,
            "task vivi {alive}, base {base_tasks}: il JoinSet non ha fatto abort"
        );
        let started = metrics.started() - base_started;
        let dropped = metrics.dropped() - base_dropped;
        assert!(started > 0, "nessuna richiesta avviata");
        assert!(
            started <= 8,
            "il semaforo deve limitare a 8, avviate {started}"
        );
        assert_eq!(
            dropped, started,
            "ogni richiesta avviata deve essere cancellata"
        );
        assert_eq!(metrics.completed(), 0);
    }
}
