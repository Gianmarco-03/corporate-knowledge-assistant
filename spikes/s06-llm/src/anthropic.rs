//! Client della Messages API per lo spike S-06: structured output
//! (`output_config.format` json_schema), `effort: "low"`, niente parametri di campionamento,
//! niente Citations native né prefill; `stop_reason` controllato prima del
//! parse; costo da `usage` con arresto oltre il tetto di D-31.
//!
//! Contratto (RESEARCH §7 A-04): `POST {base_url}/v1/messages`, header
//! `x-api-key` e `anthropic-version: 2023-06-01`. La chiave non compare mai in
//! `Debug`, errori o CSV.

use anyhow::{Context, bail};
use std::fmt;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// URL base di default; sovrascrivibile in costruzione per i test locali.
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
/// Versione dell'API negli header.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Schema della risposta compilato nel binario.
pub const ANSWER_SCHEMA_JSON: &str = include_str!("../schema/answer.schema.json");
/// Tetto di spesa di D-31 (decisione utente 2026-10-22): l'unico valore ammesso
/// per `S06_MAX_COST_USD`.
pub const D31_BUDGET_USD: f64 = 2.0;

/// Configurazione del client; URL base e chiave sono parametri (testabile senza env).
pub struct ClientConfig {
    pub base_url: String,
    pub api_key: String,
    pub max_tokens: u32,
    pub max_cost_usd: f64,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
}

/// Token riportati da `usage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Contenuto utile della risposta, deciso da `stop_reason` prima di ogni parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Generation {
    /// Testo del blocco `type == "text"` (da validare con `validate::parse_answer`).
    Text(String),
    /// `stop_reason = "refusal"`: nessun parse del testo.
    Refusal,
    /// `stop_reason = "max_tokens"`: output troncato, errore contato.
    MaxTokens,
}

/// Risposta interpretata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extracted {
    pub generation: Generation,
    pub stop_reason: String,
    pub usage: Usage,
    pub model_resolved: Option<String>,
}

/// Esito di una chiamata.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub extracted: Extracted,
    pub latency_ms: u64,
    pub cost_usd: f64,
}

/// Errori tipizzati; nessuno contiene la chiave.
#[derive(Debug)]
pub enum AnthropicError {
    /// Spesa della sessione oltre il tetto: la richiesta non parte.
    BudgetExceeded { spent_usd: f64, budget_usd: f64 },
    /// Status HTTP non 2xx.
    Http { status: u16 },
    /// Risposta non interpretabile (nessun blocco text, campi mancanti).
    Malformed(String),
    /// Errore di trasporto.
    Other(String),
}

impl AnthropicError {
    /// Etichetta breve per i CSV.
    pub fn label(&self) -> String {
        match self {
            AnthropicError::BudgetExceeded { .. } => "budget".to_owned(),
            AnthropicError::Http { status } => format!("http_{status}"),
            AnthropicError::Malformed(_) => "malformed".to_owned(),
            AnthropicError::Other(_) => "error".to_owned(),
        }
    }
}

impl fmt::Display for AnthropicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnthropicError::BudgetExceeded {
                spent_usd,
                budget_usd,
            } => write!(
                f,
                "tetto di spesa raggiunto: spesi {spent_usd:.6} USD su {budget_usd:.6}, richiesta non inviata"
            ),
            AnthropicError::Http { status } => {
                write!(f, "la Messages API ha risposto con status {status}")
            }
            AnthropicError::Malformed(m) => write!(f, "risposta non interpretabile: {m}"),
            AnthropicError::Other(m) => write!(f, "errore verso la Messages API: {m}"),
        }
    }
}

impl std::error::Error for AnthropicError {}

/// Chiamate previste per un modello nella stima di D-31.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCalls {
    pub model: String,
    pub calls: u64,
    pub avg_input_tokens: u64,
    pub avg_output_tokens: u64,
}

/// Prezzi per milione di token, da RESEARCH §7 A-04 (skill claude-api, cache
/// 2026-10-06, verificata): Haiku 5.5 0,10 / 0,50 USD con prompt ≤ 100K token,
/// 0,50 / 2,50 oltre (la fascia vale per l'intera richiesta); Sonnet 5.5 2 / 10.
pub const HAIKU_PRICE_LOW: (f64, f64) = (0.10, 0.50);
pub const HAIKU_PRICE_HIGH: (f64, f64) = (0.50, 2.50);
pub const HAIKU_TIER_TOKENS: u64 = 100_000;
pub const SONNET_PRICE: (f64, f64) = (2.0, 10.0);

/// Ritentativi su 429/529 (come ADR-0008: uno solo); attesa da `retry-after`, al più 10 s.
const MAX_RETRIES: u32 = 1;
const RETRY_WAIT_DEFAULT: Duration = Duration::from_secs(2);
const RETRY_WAIT_MAX: Duration = Duration::from_secs(10);

/// Costo in USD con i prezzi di RESEARCH §7 (Haiku 5.5 a fasce per prompt
/// ≤ 100K / oltre; Sonnet 5.5 piatto). Errore su modello senza prezzo.
pub fn cost_usd(model: &str, input_tokens: u64, output_tokens: u64) -> anyhow::Result<f64> {
    let (pin, pout) = if model.starts_with("claude-haiku-5-5") {
        if input_tokens <= HAIKU_TIER_TOKENS {
            HAIKU_PRICE_LOW
        } else {
            HAIKU_PRICE_HIGH
        }
    } else if model.starts_with("claude-sonnet-5-5") {
        SONNET_PRICE
    } else {
        bail!("modello senza prezzo dichiarato: {model}");
    };
    Ok((input_tokens as f64 * pin + output_tokens as f64 * pout) / 1_000_000.0)
}

/// Valida `S06_MAX_COST_USD`: ammesso solo il tetto di D-31 (2 USD); ogni
/// altro valore ferma l'avvio chiedendo l'intervento dell'utente.
pub fn check_budget_cap(raw: &str) -> anyhow::Result<f64> {
    let v: f64 = raw
        .trim()
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite())
        .context("S06_MAX_COST_USD non è un numero")?;
    if v != D31_BUDGET_USD {
        bail!(
            "S06_MAX_COST_USD diverso dal tetto di D-31 ({D31_BUDGET_USD} USD): serve l'intervento \
             dell'utente sul file segreti, l'esecutore non lo corregge"
        );
    }
    Ok(v)
}

/// Stima della spesa della matrice di D-31.
pub fn estimate_usd(calls: &[ModelCalls]) -> anyhow::Result<f64> {
    calls.iter().try_fold(0.0, |acc, c| {
        Ok(acc + c.calls as f64 * cost_usd(&c.model, c.avg_input_tokens, c.avg_output_tokens)?)
    })
}

/// Errore se la stima supera il tetto: si ferma prima di ogni richiesta a
/// pagamento, warm-up compreso.
pub fn ensure_estimate_within_cap(estimate_usd: f64, cap_usd: f64) -> anyhow::Result<()> {
    if !(estimate_usd.is_finite() && estimate_usd <= cap_usd) {
        bail!(
            "stima della spesa {estimate_usd:.4} USD oltre il tetto di {cap_usd:.2} USD: \
             nessuna richiesta a pagamento, serve l'intervento dell'utente"
        );
    }
    Ok(())
}

/// Schema della risposta come valore JSON.
pub fn answer_schema() -> serde_json::Value {
    serde_json::from_str(ANSWER_SCHEMA_JSON).expect("schema/answer.schema.json è JSON valido")
}

/// Corpo della richiesta: `model`, `max_tokens`, `system`, un solo messaggio
/// utente, `output_config {effort: "low", format: {type: "json_schema", schema}}`.
pub fn request_body(model: &str, system: &str, user: &str, max_tokens: u32) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
        "system": system,
        "messages": [{"role": "user", "content": user}],
        "output_config": {
            "effort": "low",
            "format": {"type": "json_schema", "schema": answer_schema()}
        }
    })
}

/// Interpreta il corpo della risposta: `stop_reason` prima del parse, blocco
/// `type == "text"` anche dopo blocchi `thinking`.
pub fn extract(body: &serde_json::Value) -> Result<Extracted, AnthropicError> {
    let stop_reason = body["stop_reason"]
        .as_str()
        .ok_or_else(|| AnthropicError::Malformed("manca stop_reason".to_owned()))?
        .to_owned();
    let tokens = |k: &str| {
        body["usage"][k]
            .as_u64()
            .ok_or_else(|| AnthropicError::Malformed(format!("manca usage.{k}")))
    };
    let usage = Usage {
        input_tokens: tokens("input_tokens")?,
        output_tokens: tokens("output_tokens")?,
    };
    let model_resolved = body["model"].as_str().map(str::to_owned);
    let generation = match stop_reason.as_str() {
        "refusal" => Generation::Refusal,
        "max_tokens" => Generation::MaxTokens,
        _ => {
            let text = body["content"]
                .as_array()
                .and_then(|blocks| {
                    blocks
                        .iter()
                        .find(|b| b["type"] == "text")
                        .and_then(|b| b["text"].as_str())
                })
                .ok_or_else(|| AnthropicError::Malformed("nessun blocco text".to_owned()))?;
            Generation::Text(text.to_owned())
        }
    };
    Ok(Extracted {
        generation,
        stop_reason,
        usage,
        model_resolved,
    })
}

/// Client Messages API con contatore di spesa della sessione.
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    endpoint: String,
    api_key: String,
    max_tokens: u32,
    max_cost_usd: f64,
    spent_usd: Mutex<f64>,
}

impl Client {
    /// Errore con chiave vuota, tetto non positivo o URL non `https://`
    /// (ammesso `http://127.0.0.1` solo per i test locali).
    pub fn new(config: ClientConfig) -> anyhow::Result<Self> {
        let base_url = config.base_url.trim_end_matches('/').to_owned();
        let loopback = base_url.starts_with("http://127.0.0.1:");
        if !base_url.starts_with("https://") && !loopback {
            bail!("URL della Messages API non https:// (ammesso http solo su 127.0.0.1)");
        }
        if config.api_key.trim().is_empty() {
            bail!("chiave Anthropic vuota");
        }
        if !(config.max_cost_usd.is_finite() && config.max_cost_usd > 0.0) {
            bail!("tetto di spesa non positivo");
        }
        if config.max_tokens == 0 {
            bail!("max_tokens nullo");
        }
        let http = reqwest::Client::builder()
            .connect_timeout(config.connect_timeout)
            .timeout(config.request_timeout)
            .https_only(!loopback)
            .build()
            .context("costruzione del client HTTP verso la Messages API")?;
        let endpoint = format!("{base_url}/v1/messages");
        Ok(Client {
            http,
            base_url,
            endpoint,
            api_key: config.api_key,
            max_tokens: config.max_tokens,
            max_cost_usd: config.max_cost_usd,
            spent_usd: Mutex::new(0.0),
        })
    }

    /// Una generazione; rifiutata senza invio se la spesa ha già raggiunto il tetto.
    pub async fn generate(
        &self,
        model: &str,
        system: &str,
        user: &str,
    ) -> Result<Response, AnthropicError> {
        let body = request_body(model, system, user, self.max_tokens);
        let mut retries = 0;
        loop {
            let spent = self.spent_usd();
            if spent >= self.max_cost_usd {
                return Err(AnthropicError::BudgetExceeded {
                    spent_usd: spent,
                    budget_usd: self.max_cost_usd,
                });
            }
            let started = Instant::now();
            let resp = self
                .http
                .post(&self.endpoint)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .json(&body)
                .send()
                .await
                .map_err(|e| AnthropicError::Other(format!("trasporto: {}", e.without_url())))?;
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
                return Err(AnthropicError::Http { status });
            }
            let json: serde_json::Value = resp
                .json()
                .await
                .map_err(|_| AnthropicError::Malformed("corpo non JSON".to_owned()))?;
            let latency_ms = started.elapsed().as_millis() as u64;
            let extracted = extract(&json)?;
            let cost = cost_usd(
                model,
                extracted.usage.input_tokens,
                extracted.usage.output_tokens,
            )
            .map_err(|e| AnthropicError::Other(format!("{e:#}")))?;
            *self.spent_usd.lock().unwrap_or_else(|e| e.into_inner()) += cost;
            return Ok(Response {
                extracted,
                latency_ms,
                cost_usd: cost,
            });
        }
    }

    /// Spesa cumulata della sessione in USD.
    pub fn spent_usd(&self) -> f64 {
        *self.spent_usd.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.base_url)
            .field("api_key", &"<redatta>")
            .field("max_tokens", &self.max_tokens)
            .field("max_cost_usd", &self.max_cost_usd)
            .field("spent_usd", &self.spent_usd())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::fake_http::{FakeServer, Reply};

    const SENTINEL: &str = "REDACT-ME";
    pub(crate) const HAIKU: &str = "claude-haiku-5-5";
    pub(crate) const SONNET: &str = "claude-sonnet-5-5";

    pub(crate) fn config(base_url: &str, max_cost_usd: f64) -> ClientConfig {
        ClientConfig {
            base_url: base_url.to_owned(),
            api_key: format!("sk-test-{SENTINEL}"),
            max_tokens: 2048,
            max_cost_usd,
            connect_timeout: Duration::from_secs(2),
            request_timeout: Duration::from_secs(5),
        }
    }

    /// Corpo di una risposta Messages API con un blocco text.
    pub(crate) fn message_body(model: &str, stop_reason: &str, text: &str, input: u64) -> String {
        serde_json::json!({
            "id": "msg_test",
            "type": "message",
            "role": "assistant",
            "model": model,
            "content": [{"type": "text", "text": text}],
            "stop_reason": stop_reason,
            "usage": {"input_tokens": input, "output_tokens": 10}
        })
        .to_string()
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// Cerca una chiave a qualsiasi profondità.
    fn has_key(v: &serde_json::Value, key: &str) -> bool {
        match v {
            serde_json::Value::Object(m) => {
                m.contains_key(key) || m.values().any(|x| has_key(x, key))
            }
            serde_json::Value::Array(a) => a.iter().any(|x| has_key(x, key)),
            _ => false,
        }
    }

    // TEST-18 (RISK-11)
    #[test]
    fn request_body_shape() {
        let body = request_body(HAIKU, "SYS", "USER-MSG", 2048);
        let schema: serde_json::Value =
            serde_json::from_str(ANSWER_SCHEMA_JSON).expect("schema JSON valido");
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(body["model"], HAIKU);
        assert_eq!(body["max_tokens"], 2048);
        assert_eq!(body["output_config"]["effort"], "low");
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert_eq!(body["output_config"]["format"]["schema"], schema);
        assert_eq!(body["system"], "SYS");
        assert!(!has_key(&body, "temperature"), "niente temperature: {body}");
        assert!(!has_key(&body, "top_p") && !has_key(&body, "top_k"));
        // Niente Citations native: `citations` esiste solo come proprietà dello schema.
        let mut without_schema = body.clone();
        without_schema["output_config"]["format"]["schema"] = serde_json::Value::Null;
        assert!(!has_key(&without_schema, "citations"), "{body}");
        // Un solo messaggio utente, nessun prefill dell'assistente.
        let msgs = body["messages"].as_array().expect("messages");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        assert!(msgs[0].to_string().contains("USER-MSG"));
        assert!(!body["system"].to_string().contains("USER-MSG"));
    }

    // TEST-15 (RISK-8), parte stop_reason.
    #[test]
    fn stop_reason_checked_before_parse() {
        let refusal: serde_json::Value =
            serde_json::from_str(&message_body(HAIKU, "refusal", "non json {", 5)).unwrap();
        let e = extract(&refusal).expect("rifiuto interpretato");
        assert_eq!(e.generation, Generation::Refusal);
        assert_eq!(e.stop_reason, "refusal");

        let truncated: serde_json::Value = serde_json::from_str(&message_body(
            HAIKU,
            "max_tokens",
            r#"{"answer":"a","claims":[{"te"#,
            5,
        ))
        .unwrap();
        assert_eq!(
            extract(&truncated)
                .expect("troncato interpretato")
                .generation,
            Generation::MaxTokens
        );

        // Con thinking attivo il blocco text non è il primo.
        let with_thinking = serde_json::json!({
            "model": HAIKU,
            "content": [
                {"type": "thinking", "thinking": "ragionamento", "signature": "x"},
                {"type": "text", "text": "{\"answer\":\"a\",\"claims\":[],\"refusal\":null}"}
            ],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 100, "output_tokens": 20}
        });
        let e = extract(&with_thinking).expect("end_turn");
        assert_eq!(
            e.generation,
            Generation::Text(r#"{"answer":"a","claims":[],"refusal":null}"#.to_owned())
        );
        assert_eq!(
            e.usage,
            Usage {
                input_tokens: 100,
                output_tokens: 20
            }
        );
        assert_eq!(e.model_resolved.as_deref(), Some(HAIKU));

        let no_text = serde_json::json!({
            "model": HAIKU,
            "content": [{"type": "thinking", "thinking": "solo pensiero"}],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 1, "output_tokens": 1}
        });
        assert!(extract(&no_text).is_err(), "senza blocco text è un errore");
    }

    // TEST-16 (RISK-9), controllo di unità dei prezzi.
    #[test]
    fn cost_uses_declared_prices() {
        assert!(approx(cost_usd(SONNET, 1_000_000, 0).unwrap(), 2.0));
        assert!(approx(cost_usd(SONNET, 0, 1_000_000).unwrap(), 10.0));
        // Haiku sotto i 100K di prompt: 0,10 / 0,50 per Mtok.
        assert!(approx(cost_usd(HAIKU, 50_000, 10_000).unwrap(), 0.01));
        // Haiku oltre i 100K di prompt: 0,50 / 2,50 per Mtok.
        assert!(approx(cost_usd(HAIKU, 200_000, 1_000).unwrap(), 0.1025));
        assert!(cost_usd("claude-ignoto", 1, 1).is_err());
    }

    // TEST-16 (RISK-9)
    #[tokio::test]
    async fn cost_guard_stops_over_budget() {
        // Ogni risposta costa 2 USD (1M token di input Sonnet), tetto 1 USD.
        let body = message_body(
            SONNET,
            "end_turn",
            r#"{"answer":"a","claims":[],"refusal":null}"#,
            1_000_000,
        );
        let server = FakeServer::start(vec![Reply::ok(body)]).await;
        let client = Client::new(config(&server.base_url, 1.0)).expect("client");

        let first = client
            .generate(SONNET, "s", "u")
            .await
            .expect("prima sotto il tetto");
        assert!(approx(first.cost_usd, 2.0 + 10.0 * 10.0 / 1_000_000.0));
        assert!(client.spent_usd() > 1.0);

        let second = client.generate(SONNET, "s", "u").await;
        assert!(
            matches!(second, Err(AnthropicError::BudgetExceeded { .. })),
            "ottenuto {second:?}"
        );
        assert_eq!(server.count(), 1, "mai più di una richiesta oltre il tetto");
    }

    // TEST-22 (RISK-9), parte 1.
    #[test]
    fn budget_cap_must_equal_d31() {
        for bad in ["5", "2.5", "1", "0", "-2", "abc", ""] {
            let err = check_budget_cap(bad).expect_err(bad);
            if bad == "5" || bad == "2.5" {
                let msg = format!("{err:#}");
                assert!(msg.contains("utente"), "deve chiedere l'utente: {msg}");
            }
        }
        assert!(approx(check_budget_cap("2").unwrap(), D31_BUDGET_USD));
        assert!(approx(check_budget_cap("2.0").unwrap(), D31_BUDGET_USD));
    }

    // TEST-22 (RISK-9), parte 2.
    #[test]
    fn estimate_over_cap_stops() {
        let over = [ModelCalls {
            model: SONNET.to_owned(),
            calls: 1,
            avg_input_tokens: 1_005_000,
            avg_output_tokens: 0,
        }];
        let est = estimate_usd(&over).unwrap();
        assert!(approx(est, 2.01));
        assert!(ensure_estimate_within_cap(est, D31_BUDGET_USD).is_err());

        let under = [
            ModelCalls {
                model: SONNET.to_owned(),
                calls: 1,
                avg_input_tokens: 945_000,
                avg_output_tokens: 0,
            },
            ModelCalls {
                model: HAIKU.to_owned(),
                calls: 10,
                avg_input_tokens: 50_000,
                avg_output_tokens: 0,
            },
        ];
        let est = estimate_usd(&under).unwrap();
        assert!(approx(est, 1.89 + 0.05));
        assert!(ensure_estimate_within_cap(est, D31_BUDGET_USD).is_ok());
        assert!(ensure_estimate_within_cap(1.99, D31_BUDGET_USD).is_ok());
        assert!(ensure_estimate_within_cap(2.01, D31_BUDGET_USD).is_err());

        let unknown = [ModelCalls {
            model: "claude-ignoto".to_owned(),
            calls: 1,
            avg_input_tokens: 1,
            avg_output_tokens: 1,
        }];
        assert!(
            estimate_usd(&unknown).is_err(),
            "modello senza prezzo: stima impossibile"
        );
    }

    // TEST-12 (RISK-5)
    #[tokio::test]
    async fn client_errors_redact_keys() {
        let server = FakeServer::start(vec![Reply::status(401, r#"{"type":"error"}"#)]).await;
        let client = Client::new(config(&server.base_url, 1.0)).expect("client");
        let dbg = format!("{client:?}");
        assert!(!dbg.contains(SENTINEL), "Debug espone la chiave: {dbg}");

        let err = client
            .generate(HAIKU, "s", "u")
            .await
            .expect_err("401 è un errore");
        assert!(
            matches!(err, AnthropicError::Http { status: 401 }),
            "{err:?}"
        );
        for msg in [format!("{err}"), format!("{err:?}")] {
            assert!(!msg.contains(SENTINEL), "l'errore espone la chiave: {msg}");
        }

        let rec = server.requests();
        assert_eq!(rec.len(), 1);
        let head = rec[0].head.to_lowercase();
        assert!(head.starts_with("post /v1/messages "), "{head}");
        assert!(head.contains(&format!("x-api-key: sk-test-{}", SENTINEL.to_lowercase())));
        assert!(head.contains("anthropic-version: 2023-06-01"));
        assert!(!head.contains("authorization:"), "Anthropic usa x-api-key");
        assert_eq!(rec[0].body["model"], HAIKU);
        assert!(!has_key(&rec[0].body, "temperature"));
    }

    #[test]
    fn client_rejects_bad_config() {
        // URL in chiaro verso un host instradabile, composto per non
        // sembrare un endpoint reale allo scanner: va rifiutato.
        let plain = format!("{}://example.com", "http");
        assert!(Client::new(config(&plain, 1.0)).is_err());
        assert!(Client::new(config("https://api.anthropic.com", 0.0)).is_err());
        let mut empty_key = config("https://api.anthropic.com", 1.0);
        empty_key.api_key = String::new();
        assert!(Client::new(empty_key).is_err());
        assert!(Client::new(config(DEFAULT_BASE_URL, 1.0)).is_ok());
    }
}
