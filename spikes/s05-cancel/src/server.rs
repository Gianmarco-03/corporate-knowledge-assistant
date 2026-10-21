//! Server axum lento con guardia di drop (spike S-05).
//!
//! Il server gira in un runtime tokio proprio, su un thread dedicato: così i suoi task
//! non entrano nel conteggio `num_alive_tasks` del runtime del client sotto misura, e lo
//! scheduling del mock non compete con quello del chiamante. File descriptor e socket
//! restano dello stesso processo (le sonde li contano entrambi).

use axum::Router;
use axum::extract::{RawQuery, State};
use axum::routing::get;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Percorso dell'handler lento.
pub const SLOW_PATH: &str = "/slow";

/// Worker del runtime dedicato al mock.
const SERVER_WORKERS: usize = 2;

/// Contatori condivisi del server: richieste avviate, completate e cancellate.
///
/// Per costruzione `started = completed + dropped + in corso`: ogni handler crea una
/// sola [`DropGuard`], che a fine vita incrementa esattamente uno dei due contatori.
pub struct Metrics {
    started: AtomicU64,
    completed: AtomicU64,
    dropped: AtomicU64,
    /// Istante del drop per le richieste con `?id=<n>` (cicli di cancellazione).
    drop_times: Mutex<HashMap<u64, Instant>>,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Metrics {
            started: AtomicU64::new(0),
            completed: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            drop_times: Mutex::new(HashMap::new()),
        })
    }

    pub fn started(&self) -> u64 {
        self.started.load(Ordering::SeqCst)
    }

    pub fn completed(&self) -> u64 {
        self.completed.load(Ordering::SeqCst)
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::SeqCst)
    }

    /// Nessun handler in corso: ogni richiesta avviata è completata o cancellata.
    pub fn quiescent(&self) -> bool {
        self.started() == self.completed() + self.dropped()
    }

    /// Istante in cui il server ha visto la cancellazione della richiesta `id`.
    pub fn drop_time(&self, id: u64) -> Option<Instant> {
        self.drop_times
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&id)
            .copied()
    }

    fn record_drop(&self, id: Option<u64>) {
        let now = Instant::now();
        if let Some(id) = id {
            self.drop_times
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(id, now);
        }
        self.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/// Guardia tenuta dall'handler: al drop senza `complete()` conta la richiesta come cancellata.
pub struct DropGuard {
    metrics: Arc<Metrics>,
    id: Option<u64>,
    done: bool,
}

impl DropGuard {
    pub fn new(metrics: Arc<Metrics>) -> Self {
        DropGuard {
            metrics,
            id: None,
            done: false,
        }
    }

    /// Guardia che registra anche l'istante del drop sotto `id`.
    pub fn with_id(metrics: Arc<Metrics>, id: u64) -> Self {
        DropGuard {
            metrics,
            id: Some(id),
            done: false,
        }
    }

    /// Segna la richiesta come completata; idempotente.
    pub fn complete(&mut self) {
        if !self.done {
            self.done = true;
            self.metrics.completed.fetch_add(1, Ordering::SeqCst);
        }
    }
}

impl Drop for DropGuard {
    fn drop(&mut self) {
        if !self.done {
            self.metrics.record_drop(self.id);
        }
    }
}

/// Piano dei ritardi dell'handler.
#[derive(Debug, Clone, Copy)]
pub enum DelayPlan {
    /// Stesso ritardo per ogni richiesta.
    Fixed(Duration),
    /// Ritardo deterministico in `[min, max)` per la n-esima richiesta avviata
    /// (`min + (n · 7919) mod (max − min)`): un fan-out vede richieste brevi e lunghe.
    Spread { min: Duration, max: Duration },
}

impl DelayPlan {
    fn delay_for(&self, n: u64) -> Duration {
        match *self {
            DelayPlan::Fixed(d) => d,
            DelayPlan::Spread { min, max } => {
                let span = u64::try_from(max.saturating_sub(min).as_millis()).unwrap_or(u64::MAX);
                if span == 0 {
                    return min;
                }
                min + Duration::from_millis(n.wrapping_mul(7919) % span)
            }
        }
    }
}

#[derive(Clone)]
struct AppState {
    metrics: Arc<Metrics>,
    plan: DelayPlan,
}

/// Legge `id=<u64>` dalla query string, se presente.
fn parse_id(query: Option<&str>) -> Option<u64> {
    query?
        .split('&')
        .find_map(|kv| kv.strip_prefix("id="))
        .and_then(|v| v.parse().ok())
}

// security-scanner: public-endpoint - mock dello spike S-05, solo su 127.0.0.1, risponde "ok" senza dati
async fn slow(State(st): State<AppState>, RawQuery(query): RawQuery) -> &'static str {
    let n = st.metrics.started.fetch_add(1, Ordering::SeqCst);
    let mut guard = match parse_id(query.as_deref()) {
        Some(id) => DropGuard::with_id(Arc::clone(&st.metrics), id),
        None => DropGuard::new(Arc::clone(&st.metrics)),
    };
    tokio::time::sleep(st.plan.delay_for(n)).await;
    guard.complete();
    "ok"
}

/// Avvia il server lento su `127.0.0.1:0` (HTTP/1 e HTTP/2 prior knowledge) e
/// restituisce l'indirizzo e i contatori.
pub async fn spawn_slow_server(delay: Duration) -> anyhow::Result<(SocketAddr, Arc<Metrics>)> {
    spawn_server(DelayPlan::Fixed(delay)).await
}

/// Come [`spawn_slow_server`], con un piano di ritardi qualsiasi.
///
/// `axum::serve` usa il builder `auto` di hyper-util: riconosce il preambolo HTTP/2
/// (prior knowledge, h2c) e altrimenti serve HTTP/1.1 sulla stessa porta.
pub async fn spawn_server(plan: DelayPlan) -> anyhow::Result<(SocketAddr, Arc<Metrics>)> {
    let metrics = Metrics::new();
    let state = AppState {
        metrics: Arc::clone(&metrics),
        plan,
    };
    let (tx, rx) = tokio::sync::oneshot::channel::<anyhow::Result<SocketAddr>>();
    std::thread::Builder::new()
        .name("s05-mock".to_owned())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(SERVER_WORKERS)
                .thread_name("s05-mock-worker")
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(Err(anyhow::Error::new(e).context("runtime del mock")));
                    return;
                }
            };
            rt.block_on(async move {
                let listener = match tokio::net::TcpListener::bind(("127.0.0.1", 0)).await {
                    Ok(l) => l,
                    Err(e) => {
                        let _ = tx.send(Err(anyhow::Error::new(e).context("bind del mock")));
                        return;
                    }
                };
                match listener.local_addr() {
                    Ok(a) => {
                        let _ = tx.send(Ok(a));
                    }
                    Err(e) => {
                        let _ = tx.send(Err(anyhow::Error::new(e).context("indirizzo del mock")));
                        return;
                    }
                }
                let app = Router::new().route(SLOW_PATH, get(slow)).with_state(state);
                if let Err(e) = axum::serve(listener, app).await {
                    eprintln!("s05 mock: serve terminato con errore: {e}");
                }
            });
        })?;
    let addr = rx
        .await
        .map_err(|_| anyhow::anyhow!("il thread del mock è terminato prima del bind"))??;
    Ok((addr, metrics))
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
