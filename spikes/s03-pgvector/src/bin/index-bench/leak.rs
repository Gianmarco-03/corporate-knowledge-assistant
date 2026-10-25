//! `leak`: per ogni ruolo di `acl.json`, per `restricted-only` e per i ruoli sintetici conta i
//! chunk nei top-k la cui `doc_id` non è visibile al ruolo, separatamente per la parte
//! semantica e lessicale, a ogni scala e configurazione (D-12, D-15, D-16). Il conteggio usa la
//! matrice attesa (`metrics::expected_visibility` per i ruoli reali, `synth::expected_visibility`
//! per i sintetici), mai `doc_acl`: `doc_acl` si confronta con quella matrice prima di misurare.
//!
//! STUB test-write 01-14: firme minime per i test rossi; il corpo lo scrive l'esecutore.
//!
//! **Modifica:** Claude (test-write 01-14) · 2026-10-25 · scheletro e test rossi del design
use s03_pgvector::metrics::Visibility;
use sqlx::PgPool;

use crate::hybrid::Table;

/// Chunk nei top-k (per `doc_key`) non visibili a `role` secondo la matrice attesa. Funzione
/// pura: non interroga il database. Ruolo o documento assenti dalla matrice contano come negati.
pub fn denied_in_topk(hit_doc_keys: &[&str], role: &str, expected: &Visibility) -> usize {
    let _ = (hit_doc_keys, role, expected);
    todo!("denied_in_topk")
}

/// Quota dei chunk di `table` visibili al profilo `restricted-only` (atteso ≤ 5% sul corpus
/// reale, verifica rinviata dal piano 01-03).
pub async fn restricted_only_selectivity(pool: &PgPool, table: Table) -> anyhow::Result<f64> {
    let _ = (pool, table);
    todo!("restricted_only_selectivity")
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
