//! `synth`: corpus sintetico a 10k/50k/100k/300k chunk (piano 01-14). Perturbazione gaussiana
//! dei vettori reali con seme fisso e rinormalizzazione; documenti sintetici con la stessa
//! distribuzione di classificazioni del corpus reale; ruoli sintetici `sel100`, `sel25`,
//! `sel5`, `sel1` in `doc_acl` per controllare la selettività. La regola di assegnazione è
//! ricalcolabile dal seme ([`expected_visibility`]): è la matrice attesa contro cui `leak`
//! conta i negati, indipendente da `doc_acl`.
//!
//! STUB test-write 01-14: firme minime per i test rossi; il corpo lo scrive l'esecutore.
//!
//! **Modifica:** Claude (test-write 01-14) · 2026-10-25 · scheletro e test rossi del design
use s03_pgvector::metrics::Visibility;
use sqlx::PgPool;

/// Seme fisso della generazione sintetica.
pub const SEED: u64 = 0x0114;

/// Ruoli sintetici e quota di chunk visibili attesa.
pub const SYNTH_ROLES: [(&str, f64); 4] = [
    ("sel100", 1.0),
    ("sel25", 0.25),
    ("sel5", 0.05),
    ("sel1", 0.01),
];

/// Scala del corpus sintetico: enum chiuso, da cui deriva il nome della tabella.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scale {
    N10k,
    N50k,
    N100k,
    N300k,
}

impl Scale {
    pub const ALL: [Scale; 4] = [Scale::N10k, Scale::N50k, Scale::N100k, Scale::N300k];

    /// Numero di chunk della scala.
    pub fn n(self) -> usize {
        todo!("Scale::n")
    }

    /// Tabella `chunk_synth_<n>`: `match` su enum chiuso, mai dall'input.
    pub fn table(self) -> &'static str {
        todo!("Scale::table")
    }
}

/// `n` vettori sintetici dai vettori reali `base`: perturbazione gaussiana con seme `seed`,
/// poi norma L2 = 1. Stesso seme, stesso risultato bit a bit.
pub fn perturb(base: &[Vec<f32>], n: usize, seed: u64) -> Vec<Vec<f32>> {
    let _ = (base, n, seed);
    todo!("perturb")
}

/// Classificazioni di `n_docs` documenti sintetici con le stesse proporzioni di `real`.
pub fn synth_classifications(real: &[&str], n_docs: usize, seed: u64) -> Vec<String> {
    let _ = (real, n_docs, seed);
    todo!("synth_classifications")
}

/// Matrice attesa sui documenti sintetici della scala, per tutti i ruoli e profili di
/// `acl.json` e per i ruoli sintetici, ricalcolata dal seme e dai documenti reali
/// `(doc_key, classification)`. Non legge `doc_acl`.
pub fn expected_visibility(
    scale: Scale,
    seed: u64,
    acl_json: &str,
    base_docs: &[(String, String)],
) -> anyhow::Result<Visibility> {
    let _ = (scale, seed, acl_json, base_docs);
    todo!("synth::expected_visibility")
}

/// Genera la scala nel database: documenti sintetici in `doc`, righe di `doc_acl` per i ruoli
/// di `acl.json` e per i ruoli sintetici, chunk in `chunk_synth_<n>` dai vettori di
/// `chunk_bgem3` (inserimento a lotti).
pub async fn generate(
    pool: &PgPool,
    scale: Scale,
    seed: u64,
    acl_json: &str,
) -> anyhow::Result<()> {
    let _ = (pool, scale, seed, acl_json);
    todo!("synth::generate")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::{self, Scratch, axis, near, union};
    use s03_pgvector::db;

    // TEST-9 (RISK-4): tabelle sintetiche da enum chiuso.
    #[test]
    fn test9_scale_tables_come_from_closed_enum() {
        assert_eq!(Scale::N10k.table(), "chunk_synth_10000");
        assert_eq!(Scale::N50k.table(), "chunk_synth_50000");
        assert_eq!(Scale::N100k.table(), "chunk_synth_100000");
        assert_eq!(Scale::N300k.table(), "chunk_synth_300000");
        let ns: Vec<usize> = Scale::ALL.iter().map(|s| s.n()).collect();
        assert_eq!(ns, [10_000, 50_000, 100_000, 300_000]);
    }

    fn base_vectors() -> Vec<Vec<f32>> {
        (0..5).map(|i| near(i * 37)).collect()
    }

    // TEST-16 (RISK-10): stesso seme → stessi vettori bit a bit, norma 1, proporzioni.
    #[test]
    fn test16_synth_is_deterministic_and_normalized() {
        let base = base_vectors();
        let a = perturb(&base, 50, SEED);
        let b = perturb(&base, 50, SEED);
        assert_eq!(a.len(), 50);
        let bits = |v: &Vec<Vec<f32>>| -> Vec<Vec<u32>> {
            v.iter()
                .map(|x| x.iter().map(|f| f.to_bits()).collect())
                .collect()
        };
        assert_eq!(bits(&a), bits(&b), "stesso seme, vettori diversi");
        for (i, v) in a.iter().enumerate() {
            assert_eq!(v.len(), testdb::DIM);
            let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() <= 1e-5, "vettore {i}: norma {norm}");
        }
        let c = perturb(&base, 50, SEED + 1);
        assert_ne!(
            bits(&a),
            bits(&c),
            "il seme deve influire sulla perturbazione"
        );
        assert!(
            a.iter().any(|v| !base.contains(v)),
            "i vettori sintetici devono essere perturbati, non copie dei reali"
        );

        let mut real = Vec::new();
        for (c, k) in [
            ("public", 4),
            ("internal", 3),
            ("confidential", 2),
            ("restricted", 1),
        ] {
            real.extend(std::iter::repeat_n(c, k));
        }
        let cls = synth_classifications(&real, 100, SEED);
        assert_eq!(
            cls,
            synth_classifications(&real, 100, SEED),
            "non deterministico"
        );
        assert_eq!(cls.len(), 100);
        for (c, want) in [
            ("public", 40),
            ("internal", 30),
            ("confidential", 20),
            ("restricted", 10),
        ] {
            let got = cls.iter().filter(|x| *x == c).count() as i64;
            assert!(
                (got - want).abs() <= 1,
                "{c}: {got} documenti, attesi {want} ± 1"
            );
        }
    }

    /// Documenti reali di base nello schema di test: uno per classificazione, 5 chunk ciascuno.
    async fn seed_base(s: &Scratch) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (i, c) in ["public", "internal", "confidential", "restricted"]
            .iter()
            .enumerate()
        {
            let key = format!("base-{c}");
            let d = s.doc(&key, c).await;
            for j in 0..5 {
                s.chunk(d, "testo di base", axis(2 + i * 5 + j)).await;
            }
            out.push((key, c.to_string()));
        }
        out
    }

    // TEST-11 (RISK-6): quota di chunk visibili ai ruoli sintetici entro ±0,5 punti dal target.
    #[tokio::test]
    #[ignore = "richiede DATABASE_URL (Postgres del compose) e sql/index.sql"]
    async fn test11_synth_role_selectivity_matches_target() {
        let s = Scratch::new().await;
        s.apply_index_sql().await;
        seed_base(&s).await;
        generate(&s.pool, Scale::N10k, SEED, &testdb::acl_json())
            .await
            .expect("synth --n 10000");
        let total: i64 = sqlx::query_scalar("SELECT count(*) FROM chunk_synth_10000")
            .fetch_one(&s.pool)
            .await
            .expect("count");
        assert_eq!(total, 10_000);
        for (role, target) in SYNTH_ROLES {
            let share: f64 = sqlx::query_scalar(
                "SELECT (count(*) FILTER (WHERE EXISTS (SELECT 1 FROM doc_acl a \
                 WHERE a.doc_id = c.doc_id AND a.role = $1)))::float8 / count(*) \
                 FROM chunk_synth_10000 c",
            )
            .bind(role)
            .fetch_one(&s.pool)
            .await
            .expect("selettività");
            assert!(
                (share - target).abs() <= 0.005,
                "{role}: selettività {share:.4}, attesa {target} ± 0,005"
            );
        }
        s.drop().await;
    }

    // TEST-7 (RISK-3, RISK-6): dopo synth, doc_acl coincide esattamente con la matrice attesa
    // (acl.json + documenti di base per i ruoli reali, regola del seme per quelli sintetici).
    // I ruoli sintetici valgono solo sui documenti sintetici.
    #[tokio::test]
    #[ignore = "richiede DATABASE_URL (Postgres del compose) e sql/index.sql"]
    async fn test7_doc_acl_matches_expected_after_synth() {
        let s = Scratch::new().await;
        s.apply_index_sql().await;
        let base = seed_base(&s).await;
        let acl = testdb::acl_json();
        generate(&s.pool, Scale::N10k, SEED, &acl)
            .await
            .expect("synth");
        let base_ref: Vec<(&str, &str)> =
            base.iter().map(|(k, c)| (k.as_str(), c.as_str())).collect();
        let expected = union(
            testdb::visibility_for(&base_ref),
            expected_visibility(Scale::N10k, SEED, &acl, &base).expect("matrice sintetica"),
        );
        let actual = db::read_doc_acl(&s.pool).await.expect("doc_acl");
        let diff = db::acl_diff(&expected, &actual);
        assert!(
            diff.is_empty(),
            "doc_acl diversa dalla matrice attesa ({} differenze): {:?}",
            diff.len(),
            &diff[..diff.len().min(20)]
        );
        s.drop().await;
    }
}
