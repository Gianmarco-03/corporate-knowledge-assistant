//! Query di ricerca del piano 01-14: parte semantica (esatta), lessicale (`tsvector` con
//! `websearch_to_tsquery` sulla configurazione della lingua), ibrida con fusione RRF (k = 60)
//! e densa su HNSW con `hnsw.ef_search` / `hnsw.iterative_scan` (RESEARCH §5). Il predicato
//! ACL è la CTE `allowed`, usata dentro ENTRAMBE le sottoquery `sem` e `lex`: nessun
//! post-filtro dopo la fusione (D-12, D-15). SQL solo con `bind`; tabelle da enum chiusi.
//!
//! Ogni istruzione è un letterale completo per tabella (in coda al file), scelto con un
//! `match` su [`Table`] e [`Dense`]: nessun nome di tabella e nessun valore entra nel testo
//! SQL a tempo di esecuzione. Le impostazioni di sessione (`enable_indexscan`,
//! `hnsw.ef_search`, `hnsw.iterative_scan`) passano da `set_config` con parametri legati.
//!
//! **Modifica:** Claude (test-write 01-14) · 2026-10-25 · scheletro e test rossi del design
//! **Modifica:** Claude (gsd-executor 01-14) · 2026-10-25 · query densa, lessicale, ibrida e HNSW con il predicato ACL in ogni parte
use std::collections::BTreeMap;

use anyhow::Context;
use pgvector::{HalfVector, Vector};
use s03_pgvector::chunk::Lang;
use s03_pgvector::db::Hit;
use sqlx::postgres::PgArguments;
use sqlx::{PgConnection, PgPool, Postgres, Row};
use uuid::Uuid;

use crate::synth::Scale;

/// Costante della Reciprocal Rank Fusion.
pub const RRF_K: f64 = 60.0;

/// Candidati per parte (`sem`, `lex`) prima della fusione.
pub const PART_DEPTH: i64 = 50;

/// Tabella dei chunk interrogata: enum chiuso, mai un nome dall'input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Table {
    /// `chunk_bgem3`, il modello scelto in `RISULTATI-embedding.md` §Esito.
    BgeM3,
    /// `chunk_synth_<n>`.
    Synth(Scale),
}

impl Table {
    pub fn name(self) -> &'static str {
        match self {
            Table::BgeM3 => "chunk_bgem3",
            Table::Synth(s) => s.table(),
        }
    }
}

/// Valori di `hnsw.iterative_scan`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IterativeScan {
    Off,
    RelaxedOrder,
    StrictOrder,
}

impl IterativeScan {
    pub const ALL: [IterativeScan; 3] = [
        IterativeScan::Off,
        IterativeScan::RelaxedOrder,
        IterativeScan::StrictOrder,
    ];

    /// Valore della GUC `hnsw.iterative_scan`.
    pub fn setting(self) -> &'static str {
        match self {
            IterativeScan::Off => "off",
            IterativeScan::RelaxedOrder => "relaxed_order",
            IterativeScan::StrictOrder => "strict_order",
        }
    }
}

/// Colonna densa interrogata: `emb` (`vector`) o `emb_h` (`halfvec`, generata da `emb`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dense {
    Vector,
    Half,
}

impl Dense {
    pub const ALL: [Dense; 2] = [Dense::Vector, Dense::Half];

    /// Nome del tipo nelle evidenze (`index_type` di `recall.csv`).
    pub fn name(self) -> &'static str {
        match self {
            Dense::Vector => "vector",
            Dense::Half => "halfvec",
        }
    }
}

/// Forma della parte densa. `Exact`: semi-join con `doc_acl` (la query del piano 01-10),
/// colonna `emb`. `Ann`: il predicato è un subplan `EXISTS (… OFFSET 0)` valutato come filtro
/// sulla scansione ordinata dell'indice HNSW (pgvector applica lì `iterative_scan`); con il
/// semi-join il planner non userebbe mai l'indice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy {
    Exact,
    Ann(Dense),
}

impl Strategy {
    pub fn dense(self) -> Dense {
        match self {
            Strategy::Exact => Dense::Vector,
            Strategy::Ann(d) => d,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Strategy::Exact => "esatta",
            Strategy::Ann(Dense::Vector) => "hnsw vector",
            Strategy::Ann(Dense::Half) => "hnsw halfvec",
        }
    }
}

/// Parametri di sessione della ricerca HNSW.
#[derive(Clone, Copy, Debug)]
pub struct HnswParams {
    pub ef_search: u32,
    pub iterative_scan: IterativeScan,
}

/// Risultato con punteggio (rango lessicale o punteggio RRF), già autorizzato.
#[derive(Clone, Debug, PartialEq)]
pub struct Ranked {
    pub chunk_id: Uuid,
    pub doc_key: String,
    pub score: f64,
}

/// Le tre liste di una query ibrida: candidati `sem` e `lex` (ciascuno già filtrato dalla
/// CTE `allowed`, in ordine di rango, `score` = rango) e fusione RRF completa.
#[derive(Clone, Debug, Default)]
pub struct Breakdown {
    pub sem: Vec<Ranked>,
    pub lex: Vec<Ranked>,
    pub fused: Vec<Ranked>,
}

type PgQuery<'q> = sqlx::query::Query<'q, Postgres, PgArguments>;

/// Lega il vettore di query come `vector` o `halfvec`.
pub(crate) fn bind_dense<'q>(q: PgQuery<'q>, dense: Dense, v: &[f32]) -> PgQuery<'q> {
    match dense {
        Dense::Vector => q.bind(Vector::from(v.to_vec())),
        Dense::Half => q.bind(HalfVector::from_f32_slice(v)),
    }
}

/// `enable_indexscan = off` per la sessione (`local = false`) o la transazione: nessun
/// piano può usare un indice approssimato, quindi la ricerca densa è esatta.
pub async fn exact_mode(conn: &mut PgConnection, local: bool) -> anyhow::Result<()> {
    sqlx::query("SELECT set_config('enable_indexscan', 'off', $1)")
        .bind(local)
        .execute(conn)
        .await
        .context("enable_indexscan = off")?;
    Ok(())
}

/// Parametri HNSW per la sessione (`local = false`) o la transazione, con
/// `enable_seqscan = off`: alle scale piccole il planner sceglierebbe la scansione
/// sequenziale (cioè l'esatta) e la configurazione HNSW non sarebbe misurata.
pub async fn set_hnsw(conn: &mut PgConnection, p: HnswParams, local: bool) -> anyhow::Result<()> {
    sqlx::query(
        "SELECT set_config('hnsw.ef_search', $1, $2), set_config('hnsw.iterative_scan', $3, $2), \
         set_config('enable_seqscan', 'off', $2)",
    )
    .bind(p.ef_search.to_string())
    .bind(local)
    .bind(p.iterative_scan.setting())
    .execute(conn)
    .await
    .context("parametri HNSW")?;
    Ok(())
}

/// Ricerca densa sulla connessione data, con le impostazioni di sessione che ha: esatta se
/// la tabella non ha indici HNSW o se la sessione è in [`exact_mode`], HNSW altrimenti.
pub async fn sem_on(
    conn: &mut PgConnection,
    table: Table,
    strategy: Strategy,
    query_vec: &[f32],
    roles: &[String],
    k: i64,
) -> anyhow::Result<Vec<Hit>> {
    let sql = sem_sql(table, strategy).with_context(|| {
        format!(
            "ricerca {} non prevista su {}",
            strategy.name(),
            table.name()
        )
    })?;
    let rows = bind_dense(sqlx::query(sql), strategy.dense(), query_vec)
        .bind(roles)
        .bind(k)
        .fetch_all(conn)
        .await
        .context("ricerca densa")?;
    rows.into_iter()
        .map(|r| {
            Ok(Hit {
                chunk_id: r.try_get("id")?,
                doc_key: r.try_get("doc_key")?,
                distance: r.try_get("dist")?,
            })
        })
        .collect()
}

/// Parte semantica: top-k esatto per distanza coseno fra i chunk visibili a `roles`.
pub async fn semantic_search(
    pool: &PgPool,
    table: Table,
    query_vec: &[f32],
    roles: &[String],
    k: i64,
) -> anyhow::Result<Vec<Hit>> {
    let mut tx = pool.begin().await?;
    exact_mode(&mut tx, true).await?;
    let hits = sem_on(&mut tx, table, Strategy::Exact, query_vec, roles, k).await?;
    tx.commit().await?;
    Ok(hits)
}

/// Parte lessicale: top-k per `ts_rank` con `websearch_to_tsquery(lang, query_text)` fra i
/// chunk visibili a `roles`.
pub async fn lexical_search(
    pool: &PgPool,
    table: Table,
    query_text: &str,
    lang: Lang,
    roles: &[String],
    k: i64,
) -> anyhow::Result<Vec<Ranked>> {
    let sql = lex_sql(table)
        .with_context(|| format!("ricerca lessicale non prevista su {}", table.name()))?;
    let rows = sqlx::query(sql)
        .bind(roles)
        .bind(lang.regconfig())
        .bind(query_text)
        .bind(k)
        .fetch_all(pool)
        .await
        .context("ricerca lessicale")?;
    rows.into_iter()
        .map(|r| {
            Ok(Ranked {
                chunk_id: r.try_get("id")?,
                doc_key: r.try_get("doc_key")?,
                score: r.try_get("score")?,
            })
        })
        .collect()
}

/// Ibrida sulla connessione data: `sem` e `lex` con `depth` candidati ciascuna, entrambe
/// filtrate dalla CTE `allowed`, e fusione RRF completa ordinata per punteggio poi `id`.
#[allow(clippy::too_many_arguments)]
pub async fn hybrid_on(
    conn: &mut PgConnection,
    table: Table,
    strategy: Strategy,
    query_vec: &[f32],
    query_text: &str,
    lang: Lang,
    roles: &[String],
    depth: i64,
) -> anyhow::Result<Breakdown> {
    let sql = hybrid_sql(table, strategy).with_context(|| {
        format!(
            "ibrida {} non prevista su {}",
            strategy.name(),
            table.name()
        )
    })?;
    let rows = bind_dense(sqlx::query(sql), strategy.dense(), query_vec)
        .bind(roles)
        .bind(lang.regconfig())
        .bind(query_text)
        .bind(depth)
        .fetch_all(conn)
        .await
        .context("ricerca ibrida")?;
    let mut b = Breakdown::default();
    let mut sem: Vec<(i64, Ranked)> = Vec::new();
    let mut lex: Vec<(i64, Ranked)> = Vec::new();
    for r in rows {
        let chunk_id: Uuid = r.try_get("id")?;
        let doc_key: String = r.try_get("doc_key")?;
        let sem_r: Option<i64> = r.try_get("sem_r")?;
        let lex_r: Option<i64> = r.try_get("lex_r")?;
        let rrf: f64 = r.try_get("rrf")?;
        let item = |score: f64| Ranked {
            chunk_id,
            doc_key: doc_key.clone(),
            score,
        };
        if let Some(s) = sem_r {
            sem.push((s, item(s as f64)));
        }
        if let Some(l) = lex_r {
            lex.push((l, item(l as f64)));
        }
        b.fused.push(item(rrf));
    }
    sem.sort_by_key(|p| p.0);
    lex.sort_by_key(|p| p.0);
    b.sem = sem.into_iter().map(|p| p.1).collect();
    b.lex = lex.into_iter().map(|p| p.1).collect();
    Ok(b)
}

/// Ibrida: RRF (k = [`RRF_K`]) di `sem` e `lex`, ciascuna filtrata dalla CTE `allowed`.
pub async fn hybrid_search(
    pool: &PgPool,
    table: Table,
    query_vec: &[f32],
    query_text: &str,
    lang: Lang,
    roles: &[String],
    k: i64,
) -> anyhow::Result<Vec<Ranked>> {
    let mut tx = pool.begin().await?;
    exact_mode(&mut tx, true).await?;
    let b = hybrid_on(
        &mut tx,
        table,
        Strategy::Exact,
        query_vec,
        query_text,
        lang,
        roles,
        PART_DEPTH,
    )
    .await?;
    tx.commit().await?;
    let mut fused = b.fused;
    fused.truncate(usize::try_from(k).unwrap_or(0));
    Ok(fused)
}

/// Densa su indice HNSW con i parametri di sessione dati e il predicato ACL nella query.
pub async fn hnsw_search(
    pool: &PgPool,
    table: Table,
    query_vec: &[f32],
    roles: &[String],
    k: i64,
    params: HnswParams,
) -> anyhow::Result<Vec<Hit>> {
    hnsw_search_dense(pool, table, Dense::Vector, query_vec, roles, k, params).await
}

/// Come [`hnsw_search`], sulla colonna densa scelta.
pub async fn hnsw_search_dense(
    pool: &PgPool,
    table: Table,
    dense: Dense,
    query_vec: &[f32],
    roles: &[String],
    k: i64,
    params: HnswParams,
) -> anyhow::Result<Vec<Hit>> {
    let mut tx = pool.begin().await?;
    set_hnsw(&mut tx, params, true).await?;
    let hits = sem_on(&mut tx, table, Strategy::Ann(dense), query_vec, roles, k).await?;
    tx.commit().await?;
    Ok(hits)
}

/// Quota dei chunk di `table` visibili a `role` (selettività del filtro).
pub async fn role_share(pool: &PgPool, table: Table, role: &str) -> anyhow::Result<f64> {
    sqlx::query_scalar(share_sql(table))
        .bind(role)
        .fetch_one(pool)
        .await
        .context("selettività")
}

/// Fusione RRF in Rust (verifica della query SQL): punteggio `Σ 1 / (k + rango)` con rango da
/// 1; ordine per punteggio decrescente, a parità per id crescente.
pub fn rrf_fuse(sem: &[Uuid], lex: &[Uuid], k: f64) -> Vec<(Uuid, f64)> {
    let mut score: BTreeMap<Uuid, f64> = BTreeMap::new();
    for list in [sem, lex] {
        for (i, id) in list.iter().enumerate() {
            *score.entry(*id).or_default() += 1.0 / (k + (i + 1) as f64);
        }
    }
    let mut out: Vec<(Uuid, f64)> = score.into_iter().collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

/// La fusione della SQL coincide con [`rrf_fuse`] sulle liste `sem` e `lex` restituite:
/// stesso ordine degli id e punteggi uguali a meno di 1e-9.
pub fn fusion_matches(b: &Breakdown) -> bool {
    let sem: Vec<Uuid> = b.sem.iter().map(|r| r.chunk_id).collect();
    let lex: Vec<Uuid> = b.lex.iter().map(|r| r.chunk_id).collect();
    let rust = rrf_fuse(&sem, &lex, RRF_K);
    rust.len() == b.fused.len()
        && rust
            .iter()
            .zip(&b.fused)
            .all(|(r, f)| r.0 == f.chunk_id && (r.1 - f.score).abs() < 1e-9)
}

// ---- SQL per tabella: un letterale completo per combinazione, scelto con `match` ---------

/// Ricerca densa con il predicato ACL nella stessa query (`$1` vettore, `$2` ruoli, `$3` k): semi-join per l'esatta, filtro come subplan sulla scansione HNSW per l'ANN.
fn sem_sql(table: Table, strategy: Strategy) -> Option<&'static str> {
    Some(match (table, strategy) {
        (Table::BgeM3, Strategy::Exact) => {
            "SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_bgem3 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2))
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::BgeM3, Strategy::Ann(Dense::Vector)) => {
            "SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_bgem3 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N10k), Strategy::Exact) => {
            "SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_10000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2))
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N10k), Strategy::Ann(Dense::Vector)) => {
            "SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_10000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N10k), Strategy::Ann(Dense::Half)) => {
            "SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb_h <=> $1 AS dist
  FROM chunk_synth_10000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb_h <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N50k), Strategy::Exact) => {
            "SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_50000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2))
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N50k), Strategy::Ann(Dense::Vector)) => {
            "SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_50000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N50k), Strategy::Ann(Dense::Half)) => {
            "SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb_h <=> $1 AS dist
  FROM chunk_synth_50000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb_h <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N100k), Strategy::Exact) => {
            "SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_100000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2))
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N100k), Strategy::Ann(Dense::Vector)) => {
            "SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_100000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N100k), Strategy::Ann(Dense::Half)) => {
            "SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb_h <=> $1 AS dist
  FROM chunk_synth_100000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb_h <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N300k), Strategy::Exact) => {
            "SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_300000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2))
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N300k), Strategy::Ann(Dense::Vector)) => {
            "SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_300000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N300k), Strategy::Ann(Dense::Half)) => {
            "SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb_h <=> $1 AS dist
  FROM chunk_synth_300000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb_h <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        _ => return None,
    })
}

/// Piano `EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)` della ricerca densa (tabelle sintetiche).
pub fn sem_explain_sql(table: Table, strategy: Strategy) -> Option<&'static str> {
    Some(match (table, strategy) {
        (Table::Synth(Scale::N10k), Strategy::Exact) => {
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_10000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2))
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N10k), Strategy::Ann(Dense::Vector)) => {
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_10000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N10k), Strategy::Ann(Dense::Half)) => {
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb_h <=> $1 AS dist
  FROM chunk_synth_10000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb_h <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N50k), Strategy::Exact) => {
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_50000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2))
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N50k), Strategy::Ann(Dense::Vector)) => {
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_50000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N50k), Strategy::Ann(Dense::Half)) => {
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb_h <=> $1 AS dist
  FROM chunk_synth_50000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb_h <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N100k), Strategy::Exact) => {
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_100000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2))
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N100k), Strategy::Ann(Dense::Vector)) => {
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_100000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N100k), Strategy::Ann(Dense::Half)) => {
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb_h <=> $1 AS dist
  FROM chunk_synth_100000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb_h <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N300k), Strategy::Exact) => {
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_300000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2))
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N300k), Strategy::Ann(Dense::Vector)) => {
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
  FROM chunk_synth_300000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        (Table::Synth(Scale::N300k), Strategy::Ann(Dense::Half)) => {
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)
SELECT s.id, d.doc_key, s.dist
FROM (
  SELECT c.id, c.doc_id, c.emb_h <=> $1 AS dist
  FROM chunk_synth_300000 c
  WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2) OFFSET 0)
  ORDER BY c.emb_h <=> $1
  LIMIT $3
) s
JOIN doc d ON d.id = s.doc_id
ORDER BY s.dist, s.id"
        }
        _ => return None,
    })
}

/// Ibrida RRF con la CTE `allowed` dentro `sem` e `lex` (`$1` vettore, `$2` ruoli, `$3` regconfig, `$4` testo, `$5` candidati per parte).
fn hybrid_sql(table: Table, strategy: Strategy) -> Option<&'static str> {
    Some(match (table, strategy) {
        (Table::BgeM3, Strategy::Exact) => {
            "WITH allowed AS (SELECT doc_id FROM doc_acl WHERE role = ANY($2)),
sem AS (
  SELECT s.id, s.doc_id, row_number() OVER (ORDER BY s.dist, s.id) AS r
  FROM (
    SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
    FROM chunk_bgem3 c
    WHERE c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY c.emb <=> $1
    LIMIT $5
  ) s
),
lex AS (
  SELECT l.id, l.doc_id, row_number() OVER (ORDER BY l.score DESC, l.id) AS r
  FROM (
    SELECT c.id, c.doc_id, ts_rank_cd(c.tsv, q.q) AS score
    FROM chunk_bgem3 c CROSS JOIN websearch_to_tsquery($3::regconfig, $4) AS q (q)
    WHERE c.tsv @@ q.q AND c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY score DESC, c.id
    LIMIT $5
  ) l
),
fused AS (
  SELECT COALESCE(sem.id, lex.id) AS id, COALESCE(sem.doc_id, lex.doc_id) AS doc_id,
         sem.r AS sem_r, lex.r AS lex_r,
         COALESCE(1.0 / (60 + sem.r), 0) + COALESCE(1.0 / (60 + lex.r), 0) AS rrf
  FROM sem FULL OUTER JOIN lex ON sem.id = lex.id
)
SELECT f.id, d.doc_key, f.sem_r, f.lex_r, f.rrf::float8 AS rrf
FROM fused f
JOIN doc d ON d.id = f.doc_id
ORDER BY f.rrf DESC, f.id"
        }
        (Table::Synth(Scale::N10k), Strategy::Exact) => {
            "WITH allowed AS (SELECT doc_id FROM doc_acl WHERE role = ANY($2)),
sem AS (
  SELECT s.id, s.doc_id, row_number() OVER (ORDER BY s.dist, s.id) AS r
  FROM (
    SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
    FROM chunk_synth_10000 c
    WHERE c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY c.emb <=> $1
    LIMIT $5
  ) s
),
lex AS (
  SELECT l.id, l.doc_id, row_number() OVER (ORDER BY l.score DESC, l.id) AS r
  FROM (
    SELECT c.id, c.doc_id, ts_rank_cd(c.tsv, q.q) AS score
    FROM chunk_synth_10000 c CROSS JOIN websearch_to_tsquery($3::regconfig, $4) AS q (q)
    WHERE c.tsv @@ q.q AND c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY score DESC, c.id
    LIMIT $5
  ) l
),
fused AS (
  SELECT COALESCE(sem.id, lex.id) AS id, COALESCE(sem.doc_id, lex.doc_id) AS doc_id,
         sem.r AS sem_r, lex.r AS lex_r,
         COALESCE(1.0 / (60 + sem.r), 0) + COALESCE(1.0 / (60 + lex.r), 0) AS rrf
  FROM sem FULL OUTER JOIN lex ON sem.id = lex.id
)
SELECT f.id, d.doc_key, f.sem_r, f.lex_r, f.rrf::float8 AS rrf
FROM fused f
JOIN doc d ON d.id = f.doc_id
ORDER BY f.rrf DESC, f.id"
        }
        (Table::Synth(Scale::N10k), Strategy::Ann(Dense::Vector)) => {
            "WITH allowed AS (SELECT doc_id FROM doc_acl WHERE role = ANY($2)),
sem AS (
  SELECT s.id, s.doc_id, row_number() OVER (ORDER BY s.dist, s.id) AS r
  FROM (
    SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
    FROM chunk_synth_10000 c
    WHERE EXISTS (SELECT 1 FROM allowed a WHERE a.doc_id = c.doc_id OFFSET 0)
    ORDER BY c.emb <=> $1
    LIMIT $5
  ) s
),
lex AS (
  SELECT l.id, l.doc_id, row_number() OVER (ORDER BY l.score DESC, l.id) AS r
  FROM (
    SELECT c.id, c.doc_id, ts_rank_cd(c.tsv, q.q) AS score
    FROM chunk_synth_10000 c CROSS JOIN websearch_to_tsquery($3::regconfig, $4) AS q (q)
    WHERE c.tsv @@ q.q AND c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY score DESC, c.id
    LIMIT $5
  ) l
),
fused AS (
  SELECT COALESCE(sem.id, lex.id) AS id, COALESCE(sem.doc_id, lex.doc_id) AS doc_id,
         sem.r AS sem_r, lex.r AS lex_r,
         COALESCE(1.0 / (60 + sem.r), 0) + COALESCE(1.0 / (60 + lex.r), 0) AS rrf
  FROM sem FULL OUTER JOIN lex ON sem.id = lex.id
)
SELECT f.id, d.doc_key, f.sem_r, f.lex_r, f.rrf::float8 AS rrf
FROM fused f
JOIN doc d ON d.id = f.doc_id
ORDER BY f.rrf DESC, f.id"
        }
        (Table::Synth(Scale::N10k), Strategy::Ann(Dense::Half)) => {
            "WITH allowed AS (SELECT doc_id FROM doc_acl WHERE role = ANY($2)),
sem AS (
  SELECT s.id, s.doc_id, row_number() OVER (ORDER BY s.dist, s.id) AS r
  FROM (
    SELECT c.id, c.doc_id, c.emb_h <=> $1 AS dist
    FROM chunk_synth_10000 c
    WHERE EXISTS (SELECT 1 FROM allowed a WHERE a.doc_id = c.doc_id OFFSET 0)
    ORDER BY c.emb_h <=> $1
    LIMIT $5
  ) s
),
lex AS (
  SELECT l.id, l.doc_id, row_number() OVER (ORDER BY l.score DESC, l.id) AS r
  FROM (
    SELECT c.id, c.doc_id, ts_rank_cd(c.tsv, q.q) AS score
    FROM chunk_synth_10000 c CROSS JOIN websearch_to_tsquery($3::regconfig, $4) AS q (q)
    WHERE c.tsv @@ q.q AND c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY score DESC, c.id
    LIMIT $5
  ) l
),
fused AS (
  SELECT COALESCE(sem.id, lex.id) AS id, COALESCE(sem.doc_id, lex.doc_id) AS doc_id,
         sem.r AS sem_r, lex.r AS lex_r,
         COALESCE(1.0 / (60 + sem.r), 0) + COALESCE(1.0 / (60 + lex.r), 0) AS rrf
  FROM sem FULL OUTER JOIN lex ON sem.id = lex.id
)
SELECT f.id, d.doc_key, f.sem_r, f.lex_r, f.rrf::float8 AS rrf
FROM fused f
JOIN doc d ON d.id = f.doc_id
ORDER BY f.rrf DESC, f.id"
        }
        (Table::Synth(Scale::N50k), Strategy::Exact) => {
            "WITH allowed AS (SELECT doc_id FROM doc_acl WHERE role = ANY($2)),
sem AS (
  SELECT s.id, s.doc_id, row_number() OVER (ORDER BY s.dist, s.id) AS r
  FROM (
    SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
    FROM chunk_synth_50000 c
    WHERE c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY c.emb <=> $1
    LIMIT $5
  ) s
),
lex AS (
  SELECT l.id, l.doc_id, row_number() OVER (ORDER BY l.score DESC, l.id) AS r
  FROM (
    SELECT c.id, c.doc_id, ts_rank_cd(c.tsv, q.q) AS score
    FROM chunk_synth_50000 c CROSS JOIN websearch_to_tsquery($3::regconfig, $4) AS q (q)
    WHERE c.tsv @@ q.q AND c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY score DESC, c.id
    LIMIT $5
  ) l
),
fused AS (
  SELECT COALESCE(sem.id, lex.id) AS id, COALESCE(sem.doc_id, lex.doc_id) AS doc_id,
         sem.r AS sem_r, lex.r AS lex_r,
         COALESCE(1.0 / (60 + sem.r), 0) + COALESCE(1.0 / (60 + lex.r), 0) AS rrf
  FROM sem FULL OUTER JOIN lex ON sem.id = lex.id
)
SELECT f.id, d.doc_key, f.sem_r, f.lex_r, f.rrf::float8 AS rrf
FROM fused f
JOIN doc d ON d.id = f.doc_id
ORDER BY f.rrf DESC, f.id"
        }
        (Table::Synth(Scale::N50k), Strategy::Ann(Dense::Vector)) => {
            "WITH allowed AS (SELECT doc_id FROM doc_acl WHERE role = ANY($2)),
sem AS (
  SELECT s.id, s.doc_id, row_number() OVER (ORDER BY s.dist, s.id) AS r
  FROM (
    SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
    FROM chunk_synth_50000 c
    WHERE EXISTS (SELECT 1 FROM allowed a WHERE a.doc_id = c.doc_id OFFSET 0)
    ORDER BY c.emb <=> $1
    LIMIT $5
  ) s
),
lex AS (
  SELECT l.id, l.doc_id, row_number() OVER (ORDER BY l.score DESC, l.id) AS r
  FROM (
    SELECT c.id, c.doc_id, ts_rank_cd(c.tsv, q.q) AS score
    FROM chunk_synth_50000 c CROSS JOIN websearch_to_tsquery($3::regconfig, $4) AS q (q)
    WHERE c.tsv @@ q.q AND c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY score DESC, c.id
    LIMIT $5
  ) l
),
fused AS (
  SELECT COALESCE(sem.id, lex.id) AS id, COALESCE(sem.doc_id, lex.doc_id) AS doc_id,
         sem.r AS sem_r, lex.r AS lex_r,
         COALESCE(1.0 / (60 + sem.r), 0) + COALESCE(1.0 / (60 + lex.r), 0) AS rrf
  FROM sem FULL OUTER JOIN lex ON sem.id = lex.id
)
SELECT f.id, d.doc_key, f.sem_r, f.lex_r, f.rrf::float8 AS rrf
FROM fused f
JOIN doc d ON d.id = f.doc_id
ORDER BY f.rrf DESC, f.id"
        }
        (Table::Synth(Scale::N50k), Strategy::Ann(Dense::Half)) => {
            "WITH allowed AS (SELECT doc_id FROM doc_acl WHERE role = ANY($2)),
sem AS (
  SELECT s.id, s.doc_id, row_number() OVER (ORDER BY s.dist, s.id) AS r
  FROM (
    SELECT c.id, c.doc_id, c.emb_h <=> $1 AS dist
    FROM chunk_synth_50000 c
    WHERE EXISTS (SELECT 1 FROM allowed a WHERE a.doc_id = c.doc_id OFFSET 0)
    ORDER BY c.emb_h <=> $1
    LIMIT $5
  ) s
),
lex AS (
  SELECT l.id, l.doc_id, row_number() OVER (ORDER BY l.score DESC, l.id) AS r
  FROM (
    SELECT c.id, c.doc_id, ts_rank_cd(c.tsv, q.q) AS score
    FROM chunk_synth_50000 c CROSS JOIN websearch_to_tsquery($3::regconfig, $4) AS q (q)
    WHERE c.tsv @@ q.q AND c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY score DESC, c.id
    LIMIT $5
  ) l
),
fused AS (
  SELECT COALESCE(sem.id, lex.id) AS id, COALESCE(sem.doc_id, lex.doc_id) AS doc_id,
         sem.r AS sem_r, lex.r AS lex_r,
         COALESCE(1.0 / (60 + sem.r), 0) + COALESCE(1.0 / (60 + lex.r), 0) AS rrf
  FROM sem FULL OUTER JOIN lex ON sem.id = lex.id
)
SELECT f.id, d.doc_key, f.sem_r, f.lex_r, f.rrf::float8 AS rrf
FROM fused f
JOIN doc d ON d.id = f.doc_id
ORDER BY f.rrf DESC, f.id"
        }
        (Table::Synth(Scale::N100k), Strategy::Exact) => {
            "WITH allowed AS (SELECT doc_id FROM doc_acl WHERE role = ANY($2)),
sem AS (
  SELECT s.id, s.doc_id, row_number() OVER (ORDER BY s.dist, s.id) AS r
  FROM (
    SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
    FROM chunk_synth_100000 c
    WHERE c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY c.emb <=> $1
    LIMIT $5
  ) s
),
lex AS (
  SELECT l.id, l.doc_id, row_number() OVER (ORDER BY l.score DESC, l.id) AS r
  FROM (
    SELECT c.id, c.doc_id, ts_rank_cd(c.tsv, q.q) AS score
    FROM chunk_synth_100000 c CROSS JOIN websearch_to_tsquery($3::regconfig, $4) AS q (q)
    WHERE c.tsv @@ q.q AND c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY score DESC, c.id
    LIMIT $5
  ) l
),
fused AS (
  SELECT COALESCE(sem.id, lex.id) AS id, COALESCE(sem.doc_id, lex.doc_id) AS doc_id,
         sem.r AS sem_r, lex.r AS lex_r,
         COALESCE(1.0 / (60 + sem.r), 0) + COALESCE(1.0 / (60 + lex.r), 0) AS rrf
  FROM sem FULL OUTER JOIN lex ON sem.id = lex.id
)
SELECT f.id, d.doc_key, f.sem_r, f.lex_r, f.rrf::float8 AS rrf
FROM fused f
JOIN doc d ON d.id = f.doc_id
ORDER BY f.rrf DESC, f.id"
        }
        (Table::Synth(Scale::N100k), Strategy::Ann(Dense::Vector)) => {
            "WITH allowed AS (SELECT doc_id FROM doc_acl WHERE role = ANY($2)),
sem AS (
  SELECT s.id, s.doc_id, row_number() OVER (ORDER BY s.dist, s.id) AS r
  FROM (
    SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
    FROM chunk_synth_100000 c
    WHERE EXISTS (SELECT 1 FROM allowed a WHERE a.doc_id = c.doc_id OFFSET 0)
    ORDER BY c.emb <=> $1
    LIMIT $5
  ) s
),
lex AS (
  SELECT l.id, l.doc_id, row_number() OVER (ORDER BY l.score DESC, l.id) AS r
  FROM (
    SELECT c.id, c.doc_id, ts_rank_cd(c.tsv, q.q) AS score
    FROM chunk_synth_100000 c CROSS JOIN websearch_to_tsquery($3::regconfig, $4) AS q (q)
    WHERE c.tsv @@ q.q AND c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY score DESC, c.id
    LIMIT $5
  ) l
),
fused AS (
  SELECT COALESCE(sem.id, lex.id) AS id, COALESCE(sem.doc_id, lex.doc_id) AS doc_id,
         sem.r AS sem_r, lex.r AS lex_r,
         COALESCE(1.0 / (60 + sem.r), 0) + COALESCE(1.0 / (60 + lex.r), 0) AS rrf
  FROM sem FULL OUTER JOIN lex ON sem.id = lex.id
)
SELECT f.id, d.doc_key, f.sem_r, f.lex_r, f.rrf::float8 AS rrf
FROM fused f
JOIN doc d ON d.id = f.doc_id
ORDER BY f.rrf DESC, f.id"
        }
        (Table::Synth(Scale::N100k), Strategy::Ann(Dense::Half)) => {
            "WITH allowed AS (SELECT doc_id FROM doc_acl WHERE role = ANY($2)),
sem AS (
  SELECT s.id, s.doc_id, row_number() OVER (ORDER BY s.dist, s.id) AS r
  FROM (
    SELECT c.id, c.doc_id, c.emb_h <=> $1 AS dist
    FROM chunk_synth_100000 c
    WHERE EXISTS (SELECT 1 FROM allowed a WHERE a.doc_id = c.doc_id OFFSET 0)
    ORDER BY c.emb_h <=> $1
    LIMIT $5
  ) s
),
lex AS (
  SELECT l.id, l.doc_id, row_number() OVER (ORDER BY l.score DESC, l.id) AS r
  FROM (
    SELECT c.id, c.doc_id, ts_rank_cd(c.tsv, q.q) AS score
    FROM chunk_synth_100000 c CROSS JOIN websearch_to_tsquery($3::regconfig, $4) AS q (q)
    WHERE c.tsv @@ q.q AND c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY score DESC, c.id
    LIMIT $5
  ) l
),
fused AS (
  SELECT COALESCE(sem.id, lex.id) AS id, COALESCE(sem.doc_id, lex.doc_id) AS doc_id,
         sem.r AS sem_r, lex.r AS lex_r,
         COALESCE(1.0 / (60 + sem.r), 0) + COALESCE(1.0 / (60 + lex.r), 0) AS rrf
  FROM sem FULL OUTER JOIN lex ON sem.id = lex.id
)
SELECT f.id, d.doc_key, f.sem_r, f.lex_r, f.rrf::float8 AS rrf
FROM fused f
JOIN doc d ON d.id = f.doc_id
ORDER BY f.rrf DESC, f.id"
        }
        (Table::Synth(Scale::N300k), Strategy::Exact) => {
            "WITH allowed AS (SELECT doc_id FROM doc_acl WHERE role = ANY($2)),
sem AS (
  SELECT s.id, s.doc_id, row_number() OVER (ORDER BY s.dist, s.id) AS r
  FROM (
    SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
    FROM chunk_synth_300000 c
    WHERE c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY c.emb <=> $1
    LIMIT $5
  ) s
),
lex AS (
  SELECT l.id, l.doc_id, row_number() OVER (ORDER BY l.score DESC, l.id) AS r
  FROM (
    SELECT c.id, c.doc_id, ts_rank_cd(c.tsv, q.q) AS score
    FROM chunk_synth_300000 c CROSS JOIN websearch_to_tsquery($3::regconfig, $4) AS q (q)
    WHERE c.tsv @@ q.q AND c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY score DESC, c.id
    LIMIT $5
  ) l
),
fused AS (
  SELECT COALESCE(sem.id, lex.id) AS id, COALESCE(sem.doc_id, lex.doc_id) AS doc_id,
         sem.r AS sem_r, lex.r AS lex_r,
         COALESCE(1.0 / (60 + sem.r), 0) + COALESCE(1.0 / (60 + lex.r), 0) AS rrf
  FROM sem FULL OUTER JOIN lex ON sem.id = lex.id
)
SELECT f.id, d.doc_key, f.sem_r, f.lex_r, f.rrf::float8 AS rrf
FROM fused f
JOIN doc d ON d.id = f.doc_id
ORDER BY f.rrf DESC, f.id"
        }
        (Table::Synth(Scale::N300k), Strategy::Ann(Dense::Vector)) => {
            "WITH allowed AS (SELECT doc_id FROM doc_acl WHERE role = ANY($2)),
sem AS (
  SELECT s.id, s.doc_id, row_number() OVER (ORDER BY s.dist, s.id) AS r
  FROM (
    SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
    FROM chunk_synth_300000 c
    WHERE EXISTS (SELECT 1 FROM allowed a WHERE a.doc_id = c.doc_id OFFSET 0)
    ORDER BY c.emb <=> $1
    LIMIT $5
  ) s
),
lex AS (
  SELECT l.id, l.doc_id, row_number() OVER (ORDER BY l.score DESC, l.id) AS r
  FROM (
    SELECT c.id, c.doc_id, ts_rank_cd(c.tsv, q.q) AS score
    FROM chunk_synth_300000 c CROSS JOIN websearch_to_tsquery($3::regconfig, $4) AS q (q)
    WHERE c.tsv @@ q.q AND c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY score DESC, c.id
    LIMIT $5
  ) l
),
fused AS (
  SELECT COALESCE(sem.id, lex.id) AS id, COALESCE(sem.doc_id, lex.doc_id) AS doc_id,
         sem.r AS sem_r, lex.r AS lex_r,
         COALESCE(1.0 / (60 + sem.r), 0) + COALESCE(1.0 / (60 + lex.r), 0) AS rrf
  FROM sem FULL OUTER JOIN lex ON sem.id = lex.id
)
SELECT f.id, d.doc_key, f.sem_r, f.lex_r, f.rrf::float8 AS rrf
FROM fused f
JOIN doc d ON d.id = f.doc_id
ORDER BY f.rrf DESC, f.id"
        }
        (Table::Synth(Scale::N300k), Strategy::Ann(Dense::Half)) => {
            "WITH allowed AS (SELECT doc_id FROM doc_acl WHERE role = ANY($2)),
sem AS (
  SELECT s.id, s.doc_id, row_number() OVER (ORDER BY s.dist, s.id) AS r
  FROM (
    SELECT c.id, c.doc_id, c.emb_h <=> $1 AS dist
    FROM chunk_synth_300000 c
    WHERE EXISTS (SELECT 1 FROM allowed a WHERE a.doc_id = c.doc_id OFFSET 0)
    ORDER BY c.emb_h <=> $1
    LIMIT $5
  ) s
),
lex AS (
  SELECT l.id, l.doc_id, row_number() OVER (ORDER BY l.score DESC, l.id) AS r
  FROM (
    SELECT c.id, c.doc_id, ts_rank_cd(c.tsv, q.q) AS score
    FROM chunk_synth_300000 c CROSS JOIN websearch_to_tsquery($3::regconfig, $4) AS q (q)
    WHERE c.tsv @@ q.q AND c.doc_id IN (SELECT doc_id FROM allowed)
    ORDER BY score DESC, c.id
    LIMIT $5
  ) l
),
fused AS (
  SELECT COALESCE(sem.id, lex.id) AS id, COALESCE(sem.doc_id, lex.doc_id) AS doc_id,
         sem.r AS sem_r, lex.r AS lex_r,
         COALESCE(1.0 / (60 + sem.r), 0) + COALESCE(1.0 / (60 + lex.r), 0) AS rrf
  FROM sem FULL OUTER JOIN lex ON sem.id = lex.id
)
SELECT f.id, d.doc_key, f.sem_r, f.lex_r, f.rrf::float8 AS rrf
FROM fused f
JOIN doc d ON d.id = f.doc_id
ORDER BY f.rrf DESC, f.id"
        }
        _ => return None,
    })
}

/// Parte lessicale (`$1` ruoli, `$2` regconfig, `$3` testo, `$4` k): solo sul corpus reale.
fn lex_sql(table: Table) -> Option<&'static str> {
    match table {
        Table::BgeM3 => Some(
            "WITH allowed AS (SELECT doc_id FROM doc_acl WHERE role = ANY($1))
SELECT l.id, d.doc_key, l.score::float8 AS score
FROM (
  SELECT c.id, c.doc_id, ts_rank_cd(c.tsv, q.q) AS score
  FROM chunk_bgem3 c CROSS JOIN websearch_to_tsquery($2::regconfig, $3) AS q (q)
  WHERE c.tsv @@ q.q AND c.doc_id IN (SELECT doc_id FROM allowed)
  ORDER BY score DESC, c.id
  LIMIT $4
) l
JOIN doc d ON d.id = l.doc_id
ORDER BY l.score DESC, l.id",
        ),
        Table::Synth(_) => None,
    }
}

/// Quota dei chunk visibili al ruolo `$1` (selettività del filtro).
pub fn share_sql(table: Table) -> &'static str {
    match table {
        Table::BgeM3 => {
            "SELECT (count(*) FILTER (WHERE EXISTS (SELECT 1 FROM doc_acl a
        WHERE a.doc_id = c.doc_id AND a.role = $1)))::float8 / greatest(count(*), 1)
FROM chunk_bgem3 c"
        }
        Table::Synth(Scale::N10k) => {
            "SELECT (count(*) FILTER (WHERE EXISTS (SELECT 1 FROM doc_acl a
        WHERE a.doc_id = c.doc_id AND a.role = $1)))::float8 / greatest(count(*), 1)
FROM chunk_synth_10000 c"
        }
        Table::Synth(Scale::N50k) => {
            "SELECT (count(*) FILTER (WHERE EXISTS (SELECT 1 FROM doc_acl a
        WHERE a.doc_id = c.doc_id AND a.role = $1)))::float8 / greatest(count(*), 1)
FROM chunk_synth_50000 c"
        }
        Table::Synth(Scale::N100k) => {
            "SELECT (count(*) FILTER (WHERE EXISTS (SELECT 1 FROM doc_acl a
        WHERE a.doc_id = c.doc_id AND a.role = $1)))::float8 / greatest(count(*), 1)
FROM chunk_synth_100000 c"
        }
        Table::Synth(Scale::N300k) => {
            "SELECT (count(*) FILTER (WHERE EXISTS (SELECT 1 FROM doc_acl a
        WHERE a.doc_id = c.doc_id AND a.role = $1)))::float8 / greatest(count(*), 1)
FROM chunk_synth_300000 c"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::{Scratch, far, query_vec, roles};

    // TEST-18 (RISK-1, RISK-7): RRF con k = 60, valori noti e parità deterministica.
    #[test]
    fn test18_rrf_fusion_known_values() {
        let (a, b, c) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
        let f = rrf_fuse(&[a, b], &[a, c], RRF_K);
        assert_eq!(f.len(), 3);
        assert_eq!(f[0].0, a);
        assert!(
            (f[0].1 - 2.0 / 61.0).abs() < 1e-12,
            "primo in entrambe: 2/61"
        );
        assert_eq!(
            (f[1].0, f[2].0),
            (b, c),
            "a parità di punteggio, ordine per id"
        );
        assert!((f[1].1 - 1.0 / 62.0).abs() < 1e-12);

        let (x, y) = (Uuid::from_u128(20), Uuid::from_u128(10));
        let g = rrf_fuse(&[x], &[y], RRF_K);
        assert_eq!(g.iter().map(|p| p.0).collect::<Vec<_>>(), [y, x]);
        assert!(
            g.iter().all(|p| (p.1 - 1.0 / 61.0).abs() < 1e-12),
            "primo in una sola: 1/61"
        );
        assert_eq!(g, rrf_fuse(&[x], &[y], RRF_K), "non deterministico");
    }

    // TEST-9 (RISK-4): tabella da enum chiuso.
    #[test]
    fn test9_tables_and_settings_from_closed_enums() {
        assert_eq!(Table::BgeM3.name(), "chunk_bgem3");
        assert_eq!(Table::Synth(Scale::N10k).name(), "chunk_synth_10000");
        assert_eq!(Table::Synth(Scale::N300k).name(), "chunk_synth_300000");
        let s: Vec<&str> = IterativeScan::ALL.iter().map(|i| i.setting()).collect();
        assert_eq!(s, ["off", "relaxed_order", "strict_order"]);
    }

    // TEST-3 (RISK-1): la CTE `allowed` filtra sia `sem` sia `lex`, con i ruoli legati.
    #[test]
    fn test3_allowed_cte_in_both_parts() {
        let src = include_str!("hybrid.rs");
        let code = src.split("#[cfg(test)]").next().unwrap();
        let n = code.matches("IN (SELECT doc_id FROM allowed)").count();
        assert!(
            n >= 2,
            "`IN (SELECT doc_id FROM allowed)` in sem e lex: trovato {n} volte"
        );
        assert!(code.contains("allowed AS"), "CTE `allowed` assente");
        assert!(
            code.contains("role = ANY($"),
            "ruoli della CTE legati a parametro"
        );
    }

    const RARE: &str = "zettabilancio";

    // TEST-1 (RISK-1): un match lessicale su documento negato non entra né in lex né nella fusione.
    #[tokio::test]
    #[ignore = "richiede DATABASE_URL (Postgres del compose)"]
    async fn test1_hybrid_lexical_match_on_denied_doc_not_returned() {
        let s = Scratch::new().await;
        let d_res = s.doc("D_res", "restricted").await;
        let d_pub = s.doc("D_pub", "public").await;
        let res_chunk = s
            .chunk(d_res, "Il zettabilancio consolidato del gruppo", far(0))
            .await;
        s.chunk(
            d_pub,
            "Relazione sulla gestione dell'esercizio",
            query_vec(),
        )
        .await;
        let t = Table::BgeM3;

        let lex = lexical_search(&s.pool, t, RARE, Lang::Italian, &roles(&["employee"]), 5)
            .await
            .expect("lex employee");
        assert!(
            lex.iter().all(|h| h.doc_key != "D_res"),
            "leak in lex: {lex:?}"
        );
        let hyb = hybrid_search(
            &s.pool,
            t,
            &query_vec(),
            RARE,
            Lang::Italian,
            &roles(&["employee"]),
            5,
        )
        .await
        .expect("ibrida employee");
        assert!(
            hyb.iter().all(|h| h.doc_key != "D_res"),
            "leak nella fusione: {hyb:?}"
        );
        assert!(
            hyb.iter().any(|h| h.doc_key == "D_pub"),
            "la parte sem deve restare: {hyb:?}"
        );

        let lex_a = lexical_search(&s.pool, t, RARE, Lang::Italian, &roles(&["admin"]), 5)
            .await
            .expect("lex admin");
        assert_eq!(
            lex_a.first().map(|h| h.chunk_id),
            Some(res_chunk),
            "{lex_a:?}"
        );
        let hyb_a = hybrid_search(
            &s.pool,
            t,
            &query_vec(),
            RARE,
            Lang::Italian,
            &roles(&["admin"]),
            5,
        )
        .await
        .expect("ibrida admin");
        assert_eq!(
            hyb_a.first().map(|h| h.chunk_id),
            Some(res_chunk),
            "{hyb_a:?}"
        );
        s.drop().await;
    }

    // TEST-2 (RISK-1): un match semantico su documento negato non entra né in sem né nella fusione.
    #[tokio::test]
    #[ignore = "richiede DATABASE_URL (Postgres del compose)"]
    async fn test2_hybrid_semantic_match_on_denied_doc_not_returned() {
        let s = Scratch::new().await;
        let d_res = s.doc("D_res", "restricted").await;
        let d_pub = s.doc("D_pub", "public").await;
        let res_chunk = s.chunk(d_res, "alfa beta gamma", query_vec()).await;
        s.chunk(d_pub, "Il zettabilancio consolidato del gruppo", far(0))
            .await;
        let t = Table::BgeM3;

        let sem = semantic_search(&s.pool, t, &query_vec(), &roles(&["employee"]), 5)
            .await
            .expect("sem employee");
        assert!(
            sem.iter().all(|h| h.doc_key != "D_res"),
            "leak in sem: {sem:?}"
        );
        assert!(sem.iter().any(|h| h.doc_key == "D_pub"));
        let hyb = hybrid_search(
            &s.pool,
            t,
            &query_vec(),
            RARE,
            Lang::Italian,
            &roles(&["employee"]),
            5,
        )
        .await
        .expect("ibrida employee");
        assert!(
            hyb.iter().all(|h| h.doc_key != "D_res"),
            "leak nella fusione: {hyb:?}"
        );
        assert!(hyb.iter().any(|h| h.doc_key == "D_pub"), "{hyb:?}");

        let sem_a = semantic_search(&s.pool, t, &query_vec(), &roles(&["admin"]), 5)
            .await
            .expect("sem admin");
        assert_eq!(
            sem_a.first().map(|h| h.chunk_id),
            Some(res_chunk),
            "{sem_a:?}"
        );
        s.drop().await;
    }

    // TEST-8 (RISK-4): ruolo e testo di query ostili passano come parametri.
    #[tokio::test]
    #[ignore = "richiede DATABASE_URL (Postgres del compose)"]
    async fn test8_hostile_role_and_query_text_are_bound() {
        let s = Scratch::new().await;
        let d_pub = s.doc("D_pub", "public").await;
        let d_res = s.doc("D_res", "restricted").await;
        s.chunk(d_pub, "Il zettabilancio consolidato", far(0)).await;
        s.chunk(d_res, "Il zettabilancio riservato", query_vec())
            .await;
        let t = Table::BgeM3;

        let hostile = roles(&["employee' OR '1'='1", "admin') OR TRUE --"]);
        let hyb = hybrid_search(&s.pool, t, &query_vec(), RARE, Lang::Italian, &hostile, 10)
            .await
            .expect("la query non deve fallire per sintassi");
        assert!(hyb.is_empty(), "nessun ruolo con quel nome: {hyb:?}");

        let evil = "'); DROP TABLE doc; --";
        lexical_search(&s.pool, t, evil, Lang::Italian, &roles(&["employee"]), 10)
            .await
            .expect("testo ostile: nessun errore");
        hybrid_search(
            &s.pool,
            t,
            &query_vec(),
            evil,
            Lang::Italian,
            &roles(&["employee"]),
            10,
        )
        .await
        .expect("testo ostile nell'ibrida: nessun errore");
        let docs: i64 = sqlx::query_scalar("SELECT count(*) FROM doc")
            .fetch_one(&s.pool)
            .await
            .expect("la tabella doc esiste ancora");
        assert_eq!(docs, 2);
        s.drop().await;
    }
}
