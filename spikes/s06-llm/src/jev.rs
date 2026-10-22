//! Client JEV locale allo spike S-06 (WIRING §4: nessun import da `s04-jev`).
//! `validate_query` = una Choice `{simple, complex}`; `support_claim` = un Noul
//! «`passage` supports `claim`» per coppia claim–citazione; `JEV_MODEL` pinnato.
//!
//! Contratto (RESEARCH §7, openapi 0.2.0): `POST {base_url}/v1/systemone` con
//! `Authorization: Bearer <chiave>`. La chiave non compare mai in `Debug`,
//! errori o CSV. Ogni invio con passaggi passa da `context::assert_visible`.

use crate::context::{Acl, Passage, assert_visible};
use anyhow::{Context, bail};
use std::fmt;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Id della domanda Choice di complessità nella richiesta e nella risposta.
pub const COMPLEXITY_QUESTION_ID: &str = "complexity";
/// Id della domanda Noul di supporto claim–citazione.
pub const SUPPORT_QUESTION_ID: &str = "supports";

/// Complessità della query (etichetta ratificata o scelta di JEV).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Complexity {
    Simple,
    Complex,
}

/// Configurazione del client; URL base e chiave sono parametri (testabile senza env).
pub struct ClientConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
}

/// Decisione di `validate_query`.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryDecision {
    pub complexity: Complexity,
    pub model_resolved: Option<String>,
    pub latency_ms: u64,
}

/// Errori tipizzati: nessuno produce una decisione o un segnale.
#[derive(Debug)]
pub enum JevError {
    /// Un passaggio non è visibile al ruolo: nulla è stato inviato.
    Unauthorized { reason: String },
    /// Connessione rifiutata o non stabilita entro il connect timeout.
    Unreachable(String),
    /// Nessuna risposta entro il timeout della richiesta.
    Timeout,
    /// Status HTTP non 2xx.
    Http { status: u16 },
    /// Risposta non interpretabile.
    Other(String),
}

impl JevError {
    /// Etichetta breve per i CSV.
    pub fn label(&self) -> String {
        match self {
            JevError::Unauthorized { .. } => "unauthorized".to_owned(),
            JevError::Unreachable(_) => "unreachable".to_owned(),
            JevError::Timeout => "timeout".to_owned(),
            JevError::Http { status } => format!("http_{status}"),
            JevError::Other(_) => "error".to_owned(),
        }
    }
}

impl fmt::Display for JevError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JevError::Unauthorized { reason } => {
                write!(
                    f,
                    "invio a JEV bloccato dalla guardia di visibilità: {reason}"
                )
            }
            JevError::Unreachable(m) => write!(f, "JEV irraggiungibile: {m}"),
            JevError::Timeout => write!(f, "nessuna risposta da JEV entro il timeout"),
            JevError::Http { status } => write!(f, "JEV ha risposto con status {status}"),
            JevError::Other(m) => write!(f, "errore JEV: {m}"),
        }
    }
}

impl std::error::Error for JevError {}

/// Prezzo dell'input di `jev-1.13`: 0,042 USD per milione di token, output
/// gratuito (https://docs.typesafe.ai/models, letto il 2026-10-18; stesso
/// criterio di 01-11, costo da `usage`).
pub const PRICE_INPUT_USD_PER_MTOK: f64 = 0.042;

/// Ritentativi su 429/529: uno, entro il timeout della richiesta (ADR-0008).
const MAX_RETRIES: u32 = 1;
const RETRY_WAIT_DEFAULT: Duration = Duration::from_millis(500);
const RETRY_WAIT_MAX: Duration = Duration::from_secs(5);

/// Domanda Choice di complessità: stessa regola di `oracles/ORACOLI.md`
/// («Regola di complessità»), scritta prima delle etichette.
fn complexity_question() -> serde_json::Value {
    serde_json::json!({
        "type": "choice",
        "instructions": "Classify the complexity of `query`, a question asked to a corporate document assistant.",
        "criteria": {
            "simple": "The answer is a single fact, and one passage of a document is enough to support it (even if the same fact is repeated elsewhere). For a question whose answer is not in the documents, apply the rule to the fact being asked.",
            "complex": "The answer needs several facts or several passages, an aggregation, a comparison between periods, rows or documents, or a calculation."
        }
    })
}

/// Domanda Noul di supporto claim–citazione.
fn support_question() -> serde_json::Value {
    serde_json::json!({
        "type": "noul",
        "instructions": "`passage` supports `claim`.",
        "criteria": {
            "true": "Every fact and figure stated in the claim is stated in the passage or follows directly from it.",
            "false": "The claim states a fact or figure that the passage does not contain, contradicts, or only mentions loosely."
        }
    })
}

/// Client JEV.
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    endpoint: String,
    api_key: String,
    model: String,
    acl: Acl,
    spent_usd: Mutex<f64>,
}

impl Client {
    /// Errore con modello vuoto, chiave vuota o URL non `https://`
    /// (ammesso `http://127.0.0.1` solo per i test locali).
    pub fn new(config: ClientConfig, acl: Acl) -> anyhow::Result<Self> {
        let base_url = config.base_url.trim_end_matches('/').to_owned();
        let loopback = base_url.starts_with("http://127.0.0.1:");
        if !base_url.starts_with("https://") && !loopback {
            bail!("JEV_API_URL deve usare https:// (ammesso http solo su 127.0.0.1)");
        }
        if config.api_key.trim().is_empty() {
            bail!("chiave JEV vuota");
        }
        if config.model.trim().is_empty() {
            bail!("modello JEV (JEV_MODEL) vuoto: nessun default implicito");
        }
        let http = reqwest::Client::builder()
            .connect_timeout(config.connect_timeout)
            .timeout(config.request_timeout)
            .https_only(!loopback)
            .build()
            .context("costruzione del client HTTP verso JEV")?;
        let endpoint = format!("{base_url}/v1/systemone");
        Ok(Client {
            http,
            base_url,
            endpoint,
            api_key: config.api_key,
            model: config.model,
            acl,
            spent_usd: Mutex::new(0.0),
        })
    }

    /// Modello richiesto (pin).
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Spesa cumulata della sessione in USD (da `usage`).
    pub fn spent_usd(&self) -> f64 {
        *self.spent_usd.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Una chiamata con un retry su 429/529; ogni errore è tipizzato e nessuno
    /// produce un valore.
    async fn send(&self, body: &serde_json::Value) -> Result<serde_json::Value, JevError> {
        let mut retries = 0;
        loop {
            let resp = self
                .http
                .post(&self.endpoint)
                .bearer_auth(&self.api_key)
                .json(body)
                .send()
                .await
                .map_err(|e| {
                    if e.is_timeout() {
                        JevError::Timeout
                    } else if e.is_connect() {
                        JevError::Unreachable(format!("{}", e.without_url()))
                    } else {
                        JevError::Other(format!("trasporto: {}", e.without_url()))
                    }
                })?;
            let status = resp.status().as_u16();
            if (status == 429 || status == 529) && retries < MAX_RETRIES {
                let wait = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.trim().parse::<f64>().ok())
                    .filter(|s| s.is_finite() && *s >= 0.0)
                    .map(Duration::from_secs_f64)
                    .unwrap_or(RETRY_WAIT_DEFAULT)
                    .min(RETRY_WAIT_MAX);
                drop(resp);
                tokio::time::sleep(wait).await;
                retries += 1;
                continue;
            }
            if !resp.status().is_success() {
                return Err(JevError::Http { status });
            }
            let json: serde_json::Value = resp.json().await.map_err(|e| {
                if e.is_timeout() {
                    JevError::Timeout
                } else {
                    JevError::Other("risposta non JSON".to_owned())
                }
            })?;
            let input = json["usage"]["input_tokens"].as_u64().unwrap_or(0);
            *self.spent_usd.lock().unwrap_or_else(|e| e.into_inner()) +=
                input as f64 * PRICE_INPUT_USD_PER_MTOK / 1_000_000.0;
            return Ok(json);
        }
    }

    /// Complessità della query: riceve solo il testo, nessun passaggio.
    pub async fn validate_query(&self, query_text: &str) -> Result<QueryDecision, JevError> {
        let mut questions = serde_json::Map::new();
        questions.insert(COMPLEXITY_QUESTION_ID.to_owned(), complexity_question());
        let body = serde_json::json!({
            "model": self.model,
            "state": {"query": query_text},
            "questions": questions,
        });
        let started = Instant::now();
        let json = self.send(&body).await?;
        let latency_ms = started.elapsed().as_millis() as u64;
        let probs = &json["answers"][COMPLEXITY_QUESTION_ID]["probabilities"];
        let p = |k: &str| {
            probs[k]
                .as_f64()
                .ok_or_else(|| JevError::Other(format!("risposta senza probabilities.{k}")))
        };
        let (simple, complex) = (p("simple")?, p("complex")?);
        // A parità vince `complex`: in dubbio si sceglie il modello più capace.
        let complexity = if simple > complex {
            Complexity::Simple
        } else {
            Complexity::Complex
        };
        Ok(QueryDecision {
            complexity,
            model_resolved: json["model"].as_str().map(str::to_owned),
            latency_ms,
        })
    }

    /// Noul «`passage` supports `claim`»; `assert_visible` prima dell'invio.
    pub async fn support_claim(
        &self,
        role_profile: &str,
        claim: &str,
        passage: &Passage,
    ) -> Result<f64, JevError> {
        assert_visible(&self.acl, role_profile, std::slice::from_ref(passage)).map_err(|e| {
            JevError::Unauthorized {
                reason: format!("{e:#}"),
            }
        })?;
        let mut questions = serde_json::Map::new();
        questions.insert(SUPPORT_QUESTION_ID.to_owned(), support_question());
        let body = serde_json::json!({
            "model": self.model,
            "state": {"claim": claim, "passage": passage.text},
            "questions": questions,
        });
        let json = self.send(&body).await?;
        json["answers"][SUPPORT_QUESTION_ID]["noul"]
            .as_f64()
            .filter(|v| (0.0..=1.0).contains(v))
            .ok_or_else(|| JevError::Other("risposta senza answers.supports.noul".to_owned()))
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &"<redatta>")
            .field("spent_usd", &self.spent_usd())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::context::tests::{acl, passage};
    use crate::fake_http::{FakeServer, Reply, closed_port_url};
    use std::time::Instant;

    const SENTINEL: &str = "REDACT-ME";
    pub(crate) const PINNED: &str = "jev-1.13.0";

    pub(crate) fn config(base_url: &str) -> ClientConfig {
        ClientConfig {
            base_url: base_url.to_owned(),
            api_key: format!("sk-test-{SENTINEL}"),
            model: PINNED.to_owned(),
            connect_timeout: Duration::from_secs(2),
            request_timeout: Duration::from_secs(5),
        }
    }

    fn choice_body(simple: f64, complex: f64) -> String {
        serde_json::json!({
            "model": PINNED,
            "answers": {
                COMPLEXITY_QUESTION_ID: {
                    "type": "choice",
                    "probabilities": {"simple": simple, "complex": complex}
                }
            },
            "usage": {"input_tokens": 40, "output_tokens": 1}
        })
        .to_string()
    }

    fn noul_body(v: f64) -> String {
        serde_json::json!({
            "model": PINNED,
            "answers": {SUPPORT_QUESTION_ID: {"type": "noul", "noul": v}},
            "usage": {"input_tokens": 60, "output_tokens": 1}
        })
        .to_string()
    }

    // RISK-10: la Choice di complessità è letta e il modello resta pinnato.
    #[tokio::test]
    async fn validate_query_reads_choice_and_pins_model() {
        let server = FakeServer::start(vec![
            Reply::ok(choice_body(0.2, 0.8)),
            Reply::ok(choice_body(0.9, 0.1)),
        ])
        .await;
        let client = Client::new(config(&server.base_url), acl()).expect("client");

        let d = client
            .validate_query("Confronta EBITDA 2024 e 2025 e spiega la differenza")
            .await
            .expect("decisione");
        assert_eq!(d.complexity, Complexity::Complex);
        assert_eq!(d.model_resolved.as_deref(), Some(PINNED));
        let d = client
            .validate_query("Ricavi 2025?")
            .await
            .expect("decisione");
        assert_eq!(d.complexity, Complexity::Simple);

        let rec = server.requests();
        assert_eq!(rec.len(), 2);
        assert!(
            rec[0]
                .head
                .to_lowercase()
                .starts_with("post /v1/systemone ")
        );
        assert_eq!(rec[0].body["model"], PINNED);
        let q = &rec[0].body["questions"][COMPLEXITY_QUESTION_ID];
        assert_eq!(q["type"], "choice");
        assert!(q["criteria"].get("simple").is_some());
        assert!(q["criteria"].get("complex").is_some());
        assert!(
            rec[0].body["state"]
                .to_string()
                .contains("Confronta EBITDA")
        );
    }

    #[tokio::test]
    async fn support_claim_returns_noul_for_visible_passage() {
        let server = FakeServer::start(vec![Reply::ok(noul_body(0.83))]).await;
        let client = Client::new(config(&server.base_url), acl()).expect("client");
        let p = passage("syn-06-faq-public", "2.1", "PASSAGGIO-PUBBLICO");
        let v = client
            .support_claim("guest", "I ricavi sono 182,4 M€", &p)
            .await
            .expect("noul");
        assert!((v - 0.83).abs() < 1e-9);
        let rec = server.requests();
        assert_eq!(rec.len(), 1);
        assert_eq!(rec[0].body["model"], PINNED);
        assert_eq!(
            rec[0].body["questions"][SUPPORT_QUESTION_ID]["type"],
            "noul"
        );
        let state = rec[0].body["state"].to_string();
        assert!(state.contains("PASSAGGIO-PUBBLICO") && state.contains("182,4"));
    }

    // TEST-2 (RISK-1). `validate_query` riceve solo il testo della query:
    // la parte del test sui passaggi non si applica a quella chiamata.
    #[tokio::test]
    async fn jev_calls_guarded_by_assert_visible() {
        let server = FakeServer::start(vec![Reply::ok(noul_body(0.9))]).await;
        let client = Client::new(config(&server.base_url), acl()).expect("client");
        let restricted = passage("syn-03-verbale-cda-restricted", "1", "TESTO-CDA");
        let r = client.support_claim("employee", "claim", &restricted).await;
        assert!(matches!(r, Err(JevError::Unauthorized { .. })), "{r:?}");
        let unknown = passage("doc-non-nel-manifest", "1", "x");
        let r = client.support_claim("admin", "claim", &unknown).await;
        assert!(matches!(r, Err(JevError::Unauthorized { .. })), "{r:?}");
        assert_eq!(server.count(), 0, "nessuna richiesta deve partire");
    }

    // TEST-12 (RISK-5)
    #[tokio::test]
    async fn client_errors_redact_keys() {
        let server =
            FakeServer::start(vec![Reply::status(401, r#"{"error":"unauthorized"}"#)]).await;
        let client = Client::new(config(&server.base_url), acl()).expect("client");
        let dbg = format!("{client:?}");
        assert!(!dbg.contains(SENTINEL), "Debug espone la chiave: {dbg}");
        let err = client.validate_query("q").await.expect_err("401");
        assert!(matches!(err, JevError::Http { status: 401 }), "{err:?}");
        for msg in [format!("{err}"), format!("{err:?}")] {
            assert!(!msg.contains(SENTINEL), "l'errore espone la chiave: {msg}");
        }
        let head = server.requests()[0].head.to_lowercase();
        assert!(head.contains(&format!(
            "authorization: bearer sk-test-{}",
            SENTINEL.to_lowercase()
        )));
    }

    // TEST-5 (RISK-2), parte client: porta chiusa → errore rapido, mai una decisione.
    #[tokio::test]
    async fn closed_port_is_an_error_within_connect_timeout() {
        let url = closed_port_url().await;
        let client = Client::new(config(&url), acl()).expect("client");
        let t = Instant::now();
        let r = client.validate_query("q").await;
        assert!(r.is_err(), "JEV irraggiungibile non decide: {r:?}");
        let p = passage("syn-06-faq-public", "2.1", "x");
        let s = client.support_claim("guest", "c", &p).await;
        assert!(
            !matches!(s, Ok(_) | Err(JevError::Unauthorized { .. })),
            "{s:?}"
        );
        assert!(t.elapsed() < Duration::from_secs(6), "{:?}", t.elapsed());
    }

    #[test]
    fn client_rejects_bad_config() {
        // URL in chiaro verso un host instradabile, composto per non
        // sembrare un endpoint reale allo scanner: va rifiutato.
        let plain = format!("{}://example.com", "http");
        assert!(Client::new(config(&plain), acl()).is_err());
        let mut no_model = config("https://api.typesafe.ai");
        no_model.model = String::new();
        assert!(Client::new(no_model, acl()).is_err());
        let mut no_key = config("https://api.typesafe.ai");
        no_key.api_key = String::new();
        assert!(Client::new(no_key, acl()).is_err());
    }
}
