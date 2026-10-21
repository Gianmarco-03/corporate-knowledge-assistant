//! Insiemi di candidati autorizzati per lo spike S-04 e guardia di visibilità.
//!
//! I passaggi vengono solo dagli oracoli ratificati (tre assi verdi) e ogni
//! insieme contiene soltanto testo visibile al `role_profile` della query (D-16).

use std::path::Path;

/// Passaggio candidato inviato a JEV.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passage {
    pub id: String,
    pub doc_key: String,
    pub classification: String,
    pub text: String,
}

/// Query di cui si ordinano i candidati: il ruolo decide cosa può uscire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub id: String,
    pub text: String,
    pub role_profile: String,
}

/// Insieme di candidati di una query, con gli id dei passaggi rilevanti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateSet {
    pub query: Query,
    pub lang: String,
    pub passages: Vec<Passage>,
    pub relevant_ids: Vec<String>,
}

/// Oracolo di visibilità costruito da `acl.json` e `manifest.json`.
#[derive(Debug, Clone)]
pub struct Acl {
    // stub del test-write: da implementare in 01-11
    _stub: (),
}

impl Acl {
    /// Costruisce l'oracolo dal testo di `acl.json` e di `manifest.json`.
    pub fn from_json(acl_json: &str, manifest_json: &str) -> anyhow::Result<Acl> {
        // stub del test-write: da implementare in 01-11
        let _ = (acl_json, manifest_json);
        todo!()
    }

    /// Legge `acl.json` e `manifest.json` da `corpus_dir`.
    pub fn load(corpus_dir: &Path) -> anyhow::Result<Acl> {
        // stub del test-write: da implementare in 01-11
        let _ = corpus_dir;
        todo!()
    }

    /// Vero solo se il documento è nel manifest e la sua classificazione è
    /// visibile al ruolo o profilo (deny by default).
    pub fn is_visible(&self, doc_key: &str, role_profile: &str) -> bool {
        // stub del test-write: da implementare in 01-11
        let _ = (doc_key, role_profile);
        todo!()
    }
}

/// Errore se anche un solo passaggio non è visibile a `role_profile`.
pub fn assert_visible(passages: &[Passage], role_profile: &str, acl: &Acl) -> anyhow::Result<()> {
    // stub del test-write: da implementare in 01-11
    let _ = (passages, role_profile, acl);
    todo!()
}

/// Costruisce `n_queries` insiemi di `set_size` passaggi dagli oracoli di
/// `corpus_dir`, con selezione deterministica dal `seed`.
pub fn build_candidate_sets(
    corpus_dir: &Path,
    n_queries: usize,
    set_size: usize,
    seed: u64,
) -> anyhow::Result<Vec<CandidateSet>> {
    // stub del test-write: da implementare in 01-11
    let _ = (corpus_dir, n_queries, set_size, seed);
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

    const ACL_JSON: &str = r#"{
        "classifications": ["public", "internal", "confidential", "restricted"],
        "roles": {
            "admin": ["public", "internal", "confidential", "restricted"],
            "manager": ["public", "internal", "confidential"],
            "employee": ["public", "internal"],
            "guest": ["public"]
        },
        "profiles": {
            "restricted-only": {"visible": ["restricted"]}
        }
    }"#;

    const MANIFEST_JSON: &str = r#"[
        {"doc_id": "doc-pub", "classification": "public"},
        {"doc_id": "doc-int", "classification": "internal"},
        {"doc_id": "doc-res", "classification": "restricted"}
    ]"#;

    fn acl() -> Acl {
        Acl::from_json(ACL_JSON, MANIFEST_JSON).expect("acl e manifest di test validi")
    }

    fn passage(id: &str, doc_key: &str, classification: &str) -> Passage {
        Passage {
            id: id.to_owned(),
            doc_key: doc_key.to_owned(),
            classification: classification.to_owned(),
            text: format!("testo di {id}"),
        }
    }

    // TEST-1 (RISK-1)
    #[test]
    fn assert_visible_rejects_invisible_passage() {
        let acl = acl();
        let visible = vec![
            passage("p-0", "doc-pub", "public"),
            passage("p-1", "doc-int", "internal"),
        ];
        assert!(assert_visible(&visible, "employee", &acl).is_ok());

        let mut with_restricted = visible.clone();
        with_restricted.push(passage("p-2", "doc-res", "restricted"));
        let err = assert_visible(&with_restricted, "employee", &acl)
            .expect_err("un passaggio restricted non è visibile a employee");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("doc-res"),
            "l'errore deve nominare il documento: {msg}"
        );
    }

    // TEST-1 (RISK-1): documento sconosciuto al manifest → mai visibile.
    #[test]
    fn assert_visible_rejects_unknown_document() {
        let acl = acl();
        let unknown = vec![passage("p-0", "doc-ignoto", "public")];
        assert!(assert_visible(&unknown, "admin", &acl).is_err());
        assert!(!acl.is_visible("doc-ignoto", "admin"));
    }

    // TEST-1 (RISK-1): la verità è il manifest, non la classificazione dichiarata dal passaggio.
    #[test]
    fn assert_visible_uses_manifest_classification() {
        let acl = acl();
        let lying = vec![passage("p-0", "doc-res", "public")];
        assert!(assert_visible(&lying, "guest", &acl).is_err());
    }

    // TEST-1 (RISK-1): ruoli, profili e ruolo sconosciuto (deny by default).
    #[test]
    fn is_visible_follows_roles_and_profiles() {
        let acl = acl();
        assert!(acl.is_visible("doc-pub", "guest"));
        assert!(!acl.is_visible("doc-int", "guest"));
        assert!(acl.is_visible("doc-int", "employee"));
        assert!(!acl.is_visible("doc-res", "manager"));
        assert!(acl.is_visible("doc-res", "admin"));
        assert!(acl.is_visible("doc-res", "restricted-only"));
        assert!(!acl.is_visible("doc-pub", "restricted-only"));
        assert!(!acl.is_visible("doc-pub", "ruolo-inesistente"));
        let any = vec![passage("p-0", "doc-pub", "public")];
        assert!(assert_visible(&any, "ruolo-inesistente", &acl).is_err());
    }

    fn corpus_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/corpus")
    }

    /// Oracoli con i tre assi verdi, letti in modo indipendente dalla libreria.
    fn green_oracles() -> Vec<serde_json::Value> {
        let text = std::fs::read_to_string(corpus_dir().join("oracles/queries.json"))
            .expect("queries.json leggibile");
        let all: Vec<serde_json::Value> = serde_json::from_str(&text).expect("queries.json valido");
        all.into_iter()
            .filter(|q| {
                let g = &q["guardrail"];
                g["claude"].as_str().is_some_and(|s| !s.trim().is_empty())
                    && g["gpt"] == "CONFIRMED"
                    && g["user"] == "ratified"
                    && q["todo_ref"].is_null()
            })
            .collect()
    }

    const SEED: u64 = 0x0111_5EED;

    // TEST-3 (RISK-1, RISK-7)
    #[test]
    fn candidate_sets_only_visible_to_role() {
        let dir = corpus_dir();
        let acl = Acl::load(&dir).expect("acl e manifest del corpus");
        let green = green_oracles();
        let green_by_id: std::collections::HashMap<&str, &serde_json::Value> = green
            .iter()
            .map(|q| (q["oracle_id"].as_str().expect("oracle_id"), q))
            .collect();
        let green_excerpts: HashSet<(String, String)> = green
            .iter()
            .flat_map(|q| {
                q["expected_passages"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
            })
            .map(|p| {
                (
                    p["document_id"].as_str().expect("document_id").to_owned(),
                    p["excerpt"].as_str().expect("excerpt").to_owned(),
                )
            })
            .collect();

        let sets = build_candidate_sets(&dir, 30, 20, SEED).expect("insiemi costruiti");
        assert_eq!(sets.len(), 30, "30 query con risposta");

        let ids: HashSet<&str> = sets.iter().map(|s| s.query.id.as_str()).collect();
        assert_eq!(ids.len(), 30, "query distinte");

        let langs: HashSet<&str> = sets.iter().map(|s| s.lang.as_str()).collect();
        assert!(langs.contains("it"), "mix: manca IT");
        assert!(langs.contains("en"), "mix: manca EN");
        assert!(
            langs.iter().any(|l| l.starts_with("cross")),
            "mix: manca cross"
        );

        for set in &sets {
            let oracle = green_by_id
                .get(set.query.id.as_str())
                .unwrap_or_else(|| panic!("{} non è un oracolo verde", set.query.id));
            assert_eq!(
                oracle["no_answer"], false,
                "{}: query senza risposta",
                set.query.id
            );
            assert_eq!(
                oracle["role_profile"].as_str(),
                Some(set.query.role_profile.as_str()),
                "{}: role_profile diverso dall'oracolo",
                set.query.id
            );
            assert_eq!(
                set.passages.len(),
                20,
                "{}: insieme non da 20",
                set.query.id
            );

            let pids: HashSet<&str> = set.passages.iter().map(|p| p.id.as_str()).collect();
            assert_eq!(
                pids.len(),
                20,
                "{}: id di passaggio duplicati",
                set.query.id
            );

            for p in &set.passages {
                assert!(
                    acl.is_visible(&p.doc_key, &set.query.role_profile),
                    "{}: passaggio {} di {} non visibile a {}",
                    set.query.id,
                    p.id,
                    p.doc_key,
                    set.query.role_profile
                );
                assert!(
                    green_excerpts.contains(&(p.doc_key.clone(), p.text.clone())),
                    "{}: il passaggio {} non è un excerpt di un oracolo verde",
                    set.query.id,
                    p.id
                );
            }
            assert_visible(&set.passages, &set.query.role_profile, &acl)
                .expect("guardia verde sull'insieme costruito");

            // Gli expected_passages della query sono nell'insieme e sono i rilevanti.
            for exp in oracle["expected_passages"]
                .as_array()
                .expect("expected_passages")
            {
                let key = (
                    exp["document_id"].as_str().expect("document_id"),
                    exp["excerpt"].as_str().expect("excerpt"),
                );
                let hit = set
                    .passages
                    .iter()
                    .find(|p| p.doc_key == key.0 && p.text == key.1)
                    .unwrap_or_else(|| panic!("{}: expected_passage assente", set.query.id));
                assert!(
                    set.relevant_ids.contains(&hit.id),
                    "{}: expected_passage non marcato rilevante",
                    set.query.id
                );
            }
            for rid in &set.relevant_ids {
                assert!(
                    pids.contains(rid.as_str()),
                    "{}: rilevante fuori insieme",
                    set.query.id
                );
            }
        }

        let again = build_candidate_sets(&dir, 30, 20, SEED).expect("seconda costruzione");
        assert_eq!(sets, again, "stesso seme, stessi insiemi");
    }
}
