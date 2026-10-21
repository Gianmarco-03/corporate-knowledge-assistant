//! Client reqwest verso i provider e fan-out con deadline globale (spike S-05).

use futures::StreamExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::Instant;

/// Connessioni inattive tenute nel pool per host: è il tetto delle connessioni stabilite
/// verso il mock nella soglia di leak (RISULTATI §Soglie).
pub const POOL_MAX_IDLE_PER_HOST: usize = 8;

/// Protocollo verso il mock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proto {
    H1,
    H2,
}

/// Esito di un fan-out con deadline.
#[derive(Debug, Clone)]
pub struct FanoutOutcome {
    /// Microsecondi fra la deadline e il ritorno del chiamante (negativo se prima).
    pub exit_after_deadline_us: i64,
    pub completed: usize,
    pub cancelled: usize,
}

/// `reqwest::Client` con `connect_timeout(2 s)`, `pool_idle_timeout(90 s)`;
/// `Proto::H2` usa `http2_prior_knowledge()`.
///
/// Client per il solo mock su loopback: nessun proxy di sistema (un `HTTP_PROXY`
/// dell'ambiente deviarebbe le richieste fuori dal processo). TLS di default di
/// reqwest 0.13 (Rustls), qui non usato perché il mock è in chiaro su 127.0.0.1.
pub fn build_client(proto: Proto) -> anyhow::Result<reqwest::Client> {
    let builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(POOL_MAX_IDLE_PER_HOST)
        .no_proxy();
    let builder = match proto {
        Proto::H1 => builder.http1_only(),
        Proto::H2 => builder.http2_prior_knowledge(),
    };
    Ok(builder.build()?)
}

/// Tetto del corpo letto dal mock (risponde `ok`).
const MAX_BODY: usize = 64 * 1024;

/// Una richiesta GET completa: `true` se la risposta è 2xx e il corpo, letto a pezzi
/// entro [`MAX_BODY`], arriva per intero.
pub async fn call(client: &reqwest::Client, endpoint: &str) -> bool {
    let Ok(mut resp) = client.get(endpoint).send().await else {
        return false;
    };
    if !resp.status().is_success() {
        return false;
    }
    let mut total = 0usize;
    while let Ok(next) = resp.chunk().await {
        match next {
            Some(piece) => {
                total += piece.len();
                if total > MAX_BODY {
                    return false;
                }
            }
            None => return true,
        }
    }
    false
}

/// Microsecondi con segno fra `deadline` e `now` (positivo = dopo la deadline).
pub fn signed_us_after(now: Instant, deadline: Instant) -> i64 {
    match now.checked_duration_since(deadline) {
        Some(after) => i64::try_from(after.as_micros()).unwrap_or(i64::MAX),
        None => -i64::try_from((deadline - now).as_micros()).unwrap_or(i64::MAX),
    }
}

/// Variante A: `timeout_at(deadline, iter.map(call).buffer_unordered(concurrency).collect())`.
///
/// Allo scadere `timeout_at` restituisce `Elapsed` e distrugge lo stream: i future
/// reqwest in volo vengono droppati (h2: `RST_STREAM`; h1: chiusura della connessione)
/// e quelli non ancora avviati non partono mai.
pub async fn fanout_variant_a(
    client: &reqwest::Client,
    url: &str,
    fanout: usize,
    concurrency: usize,
    deadline: Instant,
) -> FanoutOutcome {
    let completed = AtomicUsize::new(0);
    let work = futures::stream::iter(0..fanout)
        .map(|_| {
            let completed = &completed;
            async move {
                if call(client, url).await {
                    completed.fetch_add(1, Ordering::SeqCst);
                }
            }
        })
        .buffer_unordered(concurrency.max(1))
        .collect::<Vec<()>>();
    let _ = tokio::time::timeout_at(deadline, work).await;
    let now = Instant::now();
    outcome(now, deadline, fanout, completed.load(Ordering::SeqCst))
}

/// Variante B: `JoinSet` + `Arc<Semaphore>(concurrency)`, abort allo scadere della deadline.
///
/// Tutti i task partono subito e attendono un permesso; allo scadere il `JoinSet` viene
/// distrutto, il che fa `abort` di ogni task (in volo o in attesa del permesso).
pub async fn fanout_variant_b(
    client: &reqwest::Client,
    url: &str,
    fanout: usize,
    concurrency: usize,
    deadline: Instant,
) -> FanoutOutcome {
    let sem = Arc::new(Semaphore::new(concurrency.max(1)));
    let completed = Arc::new(AtomicUsize::new(0));
    let mut set = JoinSet::new();
    for _ in 0..fanout {
        let client = client.clone();
        let endpoint = url.to_owned();
        let sem = Arc::clone(&sem);
        let completed = Arc::clone(&completed);
        set.spawn(async move {
            let Ok(_permit) = sem.acquire_owned().await else {
                return;
            };
            if call(&client, &endpoint).await {
                completed.fetch_add(1, Ordering::SeqCst);
            }
        });
    }
    while let Ok(Some(_)) = tokio::time::timeout_at(deadline, set.join_next()).await {}
    drop(set);
    let now = Instant::now();
    outcome(now, deadline, fanout, completed.load(Ordering::SeqCst))
}

fn outcome(now: Instant, deadline: Instant, fanout: usize, completed: usize) -> FanoutOutcome {
    FanoutOutcome {
        exit_after_deadline_us: signed_us_after(now, deadline),
        completed,
        cancelled: fanout.saturating_sub(completed),
    }
}

/// Esito di un ciclo di cancellazione verso il mock.
#[derive(Debug, Clone, Copy)]
pub struct CycleOutcome {
    /// Istante (std) in cui scade la cancellazione: riferimento per `server_saw_cancel_ms`.
    pub cancel_at: std::time::Instant,
    /// Microsecondi fra la cancellazione programmata e il ritorno del chiamante.
    pub exit_after_cancel_us: i64,
    /// La richiesta è terminata prima della cancellazione (non dovrebbe accadere).
    pub finished_before_cancel: bool,
}

/// Un ciclo: GET verso `endpoint`, cancellato (drop del future) a `cancel_after`.
pub async fn cancel_cycle(
    client: &reqwest::Client,
    endpoint: &str,
    cancel_after: Duration,
) -> CycleOutcome {
    let start = Instant::now();
    let cancel_at = start + cancel_after;
    let res = tokio::time::timeout_at(cancel_at, call(client, endpoint)).await;
    let now = Instant::now();
    CycleOutcome {
        cancel_at: cancel_at.into_std(),
        exit_after_cancel_us: signed_us_after(now, cancel_at),
        finished_before_cancel: res.is_ok(),
    }
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
