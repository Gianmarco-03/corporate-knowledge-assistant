//! Client JEV locale al crate (spike S-05).
//!
//! Contratto (RESEARCH §7, openapi 0.2.0): `POST {base_url}/v1/systemone` con
//! `Authorization: Bearer <chiave>`, corpo `SystemOneRequest{model, state, questions}`,
//! risposta `SystemOneResponse{model, answers, usage{input_tokens, output_tokens}}`.
//! La chiave resta nel campo privato `api_key`: non compare in `Debug`, negli errori
//! (gli errori di reqwest riportano l'URL, mai gli header) né nei CSV.
//! Nessun reinvio automatico: ogni chiamata a [`JevClient::send`] è una sola richiesta HTTP.

use anyhow::{Context, bail};
use std::fmt;
use std::time::Duration;

/// Stato sintetico fisso (D-16): due frasi inventate su una società fittizia, nessun
/// dato reale. Lo spike non dipende dal corpus.
pub const SYNTHETIC_STATE: &str = "Brughiera Strumenti S.r.l. è una società inventata \
usata solo per questo test. Nel 2025 ha dichiarato ricavi per 12 milioni di euro e un \
utile netto di 1,1 milioni di euro.";

/// Domanda `noul` fissa sullo stato sintetico.
const QUESTION_INSTRUCTIONS: &str = "`state` reports a positive net income for the company.";

/// Tetto dei byte letti dal riscaldamento (il resto si scarta chiudendo la risposta).
const WARM_UP_MAX_BODY: usize = 1024 * 1024;

/// Timeout totale di una richiesta non cancellata (richieste di controllo).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Esito di una richiesta JEV arrivata a risposta.
#[derive(Debug, Clone)]
pub struct JevReply {
    pub status: u16,
    pub model_resolved: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub http_version: String,
}

impl JevReply {
    /// Estrae `model` e `usage` dalla risposta JSON (campi assenti = `None`).
    pub fn from_parts(status: u16, http_version: String, json: Option<&serde_json::Value>) -> Self {
        let usage = json.and_then(|j| j.get("usage"));
        JevReply {
            status,
            model_resolved: json
                .and_then(|j| j.get("model"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            input_tokens: usage
                .and_then(|u| u.get("input_tokens"))
                .and_then(serde_json::Value::as_u64),
            output_tokens: usage
                .and_then(|u| u.get("output_tokens"))
                .and_then(serde_json::Value::as_u64),
            http_version,
        }
    }
}

/// Client JEV: `POST {base_url}/v1/systemone` con chiave Bearer.
pub struct JevClient {
    http: reqwest::Client,
    base_url: String,
    endpoint: String,
    api_key: String,
    model: String,
}

impl JevClient {
    /// Costruttore con chiave passata come parametro (testabile senza env).
    ///
    /// `base_url` deve essere `https://` (TLS Rustls con verifica dei certificati di
    /// default) oppure `http://127.0.0.1` (solo test locali).
    pub fn new(base_url: &str, api_key: String, model: String) -> anyhow::Result<Self> {
        let base_url = base_url.trim_end_matches('/').to_owned();
        let loopback = base_url.starts_with("http://127.0.0.1:");
        if !base_url.starts_with("https://") && !loopback {
            bail!("JEV_API_URL deve usare https:// (ammesso http solo su 127.0.0.1)");
        }
        if api_key.is_empty() {
            bail!("chiave JEV vuota");
        }
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .pool_idle_timeout(Duration::from_secs(90))
            .timeout(REQUEST_TIMEOUT)
            .https_only(!loopback)
            .build()
            .context("costruzione del client HTTP verso JEV")?;
        let endpoint = format!("{base_url}/v1/systemone");
        Ok(JevClient {
            http,
            base_url,
            endpoint,
            api_key,
            model,
        })
    }

    fn body(&self) -> serde_json::Value {
        serde_json::json!({
            "model": self.model,
            "state": SYNTHETIC_STATE,
            "questions": {
                "positive_income": {"type": "noul", "instructions": QUESTION_INSTRUCTIONS}
            }
        })
    }

    /// Una sola richiesta `systemone` con lo `state` sintetico; restituisce status,
    /// modello risolto e token, anche se lo status non è 2xx.
    pub async fn send_reply(&self) -> anyhow::Result<JevReply> {
        let resp = self
            .http
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&self.body())
            .send()
            .await
            .context("richiesta JEV non riuscita")?;
        let status = resp.status().as_u16();
        let http_version = format!("{:?}", resp.version());
        let json: Option<serde_json::Value> = resp.json().await.ok();
        Ok(JevReply::from_parts(status, http_version, json.as_ref()))
    }

    /// Invia una richiesta con lo `state` sintetico fisso; errore se lo status non è 2xx.
    pub async fn send(&self) -> anyhow::Result<serde_json::Value> {
        let resp = self
            .http
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&self.body())
            .send()
            .await
            .context("richiesta JEV non riuscita")?;
        let status = resp.status();
        if !status.is_success() {
            bail!("JEV ha risposto {status}");
        }
        resp.json().await.context("risposta JEV non JSON")
    }

    /// Richiesta di riscaldamento non addebitabile (`GET /openapi.json`): apre la
    /// connessione TLS prima di una richiesta da cancellare, così la cancellazione a
    /// 50 ms colpisce una richiesta già inviata e non l'handshake. Restituisce la
    /// versione HTTP negoziata.
    pub async fn warm_up(&self) -> anyhow::Result<String> {
        let resp = self
            .http
            .get(format!("{}/openapi.json", self.base_url))
            .send()
            .await
            .context("riscaldamento della connessione JEV non riuscito")?;
        let version = format!("{:?}", resp.version());
        let mut resp = resp;
        let mut total = 0usize;
        while let Ok(Some(piece)) = resp.chunk().await {
            total += piece.len();
            if total > WARM_UP_MAX_BODY {
                break;
            }
        }
        Ok(version)
    }
}

impl fmt::Debug for JevClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JevClient")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &"<redatta>")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SENTINEL: &str = "REDACT-ME";

    // TEST-2 (RISK-1)
    #[tokio::test]
    async fn jev_client_debug_and_errors_redact_key() {
        // Porta loopback chiusa: bind, lettura della porta, rilascio.
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
            l.local_addr().expect("addr").port()
        };
        let base_url = format!("http://127.0.0.1:{port}");
        let client = JevClient::new(
            &base_url,
            format!("sk-test-{SENTINEL}"),
            "jev-1.13.0".to_owned(),
        )
        .expect("client costruito");

        let dbg = format!("{client:?}");
        assert!(!dbg.contains(SENTINEL), "Debug espone la chiave: {dbg}");

        let err = client
            .send()
            .await
            .expect_err("la connessione alla porta chiusa deve fallire");
        for msg in [format!("{err:#}"), format!("{err:?}")] {
            assert!(!msg.contains(SENTINEL), "l'errore espone la chiave: {msg}");
        }
    }
}
