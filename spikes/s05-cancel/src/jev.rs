//! Client JEV locale al crate (spike S-05).

use std::fmt;

/// Client JEV: `POST {base_url}/v1/systemone` con chiave Bearer.
// stub del test-write: da implementare in 01-05
pub struct JevClient {}

impl JevClient {
    /// Costruttore con chiave passata come parametro (testabile senza env).
    // stub del test-write: da implementare in 01-05
    pub fn new(_base_url: &str, _api_key: String, _model: String) -> anyhow::Result<Self> {
        todo!("stub del test-write: JevClient::new")
    }

    /// Invia una richiesta con lo `state` sintetico fisso.
    // stub del test-write: da implementare in 01-05
    pub async fn send(&self) -> anyhow::Result<serde_json::Value> {
        todo!("stub del test-write: JevClient::send")
    }
}

// stub del test-write: da implementare in 01-05 (Debug senza chiave)
impl fmt::Debug for JevClient {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!("stub del test-write: Debug for JevClient")
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
