//! Insiemi di candidati autorizzati per lo spike S-04 e guardia di visibilità.
//!
//! I passaggi vengono solo dagli oracoli ratificati (tre assi verdi) e ogni
//! insieme contiene soltanto testo visibile al `role_profile` della query (D-16).

use anyhow::{Context, bail};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
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
///
/// Un documento è visibile a un ruolo o profilo se e solo se è nel manifest e
/// la sua classificazione sta nell'elenco del ruolo (o nel `visible` del
/// profilo). Tutto il resto è negato: documento ignoto, ruolo ignoto,
/// classificazione assente dall'elenco.
#[derive(Debug, Clone)]
pub struct Acl {
    /// `doc_id` → classificazione dal manifest (unica fonte di verità).
    classification_of: HashMap<String, String>,
    /// ruolo o profilo → classificazioni visibili.
    visible_to: HashMap<String, HashSet<String>>,
}

impl Acl {
    /// Costruisce l'oracolo dal testo di `acl.json` e di `manifest.json`.
    pub fn from_json(acl_json: &str, manifest_json: &str) -> anyhow::Result<Acl> {
        let acl: serde_json::Value =
            serde_json::from_str(acl_json).context("acl.json non valido")?;
        let manifest: serde_json::Value =
            serde_json::from_str(manifest_json).context("manifest.json non valido")?;

        let mut visible_to: HashMap<String, HashSet<String>> = HashMap::new();
        let roles = acl
            .get("roles")
            .and_then(serde_json::Value::as_object)
            .context("acl.json: manca l'oggetto roles")?;
        for (role, list) in roles {
            let set = string_set(list).with_context(|| format!("acl.json: ruolo {role}"))?;
            visible_to.insert(role.clone(), set);
        }
        if let Some(profiles) = acl.get("profiles").and_then(serde_json::Value::as_object) {
            for (profile, body) in profiles {
                let list = body
                    .get("visible")
                    .with_context(|| format!("acl.json: profilo {profile} senza visible"))?;
                let set =
                    string_set(list).with_context(|| format!("acl.json: profilo {profile}"))?;
                if visible_to.insert(profile.clone(), set).is_some() {
                    bail!("acl.json: {profile} è sia ruolo sia profilo");
                }
            }
        }

        let docs = manifest
            .as_array()
            .context("manifest.json: atteso un array di documenti")?;
        let mut classification_of = HashMap::new();
        for d in docs {
            let id = d
                .get("doc_id")
                .and_then(serde_json::Value::as_str)
                .context("manifest.json: voce senza doc_id")?;
            let class = d
                .get("classification")
                .and_then(serde_json::Value::as_str)
                .with_context(|| format!("manifest.json: {id} senza classification"))?;
            if classification_of
                .insert(id.to_owned(), class.to_owned())
                .is_some()
            {
                bail!("manifest.json: doc_id {id} duplicato");
            }
        }
        Ok(Acl {
            classification_of,
            visible_to,
        })
    }

    /// Legge `acl.json` e `manifest.json` da `corpus_dir`.
    pub fn load(corpus_dir: &Path) -> anyhow::Result<Acl> {
        let acl = read(&corpus_dir.join("acl.json"))?;
        let manifest = read(&corpus_dir.join("manifest.json"))?;
        Acl::from_json(&acl, &manifest)
    }

    /// Vero solo se il documento è nel manifest e la sua classificazione è
    /// visibile al ruolo o profilo (deny by default).
    pub fn is_visible(&self, doc_key: &str, role_profile: &str) -> bool {
        match (
            self.classification_of.get(doc_key),
            self.visible_to.get(role_profile),
        ) {
            (Some(class), Some(visible)) => visible.contains(class),
            _ => false,
        }
    }

    /// Classificazione del documento secondo il manifest.
    pub fn classification(&self, doc_key: &str) -> Option<&str> {
        self.classification_of.get(doc_key).map(String::as_str)
    }
}

fn string_set(v: &serde_json::Value) -> anyhow::Result<HashSet<String>> {
    v.as_array()
        .context("atteso un array di classificazioni")?
        .iter()
        .map(|c| {
            c.as_str()
                .map(str::to_owned)
                .context("classificazione non stringa")
        })
        .collect()
}

fn read(path: &Path) -> anyhow::Result<String> {
    std::fs::read_to_string(path)
        .with_context(|| format!("lettura di {} non riuscita", path.display()))
}

/// Errore se anche un solo passaggio non è visibile a `role_profile`.
///
/// La classificazione dichiarata dal passaggio non conta: decide il manifest.
/// L'errore nomina passaggio e documento, mai il testo.
pub fn assert_visible(passages: &[Passage], role_profile: &str, acl: &Acl) -> anyhow::Result<()> {
    let denied: Vec<String> = passages
        .iter()
        .filter(|p| !acl.is_visible(&p.doc_key, role_profile))
        .map(|p| format!("{} ({})", p.id, p.doc_key))
        .collect();
    if denied.is_empty() {
        Ok(())
    } else {
        bail!(
            "{} passaggi non visibili al ruolo {role_profile}: {}",
            denied.len(),
            denied.join(", ")
        )
    }
}

/// Generatore deterministico (splitmix64): la selezione dipende solo dal seme.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Mescolamento di Fisher-Yates.
    fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
    }
}

/// Oracolo con i tre assi verdi: Claude concorda, GPT `CONFIRMED`, utente
/// `ratified`, nessun `todo_ref` aperto.
pub fn is_green(q: &serde_json::Value) -> bool {
    let g = &q["guardrail"];
    g["claude"].as_str().is_some_and(|s| !s.trim().is_empty())
        && g["gpt"] == "CONFIRMED"
        && g["user"] == "ratified"
        && q["todo_ref"].is_null()
}

/// Id stabile di un excerpt: documento + 8 byte di sha256 del testo.
fn passage_id(doc_key: &str, text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    format!("{doc_key}#{}", hex::encode(&digest[..8]))
}

/// Gruppo di lingua per il mix: `it`, `en` o `cross`.
fn lang_group(lang: &str) -> &'static str {
    if lang.starts_with("cross") {
        "cross"
    } else if lang == "en" {
        "en"
    } else {
        "it"
    }
}

/// Excerpt ratificato del corpus, candidato a entrare in un insieme.
struct PoolEntry {
    passage: Passage,
    injection: bool,
}

type Pool = BTreeMap<(String, String), PoolEntry>;

/// Excerpt `injection_case` garantiti in ogni insieme, dove visibili: bastano
/// a misurare `instructs_model` senza fare dell'insieme un insieme ostile.
pub const MAX_INJECTION_DISTRACTORS: usize = 2;

/// Id dei passaggi che vengono da oracoli `injection_case` (per `instructs_model`).
pub fn injection_passage_ids(corpus_dir: &Path) -> anyhow::Result<HashSet<String>> {
    let (_, pool) = load_green(corpus_dir, &Acl::load(corpus_dir)?)?;
    Ok(pool
        .values()
        .filter(|e| e.injection)
        .map(|e| e.passage.id.clone())
        .collect())
}

/// Oracoli verdi e pool degli excerpt (chiave `(doc, excerpt)`, ordinata).
fn load_green(corpus_dir: &Path, acl: &Acl) -> anyhow::Result<(Vec<serde_json::Value>, Pool)> {
    let text = read(&corpus_dir.join("oracles/queries.json"))?;
    let all: Vec<serde_json::Value> =
        serde_json::from_str(&text).context("queries.json non valido")?;
    let green: Vec<serde_json::Value> = all.into_iter().filter(is_green).collect();

    let mut pool = Pool::new();
    for q in &green {
        let injection = q.get("injection_case").is_some_and(|v| !v.is_null());
        for p in q["expected_passages"].as_array().into_iter().flatten() {
            let doc = p["document_id"]
                .as_str()
                .context("expected_passage senza document_id")?;
            let excerpt = p["excerpt"]
                .as_str()
                .context("expected_passage senza excerpt")?;
            let class = acl
                .classification(doc)
                .with_context(|| format!("documento {doc} assente dal manifest"))?;
            let entry = pool
                .entry((doc.to_owned(), excerpt.to_owned()))
                .or_insert_with(|| PoolEntry {
                    passage: Passage {
                        id: passage_id(doc, excerpt),
                        doc_key: doc.to_owned(),
                        classification: class.to_owned(),
                        text: excerpt.to_owned(),
                    },
                    injection: false,
                });
            entry.injection |= injection;
        }
    }
    let ids: HashSet<&str> = pool.values().map(|e| e.passage.id.as_str()).collect();
    if ids.len() != pool.len() {
        bail!("collisione di id fra excerpt diversi");
    }
    Ok((green, pool))
}

/// Costruisce `n_queries` insiemi di `set_size` passaggi dagli oracoli di
/// `corpus_dir`, con selezione deterministica dal `seed`.
///
/// Query ammesse: oracoli verdi con risposta, con almeno un `expected_passage`
/// e con almeno `set_size` excerpt visibili al loro `role_profile`. Il mix di
/// lingue è a turno fra `it`, `en` e `cross`, nell'ordine mescolato dal seme.
/// Ogni insieme = gli `expected_passages` + fino a
/// [`MAX_INJECTION_DISTRACTORS`] excerpt `injection_case` visibili +
/// distrattori visibili scelti dal seme, in ordine mescolato.
pub fn build_candidate_sets(
    corpus_dir: &Path,
    n_queries: usize,
    set_size: usize,
    seed: u64,
) -> anyhow::Result<Vec<CandidateSet>> {
    let acl = Acl::load(corpus_dir)?;
    let (green, pool) = load_green(corpus_dir, &acl)?;
    let mut rng = SplitMix64(seed);

    let mut groups: BTreeMap<&'static str, Vec<&serde_json::Value>> = BTreeMap::new();
    for q in &green {
        if q["no_answer"] != false {
            continue;
        }
        let role = q["role_profile"]
            .as_str()
            .context("oracolo senza role_profile")?;
        let expected = q["expected_passages"].as_array().map_or(0, Vec::len);
        let visible = pool
            .values()
            .filter(|e| acl.is_visible(&e.passage.doc_key, role))
            .count();
        if expected == 0 || visible < set_size {
            continue;
        }
        let lang = q["lang"].as_str().context("oracolo senza lang")?;
        groups.entry(lang_group(lang)).or_default().push(q);
    }
    for g in groups.values_mut() {
        rng.shuffle(g);
    }

    let order = ["it", "en", "cross"];
    let mut cursor: HashMap<&str, usize> = HashMap::new();
    let mut picked: Vec<&serde_json::Value> = Vec::new();
    while picked.len() < n_queries {
        let before = picked.len();
        for g in order {
            if picked.len() == n_queries {
                break;
            }
            let i = cursor.entry(g).or_insert(0);
            if let Some(q) = groups.get(g).and_then(|v| v.get(*i)) {
                picked.push(q);
                *i += 1;
            }
        }
        if picked.len() == before {
            bail!(
                "solo {} query ammissibili, richieste {n_queries}",
                picked.len()
            );
        }
    }

    let mut sets = Vec::with_capacity(picked.len());
    for q in picked {
        let id = q["oracle_id"].as_str().context("oracolo senza oracle_id")?;
        let role = q["role_profile"]
            .as_str()
            .context("oracolo senza role_profile")?;
        let mut chosen: Vec<Passage> = Vec::with_capacity(set_size);
        let mut taken: HashSet<String> = HashSet::new();
        let mut relevant_ids = Vec::new();

        for p in q["expected_passages"].as_array().into_iter().flatten() {
            let key = (
                p["document_id"].as_str().unwrap_or_default().to_owned(),
                p["excerpt"].as_str().unwrap_or_default().to_owned(),
            );
            let e = pool
                .get(&key)
                .with_context(|| format!("{id}: excerpt fuori pool"))?;
            if !acl.is_visible(&e.passage.doc_key, role) {
                bail!(
                    "{id}: expected_passage di {} non visibile a {role}",
                    e.passage.doc_key
                );
            }
            if taken.insert(e.passage.id.clone()) {
                relevant_ids.push(e.passage.id.clone());
                chosen.push(e.passage.clone());
            }
        }

        let (mut injection, mut others): (Vec<&PoolEntry>, Vec<&PoolEntry>) = pool
            .values()
            .filter(|e| acl.is_visible(&e.passage.doc_key, role) && !taken.contains(&e.passage.id))
            .partition(|e| e.injection);
        rng.shuffle(&mut injection);
        // Al più MAX_INJECTION_DISTRACTORS excerpt injection come distrattori
        // garantiti; gli altri tornano nel pool comune.
        let rest = injection.split_off(injection.len().min(MAX_INJECTION_DISTRACTORS));
        others.extend(rest);
        rng.shuffle(&mut others);
        for e in injection.into_iter().chain(others) {
            if chosen.len() == set_size {
                break;
            }
            chosen.push(e.passage.clone());
        }
        if chosen.len() != set_size {
            bail!(
                "{id}: {} passaggi visibili, servono {set_size}",
                chosen.len()
            );
        }
        rng.shuffle(&mut chosen);
        // Guardia anche in costruzione: l'insieme nasce già autorizzato.
        assert_visible(&chosen, role, &acl).with_context(|| format!("insieme di {id}"))?;

        sets.push(CandidateSet {
            query: Query {
                id: id.to_owned(),
                text: q["text"].as_str().context("oracolo senza text")?.to_owned(),
                role_profile: role.to_owned(),
            },
            lang: q["lang"].as_str().unwrap_or_default().to_owned(),
            passages: chosen,
            relevant_ids,
        });
    }
    Ok(sets)
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
