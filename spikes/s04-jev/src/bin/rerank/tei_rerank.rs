//! Client del reranker TEI (bge-reranker-v2-m3) per lo spike A-06 (piano 01-15).
//!
//! Contratto (RESEARCH §11.10, verificato sulla risposta reale di TEI 1.9.4):
//! `POST {TEI_RERANK_URL}/rerank` con `{"query", "texts", "raw_scores": false}`,
//! risposta `[{index, score}]` ordinata per punteggio. `index` è la posizione del
//! testo nella richiesta: va ricollegato al candidato originale, mai indovinato.
//! Una risposta incompleta, con un indice ripetuto o fuori intervallo è un errore
//! (fail-closed): nessun ordine si completa a caso.
//!
//! L'endpoint viene solo dall'ambiente (`TEI_RERANK_URL`); TEI è un servizio locale
//! della rete del compose, quindi lo schema può essere `http`.
//!
//! **Modifica:** Claude (test-write 01-15) · 2026-10-25 · scheletro e test rossi del design
//! **Modifica:** Claude (gsd-executor 01-15) · 2026-10-25 · client TEI `/rerank` e `/info`, mapping fail-closed

use anyhow::{Context, bail};
use s04_jev::candidates::Passage;
use serde::Deserialize;
use std::time::Duration;

/// Timeout di connessione verso TEI.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// Timeout per richiesta: 20 candidati su GPU stanno ben sotto; su CPU servono secondi.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Una voce della risposta di `/rerank`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RerankHit {
    pub index: usize,
    pub score: f64,
}

/// Client HTTP verso il servizio `tei-rerank`; l'URL base viene dall'ambiente.
pub struct TeiReranker {
    http: reqwest::Client,
    base: String,
    endpoint: String,
}

impl TeiReranker {
    /// Client con URL base esplicito (senza `/` finale; `/rerank` lo aggiunge il client).
    pub fn new(base_url: &str) -> anyhow::Result<TeiReranker> {
        let base = base_url.trim().trim_end_matches('/').to_owned();
        let url = reqwest::Url::parse(&base).context("URL base di TEI non valido")?;
        if !matches!(url.scheme(), "http" | "https") {
            bail!("URL base di TEI con schema non ammesso: {}", url.scheme());
        }
        if url.host_str().is_none() {
            bail!("URL base di TEI senza host");
        }
        if !url.username().is_empty() || url.password().is_some() || url.query().is_some() {
            bail!("URL base di TEI con credenziali o query: non ammesso");
        }
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .context("costruzione del client HTTP verso TEI")?;
        let endpoint = format!("{base}/rerank");
        Ok(TeiReranker {
            http,
            base,
            endpoint,
        })
    }

    /// Client da `TEI_RERANK_URL`; errore che nomina la variabile se manca.
    pub fn from_env() -> anyhow::Result<TeiReranker> {
        let base = common::env::required("TEI_RERANK_URL")?;
        TeiReranker::new(&base).context("TEI_RERANK_URL")
    }

    /// `GET /info` di TEI (modello, `model_sha`, versione): serve al pin dei pesi.
    pub async fn info(&self) -> anyhow::Result<serde_json::Value> {
        let resp = self
            .http
            .get(format!("{}/info", self.base))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("TEI /info: {}", e.without_url()))?;
        let status = resp.status();
        if !status.is_success() {
            bail!("TEI /info: status {status}");
        }
        resp.json().await.context("TEI /info: risposta non JSON")
    }

    /// Riordina `passages` per la query: id del passaggio e punteggio, dal più
    /// rilevante. Un solo `POST /rerank` per chiamata.
    pub async fn rerank(
        &self,
        query: &str,
        passages: &[Passage],
    ) -> anyhow::Result<Vec<(String, f64)>> {
        if passages.is_empty() {
            bail!("rerank senza candidati");
        }
        let body = request_body(query, passages);
        let resp = self
            .http
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("TEI /rerank: {}", e.without_url()))?;
        let status = resp.status();
        if !status.is_success() {
            // Il corpo d'errore di TEI non contiene testo dei passaggi: si riporta corto.
            let detail = resp.text().await.unwrap_or_default();
            let detail: String = detail.chars().take(200).collect();
            bail!("TEI /rerank: status {status}: {detail}");
        }
        let hits: Vec<RerankHit> = resp
            .json()
            .await
            .context("TEI /rerank: risposta non nella forma [{index, score}]")?;
        ranked_ids(passages, &hits)
    }
}

/// Corpo della richiesta: `query`, `texts` nell'ordine dei candidati, `raw_scores: false`.
pub fn request_body(query: &str, passages: &[Passage]) -> serde_json::Value {
    let texts: Vec<&str> = passages.iter().map(|p| p.text.as_str()).collect();
    serde_json::json!({
        "query": query,
        "texts": texts,
        "raw_scores": false,
    })
}

/// Ricollega la risposta ai candidati: id e punteggio in ordine di punteggio
/// decrescente. Errore (mai panico, mai candidato inventato) se un `index` è fuori
/// intervallo, ripetuto, o se la risposta non copre tutti i candidati inviati.
/// A parità di punteggio vince la posizione d'ingresso (l'ordine denso).
pub fn ranked_ids(passages: &[Passage], hits: &[RerankHit]) -> anyhow::Result<Vec<(String, f64)>> {
    let n = passages.len();
    if hits.len() != n {
        bail!(
            "risposta di TEI incompleta: {} voci per {n} candidati",
            hits.len()
        );
    }
    let mut seen = vec![false; n];
    let mut scored: Vec<(usize, f64)> = Vec::with_capacity(n);
    for h in hits {
        if h.index >= n {
            bail!("indice {} fuori intervallo su {n} candidati", h.index);
        }
        if seen[h.index] {
            bail!("indice {} ripetuto nella risposta di TEI", h.index);
        }
        if !h.score.is_finite() {
            bail!("punteggio non finito per l'indice {}", h.index);
        }
        seen[h.index] = true;
        scored.push((h.index, h.score));
    }
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    Ok(scored
        .into_iter()
        .map(|(i, s)| (passages[i].id.clone(), s))
        .collect())
}

/// Server HTTP/1.1 finto su loopback per i test del bin: conta le richieste,
/// registra percorso e corpo, risponde sempre con lo stesso corpo JSON.
#[cfg(test)]
pub(crate) mod fake {
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    pub struct Recorded {
        pub head: String,
        pub body: serde_json::Value,
    }

    pub struct FakeServer {
        pub base_url: String,
        pub log: Arc<Mutex<Vec<Recorded>>>,
    }

    impl FakeServer {
        pub async fn start(reply: String) -> FakeServer {
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
                    let reply = reply.clone();
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
                        log.lock().expect("log").push(Recorded { head, body });
                        let out = format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                            reply.len()
                        );
                        let _ = sock.write_all(out.as_bytes()).await;
                        let _ = sock.shutdown().await;
                    });
                }
            });
            FakeServer { base_url, log }
        }

        pub fn count(&self) -> usize {
            self.log.lock().expect("log").len()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeServer;
    use super::*;

    fn p(id: &str) -> Passage {
        Passage {
            id: id.to_owned(),
            doc_key: "doc-pub".to_owned(),
            classification: "public".to_owned(),
            text: format!("testo di {id}"),
        }
    }

    fn hits(json: &str) -> Vec<RerankHit> {
        serde_json::from_str(json).expect("risposta TEI di test valida")
    }

    fn ids(ranked: &[(String, f64)]) -> Vec<&str> {
        ranked.iter().map(|(id, _)| id.as_str()).collect()
    }

    // TEST-6 (RISK-4): index della risposta → candidato originale, in ordine di punteggio.
    #[test]
    fn tei_rerank_maps_index_to_candidate() {
        let cands = vec![p("A"), p("B"), p("C")];
        let sorted = hits(
            r#"[{"index": 2, "score": 0.9}, {"index": 0, "score": 0.5}, {"index": 1, "score": 0.1}]"#,
        );
        let out = ranked_ids(&cands, &sorted).expect("risposta completa e valida");
        assert_eq!(ids(&out), vec!["C", "A", "B"]);
        assert_eq!(out[0].1, 0.9, "il punteggio resta legato al suo candidato");
        assert_eq!(out[2].1, 0.1);

        // La posizione nella risposta non conta: decide il punteggio.
        let unsorted = hits(
            r#"[{"index": 0, "score": 0.5}, {"index": 1, "score": 0.1}, {"index": 2, "score": 0.9}]"#,
        );
        let out = ranked_ids(&cands, &unsorted).expect("risposta non ordinata ma completa");
        assert_eq!(ids(&out), vec!["C", "A", "B"]);
    }

    // TEST-6 (RISK-4): index fuori intervallo → errore, non panico né candidato inventato.
    #[test]
    fn tei_rerank_rejects_out_of_range_index() {
        let cands = vec![p("A"), p("B"), p("C")];
        let bad = hits(
            r#"[{"index": 3, "score": 0.9}, {"index": 0, "score": 0.5}, {"index": 1, "score": 0.1}]"#,
        );
        assert!(ranked_ids(&cands, &bad).is_err(), "index 3 su 3 candidati");
    }

    // TEST-6 (RISK-4): risposta incompleta o con index ripetuto → errore.
    #[test]
    fn tei_rerank_rejects_incomplete_response() {
        let cands = vec![p("A"), p("B"), p("C")];
        let short = hits(r#"[{"index": 2, "score": 0.9}, {"index": 0, "score": 0.5}]"#);
        assert!(ranked_ids(&cands, &short).is_err(), "manca un candidato");
        let dup = hits(
            r#"[{"index": 2, "score": 0.9}, {"index": 2, "score": 0.5}, {"index": 1, "score": 0.1}]"#,
        );
        assert!(
            ranked_ids(&cands, &dup).is_err(),
            "index ripetuto, candidato 0 assente"
        );
    }

    // TEST-6 (RISK-4): forma del corpo della richiesta.
    #[test]
    fn tei_rerank_request_body_shape() {
        let cands = vec![p("A"), p("B"), p("C")];
        let body = request_body("Qual è l'EBITDA 2025?", &cands);
        assert_eq!(body["query"], "Qual è l'EBITDA 2025?");
        assert_eq!(
            body["texts"],
            serde_json::json!(["testo di A", "testo di B", "testo di C"]),
            "texts nell'ordine dei candidati"
        );
        assert_eq!(body["raw_scores"], false);
    }

    // TEST-6 (RISK-4): sul filo, un solo POST /rerank con il corpo atteso e mapping corretto.
    #[tokio::test]
    async fn tei_rerank_posts_rerank_and_maps_reply() {
        let server = FakeServer::start(
            r#"[{"index": 2, "score": 0.9}, {"index": 0, "score": 0.5}, {"index": 1, "score": 0.1}]"#.to_owned(),
        )
        .await;
        let tei = TeiReranker::new(&server.base_url).expect("client TEI su loopback");
        let cands = vec![p("A"), p("B"), p("C")];
        let out = tei
            .rerank("q", &cands)
            .await
            .expect("rerank su server finto");
        assert_eq!(ids(&out), vec!["C", "A", "B"]);

        let log = server.log.lock().expect("log");
        assert_eq!(log.len(), 1, "una sola richiesta per query");
        let first = log[0].head.lines().next().unwrap_or_default();
        assert!(
            first.starts_with("POST /rerank "),
            "richiesta inattesa: {first}"
        );
        assert_eq!(log[0].body["query"], "q");
        assert_eq!(
            log[0].body["texts"],
            serde_json::json!(["testo di A", "testo di B", "testo di C"])
        );
        assert_eq!(log[0].body["raw_scores"], false);
    }
}
