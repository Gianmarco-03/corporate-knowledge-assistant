//! Spike A-06 (piano 01-15): confronto fra no-rerank, bge-reranker-v2-m3 su TEI e
//! JEV rank (forma A, concorrenza 16, deadline 2 s) sui candidati top-20 autorizzati
//! di S-03. Usa la libreria del piano 01-11 (`s04_jev::`) senza modificarla.
//!
//! Prima di ogni invio a TEI o a JEV l'insieme passa da `assert_visible` sul
//! `role_profile` della query (D-16). Il file dei candidati si verifica contro lo
//! sha256 di `candidates_manifest.json`; una divergenza passa solo con
//! `--accept-sha <hex>` uguale allo sha calcolato.
//!
//! Uso (da `spikes/`, nel runner via `session.sh oneshot`; il pacchetto ha due bin, quindi
//! `cargo run -p s04-jev --release --bin rerank -- …` oppure `/target/release/rerank …`):
//! - `rerank --info`: `/info` di TEI (modello e `model_sha`), nessun candidato inviato;
//! - `rerank --methods none,tei,jev --runs 3`: misura, una riga di `rerank.csv` per
//!   query, metodo e ripetizione, e tabella riassuntiva sulle ripetizioni ≥ 2;
//! - `rerank --methods tei --duration-s 180 --out <file>`: carico per `vram.sh`.
//!
//! **Modifica:** Claude (test-write 01-15) · 2026-10-25 · scheletro e test rossi del design
//! **Modifica:** Claude (gsd-executor 01-15) · 2026-10-25 · rank_query, verifica sha, CLI e CSV

mod tei_rerank;

use anyhow::{Context, bail};
use clap::{Parser, ValueEnum};
use s04_jev::candidates::{Acl, Passage, Query, assert_visible, is_green};
use s04_jev::jev::Client;
use s04_jev::metrics::{ndcg_at_k, recall_at_k};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tei_rerank::TeiReranker;
use tokio::time::Instant;

/// Concorrenza di JEV scelta nel piano 01-11 (RISULTATI-jev §Esito).
pub const JEV_CONCURRENCY: usize = 16;
/// Deadline globale del rank JEV di una query.
pub const JEV_DEADLINE: Duration = Duration::from_secs(2);
/// Candidati per query (top-k di S-03).
const K: usize = 20;

/// Header fisso di `rerank.csv` (piano 01-15, Task 2).
const RERANK_HEADER: &str = "method,device,query_id,run,candidates,ndcg_at_5,recall_at_8,latency_ms,unauthorized_sent,model_resolved";

/// Metodo di riordino; nel CSV vale `none`, `tei` o `jev`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Method {
    /// Ordine denso di S-03, nessun invio.
    #[value(name = "none")]
    NoRerank,
    #[value(name = "tei")]
    Tei,
    #[value(name = "jev")]
    Jev,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::NoRerank => "none",
            Method::Tei => "tei",
            Method::Jev => "jev",
        }
    }
}

/// Backend disponibili nella sessione: un metodo senza il suo backend è un errore.
pub struct Backends {
    pub tei: Option<TeiReranker>,
    pub jev: Option<Client>,
}

impl Backends {
    pub fn new(tei: Option<TeiReranker>, jev: Option<Client>) -> Backends {
        Backends { tei, jev }
    }
}

/// Esito del riordino di una query.
#[derive(Debug, Clone, PartialEq)]
pub struct Ranked {
    /// Id dei passaggi, dal più rilevante.
    pub order: Vec<String>,
    pub latency_ms: u64,
    /// Versione risolta di JEV (`response.model`); `None` per `none` e `tei`.
    pub model_resolved: Option<String>,
    /// Passaggi non visibili al ruolo contati sul punto di invio (hard gate: 0).
    pub unauthorized_sent: u64,
}

/// Riordina i candidati di una query con `method`. Guardia di visibilità
/// sull'intero insieme prima di qualunque invio: con un passaggio non visibile
/// al ruolo l'esito è un errore e nessuna richiesta parte.
pub async fn rank_query(
    method: Method,
    query: &Query,
    passages: &[Passage],
    acl: &Acl,
    backends: &Backends,
) -> anyhow::Result<Ranked> {
    let started = Instant::now();
    assert_visible(passages, &query.role_profile, acl)
        .with_context(|| format!("guardia di visibilità, query {}", query.id))?;
    let (order, model_resolved, unauthorized_sent) = match method {
        Method::NoRerank => (passages.iter().map(|p| p.id.clone()).collect(), None, 0),
        Method::Tei => {
            let tei = backends
                .tei
                .as_ref()
                .context("metodo tei senza client TEI (TEI_RERANK_URL)")?;
            // Riconteggio indipendente sul punto di invio, come il client JEV di 01-11.
            let sent = passages
                .iter()
                .filter(|p| !acl.is_visible(&p.doc_key, &query.role_profile))
                .count() as u64;
            if sent > 0 {
                bail!("{sent} passaggi non visibili sul punto di invio a TEI: invio annullato");
            }
            let ranked = tei.rerank(&query.text, passages).await?;
            (ranked.into_iter().map(|(id, _)| id).collect(), None, sent)
        }
        Method::Jev => {
            let jev = backends
                .jev
                .as_ref()
                .context("metodo jev senza client JEV")?;
            let before = jev.unauthorized_sent();
            let out = jev
                .rank_form_a(query, passages, JEV_CONCURRENCY, JEV_DEADLINE)
                .await
                .map_err(|e| anyhow::anyhow!("JEV ({}): {e}", e.label()))?;
            let unauthorized = jev.unauthorized_sent() - before;
            // Ordine stabile: a parità di `relevant` resta l'ordine denso d'ingresso.
            let mut scored: Vec<(usize, String, f64)> = out
                .relevance
                .into_iter()
                .enumerate()
                .map(|(i, (id, s))| (i, id, s))
                .collect();
            if scored.len() != passages.len() {
                bail!(
                    "JEV: {} segnali per {} candidati",
                    scored.len(),
                    passages.len()
                );
            }
            scored.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
            let mut models: Vec<String> = out
                .calls
                .iter()
                .filter_map(|c| c.model_resolved.clone())
                .collect();
            models.sort();
            models.dedup();
            let model = (!models.is_empty()).then(|| models.join("|"));
            (
                scored.into_iter().map(|(_, id, _)| id).collect(),
                model,
                unauthorized,
            )
        }
    };
    Ok(Ranked {
        order,
        latency_ms: started.elapsed().as_millis() as u64,
        model_resolved,
        unauthorized_sent,
    })
}

/// Esito della verifica dello sha256 del file dei candidati.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaCheck {
    /// sha256 dichiarato da `candidates_manifest.json`.
    pub expected: String,
    /// sha256 calcolato sul file.
    pub actual: String,
    /// Vero se la divergenza è stata accettata con `--accept-sha`.
    pub accepted_override: bool,
}

impl ShaCheck {
    /// Riga per RISULTATI-rerank §Anomalie con i due sha, solo se la divergenza
    /// è stata accettata; `None` se il file coincide con il manifest.
    pub fn anomaly(&self) -> Option<String> {
        self.accepted_override.then(|| {
            format!(
                "candidates_top20.jsonl diverso dal manifest di 01-10: sha256 atteso {}, calcolato {}, accettato con --accept-sha",
                self.expected, self.actual
            )
        })
    }
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Verifica `candidates` contro il campo `sha256` di `manifest` (sola lettura).
/// Divergenza senza `accept_sha` → errore con entrambi gli sha; con `accept_sha`
/// procede solo se è uguale allo sha calcolato (nessun jolly).
pub fn verify_candidates_sha(
    candidates: &Path,
    manifest: &Path,
    accept_sha: Option<&str>,
) -> anyhow::Result<ShaCheck> {
    let text = std::fs::read_to_string(manifest)
        .with_context(|| format!("lettura di {}", manifest.display()))?;
    let v: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("{} non è JSON", manifest.display()))?;
    let expected = v["sha256"]
        .as_str()
        .filter(|s| is_sha256_hex(s))
        .with_context(|| format!("{}: campo sha256 assente o non valido", manifest.display()))?
        .to_ascii_lowercase();
    let bytes = std::fs::read(candidates)
        .with_context(|| format!("lettura di {}", candidates.display()))?;
    let actual = hex::encode(Sha256::digest(&bytes));
    let accepted = match accept_sha {
        None => None,
        Some(a) => {
            let a = a.trim().to_ascii_lowercase();
            if !is_sha256_hex(&a) {
                bail!("--accept-sha vuole uno sha256 esadecimale di 64 caratteri, nessun jolly");
            }
            if a != actual {
                bail!(
                    "--accept-sha {a} diverso dallo sha calcolato {actual} (manifest {expected})"
                );
            }
            Some(a)
        }
    };
    if actual == expected {
        return Ok(ShaCheck {
            expected,
            actual,
            accepted_override: false,
        });
    }
    if accepted.is_none() {
        bail!(
            "sha256 dei candidati diverso dal manifest: atteso {expected}, calcolato {actual}; \
             per usare il file rigenerato serve --accept-sha {actual} e la riga in §Anomalie"
        );
    }
    Ok(ShaCheck {
        expected,
        actual,
        accepted_override: true,
    })
}

// ---------------------------------------------------------------- candidati e rilevanza

/// Riga di `candidates_top20.jsonl` (piano 01-10, `db::export_candidates`).
#[derive(Deserialize)]
struct CandidateLine {
    oracle_id: String,
    roles: Vec<String>,
    candidates: Vec<CandidateRow>,
}

#[derive(Deserialize)]
struct CandidateRow {
    rank: usize,
    chunk_id: String,
    doc_key: String,
    page: Option<u32>,
    section_path: Option<String>,
    body: String,
}

/// `expected_passages` di un oracolo.
#[derive(Deserialize)]
struct ExpectedPassage {
    document_id: String,
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    section_path: Option<String>,
    excerpt: String,
}

/// Una query con i suoi candidati autorizzati e gli id dei rilevanti fra questi.
struct QuerySet {
    query: Query,
    passages: Vec<Passage>,
    relevant: Vec<String>,
}

/// Normalizzazione di confronto di S-03 (`s03_pgvector::metrics::normalize`, ORACOLI.md):
/// legature U+FB00…U+FB06 in lettere separate, U+2212 → `-`, trattino morbido rimosso,
/// sillabazione a fine riga unita, spazi bianchi collassati in uno. Il passo NFC di S-03
/// manca (nessuna dipendenza Unicode nel crate, `Cargo.toml` chiuso): testi e excerpt sono
/// già in NFC, verificato prima della misura (RISULTATI-rerank §Setup).
fn normalize(s: &str) -> String {
    let mut chars: Vec<char> = Vec::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\u{FB00}' => chars.extend(['f', 'f']),
            '\u{FB01}' => chars.extend(['f', 'i']),
            '\u{FB02}' => chars.extend(['f', 'l']),
            '\u{FB03}' => chars.extend(['f', 'f', 'i']),
            '\u{FB04}' => chars.extend(['f', 'f', 'l']),
            '\u{FB05}' | '\u{FB06}' => chars.extend(['s', 't']),
            '\u{2212}' => chars.push('-'),
            '\u{AD}' => {}
            other => chars.push(other),
        }
    }
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    let mut pending_space = false;
    while i < chars.len() {
        let c = chars[i];
        if c == '-' && i > 0 && chars[i - 1].is_alphabetic() {
            let mut j = i + 1;
            while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t' || chars[j] == '\r') {
                j += 1;
            }
            if j < chars.len() && chars[j] == '\n' {
                let mut k = j + 1;
                while k < chars.len() && chars[k].is_whitespace() {
                    k += 1;
                }
                if k < chars.len() && chars[k].is_lowercase() {
                    i = k;
                    continue;
                }
            }
        }
        if c.is_whitespace() {
            pending_space = true;
        } else {
            if pending_space {
                out.push(' ');
            }
            pending_space = false;
            out.push(c);
        }
        i += 1;
    }
    if pending_space {
        out.push(' ');
    }
    out
}

/// Lunghezza, in caratteri, della più lunga sottostringa contigua comune.
fn longest_common_substring(a: &[char], b: &[char]) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    let mut prev = vec![0usize; b.len() + 1];
    let mut cur = vec![0usize; b.len() + 1];
    let mut best = 0;
    for &ca in a {
        for (j, &cb) in b.iter().enumerate() {
            cur[j + 1] = if ca == cb { prev[j] + 1 } else { 0 };
            best = best.max(cur[j + 1]);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    best
}

/// Regola di rilevanza di S-03: stesso documento (anche `<doc>-docx`) e stessa pagina o
/// sezione; il chunk contiene l'excerpt normalizzato o almeno il 50% dei suoi caratteri
/// come sottostringa contigua.
fn relevant_to(c: &CandidateRow, p: &ExpectedPassage) -> bool {
    let doc_ok = c.doc_key == p.document_id
        || c.doc_key
            .strip_suffix("-docx")
            .is_some_and(|k| k == p.document_id);
    if !doc_ok {
        return false;
    }
    let place_ok = match (&p.page, &p.section_path) {
        (Some(page), _) => c.page == Some(*page),
        (None, Some(sec)) => c.section_path.as_deref() == Some(sec.as_str()),
        (None, None) => true,
    };
    if !place_ok {
        return false;
    }
    let ex: Vec<char> = normalize(&p.excerpt).chars().collect();
    if ex.is_empty() {
        return false;
    }
    let body = normalize(&c.body);
    let ex_s: String = ex.iter().collect();
    if body.contains(&ex_s) {
        return true;
    }
    let body: Vec<char> = body.chars().collect();
    2 * longest_common_substring(&ex, &body) >= ex.len()
}

/// Carica i candidati e li lega agli oracoli ratificati: testo e ruolo della query
/// vengono da `queries.json`, il ruolo deve coincidere con quello dell'export, ogni
/// insieme passa da `assert_visible` già al caricamento.
fn load_sets(candidates: &Path, corpus: &Path, acl: &Acl) -> anyhow::Result<Vec<QuerySet>> {
    let text = std::fs::read_to_string(corpus.join("oracles/queries.json"))
        .context("lettura di oracles/queries.json")?;
    let all: Vec<serde_json::Value> =
        serde_json::from_str(&text).context("queries.json non valido")?;
    let mut oracles: HashMap<String, serde_json::Value> = HashMap::new();
    for q in all.into_iter().filter(is_green) {
        let id = q["oracle_id"]
            .as_str()
            .context("oracolo senza oracle_id")?
            .to_owned();
        oracles.insert(id, q);
    }

    let raw = std::fs::read_to_string(candidates)
        .with_context(|| format!("lettura di {}", candidates.display()))?;
    let mut sets = Vec::new();
    let mut seen_q: HashSet<String> = HashSet::new();
    for (n, line) in raw.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let cl: CandidateLine = serde_json::from_str(line)
            .with_context(|| format!("candidati, riga {}: JSON non valido", n + 1))?;
        if !seen_q.insert(cl.oracle_id.clone()) {
            bail!("candidati: query {} ripetuta", cl.oracle_id);
        }
        let o = oracles
            .get(&cl.oracle_id)
            .with_context(|| format!("{}: non è un oracolo ratificato", cl.oracle_id))?;
        let role = o["role_profile"]
            .as_str()
            .with_context(|| format!("{}: oracolo senza role_profile", cl.oracle_id))?;
        if cl.roles.len() != 1 || cl.roles[0] != role {
            bail!(
                "{}: ruoli dell'export {:?} diversi dal role_profile {role}",
                cl.oracle_id,
                cl.roles
            );
        }
        if cl.candidates.len() > K {
            bail!("{}: più di {K} candidati", cl.oracle_id);
        }
        let expected: Vec<ExpectedPassage> = serde_json::from_value(o["expected_passages"].clone())
            .with_context(|| format!("{}: expected_passages", cl.oracle_id))?;
        let mut ids: HashSet<&str> = HashSet::new();
        let mut passages = Vec::with_capacity(cl.candidates.len());
        let mut relevant = Vec::new();
        for (i, c) in cl.candidates.iter().enumerate() {
            if c.rank != i + 1 {
                bail!("{}: rank {} fuori ordine", cl.oracle_id, c.rank);
            }
            if !ids.insert(c.chunk_id.as_str()) {
                bail!("{}: chunk {} ripetuto", cl.oracle_id, c.chunk_id);
            }
            if expected.iter().any(|p| relevant_to(c, p)) {
                relevant.push(c.chunk_id.clone());
            }
            passages.push(Passage {
                id: c.chunk_id.clone(),
                doc_key: c.doc_key.clone(),
                classification: acl.classification(&c.doc_key).unwrap_or("").to_owned(),
                text: c.body.clone(),
            });
        }
        let query = Query {
            id: cl.oracle_id.clone(),
            text: o["text"]
                .as_str()
                .with_context(|| format!("{}: oracolo senza text", cl.oracle_id))?
                .to_owned(),
            role_profile: role.to_owned(),
        };
        assert_visible(&passages, role, acl)
            .with_context(|| format!("insieme di {}: rifiutato al caricamento", query.id))?;
        sets.push(QuerySet {
            query,
            passages,
            relevant,
        });
    }
    Ok(sets)
}

// ---------------------------------------------------------------- CLI

#[derive(Parser)]
#[command(
    name = "rerank",
    about = "Spike A-06: no-rerank, bge-reranker-v2-m3 su TEI e JEV rank sui top-20 autorizzati"
)]
struct Cli {
    /// Corpus comune (acl.json, manifest.json, oracles/queries.json).
    #[arg(long, default_value = "data/corpus")]
    corpus: PathBuf,
    /// Candidati top-20 di `export-candidates` (ignorati da git).
    #[arg(
        long,
        default_value = "s03-pgvector/results/raw/candidates_top20.jsonl"
    )]
    candidates: PathBuf,
    /// Manifest versionato del piano 01-10 (sola lettura).
    #[arg(
        long,
        default_value = "s03-pgvector/results/embedding/candidates_manifest.json"
    )]
    manifest: PathBuf,
    /// sha256 del file rigenerato, se diverge dal manifest (va riportato in §Anomalie).
    #[arg(long)]
    accept_sha: Option<String>,
    /// Metodi, separati da virgola.
    #[arg(long, value_delimiter = ',', default_value = "none,tei,jev")]
    methods: Vec<Method>,
    /// Ripetizioni per query e metodo (la prima si scarta in analisi).
    #[arg(long, default_value_t = 3)]
    runs: u32,
    /// Dispositivo di TEI registrato nella colonna `device` (`gpu` o `cpu`).
    #[arg(long, default_value = "gpu")]
    device: String,
    /// CSV di uscita: si crea con l'header o si accoda se l'header coincide.
    #[arg(long, default_value = "s04-jev/results/rerank/rerank.csv")]
    out: PathBuf,
    /// Modalità carico: ripete le query a ciclo per questi secondi (per `vram.sh`).
    #[arg(long)]
    duration_s: Option<u64>,
    /// Solo le prime N query (prove brevi).
    #[arg(long)]
    queries: Option<usize>,
    /// Stampa `/info` di TEI ed esce.
    #[arg(long)]
    info: bool,
}

/// Una riga di misura tenuta in memoria per la tabella riassuntiva.
struct Row {
    method: Method,
    device: String,
    run: u32,
    ndcg5: f64,
    recall8: f64,
    latency_ms: u64,
}

fn device_of(method: Method, tei_device: &str) -> String {
    match method {
        Method::NoRerank => "na".to_owned(),
        Method::Tei => tei_device.to_owned(),
        Method::Jev => "api".to_owned(),
    }
}

/// Apre il CSV: header nuovo o accodamento solo con lo stesso header.
fn open_csv(path: &Path) -> anyhow::Result<std::fs::File> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("cartella {}", dir.display()))?;
    }
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("apertura di {}", path.display()))?;
    match existing.lines().next() {
        None => writeln!(f, "{RERANK_HEADER}")?,
        Some(h) if h.trim_end() == RERANK_HEADER => {}
        Some(_) => bail!("{}: header inatteso", path.display()),
    }
    Ok(f)
}

/// Percentile a rango più vicino.
fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p * sorted.len() as f64).ceil() as usize).max(1) - 1;
    sorted[idx.min(sorted.len() - 1)]
}

/// Numero con 4 decimali e la virgola, come nelle tabelle di RISULTATI.
fn it4(v: f64) -> String {
    format!("{v:.4}").replace('.', ",")
}

fn print_summary(rows: &[Row]) {
    let mut by: BTreeMap<(Method, String), Vec<&Row>> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.run >= 2) {
        by.entry((r.method, r.device.clone())).or_default().push(r);
    }
    println!("| metodo | device | righe | nDCG@5 | recall@8 | p50 ms | p95 ms | p99 ms |");
    println!("|---|---|---|---|---|---|---|---|");
    for ((m, d), rs) in &by {
        let n = rs.len() as f64;
        let nd = rs.iter().map(|r| r.ndcg5).sum::<f64>() / n;
        let rc = rs.iter().map(|r| r.recall8).sum::<f64>() / n;
        let mut lat: Vec<u64> = rs.iter().map(|r| r.latency_ms).collect();
        lat.sort_unstable();
        println!(
            "| {} | {d} | {} | {} | {} | {} | {} | {} |",
            m.as_str(),
            rs.len(),
            it4(nd),
            it4(rc),
            percentile(&lat, 0.50),
            percentile(&lat, 0.95),
            percentile(&lat, 0.99)
        );
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.info {
        let tei = TeiReranker::from_env()?;
        let info = tei.info().await?;
        println!(
            "tei-rerank info: model_id={} model_sha={} version={} max_batch_tokens={}",
            info["model_id"].as_str().unwrap_or("?"),
            info["model_sha"].as_str().unwrap_or("?"),
            info["version"].as_str().unwrap_or("?"),
            info["max_batch_tokens"]
        );
        return Ok(());
    }

    let sha = verify_candidates_sha(&cli.candidates, &cli.manifest, cli.accept_sha.as_deref())?;
    println!(
        "candidati: sha256 manifest {} calcolato {}",
        sha.expected, sha.actual
    );
    if let Some(a) = sha.anomaly() {
        println!("ANOMALIA: {a}");
    }

    let acl = Acl::load(&cli.corpus)?;
    let mut sets = load_sets(&cli.candidates, &cli.corpus, &acl)?;
    if let Some(n) = cli.queries {
        sets.truncate(n);
    }
    let short = sets.iter().filter(|s| s.passages.len() < K).count();
    let empty = sets.iter().filter(|s| s.passages.is_empty()).count();
    let no_rel = sets.iter().filter(|s| s.relevant.is_empty()).count();
    println!(
        "query: {} (con meno di {K} candidati: {short}; senza candidati, escluse: {empty}; senza rilevanti fra i candidati: {no_rel})",
        sets.len()
    );
    sets.retain(|s| !s.passages.is_empty());

    let mut methods = cli.methods.clone();
    methods.dedup();
    let tei = if methods.contains(&Method::Tei) {
        Some(TeiReranker::from_env()?)
    } else {
        None
    };
    let jev = if methods.contains(&Method::Jev) {
        Some(Client::new_from_env(acl.clone())?)
    } else {
        None
    };
    let backends = Backends::new(tei, jev);

    let mut csv = open_csv(&cli.out)?;
    let mut rows: Vec<Row> = Vec::new();
    let mut errors = 0usize;
    let mut unauthorized_total = 0u64;
    let started = std::time::Instant::now();
    let deadline = cli.duration_s.map(|s| started + Duration::from_secs(s));
    let mut run = 0u32;
    'outer: loop {
        run += 1;
        if deadline.is_none() && run > cli.runs {
            break;
        }
        for set in &sets {
            if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
                break 'outer;
            }
            let rel: Vec<&str> = set.relevant.iter().map(String::as_str).collect();
            for &m in &methods {
                match rank_query(m, &set.query, &set.passages, &acl, &backends).await {
                    Ok(r) => {
                        let ranked: Vec<&str> = r.order.iter().map(String::as_str).collect();
                        let ndcg5 = ndcg_at_k(&ranked, &rel, 5);
                        let recall8 = recall_at_k(&ranked, &rel, 8);
                        unauthorized_total += r.unauthorized_sent;
                        let device = device_of(m, &cli.device);
                        writeln!(
                            csv,
                            "{},{device},{},{run},{},{ndcg5:.6},{recall8:.6},{},{},{}",
                            m.as_str(),
                            set.query.id,
                            set.passages.len(),
                            r.latency_ms,
                            r.unauthorized_sent,
                            r.model_resolved.as_deref().unwrap_or("")
                        )?;
                        rows.push(Row {
                            method: m,
                            device,
                            run,
                            ndcg5,
                            recall8,
                            latency_ms: r.latency_ms,
                        });
                    }
                    Err(e) => {
                        errors += 1;
                        eprintln!(
                            "errore: query {} metodo {} run {run}: {e:#}",
                            set.query.id,
                            m.as_str()
                        );
                    }
                }
            }
        }
        csv.flush()?;
    }
    csv.flush()?;

    print_summary(&rows);
    if let Some(j) = backends.jev.as_ref() {
        println!(
            "jev: spesa {:.6} USD, unauthorized_sent {}",
            j.spent_usd(),
            j.unauthorized_sent()
        );
    }
    println!(
        "righe {}, errori {errors}, unauthorized_sent totale {unauthorized_total}, {:.1} s",
        rows.len(),
        started.elapsed().as_secs_f64()
    );
    if unauthorized_total > 0 {
        bail!("hard gate violato: {unauthorized_total} passaggi non visibili inviati");
    }
    if errors > 0 {
        bail!("{errors} riordini non riusciti (vedi stderr)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::tei_rerank::fake::FakeServer;
    use super::*;
    use s04_jev::jev::ClientConfig;
    use sha2::{Digest, Sha256};
    use std::path::PathBuf;
    use std::process::Command;

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

    const PINNED: &str = "jev-1.13.0";

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

    /// 20 candidati visibili a `employee`.
    fn visible_set() -> Vec<Passage> {
        (0..20)
            .map(|i| {
                if i % 2 == 0 {
                    passage(i, "doc-pub", "public")
                } else {
                    passage(i, "doc-int", "internal")
                }
            })
            .collect()
    }

    /// Lo stesso insieme con un passaggio restricted a metà, che dichiara di essere public:
    /// decide il manifest, non la classificazione del passaggio.
    fn set_with_invisible() -> Vec<Passage> {
        let mut s = visible_set();
        s[10] = passage(10, "doc-res", "public");
        s
    }

    /// Risposta TEI valida per n candidati (punteggi decrescenti con l'indice).
    fn tei_reply(n: usize) -> String {
        let hits: Vec<serde_json::Value> = (0..n)
            .map(|i| serde_json::json!({"index": i, "score": 1.0 - (i as f64) / (n as f64)}))
            .collect();
        serde_json::Value::from(hits).to_string()
    }

    fn jev_reply() -> String {
        serde_json::json!({
            "model": PINNED,
            "answers": {
                "relevant": {"type": "noul", "noul": 0.9},
                "sufficient": {"type": "noul", "noul": 0.5},
                "instructs_model": {"type": "noul", "noul": 0.0}
            },
            "usage": {"input_tokens": 10, "output_tokens": 3}
        })
        .to_string()
    }

    fn jev_client(base_url: &str) -> Client {
        let config = ClientConfig {
            base_url: base_url.to_owned(),
            api_key: "fake_local_key".to_owned(),
            model: PINNED.to_owned(),
            request_timeout: Duration::from_secs(5),
            max_cost_usd: 1.0,
        };
        Client::new(config, acl()).expect("client JEV su loopback")
    }

    // TEST-1 (RISK-1): guardia di visibilità prima dell'invio a TEI.
    #[tokio::test]
    async fn rerank_blocks_invisible_candidate_before_tei() {
        let server = FakeServer::start(tei_reply(20)).await;
        let backends = Backends::new(
            Some(TeiReranker::new(&server.base_url).expect("client TEI")),
            None,
        );

        let blocked = rank_query(
            Method::Tei,
            &query(),
            &set_with_invisible(),
            &acl(),
            &backends,
        )
        .await;
        assert!(
            blocked.is_err(),
            "atteso errore con un candidato non visibile, ottenuto {blocked:?}"
        );
        assert_eq!(server.count(), 0, "nessuna richiesta deve raggiungere TEI");

        // Controllo positivo: lo stesso server riceve l'insieme visibile.
        let ok = rank_query(Method::Tei, &query(), &visible_set(), &acl(), &backends)
            .await
            .expect("insieme visibile riordinato da TEI");
        assert_eq!(server.count(), 1, "un solo POST /rerank per query");
        assert_eq!(ok.order.len(), 20);
        assert_eq!(ok.model_resolved, None);
    }

    // TEST-2 (RISK-1): guardia di visibilità prima dell'invio a JEV.
    #[tokio::test]
    async fn rerank_blocks_invisible_candidate_before_jev() {
        let server = FakeServer::start(jev_reply()).await;
        let backends = Backends::new(None, Some(jev_client(&server.base_url)));

        let blocked = rank_query(
            Method::Jev,
            &query(),
            &set_with_invisible(),
            &acl(),
            &backends,
        )
        .await;
        assert!(
            blocked.is_err(),
            "atteso errore con un candidato non visibile, ottenuto {blocked:?}"
        );
        assert_eq!(server.count(), 0, "nessuna richiesta deve raggiungere JEV");
        assert_eq!(
            backends.jev.as_ref().map(Client::unauthorized_sent),
            Some(0)
        );

        // Controllo positivo: forma A, una richiesta per candidato.
        let ok = rank_query(Method::Jev, &query(), &visible_set(), &acl(), &backends)
            .await
            .expect("insieme visibile riordinato da JEV");
        assert_eq!(server.count(), 20, "forma A: una richiesta per candidato");
        assert_eq!(ok.order.len(), 20);
        assert_eq!(ok.model_resolved.as_deref(), Some(PINNED));
    }

    // TEST-1/TEST-2 (RISK-1): anche `none` rifiuta un insieme con candidati non visibili
    // (unauthorized_sent = 0 vale per tutti i metodi, TEST-3).
    #[tokio::test]
    async fn rerank_none_also_refuses_invisible_set() {
        let backends = Backends::new(None, None);
        let blocked = rank_query(
            Method::NoRerank,
            &query(),
            &set_with_invisible(),
            &acl(),
            &backends,
        )
        .await;
        assert!(blocked.is_err(), "ottenuto {blocked:?}");
        let dense = rank_query(
            Method::NoRerank,
            &query(),
            &visible_set(),
            &acl(),
            &backends,
        )
        .await
        .expect("ordine denso");
        let expected: Vec<String> = visible_set().into_iter().map(|p| p.id).collect();
        assert_eq!(
            dense.order, expected,
            "none conserva l'ordine denso di S-03"
        );
    }

    // ---- TEST-8 (RISK-5): sha256 del file dei candidati -------------------------------------

    /// Cartella temporanea propria del test (niente `tempfile`: non è fra le dipendenze del crate).
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("s04-rerank-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("cartella temporanea");
        dir
    }

    fn sha_hex(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn write_fixture(dir: &Path, content: &[u8], manifest_sha: &str) -> (PathBuf, PathBuf) {
        let cand = dir.join("candidates_top20.jsonl");
        let man = dir.join("candidates_manifest.json");
        std::fs::write(&cand, content).expect("scrittura candidati");
        std::fs::write(
            &man,
            serde_json::json!({"k": 20, "sha256": manifest_sha}).to_string(),
        )
        .expect("scrittura manifest");
        (cand, man)
    }

    #[test]
    fn candidates_sha_mismatch_refused_without_explicit_override() {
        let dir = scratch("mismatch");
        let content = b"{\"oracle_id\":\"Q-1\",\"candidates\":[]}\n";
        let actual = sha_hex(content);
        let expected = sha_hex(b"altro contenuto");
        let (cand, man) = write_fixture(&dir, content, &expected);
        let manifest_before = std::fs::read(&man).expect("manifest");

        // Senza opzione: rifiuto che mostra entrambi gli sha.
        let err =
            verify_candidates_sha(&cand, &man, None).expect_err("divergenza senza --accept-sha");
        let msg = format!("{err:#}");
        assert!(
            msg.contains(&expected),
            "l'errore mostra lo sha atteso: {msg}"
        );
        assert!(
            msg.contains(&actual),
            "l'errore mostra lo sha calcolato: {msg}"
        );

        // Con un hash diverso da quello calcolato: rifiuto.
        let other = sha_hex(b"terzo contenuto");
        assert!(verify_candidates_sha(&cand, &man, Some(&other)).is_err());
        // Nessun jolly.
        for wildcard in ["*", "any", "", "0"] {
            assert!(
                verify_candidates_sha(&cand, &man, Some(wildcard)).is_err(),
                "--accept-sha {wildcard:?} non deve accettare qualunque hash"
            );
        }

        // Con l'hash calcolato: procede e registra l'anomalia con i due sha.
        let ok = verify_candidates_sha(&cand, &man, Some(&actual)).expect("divergenza dichiarata");
        assert_eq!(ok.expected, expected);
        assert_eq!(ok.actual, actual);
        assert!(ok.accepted_override);
        let line = ok.anomaly().expect("riga per §Anomalie");
        assert!(
            line.contains(&expected) && line.contains(&actual),
            "anomalia: {line}"
        );

        // Il manifest del piano 01-10 non si tocca.
        assert_eq!(std::fs::read(&man).expect("manifest"), manifest_before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn candidates_sha_match_needs_no_override() {
        let dir = scratch("match");
        let content = b"{\"oracle_id\":\"Q-1\",\"candidates\":[]}\n";
        let actual = sha_hex(content);
        let (cand, man) = write_fixture(&dir, content, &actual);
        let ok = verify_candidates_sha(&cand, &man, None).expect("sha coincidente");
        assert_eq!(ok.actual, actual);
        assert!(!ok.accepted_override);
        assert_eq!(ok.anomaly(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- Controlli statici (TEST-4, TEST-9, TEST-14) ------------------------------------------

    fn manifest_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn repo_root() -> PathBuf {
        manifest_dir().join("../..")
    }

    fn read(p: &Path) -> String {
        std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }

    /// Codice dei file di `src/bin/rerank/`, senza il modulo di test.
    fn bin_sources() -> Vec<(String, String)> {
        let dir = manifest_dir().join("src/bin/rerank");
        let mut out = Vec::new();
        for e in std::fs::read_dir(&dir).expect("src/bin/rerank") {
            let p = e.expect("voce").path();
            if p.extension().is_some_and(|x| x == "rs") {
                let src = read(&p);
                let code = src
                    .split("#[cfg(test)]")
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                out.push((
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    code,
                ));
            }
        }
        out
    }

    fn vram_sh() -> String {
        let p = manifest_dir().join("vram.sh");
        assert!(
            p.exists(),
            "vram.sh assente: lo scrive il Task 1 del piano 01-15"
        );
        read(&p)
    }

    /// Righe di shell eseguibili (senza commenti).
    fn shell_lines(src: &str) -> Vec<String> {
        src.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_owned)
            .collect()
    }

    // TEST-4 (RISK-2): la chiave la legge solo la libreria di 01-11; vram.sh non la espone.
    #[test]
    fn test4_key_not_read_by_bin_nor_echoed_by_vram_sh() {
        for (f, code) in bin_sources() {
            assert!(
                !code.contains("TYPESAFE_API_KEY"),
                "{f}: la chiave la legge solo s04_jev::jev"
            );
        }
        let sh = vram_sh();
        assert!(
            sh.contains("CKA_SPIKES_ENV"),
            "vram.sh carica l'ambiente da CKA_SPIKES_ENV"
        );
        for l in shell_lines(&sh) {
            assert!(
                !l.contains("set -x") && !l.contains("set -o xtrace"),
                "vram.sh: traccia attiva: {l}"
            );
            assert!(!l.contains("printenv"), "vram.sh: printenv: {l}");
            let first = l.split_whitespace().next().unwrap_or_default();
            assert!(
                !(first == "env"
                    && (l == "env" || l.starts_with("env |") || l.starts_with("env >"))),
                "vram.sh: stampa dell'ambiente: {l}"
            );
            if l.starts_with("echo") || l.starts_with("printf") {
                assert!(
                    !l.contains("TYPESAFE") && !l.contains("API_KEY"),
                    "vram.sh: chiave stampata: {l}"
                );
            }
        }
    }

    // TEST-9 (RISK-6), parte statica: vram.sh avvia per nome via session.sh e non ferma
    // servizi non suoi (né compose diretto né profili).
    #[test]
    fn test9_vram_sh_only_session_sh_for_services() {
        let sh = vram_sh();
        let lines = shell_lines(&sh);
        assert!(
            lines.iter().any(|l| l.contains("session.sh up")),
            "vram.sh: manca session.sh up"
        );
        for l in &lines {
            for bad in [
                "--profile",
                "COMPOSE_PROFILES",
                "compose up",
                "compose stop",
                "compose down",
                "compose run",
            ] {
                assert!(!l.contains(bad), "vram.sh: «{bad}» vietato: {l}");
            }
            for bad in ["docker stop", "docker kill", "docker rm"] {
                assert!(
                    !l.contains(bad),
                    "vram.sh: «{bad}» vietato, usare session.sh: {l}"
                );
            }
        }
        assert!(sh.contains("CKA_RUN_ID"), "vram.sh controlla CKA_RUN_ID");
        assert!(sh.contains("exit 2"), "vram.sh esce 2 se CKA_RUN_ID manca");
        for svc in ["tei-embed", "tei-rerank", "docling"] {
            assert!(sh.contains(svc), "vram.sh: manca il servizio {svc}");
        }
        assert!(
            sh.contains("--assert-only"),
            "vram.sh verifica session.sh ps --assert-only"
        );
        assert!(sh.contains("nvidia-smi"), "vram.sh campiona con nvidia-smi");
    }

    // TEST-9 (RISK-6), comportamento: senza CKA_RUN_ID lo script esce 2 senza toccare nulla.
    #[test]
    #[ignore = "richiede Git Bash sull'host come `bash` nel PATH (non il bash di WSL)"]
    fn test9_vram_sh_exits_2_without_run_id() {
        let sh = manifest_dir().join("vram.sh");
        assert!(sh.exists(), "vram.sh assente");
        let st = Command::new("bash")
            .arg(&sh)
            .env_remove("CKA_RUN_ID")
            .status()
            .expect("bash");
        assert_eq!(st.code(), Some(2), "vram.sh senza CKA_RUN_ID: {st}");
    }

    // TEST-9 (RISK-6): la prova con un tei-embed avviato a mano e l'interruzione con Ctrl-C
    // è manuale (Task 1, esito nel SUMMARY): richiede Docker, GPU e una sessione prenotata.
    #[test]
    #[ignore = "prova manuale del Task 1 con Docker e GPU: tei-embed altrui resta running, Ctrl-C ferma solo i propri servizi; esito nel SUMMARY"]
    fn test9_vram_sh_respects_foreign_services_manual() {
        panic!(
            "prova manuale: eseguirla come descritto nel test design (TEST-9) e riportarla nel SUMMARY"
        );
    }

    // TEST-14 (RISK-10): nessun URL letterale nel codice del bin né in vram.sh (endpoint da env).
    #[test]
    fn test14_no_literal_urls() {
        for (f, code) in bin_sources() {
            assert!(
                !code.contains("http://") && !code.contains("https://"),
                "{f}: URL letterale"
            );
        }
        let sh = vram_sh();
        for l in shell_lines(&sh) {
            assert!(
                !l.contains("http://") && !l.contains("https://"),
                "vram.sh: URL letterale: {l}"
            );
        }
        let rerank_rs = read(&manifest_dir().join("src/bin/rerank/tei_rerank.rs"));
        let code = rerank_rs.split("#[cfg(test)]").next().unwrap_or_default();
        assert!(
            code.contains("TEI_RERANK_URL"),
            "l'endpoint TEI viene da TEI_RERANK_URL"
        );
    }

    // TEST-14 (RISK-10, RISK-2): scanner di sicurezza verde.
    #[test]
    #[ignore = "richiede python e lo scanner del repo (skills/security-scanner)"]
    fn test14_security_scanner_clean() {
        let st = Command::new("python")
            .arg(repo_root().join("skills/security-scanner/security_scan.py"))
            .arg(manifest_dir().join("src/bin"))
            .arg(manifest_dir().join("vram.sh"))
            .status()
            .expect("python");
        assert!(st.success(), "security_scan.py: {st}");
    }

    // TEST-4 (RISK-2), runtime: il valore della chiave non compare negli artefatti.
    #[test]
    #[ignore = "evidenza del Task 2: richiede TYPESAFE_API_KEY nell'ambiente (da CKA_SPIKES_ENV) e gli artefatti scritti"]
    fn test4_key_value_absent_from_artifacts() {
        let key = std::env::var("TYPESAFE_API_KEY").expect("chiave nell'ambiente del test");
        assert!(
            key.len() >= 8,
            "chiave di test troppo corta per una ricerca significativa"
        );
        let mut files = Vec::new();
        collect_files(&manifest_dir(), &mut files);
        let adr = repo_root().join("docs/adr/0009-reranker-baseline.md");
        if adr.exists() {
            files.push(adr);
        }
        for f in files {
            if let Ok(bytes) = std::fs::read(&f) {
                let s = String::from_utf8_lossy(&bytes);
                assert!(!s.contains(&key), "la chiave compare in {}", f.display());
            }
        }
    }

    fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.file_name().is_some_and(|n| n == "target" || n == ".git") {
                continue;
            }
            if p.is_dir() {
                collect_files(&p, out);
            } else {
                out.push(p);
            }
        }
    }

    // ---- Evidenze del Task 2 (dopo le misure) ------------------------------------------------

    const RERANK_HEADER: &str = "method,device,query_id,run,candidates,ndcg_at_5,recall_at_8,latency_ms,unauthorized_sent,model_resolved";
    const VRAM_HEADER: &str = "t_ms,scenario,max_batch_tokens_embed,max_batch_tokens_rerank,used_mib_total,used_mib_tei_embed,used_mib_tei_rerank,used_mib_docling,oom";
    const SOGLIE: &str = "## Soglie (scritte PRIMA della misura)";
    const BASE: &str = "829edaa";

    fn results() -> PathBuf {
        manifest_dir().join("results/rerank")
    }

    /// CSV come header + righe (nessun campo quotato nei file dello spike).
    fn csv(name: &str, header: &str) -> Vec<Vec<String>> {
        let s = read(&results().join(name));
        let mut lines = s.lines().filter(|l| !l.trim().is_empty());
        assert_eq!(lines.next().unwrap_or_default(), header, "{name}: header");
        let n = header.split(',').count();
        lines
            .map(|l| {
                let r: Vec<String> = l.split(',').map(str::to_owned).collect();
                assert_eq!(r.len(), n, "{name}: riga con {} campi: {l}", r.len());
                r
            })
            .collect()
    }

    fn num(s: &str) -> f64 {
        s.trim()
            .parse()
            .unwrap_or_else(|_| panic!("valore non numerico: {s:?}"))
    }

    fn section(md: &str, heading: &str) -> String {
        let start = md
            .find(heading)
            .unwrap_or_else(|| panic!("sezione «{heading}» assente"));
        let rest = &md[start + heading.len()..];
        let end = rest.find("\n## ").unwrap_or(rest.len());
        rest[..end].to_owned()
    }

    fn risultati_rerank() -> String {
        read(&manifest_dir().join("RISULTATI-rerank.md"))
    }

    fn git(args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo_root())
            .args(args)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// Formato delle tabelle di RISULTATI: 4 decimali con la virgola (come RISULTATI-jev).
    fn it4(v: f64) -> String {
        format!("{v:.4}").replace('.', ",")
    }

    /// p95 a rango più vicino.
    fn p95(mut v: Vec<f64>) -> f64 {
        assert!(!v.is_empty(), "p95 di un insieme vuoto");
        v.sort_by(f64::total_cmp);
        let idx = ((0.95 * v.len() as f64).ceil() as usize).max(1) - 1;
        v[idx]
    }

    // TEST-3 (RISK-1): hard gate unauthorized_sent = 0 su ogni riga, per ogni metodo.
    #[test]
    #[ignore = "evidenza del Task 2: richiede results/rerank/rerank.csv dopo le misure"]
    fn test3_unauthorized_sent_zero_everywhere() {
        let rows = csv("rerank.csv", RERANK_HEADER);
        assert!(!rows.is_empty(), "rerank.csv senza righe");
        for r in &rows {
            assert_eq!(r[8], "0", "unauthorized_sent ≠ 0: {r:?}");
        }
        for m in ["none", "tei", "jev"] {
            assert!(
                rows.iter().any(|r| r[0] == m),
                "rerank.csv: manca il metodo {m}"
            );
        }
    }

    // TEST-5 (RISK-3): Soglie committate prima di ogni file in results/rerank/ e invariate.
    #[test]
    #[ignore = "verifica git del Task 2: richiede la storia del branch con results/rerank/"]
    fn test5_thresholds_committed_before_results() {
        let ris = "spikes/s04-jev/RISULTATI-rerank.md";
        let intro = git(&["log", "--reverse", "--format=%H", "-S", SOGLIE, "--", ris]);
        let intro = intro
            .lines()
            .next()
            .expect("commit che introduce le Soglie")
            .to_owned();
        let first = git(&[
            "log",
            "--reverse",
            "--diff-filter=A",
            "--format=%H",
            "--",
            "spikes/s04-jev/results/rerank",
        ]);
        let first = first
            .lines()
            .next()
            .expect("primo commit di results/rerank")
            .to_owned();
        assert_ne!(intro, first, "Soglie e risultati nello stesso commit");
        let st = Command::new("git")
            .arg("-C")
            .arg(repo_root())
            .args(["merge-base", "--is-ancestor", &intro, &first])
            .status()
            .expect("git");
        assert!(
            st.success(),
            "le Soglie ({intro}) non precedono i risultati ({first})"
        );
        let at_intro = section(&git(&["show", &format!("{intro}:{ris}")]), SOGLIE);
        let at_head = section(&read(&repo_root().join(ris)), SOGLIE);
        assert_eq!(
            at_intro, at_head,
            "le Soglie sono state ritoccate dopo l'introduzione"
        );
        for needle in [
            "bge-reranker-v2-m3",
            "0,02",
            "300 ms",
            "JEV",
            "5,5 GiB",
            "OOM",
            "max-batch-tokens",
            "= 0",
        ] {
            assert!(at_head.contains(needle), "Soglie: manca «{needle}»");
        }
    }

    // TEST-7 (RISK-4, RISK-9): medie per metodo (run 2..3) uguali alla tabella e baseline
    // coerente con la regola delle Soglie.
    #[test]
    #[ignore = "evidenza del Task 2: richiede rerank.csv e RISULTATI-rerank.md dopo le misure"]
    fn test7_metrics_recomputed_match_results() {
        use std::collections::BTreeMap;
        let rows = csv("rerank.csv", RERANK_HEADER);
        /// (nDCG@5, recall@8, latenza) per (metodo, device).
        type Series = (Vec<f64>, Vec<f64>, Vec<f64>);
        let mut by: BTreeMap<(String, String), Series> = BTreeMap::new();
        for r in rows.iter().filter(|r| num(&r[3]) >= 2.0) {
            let e = by.entry((r[0].clone(), r[1].clone())).or_default();
            e.0.push(num(&r[5]));
            e.1.push(num(&r[6]));
            e.2.push(num(&r[7]));
        }
        let md = risultati_rerank();
        let risultati = section(&md, "## Risultati");
        let mean = |v: &Vec<f64>| v.iter().sum::<f64>() / v.len() as f64;
        for ((m, d), (nd, rc, _)) in &by {
            for (what, v) in [("nDCG@5", mean(nd)), ("recall@8", mean(rc))] {
                assert!(
                    risultati.contains(&it4(v)),
                    "{m}/{d}: {what} medio {} assente dalla tabella di Risultati",
                    it4(v)
                );
            }
        }
        let none = by
            .iter()
            .find(|((m, _), _)| m == "none")
            .map(|(_, v)| mean(&v.0))
            .expect("metodo none");
        let (tei_nd, tei_p95) = by
            .get(&("tei".to_owned(), "gpu".to_owned()))
            .map(|v| (mean(&v.0), p95(v.2.clone())))
            .expect("metodo tei su gpu");
        let expected = if tei_nd >= none + 0.02 && tei_p95 <= 300.0 {
            "bge-reranker-v2-m3"
        } else {
            "no-rerank"
        };
        let esito = section(&md, "## Esito parziale");
        assert!(
            esito.contains(expected),
            "Esito parziale: la regola dà {expected} (nDCG@5 tei {tei_nd:.4} vs none {none:.4}, p95 {tei_p95} ms)"
        );
    }

    // TEST-10 (RISK-6): scenario con i tre servizi caricati per ≥ 3 minuti, oom valorizzato,
    // picco coerente con RISULTATI, riduzione del batch dopo un OOM.
    #[test]
    #[ignore = "evidenza del Task 2: richiede results/rerank/vram.csv dopo vram.sh"]
    fn test10_vram_three_services_under_load() {
        use std::collections::BTreeMap;
        let rows = csv("vram.csv", VRAM_HEADER);
        assert!(!rows.is_empty(), "vram.csv senza righe");
        let mut order: Vec<String> = Vec::new();
        let mut span: BTreeMap<String, (f64, f64)> = BTreeMap::new();
        let mut three: BTreeMap<String, bool> = BTreeMap::new();
        let mut oom: BTreeMap<String, bool> = BTreeMap::new();
        let mut batch: BTreeMap<String, (f64, f64)> = BTreeMap::new();
        let mut peak = 0f64;
        for r in &rows {
            let sc = r[1].clone();
            if !order.contains(&sc) {
                order.push(sc.clone());
            }
            let t = num(&r[0]);
            let e = span.entry(sc.clone()).or_insert((t, t));
            e.0 = e.0.min(t);
            e.1 = e.1.max(t);
            let all = num(&r[5]) > 0.0 && num(&r[6]) > 0.0 && num(&r[7]) > 0.0;
            *three.entry(sc.clone()).or_default() |= all;
            let o = r[8].trim();
            assert!(
                ["true", "false", "0", "1"].contains(&o),
                "oom non valorizzato: {r:?}"
            );
            *oom.entry(sc.clone()).or_default() |= o == "true" || o == "1";
            batch.insert(sc.clone(), (num(&r[2]), num(&r[3])));
            peak = peak.max(num(&r[4]));
        }
        assert!(
            order
                .iter()
                .any(|s| three[s] && span[s].1 - span[s].0 >= 180_000.0),
            "nessuno scenario con i tre servizi caricati per almeno 3 minuti"
        );
        for (i, s) in order.iter().enumerate() {
            if oom[s] {
                let (e, r) = batch[s];
                assert!(
                    order[i + 1..]
                        .iter()
                        .any(|n| batch[n].0 < e || batch[n].1 < r),
                    "scenario {s} con OOM senza uno scenario successivo a batch ridotto"
                );
            }
        }
        let md = risultati_rerank();
        let plain = format!("{}", peak as u64);
        let dotted = {
            let s = plain.clone();
            if s.len() > 3 {
                format!("{}.{}", &s[..s.len() - 3], &s[s.len() - 3..])
            } else {
                s
            }
        };
        assert!(
            md.contains(&plain) || md.contains(&dotted),
            "il picco di used_mib_total ({plain} MiB) non compare in RISULTATI-rerank"
        );
    }

    // TEST-11 (RISK-7): revisione HF dei pesi registrata e coincidente con model_sha; hw.json.
    #[test]
    #[ignore = "evidenza del Task 2: richiede RISULTATI-rerank §Setup e results/rerank/hw.json"]
    fn test11_reranker_revision_and_hw_manifest() {
        let setup = section(&risultati_rerank(), "## Setup");
        assert!(
            setup.contains("model_sha"),
            "§Setup: manca model_sha di /info"
        );
        let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
        for tok in setup.split(|c: char| !c.is_ascii_hexdigit()) {
            if tok.len() == 40 {
                *counts.entry(tok.to_ascii_lowercase()).or_default() += 1;
            }
        }
        assert!(
            counts.values().any(|&n| n >= 2),
            "§Setup: serve lo sha di 40 caratteri della revisione ripetuto come model_sha di /info ({counts:?})"
        );
        let hw_text = read(&results().join("hw.json"));
        let hw: serde_json::Value = serde_json::from_str(&hw_text).expect("hw.json valido");
        assert!(
            hw_text.contains("86-1.9.4"),
            "hw.json: manca il digest TEI 86-1.9.4"
        );
        assert!(
            hw_text.contains("docling-serve-cu128"),
            "hw.json: manca il digest docling-serve-cu128"
        );
        assert_eq!(hw["tools"]["reranker"], "BAAI/bge-reranker-v2-m3");
        assert_eq!(hw["tools"]["jev_model"], PINNED);
    }

    // TEST-12 (RISK-8): il piano non tocca i file del piano 01-11 né s03-pgvector.
    #[test]
    #[ignore = "verifica git: richiede il repo sull'host con la storia del branch"]
    fn test12_plan_0111_files_untouched() {
        let changed = git(&["diff", "--name-only", &format!("{BASE}..HEAD")]);
        let forbidden = [
            "spikes/s04-jev/src/lib.rs",
            "spikes/s04-jev/src/jev.rs",
            "spikes/s04-jev/src/candidates.rs",
            "spikes/s04-jev/src/metrics.rs",
            "spikes/s04-jev/src/main.rs",
            "spikes/s04-jev/RISULTATI-jev.md",
        ];
        let touched: Vec<&str> = changed
            .lines()
            .filter(|l| {
                forbidden.contains(l)
                    || l.starts_with("spikes/s04-jev/questions/")
                    || l.starts_with("spikes/s03-pgvector/")
            })
            .collect();
        assert!(
            touched.is_empty(),
            "file fuori perimetro modificati: {touched:?}"
        );
    }

    // TEST-13 (RISK-9): completezza di rerank.csv, gate GPU, link di RISULTATI e ADR-0009.
    #[test]
    #[ignore = "evidenza del Task 2: richiede results/rerank/, RISULTATI.md e ADR-0009"]
    fn test13_evidence_and_links() {
        use std::collections::{BTreeMap, BTreeSet};
        let rows = csv("rerank.csv", RERANK_HEADER);
        let mut seen: BTreeMap<String, BTreeSet<(String, String)>> = BTreeMap::new();
        let mut cands: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for r in &rows {
            seen.entry(r[2].clone())
                .or_default()
                .insert((r[0].clone(), r[3].clone()));
            cands.entry(r[2].clone()).or_default().insert(r[4].clone());
            let c = num(&r[4]);
            assert!(c > 0.0 && c <= 20.0, "candidates fuori da 1..=20: {r:?}");
            if r[0] == "jev" {
                assert!(
                    !r[9].trim().is_empty(),
                    "model_resolved vuoto per jev: {r:?}"
                );
            }
        }
        assert!(!seen.is_empty(), "rerank.csv senza query");
        for (q, s) in &seen {
            for m in ["none", "tei", "jev"] {
                for run in ["2", "3"] {
                    assert!(
                        s.contains(&(m.to_owned(), run.to_owned())),
                        "{q}: manca {m} run {run}"
                    );
                }
            }
            assert_eq!(
                cands[q].len(),
                1,
                "{q}: numero di candidati diverso fra metodi o run"
            );
        }
        let n20 = cands.values().filter(|c| c.contains("20")).count();
        assert!(
            n20 * 10 >= cands.len() * 9,
            "meno del 90% delle query ha 20 candidati"
        );

        let gpu = read(&manifest_dir().join("../results/gpu-check.txt"));
        assert!(gpu.contains("VERDETTO: VERDE"), "gate GPU non verde");

        let idx = read(&manifest_dir().join("RISULTATI.md"));
        for link in [
            "RISULTATI-jev.md",
            "RISULTATI-rerank.md",
            "results/jev/hw.json",
            "results/rerank/hw.json",
        ] {
            assert!(idx.contains(link), "RISULTATI.md non collega {link}");
            assert!(manifest_dir().join(link).exists(), "{link} non esiste");
        }
        assert!(idx.contains("**Modifica:**"), "RISULTATI.md senza firma");
        for h in [
            "Esito",
            "Impatto su porte",
            "Impatto threat model",
            "Destinazione",
        ] {
            assert!(idx.contains(h), "RISULTATI.md: manca «{h}»");
        }

        let adr = read(&repo_root().join("docs/adr/0009-reranker-baseline.md"));
        assert!(
            adr.contains("s04-jev/RISULTATI.md"),
            "ADR-0009 non collega RISULTATI.md di S-04"
        );
        assert!(adr.contains("Proposed"), "ADR-0009: Stato Proposed");
        let mut paths = 0;
        for tok in adr.split(|c: char| c.is_whitespace() || "()[]`<>\"'".contains(c)) {
            if let Some(i) = tok.find("spikes/") {
                let p = tok[i..].trim_end_matches(['.', ',', ';', ':']);
                let p = p.split('#').next().unwrap_or(p);
                paths += 1;
                assert!(
                    repo_root().join(p).exists(),
                    "ADR-0009 cita {p}, che non esiste"
                );
            }
        }
        assert!(paths >= 2, "ADR-0009 cita almeno RISULTATI.md e hw.json");
        let deps = git(&[
            "diff",
            "--name-only",
            &format!("{BASE}..HEAD"),
            "--",
            "DEPENDENCIES.md",
        ]);
        assert!(
            deps.trim().is_empty(),
            "DEPENDENCIES.md non si modifica in questo piano"
        );
    }
}
