//! Accesso a PostgreSQL + pgvector (piano 01-10): ricerca esatta con il predicato ACL nella
//! stessa query del ranking (D-12), parametri solo con `bind`, tabella da enum chiuso.
//!
//! STUB test-write 01-10: `table`, `exact_search` ed `export_candidates` sono `todo!()`.
use sqlx::PgPool;
use uuid::Uuid;

use crate::Model;

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
pub fn table(_model: Model) -> &'static str {
    todo!("table: implementazione del piano 01-10")
}

/// Top-k per distanza coseno fra i soli chunk di documenti visibili a `roles`; a parità di
/// distanza ordine per `id`.
pub async fn exact_search(
    _pool: &PgPool,
    _model: Model,
    _query_vec: &[f32],
    _roles: &[String],
    _k: i64,
) -> anyhow::Result<Vec<Hit>> {
    todo!("exact_search: implementazione del piano 01-10")
}

/// Candidati top-k autorizzati per query, serializzati in JSONL in ordine deterministico.
pub async fn export_candidates(
    _pool: &PgPool,
    _model: Model,
    _queries: &[CandidateQuery],
    _k: i64,
) -> anyhow::Result<Vec<u8>> {
    todo!("export_candidates: implementazione del piano 01-10")
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
