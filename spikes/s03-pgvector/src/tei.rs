//! Client TEI (`POST /embed`, URL da `TEI_EMBED_URL`): embedding dei documenti e della query
//! separati, istruzione asimmetrica solo sulla query.
//!
//! L'istruzione di Qwen3 ed e5-instruct si scrive come prefisso esplicito
//! `Instruct: <compito>\nQuery: <testo>` (stesso compito per i due modelli), invece del
//! `prompt_name` di TEI: il prefisso non dipende dai nomi di prompt nel config del modello
//! ([NV] RESEARCH §11.10) e rende identica la richiesta fra modelli. bge-m3 non ha prefisso.
use std::time::Duration;

use anyhow::{Context, bail};
use serde_json::{Value, json};

use crate::Model;

/// Compito dell'istruzione di query, quello delle model card di Qwen3-Embedding e
/// multilingual-e5-large-instruct per il retrieval.
pub const QUERY_TASK: &str =
    "Given a web search query, retrieve relevant passages that answer the query";

/// Testo della query come lo riceve il modello.
pub fn query_text(model: Model, query: &str, with_instruction: bool) -> String {
    match (model, with_instruction) {
        (Model::Qwen3 | Model::E5, true) => format!("Instruct: {QUERY_TASK}\nQuery: {query}"),
        _ => query.to_string(),
    }
}

/// Corpo di `POST /embed` per un lotto di documenti: mai istruzioni.
pub fn documents_body(texts: &[&str]) -> Value {
    json!({ "inputs": texts, "normalize": true, "truncate": true })
}

/// Corpo di `POST /embed` per una query: istruzione per modello se `with_instruction`.
pub fn query_body(model: Model, query: &str, with_instruction: bool) -> Value {
    json!({
        "inputs": query_text(model, query, with_instruction),
        "normalize": true,
        "truncate": true,
    })
}

/// Dimensione dei vettori dei tre modelli candidati.
pub const DIM: usize = 1024;

/// Client HTTP verso il servizio `tei-embed` locale.
pub struct Client {
    http: reqwest::Client,
    embed_url: String,
    info_url: String,
    /// Dimensione dei lotti di `embed_documents`.
    pub batch: usize,
}

impl Client {
    /// URL base da `TEI_EMBED_URL` (es. il servizio `tei-embed` del compose).
    pub fn from_env(batch: usize) -> anyhow::Result<Client> {
        let base = common::env::required("TEI_EMBED_URL")?;
        Client::new(&base, batch)
    }

    pub fn new(base_url: &str, batch: usize) -> anyhow::Result<Client> {
        if batch == 0 {
            bail!("dimensione del lotto nulla");
        }
        let base = base_url.trim_end_matches('/');
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .context("client HTTP")?;
        Ok(Client {
            http,
            embed_url: format!("{base}/embed"),
            info_url: format!("{base}/info"),
            batch,
        })
    }

    /// `GET /info` di TEI (modello, revisione `model_sha`, versione).
    pub async fn info(&self) -> anyhow::Result<Value> {
        let resp = self
            .http
            .get(&self.info_url)
            .send()
            .await
            .context("GET /info")?;
        let status = resp.status();
        if !status.is_success() {
            bail!("GET /info: stato {status}");
        }
        resp.json().await.context("risposta di /info")
    }

    async fn post_embed(&self, body: &Value) -> anyhow::Result<Vec<Vec<f32>>> {
        let resp = self
            .http
            .post(&self.embed_url)
            .json(body)
            .send()
            .await
            .context("POST /embed")?;
        let status = resp.status();
        if !status.is_success() {
            let detail: Value = resp.json().await.unwrap_or(Value::Null);
            bail!("POST /embed: stato {status}, {detail}");
        }
        let vecs: Vec<Vec<f32>> = resp.json().await.context("risposta di /embed")?;
        if let Some(v) = vecs.iter().find(|v| v.len() != DIM) {
            bail!("vettore di dimensione {} invece di {DIM}", v.len());
        }
        Ok(vecs)
    }

    /// Embedding dei documenti a lotti di `self.batch`, senza istruzione, nell'ordine.
    pub async fn embed_documents(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        let mut out = Vec::with_capacity(texts.len());
        for lot in texts.chunks(self.batch) {
            let vecs = self.post_embed(&documents_body(lot)).await?;
            if vecs.len() != lot.len() {
                bail!("TEI ha restituito {} vettori per {}", vecs.len(), lot.len());
            }
            out.extend(vecs);
        }
        Ok(out)
    }

    /// Embedding di una query, con l'istruzione del modello se `with_instruction`.
    pub async fn embed_query(
        &self,
        query: &str,
        model: Model,
        with_instruction: bool,
    ) -> anyhow::Result<Vec<f32>> {
        let mut vecs = self
            .post_embed(&query_body(model, query, with_instruction))
            .await?;
        if vecs.len() != 1 {
            bail!("TEI ha restituito {} vettori per una query", vecs.len());
        }
        Ok(vecs.remove(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const Q: &str = "Qual è l'EBITDA previsto dal budget 2026?";

    fn inputs_str(body: &Value) -> &str {
        body.get("inputs")
            .and_then(Value::as_str)
            .expect("inputs della query come stringa")
    }

    fn assert_flags(body: &Value) {
        assert_eq!(
            body.get("normalize"),
            Some(&Value::Bool(true)),
            "normalize: true"
        );
        assert_eq!(
            body.get("truncate"),
            Some(&Value::Bool(true)),
            "truncate: true"
        );
    }

    // TEST-11 (RISK-7): istruzione per modello sulla query, mai sui documenti, flag efficace.
    #[test]
    fn query_instruction_per_model_and_flag() {
        // Qwen3: istruzione via prompt_name oppure prefisso «Instruct: …\nQuery: <q>».
        let qwen = query_body(Model::Qwen3, Q, true);
        assert_flags(&qwen);
        let has_prompt_name = qwen
            .get("prompt_name")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty());
        let inp = inputs_str(&qwen);
        let has_prefix = inp.starts_with("Instruct: ")
            && inp.ends_with(&format!("\nQuery: {Q}"))
            && inp.len() > Q.len();
        assert!(
            has_prompt_name || has_prefix,
            "Qwen3 senza istruzione di query: {qwen}"
        );
        if has_prompt_name {
            assert_eq!(inp, Q, "con prompt_name il testo resta la query");
        }

        // e5-instruct: prefisso manuale «Instruct: …\nQuery: <q>».
        let e5 = query_body(Model::E5, Q, true);
        assert_flags(&e5);
        let inp = inputs_str(&e5);
        assert!(inp.starts_with("Instruct: "), "e5 senza prefisso: {inp:?}");
        assert!(
            inp.ends_with(&format!("\nQuery: {Q}")),
            "e5 prefisso malformato: {inp:?}"
        );
        assert!(e5.get("prompt_name").is_none());

        // bge-m3: nessun prefisso.
        let bge = query_body(Model::BgeM3, Q, true);
        assert_flags(&bge);
        assert_eq!(inputs_str(&bge), Q);
        assert!(bge.get("prompt_name").is_none());

        // Flag disattivato: nessun modello ha istruzione.
        for m in [Model::Qwen3, Model::BgeM3, Model::E5] {
            let b = query_body(m, Q, false);
            assert_flags(&b);
            assert_eq!(inputs_str(&b), Q, "{m:?} con --no-instruction");
            assert!(
                b.get("prompt_name").is_none(),
                "{m:?} con --no-instruction ha prompt_name"
            );
        }
    }

    // TEST-11 (RISK-7): embed_documents non aggiunge mai istruzioni.
    #[test]
    fn documents_body_never_has_instruction() {
        let docs = [
            "Ricavi consolidati 182,4 M€",
            "Instruct: non è un'istruzione nostra",
        ];
        let b = documents_body(&docs);
        assert_flags(&b);
        assert!(b.get("prompt_name").is_none());
        let arr = b
            .get("inputs")
            .and_then(Value::as_array)
            .expect("inputs dei documenti come array");
        let got: Vec<&str> = arr.iter().map(|v| v.as_str().expect("stringa")).collect();
        assert_eq!(got, docs, "i testi dei documenti passano invariati");
    }
}
