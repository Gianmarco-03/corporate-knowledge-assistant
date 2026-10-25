//! `leak`: per ogni ruolo di `acl.json`, per `restricted-only` e per i ruoli sintetici conta i
//! chunk nei top-k la cui `doc_id` non è visibile al ruolo, separatamente per la parte
//! semantica e lessicale, a ogni scala e configurazione (D-12, D-15, D-16). Il conteggio usa la
//! matrice attesa (`metrics::expected_visibility` per i ruoli reali, `synth::expected_visibility`
//! per i sintetici), mai `doc_acl`: `doc_acl` si confronta con quella matrice prima di misurare.
//!
//! **Modifica:** Claude (test-write 01-14) · 2026-10-25 · scheletro e test rossi del design
//! **Modifica:** Claude (gsd-executor 01-14) · 2026-10-25 · contatore, matrice attesa completa, griglia esatta + HNSW e verifica d'uso dell'indice
use std::path::Path;

use anyhow::{Context, bail};
use s03_pgvector::db;
use s03_pgvector::metrics::{self, Visibility};
use sqlx::{Connection, PgConnection, PgPool};

use crate::hybrid::{self, Dense, HnswParams, IterativeScan, PART_DEPTH, Strategy, Table};
use crate::synth::{self, SEED, Scale};
use crate::{BenchQuery, csv_upsert, read};

/// Chunk nei top-k (per `doc_key`) non visibili a `role` secondo la matrice attesa. Funzione
/// pura: non interroga il database. Ruolo o documento assenti dalla matrice contano come negati.
pub fn denied_in_topk(hit_doc_keys: &[&str], role: &str, expected: &Visibility) -> usize {
    metrics::leaked_chunks(hit_doc_keys, role, expected)
}

/// Quota dei chunk di `table` visibili al profilo `restricted-only` (atteso ≤ 5% sul corpus
/// reale, verifica rinviata dal piano 01-03).
pub async fn restricted_only_selectivity(pool: &PgPool, table: Table) -> anyhow::Result<f64> {
    hybrid::role_share(pool, table, "restricted-only").await
}

/// Ruoli del leak test: ruoli e profilo di `acl.json`, poi i ruoli sintetici.
pub const REAL_ROLES: [&str; 5] = ["admin", "manager", "employee", "guest", "restricted-only"];

/// Unione di due matrici (ruolo → documenti).
pub fn merge(mut a: Visibility, b: Visibility) -> Visibility {
    for (role, docs) in b {
        a.entry(role).or_default().extend(docs);
    }
    a
}

/// Matrice attesa di tutto il database: corpus reale da `acl.json` + manifest e, per ogni
/// scala sintetica presente, la regola di `synth` ricalcolata dal seme. Non legge `doc_acl`.
pub async fn full_expected(pool: &PgPool, corpus: &Path) -> anyhow::Result<Visibility> {
    let acl = read(&corpus.join("acl.json"))?;
    let mut vis = metrics::expected_visibility(&acl, &read(&corpus.join("manifest.json"))?)?;
    let base = synth::base_docs(pool).await?;
    for scale in Scale::ALL {
        let like = format!("{}%", scale.key_prefix());
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM doc WHERE doc_key LIKE $1")
            .bind(&like)
            .fetch_one(pool)
            .await
            .context("documenti sintetici")?;
        if n > 0 {
            vis = merge(vis, synth::expected_visibility(scale, SEED, &acl, &base)?);
        }
    }
    Ok(vis)
}

/// Un indice esiste nello schema corrente.
pub async fn index_exists(pool: &PgPool, name: &str) -> anyhow::Result<bool> {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_indexes WHERE schemaname = current_schema() AND indexname = $1",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .context("pg_indexes")?;
    Ok(n > 0)
}

/// Scansioni dell'indice secondo le statistiche cumulative, dopo aver forzato il flush delle
/// statistiche in sospeso di questa sessione.
pub async fn index_scans(conn: &mut PgConnection, name: &str) -> anyhow::Result<i64> {
    sqlx::query("SELECT pg_stat_force_next_flush()")
        .execute(&mut *conn)
        .await
        .context("pg_stat_force_next_flush")?;
    sqlx::query("SELECT pg_stat_clear_snapshot()")
        .execute(&mut *conn)
        .await
        .context("pg_stat_clear_snapshot")?;
    let n: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT idx_scan FROM pg_stat_user_indexes \
         WHERE schemaname = current_schema() AND indexrelname = $1), 0)",
    )
    .bind(name)
    .fetch_one(&mut *conn)
    .await
    .context("pg_stat_user_indexes")?;
    Ok(n)
}

/// `doc_key` dei risultati, nell'ordine.
fn doc_keys(v: &[hybrid::Ranked]) -> Vec<&str> {
    v.iter().map(|h| h.doc_key.as_str()).collect()
}

/// Configurazione di ricerca del leak test.
#[derive(Clone, Copy, Debug)]
pub enum Config {
    Exact,
    Hnsw(Dense, HnswParams),
}

impl Config {
    pub fn label(self) -> String {
        match self {
            Config::Exact => "exact".to_string(),
            Config::Hnsw(d, p) => format!(
                "hnsw_{}_ef{}_{}",
                d.name(),
                p.ef_search,
                p.iterative_scan.setting()
            ),
        }
    }

    fn strategy(self) -> Strategy {
        match self {
            Config::Exact => Strategy::Exact,
            Config::Hnsw(d, _) => Strategy::Ann(d),
        }
    }
}

/// Le 18 combinazioni HNSW della griglia.
pub fn hnsw_grid() -> Vec<(Dense, HnswParams)> {
    let mut out = Vec::new();
    for d in Dense::ALL {
        for ef_search in [40, 100, 200] {
            for iterative_scan in IterativeScan::ALL {
                out.push((
                    d,
                    HnswParams {
                        ef_search,
                        iterative_scan,
                    },
                ));
            }
        }
    }
    out
}

/// Comando `leak` su una scala sintetica (`Some`) o sul corpus reale (`None`).
pub async fn run(
    url: &str,
    corpus: &Path,
    out: &Path,
    scale: Option<Scale>,
    queries: &[BenchQuery],
) -> anyhow::Result<()> {
    let pool = db::connect(url).await?;
    let expected = full_expected(&pool, corpus).await?;
    db::check_doc_acl(&pool, &expected)
        .await
        .context("I-8: doc_acl diversa dalla matrice attesa, misura non eseguita")?;
    println!("leak: doc_acl coincide con la matrice attesa");

    let (table, roles, configs): (Table, Vec<&str>, Vec<Config>) = match scale {
        None => (Table::BgeM3, REAL_ROLES.to_vec(), vec![Config::Exact]),
        Some(s) => {
            let mut roles = REAL_ROLES.to_vec();
            roles.extend(synth::SYNTH_ROLES.iter().map(|r| r.0));
            let mut configs = vec![Config::Exact];
            for d in Dense::ALL {
                let name = crate::hnsw_index_name(s, d);
                if !index_exists(&pool, name).await? {
                    bail!("indice {name} assente: lanciare prima `hnsw --n {}`", s.n());
                }
            }
            configs.extend(hnsw_grid().into_iter().map(|(d, p)| Config::Hnsw(d, p)));
            (Table::Synth(s), roles, configs)
        }
    };
    let scale_label = match scale {
        Some(s) => s.n().to_string(),
        None => crate::count_rows(&pool, table).await?.to_string(),
    };
    // Indici HNSW della scala, con il tipo di vettore che servono.
    let indexes: Vec<(Dense, &'static str)> = match scale {
        Some(s) => Dense::ALL
            .iter()
            .map(|d| (*d, crate::hnsw_index_name(s, *d)))
            .collect(),
        None => Vec::new(),
    };

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut total_denied = 0usize;
    let mut fusion_mismatch = 0usize;
    for cfg in configs {
        let mut conn = PgConnection::connect(url).await.context("connessione")?;
        match cfg {
            Config::Exact => hybrid::exact_mode(&mut conn, false).await?,
            Config::Hnsw(_, p) => hybrid::set_hnsw(&mut conn, p, false).await?,
        }
        let mut before = Vec::new();
        for (_, n) in &indexes {
            before.push(index_scans(&mut conn, n).await?);
        }
        let mut n_queries = 0i64;
        for role in &roles {
            let r = vec![role.to_string()];
            let (mut sem, mut lex, mut hyb) = (0usize, 0usize, 0usize);
            for q in queries {
                let b = hybrid::hybrid_on(
                    &mut conn,
                    table,
                    cfg.strategy(),
                    &q.vec,
                    &q.oracle.text,
                    q.lang,
                    &r,
                    PART_DEPTH,
                )
                .await
                .with_context(|| format!("{} {role}", cfg.label()))?;
                n_queries += 1;
                let top = b.fused.len().min(10);
                sem += denied_in_topk(&doc_keys(&b.sem), role, &expected);
                lex += denied_in_topk(&doc_keys(&b.lex), role, &expected);
                hyb += denied_in_topk(&doc_keys(&b.fused[..top]), role, &expected);
                if !hybrid::fusion_matches(&b) {
                    fusion_mismatch += 1;
                }
            }
            for (part, d) in [("sem", sem), ("lex", lex), ("hybrid", hyb)] {
                total_denied += d;
                rows.push(vec![
                    scale_label.clone(),
                    cfg.label(),
                    role.to_string(),
                    part.to_string(),
                    queries.len().to_string(),
                    d.to_string(),
                ]);
            }
        }
        // Verifica d'uso degli indici: la configurazione HNSW deve usare il proprio indice a
        // ogni query, quella esatta nessuno.
        for (i, (idx_dense, n)) in indexes.iter().enumerate() {
            let delta = index_scans(&mut conn, n).await? - before[i];
            let own = matches!(cfg, Config::Hnsw(d, _) if d == *idx_dense);
            if own && delta < n_queries {
                bail!(
                    "{}: indice {n} usato {delta} volte su {n_queries} query: configurazione non misurata",
                    cfg.label()
                );
            }
            if !own && delta != 0 {
                bail!("{}: indice {n} usato {delta} volte, atteso 0", cfg.label());
            }
        }
        conn.close().await.ok();
        println!("leak {} {}: ok", scale_label, cfg.label());
    }
    csv_upsert(
        &out.join("leak.csv"),
        &[
            "chunks",
            "config",
            "role",
            "part",
            "queries",
            "denied_in_topk",
        ],
        4,
        &rows,
    )?;
    let share = restricted_only_selectivity(&pool, table).await?;
    println!(
        "leak {scale_label}: {} righe, negati totali {total_denied}, fusioni SQL diverse da rrf_fuse {fusion_mismatch}, selettività restricted-only {:.4}",
        rows.len(),
        share
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hybrid::{HnswParams, IterativeScan, hnsw_search};
    use crate::synth::{SEED, Scale, expected_visibility as synth_visibility};
    use crate::testdb::{self, Scratch, far, near, query_vec, roles};
    use std::collections::HashSet;

    // TEST-6 (RISK-3): il contatore usa la matrice attesa, non doc_acl.
    #[test]
    fn test6_leak_counter_independent_of_doc_acl() {
        let vis = testdb::visibility_for(&[("D_pub", "public"), ("D_res", "restricted")]);
        assert_eq!(denied_in_topk(&["D_pub", "D_res"], "employee", &vis), 1);
        assert_eq!(denied_in_topk(&["D_pub", "D_pub"], "employee", &vis), 0);
        assert_eq!(denied_in_topk(&["D_res"], "admin", &vis), 0);
        assert_eq!(denied_in_topk(&["D_pub"], "restricted-only", &vis), 1);
        // Deny by default: ruolo sconosciuto o documento fuori matrice contano come negati.
        assert_eq!(denied_in_topk(&["D_pub"], "intruso", &vis), 1);
        assert_eq!(denied_in_topk(&["D_ignoto"], "admin", &vis), 1);

        // Ruoli sintetici: matrice ricalcolata dal seme, senza database.
        let base = vec![
            ("base-public".to_string(), "public".to_string()),
            ("base-restricted".to_string(), "restricted".to_string()),
        ];
        let sv = synth_visibility(Scale::N10k, SEED, &testdb::acl_json(), &base)
            .expect("matrice sintetica");
        let all100: &HashSet<String> = sv.get("sel100").expect("sel100 nella matrice");
        let sel1: &HashSet<String> = sv.get("sel1").expect("sel1 nella matrice");
        let denied_to_sel1: Vec<&str> = all100
            .iter()
            .filter(|k| !sel1.contains(*k))
            .map(String::as_str)
            .take(3)
            .collect();
        assert_eq!(
            denied_to_sel1.len(),
            3,
            "sel1 vede meno documenti di sel100"
        );
        assert_eq!(denied_in_topk(&denied_to_sel1, "sel1", &sv), 3);
        assert_eq!(denied_in_topk(&denied_to_sel1, "sel100", &sv), 0);
    }

    // TEST-4 (RISK-2): HNSW con filtro non restituisce mai un negato, a ogni ef_search e
    // iterative_scan. Con iterative_scan = off meno di k risultati è perdita di recall, non leak.
    #[tokio::test]
    #[ignore = "richiede DATABASE_URL (Postgres del compose con pgvector ≥ 0.8)"]
    async fn test4_hnsw_filtered_search_never_returns_denied() {
        let s = Scratch::new().await;
        let d_res = s.doc("D_res", "restricted").await;
        let d_pub = s.doc("D_pub", "public").await;
        s.chunks_bulk(d_res, "riservato", (0..1900).map(near).collect())
            .await;
        s.chunks_bulk(d_pub, "pubblico", (0..100).map(far).collect())
            .await;
        sqlx::raw_sql(
            "CREATE INDEX chunk_bgem3_hnsw_test ON chunk_bgem3 \
             USING hnsw (emb vector_cosine_ops) WITH (m = 16, ef_construction = 64)",
        )
        .execute(&s.pool)
        .await
        .expect("indice HNSW");
        let vis = testdb::visibility_for(&[("D_pub", "public"), ("D_res", "restricted")]);
        let mut counts = Vec::new();
        for ef_search in [40, 100, 200] {
            for iterative_scan in IterativeScan::ALL {
                let p = HnswParams {
                    ef_search,
                    iterative_scan,
                };
                let hits = hnsw_search(
                    &s.pool,
                    Table::BgeM3,
                    &query_vec(),
                    &roles(&["employee"]),
                    10,
                    p,
                )
                .await
                .unwrap_or_else(|e| panic!("{p:?}: {e}"));
                let keys: Vec<&str> = hits.iter().map(|h| h.doc_key.as_str()).collect();
                assert_eq!(
                    denied_in_topk(&keys, "employee", &vis),
                    0,
                    "{p:?}: leak {hits:?}"
                );
                assert!(hits.len() <= 10);
                counts.push((ef_search, iterative_scan, hits.len()));
            }
        }
        eprintln!("risultati per combinazione (perdita di recall attesa con off): {counts:?}");
        s.drop().await;
    }

    // TEST-12 (RISK-6): selettività di restricted-only sul corpus reale ≤ 5%, dichiarata.
    #[tokio::test]
    #[ignore = "richiede DATABASE_URL verso il volume pgdata con chunk_bgem3 caricato (piano 01-10)"]
    async fn test12_restricted_only_selectivity_on_real_corpus() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
        let pool = s03_pgvector::db::connect(&url).await.expect("connessione");
        let share = restricted_only_selectivity(&pool, Table::BgeM3)
            .await
            .expect("selettività");
        assert!(share > 0.0 && share.is_finite(), "selettività {share}");
        let ris = testdb::read(&testdb::manifest_dir().join("RISULTATI-indice.md"));
        assert!(
            ris.contains("restricted-only"),
            "RISULTATI-indice deve riportarla"
        );
        if share > 0.05 {
            assert!(
                ris.contains("sel5"),
                "restricted-only al {:.2}% > 5%: RISULTATI deve dichiararlo e usare sel5",
                share * 100.0
            );
        }
    }
}
