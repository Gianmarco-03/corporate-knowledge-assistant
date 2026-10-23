//! Accesso a PostgreSQL + pgvector (piano 01-10): ricerca esatta con il predicato ACL nella
//! stessa query del ranking (D-12), parametri solo con `bind`, tabella da enum chiuso.
//!
//! Il nome della tabella per modello non viene mai dall'input: ogni istruzione SQL che lo
//! contiene è una costante `&'static str` scelta con un `match` su [`Model`] (macro
//! `per_model!`, che concatena solo letterali a tempo di compilazione).
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;

use anyhow::{Context, bail};
use pgvector::Vector;
use serde::Serialize;
use sqlx::PgPool;
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::Model;
use crate::chunk::{Chunk, Lang};
use crate::metrics::Visibility;

/// Istruzione SQL costante per modello: `$before` + nome tabella + `$after`, tutti letterali.
macro_rules! per_model {
    ($model:expr, $before:literal, $after:literal) => {
        match $model {
            Model::Qwen3 => concat!($before, "chunk_qwen3", $after),
            Model::BgeM3 => concat!($before, "chunk_bgem3", $after),
            Model::E5 => concat!($before, "chunk_e5", $after),
        }
    };
}

/// DDL dello spike, idempotente (`IF NOT EXISTS`).
pub const SCHEMA_SQL: &str = include_str!("../sql/schema.sql");

/// Righe per istruzione di inserimento a lotti.
pub const INSERT_BATCH: usize = 500;

/// Un risultato della ricerca esatta, già autorizzato.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub chunk_id: Uuid,
    pub doc_key: String,
    pub distance: f64,
}

/// Query da esportare come candidati per il reranker (piano 01-15).
#[derive(Clone, Debug)]
pub struct CandidateQuery {
    pub oracle_id: String,
    pub roles: Vec<String>,
    pub query_vec: Vec<f32>,
}

/// Nome della tabella dei chunk per modello: `match` su enum chiuso, mai dall'input.
pub fn table(model: Model) -> &'static str {
    per_model!(model, "", "")
}

/// Pool da `DATABASE_URL` (mai stampata).
pub async fn connect_from_env() -> anyhow::Result<PgPool> {
    let url = common::env::required("DATABASE_URL")?;
    connect(&url).await
}

/// Pool verso PostgreSQL con al più 4 connessioni.
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(30))
        .connect(database_url)
        .await
        .context("connessione a PostgreSQL (DATABASE_URL)")
}

/// Applica `sql/schema.sql` (idempotente).
pub async fn apply_schema(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::raw_sql(SCHEMA_SQL)
        .execute(pool)
        .await
        .context("sql/schema.sql")?;
    Ok(())
}

/// Inserisce i documenti mancanti (`doc_key` univoco) e restituisce `doc_key → id`.
/// Una classificazione in tabella diversa da quella del manifest ferma il comando.
pub async fn upsert_docs(
    pool: &PgPool,
    docs: &[(String, String)],
) -> anyhow::Result<HashMap<String, Uuid>> {
    for (key, class) in docs {
        sqlx::query(
            "INSERT INTO doc (id, doc_key, classification) VALUES ($1, $2, $3) \
             ON CONFLICT (doc_key) DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(key)
        .bind(class)
        .execute(pool)
        .await
        .context("insert doc")?;
    }
    let rows = sqlx::query("SELECT id, doc_key, classification FROM doc")
        .fetch_all(pool)
        .await
        .context("select doc")?;
    let mut ids = HashMap::new();
    for r in rows {
        let id: Uuid = r.try_get("id")?;
        let key: String = r.try_get("doc_key")?;
        let class: String = r.try_get("classification")?;
        if let Some((_, want)) = docs.iter().find(|(k, _)| *k == key)
            && *want != class
        {
            bail!("doc {key}: classificazione in tabella {class}, nel manifest {want}");
        }
        ids.insert(key, id);
    }
    Ok(ids)
}

/// Scrive in `doc_acl` le righe della matrice attesa (ruoli e profili di `acl.json`).
/// Non cancella righe: quelle in più le trova [`acl_diff`] prima di ogni misura.
pub async fn insert_doc_acl(
    pool: &PgPool,
    expected: &Visibility,
    doc_ids: &HashMap<String, Uuid>,
) -> anyhow::Result<()> {
    for (role, docs) in expected {
        for key in docs {
            let id = doc_ids
                .get(key)
                .with_context(|| format!("doc {key} assente dalla tabella doc"))?;
            sqlx::query(
                "INSERT INTO doc_acl (doc_id, role) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(id)
            .bind(role)
            .execute(pool)
            .await
            .context("insert doc_acl")?;
        }
    }
    Ok(())
}

/// `doc_acl` letta dal DB come ruolo → insieme di `doc_key` (join con `doc`).
pub async fn read_doc_acl(pool: &PgPool) -> anyhow::Result<BTreeMap<String, BTreeSet<String>>> {
    let rows = sqlx::query(
        "SELECT a.role, d.doc_key FROM doc_acl a JOIN doc d ON d.id = a.doc_id ORDER BY 1, 2",
    )
    .fetch_all(pool)
    .await
    .context("select doc_acl")?;
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for r in rows {
        out.entry(r.try_get("role")?)
            .or_default()
            .insert(r.try_get("doc_key")?);
    }
    Ok(out)
}

/// Differenze fra `doc_acl` letta dal DB e la matrice attesa da `acl.json` + manifest:
/// righe mancanti e righe in più, una per voce. Vuoto = coincidono esattamente.
pub fn acl_diff(expected: &Visibility, actual: &BTreeMap<String, BTreeSet<String>>) -> Vec<String> {
    let mut roles: BTreeSet<&str> = expected.keys().map(String::as_str).collect();
    roles.extend(actual.keys().map(String::as_str));
    let empty_e = Default::default();
    let empty_a = BTreeSet::new();
    let mut out = Vec::new();
    for role in roles {
        let e: &std::collections::HashSet<String> = expected.get(role).unwrap_or(&empty_e);
        let a = actual.get(role).unwrap_or(&empty_a);
        let mut missing: Vec<&String> = e.iter().filter(|k| !a.contains(*k)).collect();
        missing.sort();
        for k in missing {
            out.push(format!("mancante: ({role}, {k})"));
        }
        for k in a.iter().filter(|k| !e.contains(*k)) {
            out.push(format!("in più: ({role}, {k})"));
        }
    }
    out
}

/// Ferma il comando se `doc_acl` non coincide con la matrice attesa.
pub async fn check_doc_acl(pool: &PgPool, expected: &Visibility) -> anyhow::Result<()> {
    let diff = acl_diff(expected, &read_doc_acl(pool).await?);
    if !diff.is_empty() {
        bail!(
            "doc_acl diversa dalla matrice attesa di acl.json + manifest ({} differenze): {}",
            diff.len(),
            diff.join("; ")
        );
    }
    Ok(())
}

/// Svuota la tabella dei chunk del modello.
pub async fn truncate_chunks(pool: &PgPool, model: Model) -> anyhow::Result<()> {
    sqlx::query(per_model!(model, "TRUNCATE ", ""))
        .execute(pool)
        .await
        .context("truncate")?;
    Ok(())
}

/// Riga da inserire: id stabile, documento, chunk e vettore.
pub struct ChunkRow<'a> {
    pub id: Uuid,
    pub doc_id: Uuid,
    pub chunk: &'a Chunk,
    pub emb: Vec<f32>,
}

fn to_i32(v: usize) -> anyhow::Result<i32> {
    i32::try_from(v).context("offset oltre i32")
}

/// Inserimento a lotti di [`INSERT_BATCH`] righe con `INSERT … SELECT … FROM UNNEST(...)`.
pub async fn insert_chunks(
    pool: &PgPool,
    model: Model,
    rows: &[ChunkRow<'_>],
) -> anyhow::Result<()> {
    let sql = per_model!(
        model,
        "INSERT INTO ",
        " (id, doc_id, page, section_path, char_start, char_end, lang, body, emb) \
         SELECT u.id, u.doc_id, u.page, u.section_path, u.char_start, u.char_end, \
         u.lang::regconfig, u.body, u.emb \
         FROM UNNEST($1::uuid[], $2::uuid[], $3::int4[], $4::text[], $5::int4[], $6::int4[], \
         $7::text[], $8::text[], $9::vector[]) \
         AS u(id, doc_id, page, section_path, char_start, char_end, lang, body, emb)"
    );
    for lot in rows.chunks(INSERT_BATCH) {
        let mut ids = Vec::with_capacity(lot.len());
        let mut doc_ids = Vec::with_capacity(lot.len());
        let mut pages: Vec<Option<i32>> = Vec::with_capacity(lot.len());
        let mut sections: Vec<Option<String>> = Vec::with_capacity(lot.len());
        let mut starts = Vec::with_capacity(lot.len());
        let mut ends = Vec::with_capacity(lot.len());
        let mut langs = Vec::with_capacity(lot.len());
        let mut bodies = Vec::with_capacity(lot.len());
        let mut embs = Vec::with_capacity(lot.len());
        for r in lot {
            ids.push(r.id);
            doc_ids.push(r.doc_id);
            pages.push(r.chunk.page.map(i32::try_from).transpose()?);
            sections.push(r.chunk.section_path.clone());
            starts.push(to_i32(r.chunk.char_start)?);
            ends.push(to_i32(r.chunk.char_end)?);
            langs.push(r.chunk.lang.regconfig().to_string());
            bodies.push(r.chunk.body.clone());
            embs.push(Vector::from(r.emb.clone()));
        }
        sqlx::query(sql)
            .bind(ids)
            .bind(doc_ids)
            .bind(pages)
            .bind(sections)
            .bind(starts)
            .bind(ends)
            .bind(langs)
            .bind(bodies)
            .bind(embs)
            .execute(pool)
            .await
            .context("insert dei chunk (UNNEST)")?;
    }
    Ok(())
}

/// Top-k per distanza coseno fra i soli chunk di documenti visibili a `roles`; a parità di
/// distanza ordine per `id`. Il predicato ACL sta nella stessa query del ranking (D-12):
/// il `LIMIT` si applica dopo il filtro, mai prima.
pub async fn exact_search(
    pool: &PgPool,
    model: Model,
    query_vec: &[f32],
    roles: &[String],
    k: i64,
) -> anyhow::Result<Vec<Hit>> {
    let sql = per_model!(
        model,
        "SELECT c.id, d.doc_key, (c.emb <=> $1)::float8 AS dist FROM ",
        " c JOIN doc d ON d.id = c.doc_id \
         WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2)) \
         ORDER BY c.emb <=> $1, c.id \
         LIMIT $3"
    );
    let rows = sqlx::query(sql)
        .bind(Vector::from(query_vec.to_vec()))
        .bind(roles)
        .bind(k)
        .fetch_all(pool)
        .await
        .context("ricerca esatta")?;
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

fn row_to_chunk(r: &sqlx::postgres::PgRow) -> anyhow::Result<(Uuid, Chunk)> {
    let page: Option<i32> = r.try_get("page")?;
    let start: i32 = r.try_get("char_start")?;
    let end: i32 = r.try_get("char_end")?;
    let lang: String = r.try_get("lang")?;
    Ok((
        r.try_get("id")?,
        Chunk {
            doc_key: r.try_get("doc_key")?,
            page: page.map(u32::try_from).transpose()?,
            section_path: r.try_get("section_path")?,
            char_start: usize::try_from(start)?,
            char_end: usize::try_from(end)?,
            lang: Lang::parse(&lang)?,
            body: r.try_get("body")?,
        },
    ))
}

/// Tutti i chunk del modello con il loro `doc_key`, in ordine di `id`.
pub async fn fetch_chunks(pool: &PgPool, model: Model) -> anyhow::Result<Vec<(Uuid, Chunk)>> {
    let sql = per_model!(
        model,
        "SELECT c.id, d.doc_key, c.page, c.section_path, c.char_start, c.char_end, \
         c.lang::text AS lang, c.body FROM ",
        " c JOIN doc d ON d.id = c.doc_id ORDER BY c.id"
    );
    let rows = sqlx::query(sql)
        .fetch_all(pool)
        .await
        .context("select dei chunk")?;
    rows.iter().map(row_to_chunk).collect()
}

/// Chunk dati gli id (ordine di `id`).
pub async fn fetch_chunks_by_ids(
    pool: &PgPool,
    model: Model,
    ids: &[Uuid],
) -> anyhow::Result<HashMap<Uuid, Chunk>> {
    let sql = per_model!(
        model,
        "SELECT c.id, d.doc_key, c.page, c.section_path, c.char_start, c.char_end, \
         c.lang::text AS lang, c.body FROM ",
        " c JOIN doc d ON d.id = c.doc_id WHERE c.id = ANY($1) ORDER BY c.id"
    );
    let rows = sqlx::query(sql)
        .bind(ids)
        .fetch_all(pool)
        .await
        .context("select dei chunk per id")?;
    rows.iter().map(row_to_chunk).collect()
}

/// Riga JSONL dei candidati: una per query, candidati in ordine di rango.
#[derive(Serialize)]
struct CandidateLine<'a> {
    oracle_id: &'a str,
    roles: &'a [String],
    candidates: Vec<CandidateOut>,
}

#[derive(Serialize)]
struct CandidateOut {
    rank: usize,
    chunk_id: Uuid,
    doc_key: String,
    page: Option<u32>,
    section_path: Option<String>,
    char_start: usize,
    char_end: usize,
    distance: f64,
    body: String,
}

/// Candidati top-k autorizzati per query, serializzati in JSONL in ordine deterministico
/// (query nell'ordine dato, candidati per distanza poi `id`). Contiene il testo dei chunk:
/// va scritto solo sotto `results/raw/` (ignorato da git).
pub async fn export_candidates(
    pool: &PgPool,
    model: Model,
    queries: &[CandidateQuery],
    k: i64,
) -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    for q in queries {
        let hits = exact_search(pool, model, &q.query_vec, &q.roles, k).await?;
        let ids: Vec<Uuid> = hits.iter().map(|h| h.chunk_id).collect();
        let mut details = fetch_chunks_by_ids(pool, model, &ids).await?;
        let mut candidates = Vec::with_capacity(hits.len());
        for (i, h) in hits.into_iter().enumerate() {
            let c = details
                .remove(&h.chunk_id)
                .with_context(|| format!("chunk {} sparito", h.chunk_id))?;
            candidates.push(CandidateOut {
                rank: i + 1,
                chunk_id: h.chunk_id,
                doc_key: h.doc_key,
                page: c.page,
                section_path: c.section_path,
                char_start: c.char_start,
                char_end: c.char_end,
                distance: h.distance,
                body: c.body,
            });
        }
        let line = CandidateLine {
            oracle_id: &q.oracle_id,
            roles: &q.roles,
            candidates,
        };
        serde_json::to_writer(&mut out, &line)?;
        out.push(b'\n');
    }
    Ok(out)
}

#[cfg(test)]
mod pure_tests {
    use super::*;
    use std::collections::HashSet;

    // acl_diff: la verifica di doc_acl contro la matrice attesa trova righe mancanti e in più.
    #[test]
    fn acl_diff_reports_missing_and_extra_rows() {
        let mut expected = Visibility::new();
        expected.insert(
            "employee".into(),
            HashSet::from(["a".to_string(), "b".to_string()]),
        );
        expected.insert("guest".into(), HashSet::new());
        let mut actual: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        actual.insert("employee".into(), BTreeSet::from(["a".to_string()]));
        actual.insert("guest".into(), BTreeSet::from(["c".to_string()]));
        actual.insert("intruso".into(), BTreeSet::from(["a".to_string()]));
        let d = acl_diff(&expected, &actual);
        assert_eq!(
            d,
            vec![
                "mancante: (employee, b)".to_string(),
                "in più: (guest, c)".to_string(),
                "in più: (intruso, a)".to_string(),
            ]
        );

        let same: BTreeMap<String, BTreeSet<String>> = BTreeMap::from([(
            "employee".to_string(),
            BTreeSet::from(["a".to_string(), "b".to_string()]),
        )]);
        assert!(acl_diff(&expected, &same).is_empty());
    }
}

/// Test d'integrazione `#[ignore]`: richiedono `DATABASE_URL` verso il Postgres del compose.
/// Lancio dal runner, dentro una sessione:
/// `cargo test -p s03-pgvector -- --ignored --test-threads=1 db::tests`
/// Ogni test lavora in uno schema temporaneo `test_<pid>_<uuid>` con le tabelle di
/// `sql/schema.sql` e lo elimina alla fine.
#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;
    use sqlx::{AssertSqlSafe, Executor};

    const DIM: usize = 1024;

    struct Scratch {
        pool: PgPool,
        admin: PgPool,
        schema: String,
    }

    impl Scratch {
        async fn new() -> Self {
            let url = std::env::var("DATABASE_URL").expect("DATABASE_URL richiesta dai test db");
            let admin = PgPoolOptions::new()
                .max_connections(1)
                .connect(&url)
                .await
                .expect("connessione");
            // L'estensione resta in public: DROP SCHEMA … CASCADE non deve toccarla.
            sqlx::raw_sql("CREATE EXTENSION IF NOT EXISTS vector SCHEMA public")
                .execute(&admin)
                .await
                .expect("estensione vector");
            // Nome generato dal test (pid + uuid), nessun valore in ingresso: DDL non parametrizzabile.
            let schema = format!("test_{}_{}", std::process::id(), Uuid::new_v4().simple());
            sqlx::raw_sql(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
                .execute(&admin)
                .await
                .expect("schema temporaneo");

            let sp: std::sync::Arc<str> = format!("SET search_path TO {schema}, public").into();
            let pool = PgPoolOptions::new()
                .max_connections(2)
                .after_connect(move |conn, _meta| {
                    let sp = sp.clone();
                    Box::pin(async move {
                        conn.execute(sqlx::raw_sql(AssertSqlSafe(sp))).await?;
                        Ok(())
                    })
                })
                .connect(&url)
                .await
                .expect("pool con search_path");

            let ddl =
                std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/sql/schema.sql"))
                    .expect("sql/schema.sql del piano 01-10");
            sqlx::raw_sql(AssertSqlSafe(ddl))
                .execute(&pool)
                .await
                .expect("schema.sql");
            Scratch {
                pool,
                admin,
                schema,
            }
        }

        async fn drop(self) {
            self.pool.close().await;
            let _ = sqlx::raw_sql(AssertSqlSafe(format!(
                "DROP SCHEMA {} CASCADE",
                self.schema
            )))
            .execute(&self.admin)
            .await;
        }

        /// Documento con le righe di `doc_acl` derivate da acl.json per la sua classificazione.
        async fn doc(&self, doc_key: &str, classification: &str) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO doc (id, doc_key, classification) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(doc_key)
                .bind(classification)
                .execute(&self.pool)
                .await
                .expect("insert doc");
            // acl.json: public → tutti i ruoli; restricted → solo admin.
            let roles: &[&str] = match classification {
                "public" => &["admin", "manager", "employee", "guest"],
                "restricted" => &["admin"],
                other => panic!("classificazione non prevista nei test: {other}"),
            };
            for r in roles {
                sqlx::query("INSERT INTO doc_acl (doc_id, role) VALUES ($1, $2)")
                    .bind(id)
                    .bind(*r)
                    .execute(&self.pool)
                    .await
                    .expect("insert doc_acl");
            }
            id
        }

        async fn chunk_with_id(&self, id: Uuid, doc_id: Uuid, emb: Vec<f32>) {
            sqlx::query(
                "INSERT INTO chunk_qwen3 (id, doc_id, page, section_path, char_start, char_end, lang, body, emb) \
                 VALUES ($1, $2, 1, NULL, 0, 4, 'italian'::regconfig, 'test', $3)",
            )
            .bind(id)
            .bind(doc_id)
            .bind(pgvector::Vector::from(emb))
            .execute(&self.pool)
            .await
            .expect("insert chunk");
        }

        async fn chunk(&self, doc_id: Uuid, emb: Vec<f32>) -> Uuid {
            let id = Uuid::new_v4();
            self.chunk_with_id(id, doc_id, emb).await;
            id
        }
    }

    /// Vettore unitario lungo l'asse 0 (il vettore di query).
    fn query_vec() -> Vec<f32> {
        let mut v = vec![0.0; DIM];
        v[0] = 1.0;
        v
    }

    /// Vettore lontano dalla query, distanza coseno crescente con `i`.
    fn far(i: usize) -> Vec<f32> {
        let mut v = vec![0.0; DIM];
        v[1] = 1.0;
        v[0] = -(i as f32) * 0.01;
        v
    }

    fn roles(r: &[&str]) -> Vec<String> {
        r.iter().map(|s| s.to_string()).collect()
    }

    // TEST-6 (RISK-3): tabella per modello da enum chiuso.
    #[test]
    fn model_table_comes_from_closed_enum() {
        assert_eq!(table(Model::Qwen3), "chunk_qwen3");
        assert_eq!(table(Model::BgeM3), "chunk_bgem3");
        assert_eq!(table(Model::E5), "chunk_e5");
    }

    // TEST-1 (RISK-1): il filtro ACL sta nella query, non dopo il LIMIT.
    #[tokio::test]
    #[ignore = "richiede DATABASE_URL (Postgres del compose)"]
    async fn exact_search_never_returns_invisible_doc_even_if_nearest() {
        let s = Scratch::new().await;
        let d_pub = s.doc("D_pub", "public").await;
        let d_res = s.doc("D_res", "restricted").await;
        for _ in 0..5 {
            s.chunk(d_res, query_vec()).await; // distanza 0
        }
        for i in 0..5 {
            s.chunk(d_pub, far(i)).await;
        }

        let emp = exact_search(
            &s.pool,
            Model::Qwen3,
            &query_vec(),
            &roles(&["employee"]),
            3,
        )
        .await
        .expect("ricerca employee");
        assert_eq!(
            emp.len(),
            3,
            "un post-filtro dopo LIMIT 3 restituirebbe 0 righe"
        );
        assert!(emp.iter().all(|h| h.doc_key == "D_pub"), "leak: {emp:?}");

        let adm = exact_search(&s.pool, Model::Qwen3, &query_vec(), &roles(&["admin"]), 3)
            .await
            .expect("ricerca admin");
        assert_eq!(adm.len(), 3);
        assert!(
            adm.iter().all(|h| h.doc_key == "D_res"),
            "admin deve vedere prima i più vicini: {adm:?}"
        );
        s.drop().await;
    }

    // TEST-2 (RISK-1): k risultati visibili anche quando gli invisibili dominano.
    #[tokio::test]
    #[ignore = "richiede DATABASE_URL (Postgres del compose)"]
    async fn exact_search_returns_k_visible_when_invisible_dominate() {
        let s = Scratch::new().await;
        let d_pub = s.doc("D_pub", "public").await;
        let d_res = s.doc("D_res", "restricted").await;
        for _ in 0..100 {
            s.chunk(d_res, query_vec()).await;
        }
        for i in 0..10 {
            s.chunk(d_pub, far(i)).await;
        }
        let hits = exact_search(
            &s.pool,
            Model::Qwen3,
            &query_vec(),
            &roles(&["employee"]),
            10,
        )
        .await
        .expect("ricerca");
        assert_eq!(hits.len(), 10);
        assert!(hits.iter().all(|h| h.doc_key == "D_pub"), "leak: {hits:?}");
        assert!(
            hits.windows(2).all(|w| w[0].distance <= w[1].distance),
            "ordine per distanza"
        );
        s.drop().await;
    }

    // TEST-5 (RISK-3): il ruolo passa come parametro, una stringa ostile non altera la query.
    #[tokio::test]
    #[ignore = "richiede DATABASE_URL (Postgres del compose)"]
    async fn exact_search_binds_hostile_role_string() {
        let s = Scratch::new().await;
        let d_pub = s.doc("D_pub", "public").await;
        let d_res = s.doc("D_res", "restricted").await;
        s.chunk(d_pub, far(0)).await;
        s.chunk(d_res, query_vec()).await;
        let hostile = roles(&["employee' OR '1'='1", "admin') OR TRUE --"]);
        let hits = exact_search(&s.pool, Model::Qwen3, &query_vec(), &hostile, 10)
            .await
            .expect("la query non deve fallire per sintassi");
        assert!(hits.is_empty(), "nessun ruolo con quel nome: {hits:?}");
        s.drop().await;
    }

    // TEST-13 (RISK-9): ordine deterministico, a parità di distanza per id.
    #[tokio::test]
    #[ignore = "richiede DATABASE_URL (Postgres del compose)"]
    async fn exact_search_ties_are_ordered_by_id() {
        let s = Scratch::new().await;
        let d_pub = s.doc("D_pub", "public").await;
        let mut ids = Vec::new();
        for _ in 0..30 {
            ids.push(s.chunk(d_pub, query_vec()).await); // tutti a distanza 0
        }
        ids.sort();
        let hits = exact_search(
            &s.pool,
            Model::Qwen3,
            &query_vec(),
            &roles(&["employee"]),
            20,
        )
        .await
        .expect("ricerca");
        let got: Vec<Uuid> = hits.iter().map(|h| h.chunk_id).collect();
        assert_eq!(
            got,
            ids[..20].to_vec(),
            "a parità di distanza: i 20 id più piccoli, in ordine crescente"
        );
        s.drop().await;
    }

    // TEST-13 (RISK-9): due esportazioni sullo stesso DB sono identiche byte per byte (stesso sha256).
    #[tokio::test]
    #[ignore = "richiede DATABASE_URL (Postgres del compose)"]
    async fn export_candidates_is_deterministic() {
        let s = Scratch::new().await;
        let d_pub = s.doc("D_pub", "public").await;
        let d_res = s.doc("D_res", "restricted").await;
        for i in 0..25 {
            s.chunk(d_pub, if i % 2 == 0 { query_vec() } else { far(i % 5) })
                .await;
        }
        for _ in 0..10 {
            s.chunk(d_res, query_vec()).await;
        }
        let qs = vec![
            CandidateQuery {
                oracle_id: "Q-001".into(),
                roles: roles(&["employee"]),
                query_vec: query_vec(),
            },
            CandidateQuery {
                oracle_id: "Q-002".into(),
                roles: roles(&["admin"]),
                query_vec: far(1),
            },
        ];
        let a = export_candidates(&s.pool, Model::Qwen3, &qs, 20)
            .await
            .expect("prima esportazione");
        let b = export_candidates(&s.pool, Model::Qwen3, &qs, 20)
            .await
            .expect("seconda esportazione");
        assert!(!a.is_empty());
        assert_eq!(a, b, "export-candidates non deterministico");
        s.drop().await;
    }
}
