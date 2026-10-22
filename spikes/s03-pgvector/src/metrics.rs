//! Metriche di misura (piano 01-10): regola di rilevanza delle Soglie, recall/MRR/nDCG con
//! guadagno binario, matrice di visibilità attesa da `acl.json` + manifest (mai da `doc_acl`)
//! e conteggio dei leak contro quella matrice.
//!
//! STUB test-write 01-10: le funzioni sono `todo!()`; i tipi sono il contratto minimo.
use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use crate::chunk::Chunk;

/// Ruolo o profilo (`acl.json`) → insieme dei `doc_key` (`doc_id` del manifest) visibili.
pub type Visibility = HashMap<String, HashSet<String>>;

/// Passaggio atteso di un oracolo (`expected_passages` di `queries.json`).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ExpectedPassage {
    pub document_id: String,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub section_path: Option<String>,
    pub excerpt: String,
}

/// Voce di `queries.json` usata dalle misure.
#[derive(Clone, Debug, Deserialize)]
pub struct OracleQuery {
    pub oracle_id: String,
    pub text: String,
    pub lang: String,
    pub role_profile: String,
    pub expected_passages: Vec<ExpectedPassage>,
    pub no_answer: bool,
}

/// Oracoli ammessi alle misure e numero di esclusi (da riportare in RISULTATI §Anomalie).
#[derive(Clone, Debug)]
pub struct RatifiedQueries {
    pub queries: Vec<OracleQuery>,
    pub excluded: usize,
}

/// Carica `queries.json` tenendo solo le voci con i tre assi verdi e senza `todo_ref`.
pub fn load_ratified_queries(_json: &str) -> anyhow::Result<RatifiedQueries> {
    todo!("load_ratified_queries: implementazione del piano 01-10")
}

/// Matrice attesa, calcolata in memoria da `acl.json` e `manifest.json` (deny by default).
pub fn expected_visibility(_acl_json: &str, _manifest_json: &str) -> anyhow::Result<Visibility> {
    todo!("expected_visibility: implementazione del piano 01-10")
}

/// Chunk nei top-k il cui `doc_key` non è visibile a `role_profile` secondo la matrice attesa.
pub fn leaked_chunks(
    _hit_doc_keys: &[&str],
    _role_profile: &str,
    _visibility: &Visibility,
) -> usize {
    todo!("leaked_chunks: implementazione del piano 01-10")
}

/// Per ogni chunk, in ordine: è rilevante per almeno uno dei passaggi attesi?
pub fn relevant_chunks(_chunks: &[Chunk], _passages: &[ExpectedPassage]) -> Vec<bool> {
    todo!("relevant_chunks: implementazione del piano 01-10")
}

pub fn recall_at_k(_relevance: &[bool], _total_relevant: usize, _k: usize) -> f64 {
    todo!("recall_at_k: implementazione del piano 01-10")
}

pub fn mrr_at_k(_relevance: &[bool], _k: usize) -> f64 {
    todo!("mrr_at_k: implementazione del piano 01-10")
}

pub fn ndcg_at_k(_relevance: &[bool], _total_relevant: usize, _k: usize) -> f64 {
    todo!("ndcg_at_k: implementazione del piano 01-10")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::Lang;

    const EPS: f64 = 1e-4;

    // ---------- TEST-7 (RISK-4): valori calcolati a mano ----------

    #[test]
    fn metrics_known_values() {
        // 10 risultati, rilevanti in posizione 1 e 3, 2 rilevanti totali.
        let mut rel = [false; 10];
        rel[0] = true;
        rel[2] = true;

        assert!((recall_at_k(&rel, 2, 5) - 1.0).abs() < EPS, "recall@5");
        assert!((recall_at_k(&rel, 2, 1) - 0.5).abs() < EPS, "recall@1");
        assert!((recall_at_k(&rel, 2, 10) - 1.0).abs() < EPS, "recall@10");
        assert!((mrr_at_k(&rel, 10) - 1.0).abs() < EPS, "MRR@10");
        // nDCG@10 = (1 + 1/log2(4)) / (1 + 1/log2(3)) ≈ 0,9197
        let expected = (1.0 + 1.0 / 4f64.log2()) / (1.0 + 1.0 / 3f64.log2());
        assert!((expected - 0.9197).abs() < EPS);
        assert!((ndcg_at_k(&rel, 2, 10) - expected).abs() < EPS, "nDCG@10");

        // Primo rilevante in posizione 4: MRR = 0,25; fuori da k = 3 → 0.
        let mut late = [false; 10];
        late[3] = true;
        assert!((mrr_at_k(&late, 10) - 0.25).abs() < EPS);
        assert!(mrr_at_k(&late, 3).abs() < EPS);
        assert!(recall_at_k(&late, 1, 3).abs() < EPS);
    }

    #[test]
    fn metrics_no_relevant_in_results_are_zero() {
        let rel = [false; 10];
        assert_eq!(recall_at_k(&rel, 2, 10), 0.0);
        assert_eq!(mrr_at_k(&rel, 10), 0.0);
        assert_eq!(ndcg_at_k(&rel, 2, 10), 0.0);
    }

    #[test]
    fn metrics_no_answer_query_never_nan() {
        // Query no_answer: nessun rilevante atteso. Valore documentato, mai NaN né infinito.
        let rel = [false; 10];
        for v in [
            recall_at_k(&rel, 0, 10),
            mrr_at_k(&rel, 10),
            ndcg_at_k(&rel, 0, 10),
        ] {
            assert!(
                v.is_finite(),
                "metrica non finita per query senza rilevanti: {v}"
            );
        }
        let empty: [bool; 0] = [];
        for v in [
            recall_at_k(&empty, 0, 10),
            mrr_at_k(&empty, 10),
            ndcg_at_k(&empty, 0, 10),
        ] {
            assert!(v.is_finite());
        }
    }

    // ---------- TEST-8 (RISK-4): regola di rilevanza delle Soglie ----------

    /// Excerpt di esattamente 100 caratteri, senza ripetizioni interne lunghe.
    fn excerpt100() -> String {
        let s: String = "Il PIL globale è aumentato nel 2025 del 3,4 per cento, sostenuto dai consumi delle famiglie e dagli investimenti pubblici in Europa."
            .chars()
            .take(100)
            .collect();
        assert_eq!(s.chars().count(), 100);
        s
    }

    fn pdf_chunk(doc: &str, page: u32, body: &str) -> Chunk {
        Chunk {
            doc_key: doc.to_string(),
            page: Some(page),
            section_path: None,
            char_start: 0,
            char_end: body.chars().count(),
            lang: Lang::Italian,
            body: body.to_string(),
        }
    }

    fn passage(doc: &str, page: u32, excerpt: &str) -> ExpectedPassage {
        ExpectedPassage {
            document_id: doc.to_string(),
            page: Some(page),
            section_path: None,
            excerpt: excerpt.to_string(),
        }
    }

    fn take(s: &str, a: usize, b: usize) -> String {
        s.chars().skip(a).take(b - a).collect()
    }

    #[test]
    fn relevance_rule_exact_partial_and_wrong_page() {
        let ex = excerpt100();
        let p = [passage("pub-it-01", 11, &ex)];

        // (a) excerpt intero, stesso documento e pagina.
        let a = pdf_chunk("pub-it-01", 11, &format!("Testo prima. {ex} Testo dopo."));
        assert_eq!(relevant_chunks(&[a], &p), vec![true], "(a) excerpt intero");

        // (b) 50% contiguo (prefisso o suffisso a cavallo di due chunk) → rilevante.
        let b_pre = pdf_chunk(
            "pub-it-01",
            11,
            &format!("Testo prima. {}", take(&ex, 0, 50)),
        );
        let b_suf = pdf_chunk(
            "pub-it-01",
            11,
            &format!("{} Testo dopo.", take(&ex, 50, 100)),
        );
        assert_eq!(
            relevant_chunks(&[b_pre, b_suf], &p),
            vec![true, true],
            "(b) 50%"
        );

        // (b) 49% contiguo → non rilevante.
        let b49 = pdf_chunk(
            "pub-it-01",
            11,
            &format!("Testo prima. {}", take(&ex, 0, 49)),
        );
        assert_eq!(relevant_chunks(&[b49], &p), vec![false], "(b) 49%");

        // (c) stesso testo, pagina diversa.
        let c = pdf_chunk("pub-it-01", 12, &ex);
        assert_eq!(relevant_chunks(&[c], &p), vec![false], "(c) pagina diversa");

        // (d) stesso testo, altro documento.
        let d = pdf_chunk("pub-it-02", 11, &ex);
        assert_eq!(
            relevant_chunks(&[d], &p),
            vec![false],
            "(d) altro documento"
        );
    }

    #[test]
    fn relevance_rule_normalizes_nfc_and_whitespace() {
        // (e) excerpt con spazi multipli, a capo e «è» in forma NFD; chunk in NFC a spazio singolo.
        let excerpt_nfd = "Il  PIL\n globale e\u{300}   aumentato nel 2025 del 3,4 per cento.";
        let body_nfc =
            "Premessa. Il PIL globale \u{e8} aumentato nel 2025 del 3,4 per cento. Seguito.";
        let p = [passage("pub-it-01", 11, excerpt_nfd)];
        let c = pdf_chunk("pub-it-01", 11, body_nfc);
        assert_eq!(
            relevant_chunks(&[c], &p),
            vec![true],
            "(e) NFC + collasso spazi"
        );

        // Simmetrico: chunk NFD con a capo, excerpt NFC.
        let p2 = [passage("pub-it-01", 11, "Il PIL globale \u{e8} aumentato")];
        let c2 = pdf_chunk(
            "pub-it-01",
            11,
            "Il PIL\n\nglobale e\u{300} aumentato nel 2025",
        );
        assert_eq!(relevant_chunks(&[c2], &p2), vec![true]);
    }

    #[test]
    fn relevance_rule_markdown_section() {
        let ex = "| Ricavi consolidati | 182,4 M€ |";
        let sec = "2. Dati principali > 2.1 Quali sono i risultati del 2025?";
        let p = [ExpectedPassage {
            document_id: "syn-06-faq-public".into(),
            page: None,
            section_path: Some(sec.into()),
            excerpt: ex.into(),
        }];
        let mk = |section: &str| Chunk {
            doc_key: "syn-06-faq-public".into(),
            page: None,
            section_path: Some(section.into()),
            char_start: 0,
            char_end: ex.chars().count(),
            lang: Lang::Italian,
            body: ex.into(),
        };
        assert_eq!(
            relevant_chunks(&[mk(sec)], &p),
            vec![true],
            "stessa sezione"
        );
        assert_eq!(
            relevant_chunks(
                &[mk(
                    "2. Dati principali > 2.2 La società distribuisce dividendi?"
                )],
                &p
            ),
            vec![false],
            "sezione diversa"
        );
    }

    // ---------- TEST-3 (RISK-2): leak contato contro acl.json + manifest, senza DB ----------

    const ACL: &str = include_str!("../../data/corpus/acl.json");
    const MANIFEST: &str = include_str!("../../data/corpus/manifest.json");

    #[test]
    fn leak_counter_uses_acl_json_not_doc_acl() {
        let vis = expected_visibility(ACL, MANIFEST).expect("matrice attesa");
        // syn-03 è restricted: invisibile a employee. pub-it-01 e syn-01 (internal) visibili.
        let hits = [
            "pub-it-01-bdi-relazione-annuale-2025",
            "syn-03-verbale-cda-restricted",
            "syn-01-budget-interno",
        ];
        assert_eq!(leaked_chunks(&hits, "employee", &vis), 1);
        assert_eq!(leaked_chunks(&hits, "admin", &vis), 0);
        assert_eq!(leaked_chunks(&hits, "guest", &vis), 2);
        // Profilo restricted-only: vede solo i restricted.
        assert_eq!(leaked_chunks(&hits, "restricted-only", &vis), 2);
        // Deny by default: ruolo sconosciuto → ogni chunk è un leak.
        assert_eq!(leaked_chunks(&hits, "ruolo-inesistente", &vis), 3);
        // Documento sconosciuto al manifest → leak per chiunque, anche admin.
        assert_eq!(leaked_chunks(&["doc-fantasma"], "admin", &vis), 1);
    }

    #[test]
    fn expected_visibility_matches_acl_json_and_manifest() {
        let vis = expected_visibility(ACL, MANIFEST).expect("matrice attesa");
        let set = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<HashSet<_>>();

        // Valori attesi letti da acl.json (_note, profiles) e dalle classificazioni del manifest.
        assert_eq!(
            vis["restricted-only"],
            set(&[
                "syn-03-verbale-cda-restricted",
                "syn-09-trappola-restricted",
                "syn-09-trappola-restricted-docx"
            ])
        );
        assert_eq!(
            vis["admin"].len(),
            23,
            "admin vede tutti i 23 documenti del manifest"
        );
        assert_eq!(vis["guest"].len(), 12, "10 PDF pubblici + syn-06 + syn-07");
        assert!(vis["guest"].contains("syn-07-injection-public"));
        assert!(!vis["guest"].contains("syn-01-budget-interno"));
        // employee = public + internal: 12 + syn-01, syn-04, syn-08, syn-01-docx.
        assert_eq!(vis["employee"].len(), 16);
        assert!(!vis["employee"].contains("syn-02-piano-industriale-riservato"));
        // manager = employee + confidential (syn-02, syn-05 e i loro DOCX).
        assert_eq!(vis["manager"].len(), 20);
        assert!(!vis["manager"].contains("syn-09-trappola-restricted-docx"));
    }

    #[test]
    fn expected_visibility_deny_by_default_on_unknown_classification() {
        let acl = r#"{"roles":{"employee":["public"]},"profiles":{"solo-x":{"visible":["x"]}}}"#;
        let manifest = r#"[
            {"doc_id":"a","classification":"public"},
            {"doc_id":"b","classification":"segretissimo"}
        ]"#;
        let vis = expected_visibility(acl, manifest).expect("matrice");
        assert_eq!(vis["employee"], HashSet::from(["a".to_string()]));
        assert!(vis["solo-x"].is_empty());
    }

    // ---------- TEST-17 (RISK-12): solo oracoli ratificati ----------

    fn oracle(
        id: &str,
        claude: Option<&str>,
        gpt: &str,
        user: Option<&str>,
        todo: Option<&str>,
    ) -> serde_json::Value {
        serde_json::json!({
            "oracle_id": id,
            "guardrail": {"claude": claude, "gpt": gpt, "user": user},
            "todo_ref": todo,
            "text": "domanda",
            "lang": "it",
            "role_profile": "employee",
            "expected_passages": [{"document_id": "syn-06-faq-public", "section_path": "S", "excerpt": "E"}],
            "no_answer": false
        })
    }

    #[test]
    fn only_ratified_oracles_enter_the_bench() {
        let arr = serde_json::Value::Array(vec![
            oracle("Q-A", Some("concordo"), "CONFIRMED", Some("ratified"), None),
            oracle(
                "Q-B",
                Some("concordo"),
                "PARTIALLY_CONFIRMED",
                None,
                Some("TODO-02"),
            ),
            oracle("Q-C", Some("concordo"), "CONFIRMED", Some("ratified"), None),
        ]);
        let r = load_ratified_queries(&arr.to_string()).expect("caricamento");
        let ids: Vec<_> = r.queries.iter().map(|q| q.oracle_id.as_str()).collect();
        assert_eq!(ids, ["Q-A", "Q-C"]);
        assert_eq!(r.excluded, 1);
    }

    #[test]
    fn each_red_axis_excludes_the_oracle() {
        let arr = serde_json::Value::Array(vec![
            oracle("claude-null", None, "CONFIRMED", Some("ratified"), None),
            oracle(
                "claude-vuoto",
                Some(""),
                "CONFIRMED",
                Some("ratified"),
                None,
            ),
            oracle(
                "gpt-parziale",
                Some("ok"),
                "PARTIALLY_CONFIRMED",
                Some("ratified"),
                None,
            ),
            oracle("user-null", Some("ok"), "CONFIRMED", None, None),
            // ORACOLI.md: un oracolo con todo_ref valorizzato resta fuori.
            oracle(
                "todo",
                Some("ok"),
                "CONFIRMED",
                Some("ratified"),
                Some("TODO-02"),
            ),
            oracle("verde", Some("ok"), "CONFIRMED", Some("ratified"), None),
        ]);
        let r = load_ratified_queries(&arr.to_string()).expect("caricamento");
        let ids: Vec<_> = r.queries.iter().map(|q| q.oracle_id.as_str()).collect();
        assert_eq!(ids, ["verde"]);
        assert_eq!(r.excluded, 5);
    }

    #[test]
    fn real_queries_json_keeps_74_of_80() {
        // ORACOLI.md: 74 query su 80 con tre assi verdi; esclusi Q-008, Q-010, Q-012, Q-040, Q-048, Q-056.
        let json = include_str!("../../data/corpus/oracles/queries.json");
        let r = load_ratified_queries(json).expect("caricamento queries.json");
        assert_eq!(r.queries.len(), 74);
        assert_eq!(r.excluded, 6);
        for id in ["Q-008", "Q-010", "Q-012", "Q-040", "Q-048", "Q-056"] {
            assert!(
                r.queries.iter().all(|q| q.oracle_id != id),
                "{id} pendente nel bench"
            );
        }
    }
}
