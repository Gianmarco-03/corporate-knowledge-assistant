//! `ingest-under-load`: inserimento di chunk durante le query (criterio 3 di S-03, T-01-14-04).
//!
//! Task A: query esatte di `sel100` in ciclo su una connessione per `seconds` secondi.
//! Task B: due documenti al secondo da 10 chunk (20 chunk/s), un commit per documento
//! (documento, righe di `doc_acl`, chunk) su un'altra connessione. Una riga CSV al secondo:
//! `t,query_p50,query_p95,ingest_rows,errors`. A fine misura i documenti inseriti si
//! cancellano e `doc_acl` si riconfronta con la matrice attesa.
//!
//! **Modifica:** Claude (gsd-executor 01-14) · 2026-10-25 · ingestion concorrente alle query
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use s03_pgvector::db;
use s03_pgvector::metrics;
use sqlx::{Connection, PgConnection};
use uuid::Uuid;

use crate::hybrid::{self, Dense, Strategy, Table};
use crate::leak::full_expected;
use crate::synth::{self, InsertLot, Scale};
use crate::{BenchQuery, csv_upsert, read};

/// Documenti inseriti al secondo e chunk per documento (20 chunk/s).
const DOCS_PER_S: u64 = 2;
const CHUNKS_PER_INGEST_DOC: usize = 10;

/// Prefisso dei documenti inseriti durante la misura.
const INGEST_PREFIX: &str = "ingest-";
const INGEST_LIKE: &str = "ingest-%";

/// Seme dei vettori inseriti (distinto da quello di `synth`).
const INGEST_SEED: u64 = 0x0114_1257;

/// Campioni di un secondo.
#[derive(Default, Clone)]
struct Second {
    lat_us: Vec<u64>,
    errors: u64,
    rows_done: u64,
}

/// Percentile nearest-rank (`q` in (0, 1]) di valori non ordinati; vuoto → `None`.
pub fn nearest_rank(values: &[u64], q: f64) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_unstable();
    let idx = ((q * v.len() as f64).ceil() as usize).clamp(1, v.len()) - 1;
    Some(v[idx])
}

fn ms(us: Option<u64>) -> String {
    us.map_or(String::new(), |u| format!("{:.3}", u as f64 / 1000.0))
}

async fn cleanup(conn: &mut PgConnection, scale: Scale) -> anyhow::Result<()> {
    sqlx::query(synth::delete_prefix_chunks_sql(scale))
        .bind(INGEST_LIKE)
        .execute(&mut *conn)
        .await
        .context("pulizia dei chunk inseriti")?;
    sqlx::query("DELETE FROM doc_acl WHERE doc_id IN (SELECT id FROM doc WHERE doc_key LIKE $1)")
        .bind(INGEST_LIKE)
        .execute(&mut *conn)
        .await
        .context("pulizia doc_acl")?;
    sqlx::query("DELETE FROM doc WHERE doc_key LIKE $1")
        .bind(INGEST_LIKE)
        .execute(&mut *conn)
        .await
        .context("pulizia doc")?;
    Ok(())
}

/// Un documento con i suoi chunk, in una transazione (un commit per documento).
#[allow(clippy::too_many_arguments)]
async fn insert_doc(
    conn: &mut PgConnection,
    scale: Scale,
    key: &str,
    classification: &str,
    acl_json: &str,
    vecs: &[Vec<f32>],
    bodies: &[String],
    langs: &[String],
) -> anyhow::Result<()> {
    let id = Uuid::new_v4();
    let manifest = serde_json::json!([{"doc_id": key, "classification": classification}]);
    let mut vis = metrics::expected_visibility(acl_json, &manifest.to_string())?;
    vis.entry("sel100".to_string())
        .or_default()
        .insert(key.to_string());
    let ids = HashMap::from([(key.to_string(), id)]);
    let mut tx = conn.begin().await?;
    sqlx::query("INSERT INTO doc (id, doc_key, classification) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(key)
        .bind(classification)
        .execute(&mut *tx)
        .await
        .context("insert doc")?;
    synth::insert_acl(&mut *tx, &vis, &ids).await?;
    let mut lot = InsertLot::default();
    for ((v, b), l) in vecs.iter().zip(bodies).zip(langs) {
        lot.push(Uuid::new_v4(), id, b, l, v.clone());
    }
    synth::insert_lot(&mut *tx, scale, lot).await?;
    tx.commit().await?;
    Ok(())
}

/// Comando `ingest-under-load`.
pub async fn run(
    url: &str,
    corpus: &Path,
    out: &Path,
    scale: Scale,
    seconds: u64,
    queries: &[BenchQuery],
) -> anyhow::Result<()> {
    let pool = db::connect(url).await?;
    for d in Dense::ALL {
        let name = crate::hnsw_index_name(scale, d);
        if crate::leak::index_exists(&pool, name).await? {
            bail!("indice {name} presente: la misura è sulla ricerca esatta (drop-hnsw prima)");
        }
    }
    let acl = read(&corpus.join("acl.json"))?;
    let mut setup = PgConnection::connect(url).await?;
    cleanup(&mut setup, scale).await?;
    let expected = full_expected(&pool, corpus).await?;
    db::check_doc_acl(&pool, &expected).await?;

    let base = synth::base_chunks(&pool).await?;
    let mut classes: Vec<String> = synth::base_docs(&pool)
        .await?
        .into_iter()
        .map(|d| d.1)
        .collect();
    classes.sort();
    let n_docs = (seconds * DOCS_PER_S) as usize;
    let vecs = synth::perturb(&base.vecs, n_docs * CHUNKS_PER_INGEST_DOC, INGEST_SEED);

    let secs = usize::try_from(seconds)?;
    let state = Arc::new(Mutex::new(vec![Second::default(); secs]));
    let start = Instant::now();
    let deadline = Duration::from_secs(seconds);

    // Task A: query in ciclo.
    let qa = {
        let state = state.clone();
        let url = url.to_string();
        let qvecs: Vec<Vec<f32>> = queries.iter().map(|q| q.vec.clone()).collect();
        tokio::spawn(async move {
            let mut conn = crate::conn_with(&url, crate::workers_setup(4)).await?;
            let roles = vec!["sel100".to_string()];
            let mut i = 0usize;
            loop {
                let t0 = Instant::now();
                let res = hybrid::sem_on(
                    &mut conn,
                    Table::Synth(scale),
                    Strategy::Exact,
                    &qvecs[i % qvecs.len()],
                    &roles,
                    10,
                )
                .await;
                let el = start.elapsed();
                if el >= deadline {
                    break;
                }
                let sec = el.as_secs() as usize;
                let mut st = state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("stato avvelenato"))?;
                match res {
                    Ok(_) => st[sec].lat_us.push(t0.elapsed().as_micros() as u64),
                    Err(_) => st[sec].errors += 1,
                }
                i += 1;
            }
            anyhow::Ok(i)
        })
    };

    // Task B: un documento ogni 1 / DOCS_PER_S secondi, un commit per documento.
    let ib = {
        let state = state.clone();
        let url = url.to_string();
        tokio::spawn(async move {
            let mut conn = PgConnection::connect(&url).await?;
            let mut tick = tokio::time::interval(Duration::from_millis(1000 / DOCS_PER_S));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            let mut rows = 0u64;
            let mut j = 0usize;
            while j < n_docs {
                tick.tick().await;
                if start.elapsed() >= deadline {
                    break;
                }
                let key = format!("{INGEST_PREFIX}{}-{j:06}", scale.n());
                let lo = j * CHUNKS_PER_INGEST_DOC;
                let hi = lo + CHUNKS_PER_INGEST_DOC;
                let idx: Vec<usize> = (lo..hi).map(|c| c % base.bodies.len()).collect();
                let bodies: Vec<String> = idx.iter().map(|b| base.bodies[*b].clone()).collect();
                let langs: Vec<String> = idx.iter().map(|b| base.langs[*b].clone()).collect();
                let res = insert_doc(
                    &mut conn,
                    scale,
                    &key,
                    &classes[j % classes.len()],
                    &acl,
                    &vecs[lo..hi],
                    &bodies,
                    &langs,
                )
                .await;
                let el = start.elapsed();
                let sec = (el.as_secs() as usize).min(secs - 1);
                let mut st = state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("stato avvelenato"))?;
                match res {
                    Ok(()) => {
                        rows += CHUNKS_PER_INGEST_DOC as u64;
                        st[sec].rows_done = st[sec].rows_done.max(rows);
                    }
                    Err(e) => {
                        st[sec].errors += 1;
                        eprintln!("ingest: inserimento fallito al secondo {sec}: {e:#}");
                    }
                }
                j += 1;
            }
            anyhow::Ok(rows)
        })
    };

    let n_queries = qa.await.context("task delle query")??;
    let n_rows = ib.await.context("task di ingestion")??;

    let st = state
        .lock()
        .map_err(|_| anyhow::anyhow!("stato avvelenato"))?
        .clone();
    let mut csv_rows = Vec::with_capacity(secs);
    let mut cum = 0u64;
    let mut errors = 0u64;
    let mut p95s = Vec::new();
    for (t, s) in st.iter().enumerate() {
        cum = cum.max(s.rows_done);
        errors += s.errors;
        let p95 = nearest_rank(&s.lat_us, 0.95);
        if let Some(p) = p95 {
            p95s.push(p);
        }
        csv_rows.push(vec![
            (t + 1).to_string(),
            ms(nearest_rank(&s.lat_us, 0.50)),
            ms(p95),
            cum.to_string(),
            s.errors.to_string(),
        ]);
    }
    csv_upsert(
        &out.join("ingest_under_load.csv"),
        &["t", "query_p50", "query_p95", "ingest_rows", "errors"],
        1,
        &csv_rows,
    )?;

    cleanup(&mut setup, scale).await?;
    db::check_doc_acl(&pool, &expected)
        .await
        .context("doc_acl dopo la pulizia dell'ingestion")?;
    setup.close().await.ok();
    println!(
        "ingest-under-load {}: {n_queries} query, {n_rows} righe inserite, errori {errors}, p95 (nearest-rank dei p95 al secondo) {} ms; doc_acl ripristinata",
        scale.n(),
        ms(nearest_rank(&p95s, 0.95))
    );
    Ok(())
}
