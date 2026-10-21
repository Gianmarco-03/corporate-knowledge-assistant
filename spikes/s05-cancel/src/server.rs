//! Server axum lento con guardia di drop (spike S-05).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

/// Percorso dell'handler lento.
// stub del test-write: da implementare in 01-05
pub const SLOW_PATH: &str = "/slow";

/// Contatori condivisi del server: richieste avviate, completate e cancellate.
// stub del test-write: da implementare in 01-05
pub struct Metrics {}

impl Metrics {
    // stub del test-write: da implementare in 01-05
    pub fn new() -> Arc<Self> {
        todo!("stub del test-write: Metrics::new")
    }

    // stub del test-write: da implementare in 01-05
    pub fn started(&self) -> u64 {
        todo!("stub del test-write: Metrics::started")
    }

    // stub del test-write: da implementare in 01-05
    pub fn completed(&self) -> u64 {
        todo!("stub del test-write: Metrics::completed")
    }

    // stub del test-write: da implementare in 01-05
    pub fn dropped(&self) -> u64 {
        todo!("stub del test-write: Metrics::dropped")
    }
}

/// Guardia tenuta dall'handler: al drop senza `complete()` conta la richiesta come cancellata.
// stub del test-write: da implementare in 01-05
pub struct DropGuard {}

impl DropGuard {
    // stub del test-write: da implementare in 01-05
    pub fn new(_metrics: Arc<Metrics>) -> Self {
        todo!("stub del test-write: DropGuard::new")
    }

    // stub del test-write: da implementare in 01-05
    pub fn complete(&mut self) {
        todo!("stub del test-write: DropGuard::complete")
    }
}

/// Avvia il server lento su `127.0.0.1:0` (HTTP/1 e HTTP/2 prior knowledge) e
/// restituisce l'indirizzo e i contatori.
// stub del test-write: da implementare in 01-05
pub async fn spawn_slow_server(_delay: Duration) -> anyhow::Result<(SocketAddr, Arc<Metrics>)> {
    todo!("stub del test-write: spawn_slow_server")
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-4 (RISK-3)
    #[test]
    fn drop_guard_counts_dropped_when_not_completed() {
        let metrics = Metrics::new();
        {
            let _guard = DropGuard::new(Arc::clone(&metrics));
        }
        assert_eq!(metrics.dropped(), 1, "guardia lasciata senza complete()");
        assert_eq!(metrics.completed(), 0);
    }

    // TEST-4, variante asincrona (RISK-3)
    #[tokio::test]
    async fn drop_guard_counts_dropped_when_future_times_out() {
        let metrics = Metrics::new();
        let m = Arc::clone(&metrics);
        let fut = async move {
            let mut guard = DropGuard::new(m);
            tokio::time::sleep(Duration::from_secs(1)).await;
            guard.complete();
        };
        let res = tokio::time::timeout(Duration::from_millis(10), fut).await;
        assert!(res.is_err(), "il timeout deve scadere prima del sleep");
        assert_eq!(metrics.dropped(), 1, "future cancellato dal timeout");
        assert_eq!(metrics.completed(), 0);
    }

    // TEST-5 (RISK-3)
    #[test]
    fn drop_guard_completed_is_not_dropped() {
        let metrics = Metrics::new();
        {
            let mut guard = DropGuard::new(Arc::clone(&metrics));
            guard.complete();
        }
        assert_eq!(metrics.completed(), 1);
        assert_eq!(metrics.dropped(), 0, "guardia completata non è un drop");
    }

    // TEST-14 (RISK-9)
    #[tokio::test(flavor = "multi_thread")]
    async fn h2_prior_knowledge_negotiates_http2() {
        let (addr, _metrics) = spawn_slow_server(Duration::from_millis(0))
            .await
            .expect("server avviato");
        let client = reqwest::Client::builder()
            .http2_prior_knowledge()
            .build()
            .expect("client h2");
        let resp = client
            .get(format!("http://127.0.0.1:{}{SLOW_PATH}", addr.port()))
            .send()
            .await
            .expect("risposta");
        assert_eq!(resp.version(), reqwest::Version::HTTP_2);
    }
}
