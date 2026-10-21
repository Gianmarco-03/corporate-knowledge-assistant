//! Metriche di qualità del ranking con rilevanza binaria dagli oracoli.

use std::collections::HashSet;

/// nDCG@k con guadagno binario; 0 se non ci sono rilevanti (mai NaN).
pub fn ndcg_at_k(ranked: &[&str], relevant: &[&str], k: usize) -> f64 {
    let rel: HashSet<&str> = relevant.iter().copied().collect();
    if rel.is_empty() || k == 0 {
        return 0.0;
    }
    // Un id ripetuto nel ranking guadagna una sola volta.
    let mut seen: HashSet<&str> = HashSet::new();
    let dcg: f64 = ranked
        .iter()
        .take(k)
        .enumerate()
        .filter(|(_, id)| rel.contains(**id) && seen.insert(**id))
        .map(|(i, _)| discount(i))
        .sum();
    let ideal: f64 = (0..rel.len().min(k)).map(discount).sum();
    dcg / ideal
}

/// Sconto della posizione `i` (0-based): 1 / log2(i + 2).
fn discount(i: usize) -> f64 {
    1.0 / ((i + 2) as f64).log2()
}

/// Frazione dei rilevanti presenti nelle prime `k` posizioni; 0 se non ci sono rilevanti.
pub fn recall_at_k(ranked: &[&str], relevant: &[&str], k: usize) -> f64 {
    let rel: HashSet<&str> = relevant.iter().copied().collect();
    if rel.is_empty() {
        return 0.0;
    }
    let hit: HashSet<&str> = ranked
        .iter()
        .take(k)
        .copied()
        .filter(|id| rel.contains(id))
        .collect();
    hit.len() as f64 / rel.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    const RANKED: [&str; 8] = ["a", "b", "c", "d", "e", "f", "g", "h"];
    const RELEVANT: [&str; 2] = ["b", "e"];

    // TEST-13 (RISK-8)
    #[test]
    fn ndcg_recall_known_values() {
        assert!((recall_at_k(&RANKED, &RELEVANT, 8) - 1.0).abs() < 1e-12);
        assert!((recall_at_k(&RANKED, &RELEVANT, 5) - 1.0).abs() < 1e-12);
        assert!((recall_at_k(&RANKED, &RELEVANT, 2) - 0.5).abs() < 1e-12);
        assert!(recall_at_k(&RANKED, &RELEVANT, 1).abs() < 1e-12);

        let expected = (1.0 / 3f64.log2() + 1.0 / 6f64.log2()) / (1.0 + 1.0 / 3f64.log2());
        assert!((expected - 0.6241).abs() < 1e-4, "valore atteso del design");
        let got = ndcg_at_k(&RANKED, &RELEVANT, 5);
        assert!(
            (got - expected).abs() < 1e-4,
            "nDCG@5 = {got}, atteso {expected}"
        );

        // Ranking ideale = 1.
        let ideal = ["b", "e", "a", "c"];
        assert!((ndcg_at_k(&ideal, &RELEVANT, 5) - 1.0).abs() < 1e-12);
    }

    // TEST-13 (RISK-8): nessun rilevante → 0, mai NaN; k oltre la lunghezza.
    #[test]
    fn metrics_no_relevant_is_zero_not_nan() {
        let n = ndcg_at_k(&RANKED, &[], 5);
        let r = recall_at_k(&RANKED, &[], 8);
        assert!(!n.is_nan() && n == 0.0, "nDCG senza rilevanti = {n}");
        assert!(!r.is_nan() && r == 0.0, "recall senza rilevanti = {r}");
        assert!((recall_at_k(&["b"], &RELEVANT, 8) - 0.5).abs() < 1e-12);
        assert!(ndcg_at_k(&[], &RELEVANT, 5) == 0.0);
    }
}
