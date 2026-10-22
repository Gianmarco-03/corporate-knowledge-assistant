//! Client JEV locale allo spike S-06 (WIRING §4: nessun import da `s04-jev`).
//! `validate_query` = una Choice `{simple, complex}`; `support_claim` = un Noul
//! «`passage` supports `claim`» per coppia claim–citazione; `JEV_MODEL` pinnato.
//!
//! Contratto (RESEARCH §7, openapi 0.2.0): `POST {base_url}/v1/systemone` con
//! `Authorization: Bearer <chiave>`. La chiave non compare mai in `Debug`,
//! errori o CSV. Ogni invio con passaggi passa da `context::assert_visible`.

use crate::context::{Acl, Passage};
use std::fmt;
use std::time::Duration;

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

impl fmt::Display for JevError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // stub del test-write: da implementare in 01-12
        let _ = f;
        todo!()
    }
}

impl std::error::Error for JevError {}

/// Client JEV.
pub struct Client {
    // stub del test-write: da implementare in 01-12
    _stub: (),
}

impl Client {
    /// Errore con modello vuoto, chiave vuota o URL non `https://`
    /// (ammesso `http://127.0.0.1` solo per i test locali).
    pub fn new(config: ClientConfig, acl: Acl) -> anyhow::Result<Self> {
        // stub del test-write: da implementare in 01-12
        let _ = (config, acl);
        todo!()
    }

    /// Complessità della query: riceve solo il testo, nessun passaggio.
    pub async fn validate_query(&self, query_text: &str) -> Result<QueryDecision, JevError> {
        // stub del test-write: da implementare in 01-12
        let _ = query_text;
        todo!()
    }

    /// Noul «`passage` supports `claim`»; `assert_visible` prima dell'invio.
    pub async fn support_claim(
        &self,
        role_profile: &str,
        claim: &str,
        passage: &Passage,
    ) -> Result<f64, JevError> {
        // stub del test-write: da implementare in 01-12
        let _ = (role_profile, claim, passage);
        todo!()
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // stub del test-write: da implementare in 01-12
        let _ = f;
        todo!()
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
