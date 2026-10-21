//! Client JEV dello spike S-04: forme di rank A, B, C con pin del modello,
//! retry su 429/529 entro la deadline, contatore di costo e guardia di
//! visibilità prima di ogni invio.
//!
//! Contratto (RESEARCH §7, openapi 0.2.0): `POST {base_url}/v1/systemone` con
//! `Authorization: Bearer <chiave>`. La chiave non compare mai in `Debug`, errori o CSV.

use crate::candidates::{Acl, Passage, Query};
use std::fmt;
use std::time::Duration;

/// Configurazione del client; chiave e URL base sono parametri (testabile senza env).
pub struct ClientConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub request_timeout: Duration,
    pub max_cost_usd: f64,
}

/// Metadati di una chiamata JEV arrivata a risposta.
#[derive(Debug, Clone, PartialEq)]
pub struct CallMeta {
    pub model_resolved: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub status: u16,
    pub latency_ms: u64,
    pub retry_after: Option<String>,
    pub retries: u32,
}

/// Esito di un rank: `(passage_id, segnale)` più i metadati di ogni chiamata.
#[derive(Debug, Clone, PartialEq)]
pub struct RankOutcome {
    pub relevance: Vec<(String, f64)>,
    pub calls: Vec<CallMeta>,
}

/// Errori tipizzati: nessuno produce un segnale di rilevanza.
#[derive(Debug)]
pub enum JevError {
    /// Un passaggio non è visibile al ruolo della query: nulla è stato inviato.
    Unauthorized { reason: String },
    /// 429 o 529 non risolti entro la deadline.
    RateLimited {
        status: u16,
        retry_after: Option<String>,
    },
    /// Deadline scaduta senza risposta.
    Timeout,
    /// Spesa della sessione oltre il budget: la richiesta non parte.
    BudgetExceeded { spent_usd: f64, budget_usd: f64 },
    /// Status HTTP non 2xx diverso da 429/529.
    Http { status: u16 },
    /// Errore di trasporto o di risposta non interpretabile.
    Other(String),
}

impl fmt::Display for JevError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // stub del test-write: da implementare in 01-11
        let _ = f;
        todo!()
    }
}

impl std::error::Error for JevError {}

/// Costo in USD dei token riportati da `usage`.
pub fn cost_usd(input_tokens: u64, output_tokens: u64) -> f64 {
    // stub del test-write: da implementare in 01-11
    let _ = (input_tokens, output_tokens);
    todo!()
}

/// Client JEV.
pub struct Client {
    // stub del test-write: da implementare in 01-11
    _stub: (),
}

impl Client {
    /// Costruttore da configurazione; errore con modello vuoto, chiave vuota o
    /// URL non `https://` (ammesso `http://127.0.0.1` solo per i test locali).
    pub fn new(config: ClientConfig, acl: Acl) -> anyhow::Result<Self> {
        // stub del test-write: da implementare in 01-11
        let _ = (config, acl);
        todo!()
    }

    /// Costruttore da `JEV_API_URL`, `TYPESAFE_API_KEY`, `JEV_MODEL`, `S04_MAX_COST_USD`.
    pub fn new_from_env(acl: Acl) -> anyhow::Result<Self> {
        // stub del test-write: da implementare in 01-11
        let _ = acl;
        todo!()
    }

    /// Forma A: una richiesta per candidato, al più `concurrency` in volo.
    pub async fn rank_form_a(
        &self,
        query: &Query,
        passages: &[Passage],
        concurrency: usize,
        deadline: Duration,
    ) -> Result<RankOutcome, JevError> {
        // stub del test-write: da implementare in 01-11
        let _ = (query, passages, concurrency, deadline);
        todo!()
    }

    /// Forma B: `state` unico con N domande Noul.
    pub async fn rank_form_b(
        &self,
        query: &Query,
        passages: &[Passage],
        deadline: Duration,
    ) -> Result<RankOutcome, JevError> {
        // stub del test-write: da implementare in 01-11
        let _ = (query, passages, deadline);
        todo!()
    }

    /// Forma C: una sola domanda Choice.
    pub async fn rank_form_c(
        &self,
        query: &Query,
        passages: &[Passage],
        deadline: Duration,
    ) -> Result<RankOutcome, JevError> {
        // stub del test-write: da implementare in 01-11
        let _ = (query, passages, deadline);
        todo!()
    }

    /// Passaggi non visibili effettivamente inviati (hard gate: deve restare 0).
    pub fn unauthorized_sent(&self) -> u64 {
        // stub del test-write: da implementare in 01-11
        todo!()
    }

    /// Spesa cumulata della sessione in USD.
    pub fn spent_usd(&self) -> f64 {
        // stub del test-write: da implementare in 01-11
        todo!()
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // stub del test-write: da implementare in 01-11
        let _ = f;
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const SENTINEL: &str = "REDACT-ME";
    const PINNED: &str = "jev-1.13.0";

    const ACL_JSON: &str = r#"{
        "classifications": ["public", "internal", "confidential", "restricted"],
        "roles": {
            "admin": ["public", "internal", "confidential", "restricted"],
            "manager": ["public", "internal", "confidential"],
            "employee": ["public", "internal"],
            "guest": ["public"]
        },
        "profiles": {}
    }"#;

    const MANIFEST_JSON: &str = r#"[
        {"doc_id": "doc-pub", "classification": "public"},
        {"doc_id": "doc-int", "classification": "internal"},
        {"doc_id": "doc-res", "classification": "restricted"}
    ]"#;

    fn acl() -> Acl {
        Acl::from_json(ACL_JSON, MANIFEST_JSON).expect("acl di test valida")
    }

    fn query() -> Query {
        Query {
            id: "Q-T01".to_owned(),
            text: "Qual è l'EBITDA 2025?".to_owned(),
            role_profile: "employee".to_owned(),
        }
    }

    fn passage(i: usize, doc_key: &str, classification: &str) -> Passage {
        Passage {
            id: format!("p-{i}"),
            doc_key: doc_key.to_owned(),
            classification: classification.to_owned(),
            text: format!("passaggio sintetico numero {i}"),
        }
    }

    fn config(base_url: &str) -> ClientConfig {
        ClientConfig {
            base_url: base_url.to_owned(),
            api_key: format!("sk-test-{SENTINEL}"),
            model: PINNED.to_owned(),
            request_timeout: Duration::from_secs(5),
            max_cost_usd: 1.0,
        }
    }

    /// Risposta scriptata del server finto.
    #[derive(Clone)]
    enum Reply {
        Respond {
            status: u16,
            headers: Vec<(&'static str, String)>,
            body: String,
        },
        /// Legge la richiesta e non risponde mai.
        Hang,
    }

    fn ok(body: String) -> Reply {
        Reply::Respond {
            status: 200,
            headers: vec![],
            body,
        }
    }

    fn throttled(status: u16, retry_after: &str) -> Reply {
        Reply::Respond {
            status,
            headers: vec![("retry-after", retry_after.to_owned())],
            body: r#"{"error":"rate limited"}"#.to_owned(),
        }
    }

    /// Corpo di una risposta di forma A.
    fn answer_a(model: &str, relevant: f64, input_tokens: u64) -> String {
        serde_json::json!({
            "model": model,
            "answers": {
                "relevant": {"type": "noul", "noul": relevant},
                "sufficient": {"type": "noul", "noul": 0.5},
                "instructs_model": {"type": "noul", "noul": 0.0}
            },
            "usage": {"input_tokens": input_tokens, "output_tokens": 3}
        })
        .to_string()
    }

    struct Recorded {
        head: String,
        body: serde_json::Value,
        at: Instant,
    }

    /// Server HTTP/1.1 minimo su loopback: una richiesta per connessione,
    /// risposte prese in ordine dallo script (l'ultima si ripete).
    struct FakeJev {
        base_url: String,
        log: Arc<Mutex<Vec<Recorded>>>,
    }

    impl FakeJev {
        async fn start(script: Vec<Reply>) -> FakeJev {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let port = listener.local_addr().expect("addr").port();
            let base_url = format!("http://127.0.0.1:{port}");
            let log: Arc<Mutex<Vec<Recorded>>> = Arc::default();
            let log_srv = Arc::clone(&log);
            tokio::spawn(async move {
                loop {
                    let Ok((mut sock, _)) = listener.accept().await else {
                        return;
                    };
                    let log = Arc::clone(&log_srv);
                    let script = script.clone();
                    tokio::spawn(async move {
                        let mut buf = Vec::new();
                        let mut chunk = [0u8; 8192];
                        let head_end = loop {
                            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                                break pos + 4;
                            }
                            match sock.read(&mut chunk).await {
                                Ok(0) | Err(_) => return,
                                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                            }
                        };
                        let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
                        let len = head
                            .lines()
                            .find_map(|l| {
                                let (k, v) = l.split_once(':')?;
                                k.trim()
                                    .eq_ignore_ascii_case("content-length")
                                    .then(|| v.trim().parse::<usize>().ok())?
                            })
                            .unwrap_or(0);
                        while buf.len() < head_end + len {
                            match sock.read(&mut chunk).await {
                                Ok(0) | Err(_) => return,
                                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                            }
                        }
                        let body = serde_json::from_slice(&buf[head_end..head_end + len])
                            .unwrap_or(serde_json::Value::Null);
                        let idx = {
                            let mut log = log.lock().expect("log");
                            log.push(Recorded {
                                head,
                                body,
                                at: Instant::now(),
                            });
                            log.len() - 1
                        };
                        let reply = script
                            .get(idx)
                            .or_else(|| script.last())
                            .cloned()
                            .expect("script non vuoto");
                        match reply {
                            Reply::Hang => {
                                tokio::time::sleep(Duration::from_secs(30)).await;
                            }
                            Reply::Respond {
                                status,
                                headers,
                                body,
                            } => {
                                let mut out = format!(
                                    "HTTP/1.1 {status} Test\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n",
                                    body.len()
                                );
                                for (k, v) in headers {
                                    out.push_str(&format!("{k}: {v}\r\n"));
                                }
                                out.push_str("\r\n");
                                out.push_str(&body);
                                let _ = sock.write_all(out.as_bytes()).await;
                                let _ = sock.shutdown().await;
                            }
                        }
                    });
                }
            });
            FakeJev { base_url, log }
        }

        fn count(&self) -> usize {
            self.log.lock().expect("log").len()
        }
    }

    // TEST-2 (RISK-1)
    #[tokio::test]
    async fn send_path_blocks_before_network_on_invisible_passage() {
        let server = FakeJev::start(vec![ok(answer_a(PINNED, 0.9, 10))]).await;
        let client = Client::new(config(&server.base_url), acl()).expect("client");
        let mut passages: Vec<Passage> = (0..20).map(|i| passage(i, "doc-pub", "public")).collect();
        // Il passaggio invisibile sta a metà: una guardia per singolo invio
        // avrebbe già spedito i primi dieci.
        passages[10] = passage(10, "doc-res", "restricted");
        let q = query();
        let deadline = Duration::from_secs(3);

        let a = client.rank_form_a(&q, &passages, 4, deadline).await;
        assert!(
            matches!(a, Err(JevError::Unauthorized { .. })),
            "forma A: atteso Unauthorized, ottenuto {a:?}"
        );
        let b = client.rank_form_b(&q, &passages, deadline).await;
        assert!(
            matches!(b, Err(JevError::Unauthorized { .. })),
            "forma B: atteso Unauthorized, ottenuto {b:?}"
        );
        let c = client.rank_form_c(&q, &passages, deadline).await;
        assert!(
            matches!(c, Err(JevError::Unauthorized { .. })),
            "forma C: atteso Unauthorized, ottenuto {c:?}"
        );

        assert_eq!(server.count(), 0, "nessuna richiesta deve partire");
        assert_eq!(client.unauthorized_sent(), 0);
    }

    // TEST-6 (RISK-2)
    #[tokio::test]
    async fn client_errors_and_debug_redact_key() {
        let server = FakeJev::start(vec![Reply::Respond {
            status: 401,
            headers: vec![],
            body: r#"{"error":"unauthorized"}"#.to_owned(),
        }])
        .await;
        let client = Client::new(config(&server.base_url), acl()).expect("client");

        let dbg = format!("{client:?}");
        assert!(!dbg.contains(SENTINEL), "Debug espone la chiave: {dbg}");

        let passages = vec![passage(0, "doc-pub", "public")];
        let err = client
            .rank_form_a(&query(), &passages, 1, Duration::from_secs(3))
            .await
            .expect_err("401 deve essere un errore, mai un segnale");
        assert!(
            matches!(err, JevError::Http { status: 401 }),
            "ottenuto {err:?}"
        );
        for msg in [format!("{err:#}"), format!("{err:?}")] {
            assert!(!msg.contains(SENTINEL), "l'errore espone la chiave: {msg}");
        }

        let log = server.log.lock().expect("log");
        assert_eq!(log.len(), 1);
        let auth = log[0]
            .head
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("authorization:"))
            .expect("header Authorization presente");
        assert_eq!(
            auth.split_once(':').map(|(_, v)| v.trim()),
            Some(format!("Bearer sk-test-{SENTINEL}").as_str())
        );
    }

    // TEST-7 (RISK-3)
    #[tokio::test]
    async fn retry_after_honoured_within_deadline() {
        for status in [429u16, 529] {
            let server =
                FakeJev::start(vec![throttled(status, "1"), ok(answer_a(PINNED, 0.9, 10))]).await;
            let client = Client::new(config(&server.base_url), acl()).expect("client");
            let passages = vec![passage(0, "doc-pub", "public")];

            let out = client
                .rank_form_a(&query(), &passages, 1, Duration::from_secs(3))
                .await
                .unwrap_or_else(|e| panic!("{status}: atteso esito positivo dopo il retry, {e:?}"));

            let log = server.log.lock().expect("log");
            assert_eq!(log.len(), 2, "{status}: una richiesta e un solo retry");
            let gap = log[1].at.duration_since(log[0].at);
            assert!(
                gap >= Duration::from_secs(1),
                "{status}: retry-after non rispettato ({gap:?})"
            );
            assert_eq!(out.relevance, vec![("p-0".to_owned(), 0.9)]);
            assert_eq!(
                out.calls.len(),
                1,
                "{status}: metadati di una chiamata logica"
            );
            assert_eq!(out.calls[0].retries, 1, "{status}: retries registrati");
            assert_eq!(out.calls[0].status, 200);
        }
    }

    // TEST-8 (RISK-3)
    #[tokio::test]
    async fn rate_limited_beyond_deadline_is_typed_error_not_signal() {
        let server = FakeJev::start(vec![throttled(429, "5")]).await;
        let client = Client::new(config(&server.base_url), acl()).expect("client");
        let passages = vec![passage(0, "doc-pub", "public")];
        let deadline = Duration::from_secs(1);

        let started = Instant::now();
        let res = client.rank_form_a(&query(), &passages, 1, deadline).await;
        let elapsed = started.elapsed();

        assert!(
            matches!(res, Err(JevError::RateLimited { status: 429, .. })),
            "atteso RateLimited, ottenuto {res:?}"
        );
        assert!(
            elapsed <= deadline + Duration::from_millis(100),
            "chiamata oltre la deadline: {elapsed:?}"
        );
        assert_eq!(server.count(), 1, "nessun retry oltre la deadline");
    }

    // TEST-8 (RISK-3): server che non risponde mai → Timeout entro la deadline.
    #[tokio::test]
    async fn silent_server_is_timeout_within_deadline() {
        let server = FakeJev::start(vec![Reply::Hang]).await;
        let client = Client::new(config(&server.base_url), acl()).expect("client");
        let passages = vec![passage(0, "doc-pub", "public")];
        let deadline = Duration::from_secs(1);

        let started = Instant::now();
        let res = client.rank_form_a(&query(), &passages, 1, deadline).await;
        let elapsed = started.elapsed();

        assert!(
            matches!(res, Err(JevError::Timeout)),
            "atteso Timeout, ottenuto {res:?}"
        );
        assert!(
            elapsed <= deadline + Duration::from_millis(100),
            "timeout oltre la deadline: {elapsed:?}"
        );
    }

    // TEST-10 (RISK-5)
    #[tokio::test]
    async fn request_uses_configured_model_and_records_resolved() {
        let server = FakeJev::start(vec![ok(answer_a("jev-1.13.0-build-x", 0.7, 10))]).await;
        let client = Client::new(config(&server.base_url), acl()).expect("client");
        let passages = vec![passage(0, "doc-pub", "public")];

        let out = client
            .rank_form_a(&query(), &passages, 1, Duration::from_secs(3))
            .await
            .expect("esito positivo");

        let log = server.log.lock().expect("log");
        assert_eq!(log.len(), 1);
        let body = &log[0].body;
        assert_eq!(body["model"], PINNED, "modello da configurazione");
        assert_eq!(body["state"]["query"], query().text.as_str());
        assert_eq!(body["state"]["passage"], passages[0].text.as_str());
        assert_eq!(body["questions"]["relevant"]["type"], "noul");
        assert!(
            log[0].head.starts_with("POST /v1/systemone "),
            "endpoint: {}",
            log[0].head.lines().next().unwrap_or_default()
        );

        assert_eq!(out.relevance, vec![("p-0".to_owned(), 0.7)]);
        assert_eq!(
            out.calls[0].model_resolved.as_deref(),
            Some("jev-1.13.0-build-x")
        );
        assert_eq!(out.calls[0].input_tokens, Some(10));
    }

    // TEST-10 (RISK-5): nessun default silenzioso per il modello.
    #[test]
    fn client_requires_configured_model() {
        let mut cfg = config("http://127.0.0.1:9");
        cfg.model = String::new();
        assert!(
            Client::new(cfg, acl()).is_err(),
            "modello vuoto deve fallire"
        );

        // SAFETY: unico test del crate che legge o scrive queste variabili.
        unsafe {
            std::env::set_var("JEV_API_URL", "http://127.0.0.1:9");
            std::env::set_var("TYPESAFE_API_KEY", format!("sk-test-{SENTINEL}"));
            std::env::set_var("S04_MAX_COST_USD", "1");
            std::env::remove_var("JEV_MODEL");
        }
        let err = Client::new_from_env(acl()).expect_err("senza JEV_MODEL deve fallire");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("JEV_MODEL"),
            "l'errore deve nominare JEV_MODEL: {msg}"
        );
        assert!(!msg.contains(SENTINEL), "l'errore espone la chiave: {msg}");
    }

    // TEST-12 (RISK-6)
    #[test]
    fn cost_of_one_million_input_tokens() {
        let c = cost_usd(1_000_000, 0);
        assert!(
            (c - 0.042).abs() < 1e-9,
            "1 Mtok input = 0,042 USD, ottenuto {c}"
        );
        assert!(cost_usd(0, 0) == 0.0);
    }

    // TEST-12 (RISK-6)
    #[tokio::test]
    async fn cost_guard_stops_session_over_budget() {
        // 100.000 token input = 0,0042 USD > budget 0,001 dopo la prima risposta.
        let server = FakeJev::start(vec![ok(answer_a(PINNED, 0.9, 100_000))]).await;
        let mut cfg = config(&server.base_url);
        cfg.max_cost_usd = 0.001;
        let client = Client::new(cfg, acl()).expect("client");
        let passages = vec![passage(0, "doc-pub", "public")];
        let deadline = Duration::from_secs(3);

        client
            .rank_form_a(&query(), &passages, 1, deadline)
            .await
            .expect("la prima chiamata è sotto budget");
        assert!((client.spent_usd() - cost_usd(100_000, 3)).abs() < 1e-12);
        assert_eq!(server.count(), 1);

        let second = client.rank_form_a(&query(), &passages, 1, deadline).await;
        assert!(
            matches!(second, Err(JevError::BudgetExceeded { .. })),
            "atteso BudgetExceeded, ottenuto {second:?}"
        );
        assert_eq!(server.count(), 1, "oltre budget nessuna richiesta parte");
    }
}
