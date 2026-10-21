//! Client JEV dello spike S-04: forme di rank A, B, C con pin del modello,
//! retry su 429/529 entro la deadline, contatore di costo e guardia di
//! visibilità prima di ogni invio.
//!
//! Contratto (RESEARCH §7, openapi 0.2.0): `POST {base_url}/v1/systemone` con
//! `Authorization: Bearer <chiave>`. La chiave non compare mai in `Debug`, errori o CSV.

use crate::candidates::{Acl, Passage, Query, assert_visible};
use anyhow::{Context, bail};
use futures::StreamExt;
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::time::Instant;

/// Prezzo dell'input di `jev-1.13` e `jev-1.14`: 0,042 USD per milione di token.
/// Fonte: https://docs.typesafe.ai/models (letta il 2026-10-18), riportata in
/// 01-RICERCA-DISCUSSIONE.md Area D §1.5.
pub const PRICE_INPUT_USD_PER_MTOK: f64 = 0.042;
/// Output gratuito (stessa fonte).
pub const PRICE_OUTPUT_USD_PER_MTOK: f64 = 0.0;

/// Retry massimi su 429/529 per chiamata logica, sempre entro la deadline:
/// uno, come il budget di retry di ADR-0010 (Decisione 5). L'SDK Python ne
/// fa due (RICERCA Area D §1.3), ma lì non c'è una deadline globale.
pub const MAX_RETRIES: u32 = 1;
/// Attesa iniziale quando la risposta non porta `retry-after` (raddoppia, tetto 5 s).
const BACKOFF_INITIAL: Duration = Duration::from_millis(500);
const BACKOFF_MAX: Duration = Duration::from_secs(5);
/// Timeout di connessione (piano 01-11, Task 1).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// Timeout per richiesta di `new_from_env`; la deadline del rank resta il limite vero.
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Soglia di un Noul per contare un segnale come «vero» (solo per i conteggi).
pub const NOUL_TRUE: f64 = 0.5;

/// Wording versionato delle tre forme (materiale di calibrazione).
pub const FORM_A_JSON: &str = include_str!("../questions/form-a.json");
pub const FORM_B_JSON: &str = include_str!("../questions/form-b.json");
pub const FORM_C_JSON: &str = include_str!("../questions/form-c.json");

/// Forma della chiamata `rank`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Form {
    A,
    B,
    C,
}

impl Form {
    pub fn as_str(self) -> &'static str {
        match self {
            Form::A => "a",
            Form::B => "b",
            Form::C => "c",
        }
    }

    /// sha256 esadecimale del file `questions/form-<x>.json` compilato nel binario.
    pub fn questions_sha256(self) -> String {
        let text = match self {
            Form::A => FORM_A_JSON,
            Form::B => FORM_B_JSON,
            Form::C => FORM_C_JSON,
        };
        hex::encode(Sha256::digest(text.as_bytes()))
    }
}

/// Configurazione del client; chiave e URL base sono parametri (testabile senza env).
pub struct ClientConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub request_timeout: Duration,
    pub max_cost_usd: f64,
}

impl ClientConfig {
    /// Legge URL, chiave, modello e budget dalla sezione `# s04` dell'ambiente.
    /// Gli errori nominano la variabile, mai il valore.
    pub fn from_env() -> anyhow::Result<ClientConfig> {
        let base_url = common::env::required("JEV_API_URL")?;
        let api_key = common::env::required("TYPESAFE_API_KEY")?;
        let model = common::env::required("JEV_MODEL")?;
        let max_cost_usd: f64 = common::env::required("S04_MAX_COST_USD")?
            .parse()
            .ok()
            .filter(|v: &f64| v.is_finite() && *v > 0.0)
            .context("S04_MAX_COST_USD non è un numero positivo")?;
        Ok(ClientConfig {
            base_url,
            api_key,
            model,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            max_cost_usd,
        })
    }
}

impl fmt::Debug for ClientConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &"<redatta>")
            .field("model", &self.model)
            .field("request_timeout", &self.request_timeout)
            .field("max_cost_usd", &self.max_cost_usd)
            .finish()
    }
}

/// Metadati di una chiamata JEV arrivata a risposta.
#[derive(Debug, Clone, PartialEq)]
pub struct CallMeta {
    pub model_resolved: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    /// Status dell'ultima risposta (0 se nessuna risposta è arrivata).
    pub status: u16,
    pub latency_ms: u64,
    pub retry_after: Option<String>,
    pub retries: u32,
    /// Risposte 429 e 529 viste dentro la chiamata logica (retry compresi).
    pub throttled_429: u32,
    pub throttled_529: u32,
    /// Id della richiesta lato provider, se la risposta lo porta.
    pub request_id: Option<String>,
}

/// Esito di un rank: `(passage_id, segnale)` più i metadati di ogni chiamata.
#[derive(Debug, Clone, PartialEq)]
pub struct RankOutcome {
    pub relevance: Vec<(String, f64)>,
    pub calls: Vec<CallMeta>,
}

/// Segnali della forma A per un candidato.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormAScores {
    pub relevant: f64,
    pub sufficient: Option<f64>,
    pub instructs_model: Option<f64>,
}

/// Esito della forma A per un singolo candidato, anche in errore (per le misure).
#[derive(Debug)]
pub struct CandidateOutcome {
    pub passage_id: String,
    pub result: Result<FormAScores, JevError>,
    pub meta: CallMeta,
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

impl JevError {
    /// Etichetta breve per i CSV (`status` di una riga non riuscita).
    pub fn label(&self) -> String {
        match self {
            JevError::Unauthorized { .. } => "unauthorized".to_owned(),
            JevError::RateLimited { status, .. } => status.to_string(),
            JevError::Timeout => "timeout".to_owned(),
            JevError::BudgetExceeded { .. } => "budget".to_owned(),
            JevError::Http { status } => status.to_string(),
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
            JevError::RateLimited {
                status,
                retry_after,
            } => write!(
                f,
                "JEV ha risposto {status} e la deadline non consente il retry (retry-after: {})",
                retry_after.as_deref().unwrap_or("assente")
            ),
            JevError::Timeout => write!(f, "deadline scaduta senza risposta da JEV"),
            JevError::BudgetExceeded {
                spent_usd,
                budget_usd,
            } => write!(
                f,
                "budget di sessione esaurito: spesi {spent_usd:.6} USD su {budget_usd:.6}"
            ),
            JevError::Http { status } => write!(f, "JEV ha risposto con status {status}"),
            JevError::Other(msg) => write!(f, "errore JEV: {msg}"),
        }
    }
}

impl std::error::Error for JevError {}

/// Costo in USD dei token riportati da `usage`.
pub fn cost_usd(input_tokens: u64, output_tokens: u64) -> f64 {
    input_tokens as f64 * PRICE_INPUT_USD_PER_MTOK / 1_000_000.0
        + output_tokens as f64 * PRICE_OUTPUT_USD_PER_MTOK / 1_000_000.0
}

/// Wording delle tre forme, letto dai file versionati.
struct Wording {
    a_questions: serde_json::Value,
    b_question: String,
    b_id: String,
    c_question: serde_json::Value,
    c_id: String,
    c_criterion_id: String,
    c_criterion: String,
}

impl Wording {
    fn load() -> anyhow::Result<Wording> {
        let a: serde_json::Value =
            serde_json::from_str(FORM_A_JSON).context("questions/form-a.json non valido")?;
        let b: serde_json::Value =
            serde_json::from_str(FORM_B_JSON).context("questions/form-b.json non valido")?;
        let c: serde_json::Value =
            serde_json::from_str(FORM_C_JSON).context("questions/form-c.json non valido")?;
        let a_questions = a["questions"].clone();
        if !a_questions["relevant"].is_object() {
            bail!("form-a.json: manca la domanda relevant");
        }
        let str_of = |v: &serde_json::Value, k: &str, file: &str| -> anyhow::Result<String> {
            v[k].as_str()
                .map(str::to_owned)
                .with_context(|| format!("{file}: manca {k}"))
        };
        for (v, file) in [(&b, "form-b.json"), (&c, "form-c.json")] {
            if v["placeholder"] != "{i}" {
                bail!("{file}: segnaposto atteso {{i}}");
            }
        }
        Ok(Wording {
            a_questions,
            b_question: serde_json::to_string(&b["question"]).context("form-b.json")?,
            b_id: str_of(&b, "question_id", "form-b.json")?,
            c_question: c["question"].clone(),
            c_id: str_of(&c, "question_id", "form-c.json")?,
            c_criterion_id: str_of(&c, "criterion_id", "form-c.json")?,
            c_criterion: str_of(&c, "criterion", "form-c.json")?,
        })
    }
}

fn index_into(template: &str, i: usize) -> String {
    template.replace("{i}", &i.to_string())
}

/// Client JEV.
///
/// Ogni rank passa da [`assert_visible`] sull'intero insieme prima del primo
/// invio; in più, al momento dell'invio, il client riconta in modo
/// indipendente i passaggi non visibili che stanno partendo
/// ([`Client::unauthorized_sent`], hard gate = 0).
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    endpoint: String,
    api_key: String,
    model: String,
    acl: Acl,
    max_cost_usd: f64,
    spent_usd: Mutex<f64>,
    unauthorized: AtomicU64,
    wording: Wording,
}

/// Esito grezzo di una chiamata logica (retry compresi).
struct Sent {
    result: Result<serde_json::Value, JevError>,
    meta: CallMeta,
}

impl Client {
    /// Costruttore da configurazione; errore con modello vuoto, chiave vuota o
    /// URL non `https://` (ammesso `http://127.0.0.1` solo per i test locali).
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
        if !(config.max_cost_usd.is_finite() && config.max_cost_usd >= 0.0) {
            bail!("budget S04_MAX_COST_USD non valido");
        }
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
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
            max_cost_usd: config.max_cost_usd,
            spent_usd: Mutex::new(0.0),
            unauthorized: AtomicU64::new(0),
            wording: Wording::load()?,
        })
    }

    /// Costruttore dall'ambiente (vedi [`ClientConfig::from_env`]).
    pub fn new_from_env(acl: Acl) -> anyhow::Result<Self> {
        Client::new(ClientConfig::from_env()?, acl)
    }

    /// Modello richiesto (pin o alias).
    pub fn model(&self) -> &str {
        &self.model
    }

    fn guard(&self, query: &Query, passages: &[Passage]) -> Result<(), JevError> {
        assert_visible(passages, &query.role_profile, &self.acl).map_err(|e| {
            JevError::Unauthorized {
                reason: format!("{e:#}"),
            }
        })
    }

    /// Conteggio indipendente, sul punto di invio, dei passaggi non visibili.
    fn count_unauthorized(&self, role: &str, sent: &[&Passage]) {
        let n = sent
            .iter()
            .filter(|p| !self.acl.is_visible(&p.doc_key, role))
            .count() as u64;
        if n > 0 {
            self.unauthorized.fetch_add(n, Ordering::SeqCst);
        }
    }

    fn check_budget(&self) -> Result<(), JevError> {
        let spent = self.spent_usd();
        if spent >= self.max_cost_usd {
            return Err(JevError::BudgetExceeded {
                spent_usd: spent,
                budget_usd: self.max_cost_usd,
            });
        }
        Ok(())
    }

    fn add_spent(&self, usd: f64) {
        *self.spent_usd.lock().unwrap_or_else(|e| e.into_inner()) += usd;
    }

    /// Una chiamata logica: invio, retry su 429/529 solo se l'attesa sta
    /// dentro la deadline, contatore di costo da `usage`.
    async fn send(
        &self,
        role: &str,
        sent: &[&Passage],
        body: &serde_json::Value,
        deadline: Instant,
    ) -> Sent {
        let started = Instant::now();
        let mut meta = CallMeta {
            model_resolved: None,
            input_tokens: None,
            output_tokens: None,
            status: 0,
            latency_ms: 0,
            retry_after: None,
            retries: 0,
            throttled_429: 0,
            throttled_529: 0,
            request_id: None,
        };
        let result = self.send_inner(role, sent, body, deadline, &mut meta).await;
        meta.latency_ms = started.elapsed().as_millis() as u64;
        Sent { result, meta }
    }

    async fn send_inner(
        &self,
        role: &str,
        sent: &[&Passage],
        body: &serde_json::Value,
        deadline: Instant,
        meta: &mut CallMeta,
    ) -> Result<serde_json::Value, JevError> {
        loop {
            self.check_budget()?;
            self.count_unauthorized(role, sent);
            let request = self
                .http
                .post(&self.endpoint)
                .bearer_auth(&self.api_key)
                .json(body)
                .send();
            let resp = match tokio::time::timeout_at(deadline, request).await {
                Err(_) => return Err(JevError::Timeout),
                Ok(Err(e)) if e.is_timeout() => return Err(JevError::Timeout),
                Ok(Err(e)) => {
                    return Err(JevError::Other(format!("trasporto: {}", e.without_url())));
                }
                Ok(Ok(r)) => r,
            };
            let status = resp.status().as_u16();
            meta.status = status;
            meta.retry_after = header(&resp, "retry-after");
            meta.request_id = header(&resp, "x-request-id").or_else(|| header(&resp, "request-id"));

            if status == 429 || status == 529 {
                if status == 429 {
                    meta.throttled_429 += 1;
                } else {
                    meta.throttled_529 += 1;
                }
                let wait = meta
                    .retry_after
                    .as_deref()
                    .and_then(parse_retry_after)
                    .unwrap_or_else(|| backoff(meta.retries));
                let fits = Instant::now()
                    .checked_add(wait)
                    .is_some_and(|t| t < deadline);
                if meta.retries < MAX_RETRIES && fits {
                    drop(resp);
                    tokio::time::sleep(wait).await;
                    meta.retries += 1;
                    continue;
                }
                return Err(JevError::RateLimited {
                    status,
                    retry_after: meta.retry_after.clone(),
                });
            }
            if !resp.status().is_success() {
                return Err(JevError::Http { status });
            }
            let json: serde_json::Value = match tokio::time::timeout_at(deadline, resp.json()).await
            {
                Err(_) => return Err(JevError::Timeout),
                Ok(Err(_)) => return Err(JevError::Other("risposta non JSON".to_owned())),
                Ok(Ok(v)) => v,
            };
            meta.model_resolved = json["model"].as_str().map(str::to_owned);
            meta.input_tokens = json["usage"]["input_tokens"].as_u64();
            meta.output_tokens = json["usage"]["output_tokens"].as_u64();
            self.add_spent(cost_usd(
                meta.input_tokens.unwrap_or(0),
                meta.output_tokens.unwrap_or(0),
            ));
            return Ok(json);
        }
    }

    fn body_a(&self, query: &Query, passage: &Passage) -> serde_json::Value {
        serde_json::json!({
            "model": self.model,
            "state": {"query": query.text, "passage": passage.text},
            "questions": self.wording.a_questions,
        })
    }

    async fn one_a(&self, query: &Query, passage: &Passage, deadline: Instant) -> CandidateOutcome {
        let body = self.body_a(query, passage);
        let sent = self
            .send(&query.role_profile, &[passage], &body, deadline)
            .await;
        let result = sent.result.and_then(|json| {
            let noul = |k: &str| json["answers"][k]["noul"].as_f64();
            let relevant = noul("relevant").ok_or_else(|| {
                JevError::Other("risposta senza answers.relevant.noul".to_owned())
            })?;
            Ok(FormAScores {
                relevant,
                sufficient: noul("sufficient"),
                instructs_model: noul("instructs_model"),
            })
        });
        CandidateOutcome {
            passage_id: passage.id.clone(),
            result,
            meta: sent.meta,
        }
    }

    /// Forma A per candidato, con gli esiti singoli anche in errore (misure).
    /// L'unico errore globale è la guardia di visibilità, prima di ogni invio.
    pub async fn rank_form_a_each(
        &self,
        query: &Query,
        passages: &[Passage],
        concurrency: usize,
        deadline: Duration,
    ) -> Result<Vec<CandidateOutcome>, JevError> {
        self.guard(query, passages)?;
        let deadline = Instant::now() + deadline;
        let outcomes =
            futures::stream::iter(passages.iter().map(|p| self.one_a(query, p, deadline)))
                .buffered(concurrency.max(1))
                .collect::<Vec<_>>();
        // Rete di sicurezza: ogni chiamata ha già il proprio `timeout_at`.
        tokio::time::timeout_at(deadline + Duration::from_millis(50), outcomes)
            .await
            .map_err(|_| JevError::Timeout)
    }

    /// Forma A: una richiesta per candidato, al più `concurrency` in volo.
    pub async fn rank_form_a(
        &self,
        query: &Query,
        passages: &[Passage],
        concurrency: usize,
        deadline: Duration,
    ) -> Result<RankOutcome, JevError> {
        let outcomes = self
            .rank_form_a_each(query, passages, concurrency, deadline)
            .await?;
        let mut relevance = Vec::with_capacity(outcomes.len());
        let mut calls = Vec::with_capacity(outcomes.len());
        for o in outcomes {
            let scores = o.result?;
            relevance.push((o.passage_id, scores.relevant));
            calls.push(o.meta);
        }
        Ok(RankOutcome { relevance, calls })
    }

    fn body_multi(
        &self,
        query: &Query,
        passages: &[Passage],
        questions: serde_json::Value,
    ) -> serde_json::Value {
        let texts: Vec<&str> = passages.iter().map(|p| p.text.as_str()).collect();
        serde_json::json!({
            "model": self.model,
            "state": {"query": query.text, "passages": texts},
            "questions": questions,
        })
    }

    /// Forma B: `state` unico con N domande Noul.
    pub async fn rank_form_b(
        &self,
        query: &Query,
        passages: &[Passage],
        deadline: Duration,
    ) -> Result<RankOutcome, JevError> {
        self.guard(query, passages)?;
        let mut questions = serde_json::Map::new();
        for i in 0..passages.len() {
            let q: serde_json::Value =
                serde_json::from_str(&index_into(&self.wording.b_question, i))
                    .map_err(|_| JevError::Other("wording della forma B non valido".to_owned()))?;
            questions.insert(index_into(&self.wording.b_id, i), q);
        }
        let body = self.body_multi(query, passages, questions.into());
        let refs: Vec<&Passage> = passages.iter().collect();
        let deadline = Instant::now() + deadline;
        let sent = self.send(&query.role_profile, &refs, &body, deadline).await;
        let json = sent.result?;
        let mut relevance = Vec::with_capacity(passages.len());
        for (i, p) in passages.iter().enumerate() {
            let id = index_into(&self.wording.b_id, i);
            let v = json["answers"][&id]["noul"]
                .as_f64()
                .ok_or_else(|| JevError::Other(format!("risposta senza answers.{id}.noul")))?;
            relevance.push((p.id.clone(), v));
        }
        Ok(RankOutcome {
            relevance,
            calls: vec![sent.meta],
        })
    }

    /// Forma C: una sola domanda Choice.
    pub async fn rank_form_c(
        &self,
        query: &Query,
        passages: &[Passage],
        deadline: Duration,
    ) -> Result<RankOutcome, JevError> {
        self.guard(query, passages)?;
        if passages.is_empty() || passages.len() > 255 {
            return Err(JevError::Other(
                "la forma C accetta da 1 a 255 passaggi".to_owned(),
            ));
        }
        let mut criteria = serde_json::Map::new();
        for i in 0..passages.len() {
            criteria.insert(
                index_into(&self.wording.c_criterion_id, i),
                index_into(&self.wording.c_criterion, i).into(),
            );
        }
        let mut question = self.wording.c_question.clone();
        question["criteria"] = criteria.into();
        let mut questions = serde_json::Map::new();
        questions.insert(self.wording.c_id.clone(), question);
        let body = self.body_multi(query, passages, questions.into());
        let refs: Vec<&Passage> = passages.iter().collect();
        let deadline = Instant::now() + deadline;
        let sent = self.send(&query.role_profile, &refs, &body, deadline).await;
        let json = sent.result?;
        let probs = &json["answers"][&self.wording.c_id]["probabilities"];
        let mut relevance = Vec::with_capacity(passages.len());
        for (i, p) in passages.iter().enumerate() {
            let id = index_into(&self.wording.c_criterion_id, i);
            let v = probs[&id]
                .as_f64()
                .ok_or_else(|| JevError::Other(format!("risposta senza probabilities.{id}")))?;
            relevance.push((p.id.clone(), v));
        }
        Ok(RankOutcome {
            relevance,
            calls: vec![sent.meta],
        })
    }

    /// Passaggi non visibili effettivamente inviati (hard gate: deve restare 0).
    pub fn unauthorized_sent(&self) -> u64 {
        self.unauthorized.load(Ordering::SeqCst)
    }

    /// Spesa cumulata della sessione in USD.
    pub fn spent_usd(&self) -> f64 {
        *self.spent_usd.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn header(resp: &reqwest::Response, name: &str) -> Option<String> {
    resp.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

/// `retry-after` in secondi (intero o decimale); la forma HTTP-date non è
/// interpretata e ricade sul backoff.
fn parse_retry_after(v: &str) -> Option<Duration> {
    let secs: f64 = v.trim().parse().ok()?;
    (secs.is_finite() && secs >= 0.0).then(|| Duration::from_secs_f64(secs))
}

fn backoff(retries: u32) -> Duration {
    BACKOFF_INITIAL
        .saturating_mul(2u32.saturating_pow(retries))
        .min(BACKOFF_MAX)
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &"<redatta>")
            .field("max_cost_usd", &self.max_cost_usd)
            .field("spent_usd", &self.spent_usd())
            .field("unauthorized_sent", &self.unauthorized_sent())
            .finish_non_exhaustive()
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
