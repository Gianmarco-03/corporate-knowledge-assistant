//! Query di ricerca del piano 01-14: parte semantica (esatta), lessicale (`tsvector` con
//! `websearch_to_tsquery` sulla configurazione della lingua), ibrida con fusione RRF (k = 60)
//! e densa su HNSW con `hnsw.ef_search` / `hnsw.iterative_scan` (RESEARCH §5). Il predicato
//! ACL è la CTE `allowed`, usata dentro ENTRAMBE le sottoquery `sem` e `lex`: nessun
//! post-filtro dopo la fusione (D-12, D-15). SQL solo con `bind`; tabelle da enum chiusi.
//!
//! STUB test-write 01-14: firme minime per i test rossi; il corpo lo scrive l'esecutore.
//!
//! **Modifica:** Claude (test-write 01-14) · 2026-10-25 · scheletro e test rossi del design
use s03_pgvector::chunk::Lang;
use s03_pgvector::db::Hit;
use sqlx::PgPool;
use uuid::Uuid;

use crate::synth::Scale;

/// Costante della Reciprocal Rank Fusion.
pub const RRF_K: f64 = 60.0;

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
        todo!("Table::name")
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
        todo!("IterativeScan::setting")
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

/// Parte semantica: top-k esatto per distanza coseno fra i chunk visibili a `roles`.
pub async fn semantic_search(
    pool: &PgPool,
    table: Table,
    query_vec: &[f32],
    roles: &[String],
    k: i64,
) -> anyhow::Result<Vec<Hit>> {
    let _ = (pool, table, query_vec, roles, k);
    todo!("semantic_search")
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
    let _ = (pool, table, query_text, lang, roles, k);
    todo!("lexical_search")
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
    let _ = (pool, table, query_vec, query_text, lang, roles, k);
    todo!("hybrid_search")
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
    let _ = (pool, table, query_vec, roles, k, params);
    todo!("hnsw_search")
}

/// Fusione RRF in Rust (verifica della query SQL): punteggio `Σ 1 / (k + rango)` con rango da
/// 1; ordine per punteggio decrescente, a parità per id crescente.
pub fn rrf_fuse(sem: &[Uuid], lex: &[Uuid], k: f64) -> Vec<(Uuid, f64)> {
    let _ = (sem, lex, k);
    todo!("rrf_fuse")
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
