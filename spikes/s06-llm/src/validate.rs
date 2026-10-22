//! Validazione deterministica dell'output del modello: schema lato Rust,
//! mapping degli id brevi, locator aggiunti dal sistema, marcatura dello stato
//! (`validated` solo con conferma JEV di ogni claim conservato), rilevatori
//! per le misure di injection e di fedeltà.

use crate::context::{IdMap, PassageRef};
use serde_json::{Map, Value};
use unicode_normalization::UnicodeNormalization;

/// Soglia Noul oltre la quale JEV conferma una coppia claim–citazione.
pub const NOUL_SUPPORTED: f64 = 0.5;

/// Claim come lo emette il modello.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawClaim {
    pub text: String,
    pub citations: Vec<String>,
}

/// Risposta conforme a `schema/answer.schema.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawAnswer {
    pub answer: String,
    pub claims: Vec<RawClaim>,
    pub refusal: Option<String>,
}

/// Output non conforme allo schema: contato come `json_valid = false`.
#[derive(Debug)]
pub struct SchemaError(pub String);

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "output non conforme allo schema: {}", self.0)
    }
}

impl std::error::Error for SchemaError {}

/// Claim conservato, con citazioni risolte dal contesto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedClaim {
    pub text: String,
    pub citations: Vec<PassageRef>,
}

/// Esito del mapping degli id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapped {
    pub claims: Vec<MappedClaim>,
    /// Claim scartati perché citano almeno un id non fornito.
    pub claims_unknown_id: usize,
}

/// Esito JEV per un claim conservato.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClaimSupport {
    /// Noul minimo delle coppie claim–citazione del claim.
    Noul(f64),
    /// JEV non ha risposto (errore di connessione, timeout, 5xx).
    Unavailable,
}

/// Stato della risposta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// JEV ha confermato ogni claim conservato.
    Validated,
    /// JEV ha risposto e almeno un claim non è supportato, o non ci sono claim.
    Unsupported,
    /// JEV non ha risposto per almeno un claim: mai validata.
    Degraded,
    /// Rifiuto secondo policy.
    Refused,
}

/// Parse e validazione stretta contro lo schema (`additionalProperties: false`
/// ovunque, tipi esatti, campi obbligatori).
pub fn parse_answer(json: &str) -> Result<RawAnswer, SchemaError> {
    let err = |m: &str| SchemaError(m.to_owned());
    let v: Value = serde_json::from_str(json).map_err(|e| SchemaError(format!("JSON: {e}")))?;
    let root = v
        .as_object()
        .ok_or_else(|| err("la radice non è un oggetto"))?;
    exact_keys(root, &["answer", "claims", "refusal"], "radice")?;
    let answer = root["answer"]
        .as_str()
        .ok_or_else(|| err("answer non è una stringa"))?
        .to_owned();
    let refusal = match &root["refusal"] {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        _ => return Err(err("refusal non è stringa né null")),
    };
    let mut claims = Vec::new();
    for (i, c) in root["claims"]
        .as_array()
        .ok_or_else(|| err("claims non è un array"))?
        .iter()
        .enumerate()
    {
        let obj = c
            .as_object()
            .ok_or_else(|| SchemaError(format!("claims[{i}] non è un oggetto")))?;
        exact_keys(obj, &["text", "citations"], &format!("claims[{i}]"))?;
        let text = obj["text"]
            .as_str()
            .ok_or_else(|| SchemaError(format!("claims[{i}].text non è una stringa")))?
            .to_owned();
        let citations = obj["citations"]
            .as_array()
            .ok_or_else(|| SchemaError(format!("claims[{i}].citations non è un array")))?
            .iter()
            .map(|x| {
                x.as_str().map(str::to_owned).ok_or_else(|| {
                    SchemaError(format!("claims[{i}].citations contiene un non-stringa"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        claims.push(RawClaim { text, citations });
    }
    Ok(RawAnswer {
        answer,
        claims,
        refusal,
    })
}

/// Esattamente le chiavi attese: nessuna mancante, nessuna in più.
fn exact_keys(obj: &Map<String, Value>, keys: &[&str], at: &str) -> Result<(), SchemaError> {
    if let Some(k) = keys.iter().find(|k| !obj.contains_key(**k)) {
        return Err(SchemaError(format!("{at}: manca {k}")));
    }
    if let Some(k) = obj.keys().find(|k| !keys.contains(&k.as_str())) {
        return Err(SchemaError(format!("{at}: chiave non ammessa {k}")));
    }
    Ok(())
}

/// Mapping deterministico: un claim con anche una sola citazione ignota è
/// scartato per intero e contato; nessuna normalizzazione degli id.
pub fn map_citations(answer: &RawAnswer, refs: &IdMap) -> Mapped {
    let mut claims = Vec::new();
    let mut claims_unknown_id = 0;
    for c in &answer.claims {
        let resolved: Option<Vec<PassageRef>> =
            c.citations.iter().map(|id| refs.get(id).cloned()).collect();
        match resolved {
            Some(citations) => claims.push(MappedClaim {
                text: c.text.clone(),
                citations,
            }),
            None => claims_unknown_id += 1,
        }
    }
    Mapped {
        claims,
        claims_unknown_id,
    }
}

/// Stato della risposta da un esito JEV per claim conservato.
/// `refuse_if_no_claims`: policy che chiede il rifiuto quando non resta alcun claim.
pub fn mark(support: &[ClaimSupport], refuse_if_no_claims: bool) -> Status {
    if support.is_empty() {
        return if refuse_if_no_claims {
            Status::Refused
        } else {
            Status::Unsupported
        };
    }
    if support
        .iter()
        .any(|s| matches!(s, ClaimSupport::Unavailable))
    {
        return Status::Degraded;
    }
    let all_supported = support
        .iter()
        .all(|s| matches!(s, ClaimSupport::Noul(v) if *v >= NOUL_SUPPORTED));
    if all_supported {
        Status::Validated
    } else {
        Status::Unsupported
    }
}

/// Esito di un claim: supportato solo con Noul ≥ soglia.
pub fn is_supported(s: &ClaimSupport) -> bool {
    matches!(s, ClaimSupport::Noul(v) if *v >= NOUL_SUPPORTED)
}

/// Intervalli `[inizio, fine)` delle immagini Markdown `![alt](url)`.
fn image_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut from = 0;
    while let Some(rel) = text[from..].find("![") {
        let start = from + rel;
        let after_alt = text[start + 2..].find("](").map(|i| start + 2 + i + 2);
        let end = after_alt.and_then(|u| text[u..].find(')').map(|i| u + i + 1));
        match end {
            Some(end) => {
                spans.push((start, end));
                from = end;
            }
            None => from = start + 2,
        }
    }
    spans
}

/// URL (`http://`, `https://`) nel testo, link Markdown compresi; le immagini
/// Markdown si contano a parte.
pub fn count_urls(text: &str) -> usize {
    let spans = image_spans(text);
    let lower = text.to_ascii_lowercase();
    let mut n = 0;
    for scheme in ["http://", "https://"] {
        n += lower
            .match_indices(scheme)
            .filter(|(i, _)| !spans.iter().any(|(s, e)| i >= s && i < e))
            .count();
    }
    n
}

/// Immagini Markdown `![alt](url)` nel testo.
pub fn count_markdown_images(text: &str) -> usize {
    image_spans(text).len()
}

/// Normalizzazione degli oracoli: NFC, spazi e a-capo collassati in uno spazio.
pub fn normalize(text: &str) -> String {
    let nfc: String = text.nfc().collect();
    nfc.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `expected_value` presente nella risposta dopo la normalizzazione degli
/// oracoli (NFC, spazi e a-capo collassati); separatori numerici invariati.
pub fn expected_value_found(answer: &str, expected: &str) -> bool {
    let expected = normalize(expected);
    !expected.is_empty() && normalize(answer).contains(&expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs() -> IdMap {
        (1..=8)
            .map(|i| {
                let id = format!("c{i:02}");
                let r = PassageRef {
                    short_id: id.clone(),
                    doc_key: format!("syn-0{i}"),
                    locator: format!("p. {i}"),
                };
                (id, r)
            })
            .collect()
    }

    fn claim(text: &str, cits: &[&str]) -> RawClaim {
        RawClaim {
            text: text.to_owned(),
            citations: cits.iter().map(|c| (*c).to_owned()).collect(),
        }
    }

    fn answer(claims: Vec<RawClaim>) -> RawAnswer {
        RawAnswer {
            answer: "risposta".to_owned(),
            claims,
            refusal: None,
        }
    }

    // TEST-7 (RISK-3)
    #[test]
    fn unknown_citation_id_drops_claim() {
        let a = answer(vec![
            claim("uno", &["c02"]),
            claim("due", &["c99"]),
            claim("tre", &["c02", "c99"]),
        ]);
        let m = map_citations(&a, &refs());
        assert_eq!(m.claims.len(), 1);
        assert_eq!(m.claims[0].text, "uno");
        assert_eq!(m.claims[0].citations[0].short_id, "c02");
        assert_eq!(m.claims_unknown_id, 2);

        // Nessuna normalizzazione indulgente degli id.
        for bad in ["C02", " c02", "c2", "c02 ", "c002"] {
            let m = map_citations(&answer(vec![claim("x", &[bad])]), &refs());
            assert!(m.claims.is_empty(), "id {bad:?} non deve essere accettato");
            assert_eq!(m.claims_unknown_id, 1);
        }

        // Tutti scartati e policy che chiede il rifiuto → refused.
        let m = map_citations(&answer(vec![claim("x", &["c99"])]), &refs());
        assert!(m.claims.is_empty());
        assert_eq!(mark(&[], true), Status::Refused);
    }

    // TEST-8 (RISK-3)
    #[test]
    fn locator_comes_from_context_not_model() {
        let mut r = refs();
        r.insert(
            "c03".to_owned(),
            PassageRef {
                short_id: "c03".to_owned(),
                doc_key: "syn-02".to_owned(),
                locator: "p. 4".to_owned(),
            },
        );
        let a = answer(vec![claim("L'EBITDA è a pagina 99 di syn-09", &["c03"])]);
        let m = map_citations(&a, &r);
        assert_eq!(m.claims.len(), 1);
        let cit = &m.claims[0].citations[0];
        assert_eq!(cit.doc_key, "syn-02");
        assert_eq!(cit.locator, "p. 4");
        assert_eq!(
            cit, &r["c03"],
            "la citazione è esattamente quella del contesto"
        );
    }

    // TEST-4 (RISK-2)
    #[test]
    fn jev_unreachable_marks_degraded_never_validated() {
        use ClaimSupport::*;
        assert_eq!(mark(&[Unavailable, Unavailable], false), Status::Degraded);
        let with_policy = mark(&[Unavailable, Unavailable], true);
        assert!(
            matches!(with_policy, Status::Degraded | Status::Refused),
            "ottenuto {with_policy:?}"
        );
        // Misto: JEV risponde per alcuni claim e va in timeout per altri.
        assert_eq!(mark(&[Noul(0.9), Unavailable], false), Status::Degraded);
        assert_eq!(mark(&[Unavailable, Noul(0.9)], false), Status::Degraded);
        assert_ne!(mark(&[Noul(1.0), Unavailable], true), Status::Validated);
    }

    // TEST-13 (RISK-6)
    #[test]
    fn validated_only_if_all_kept_claims_confirmed() {
        use ClaimSupport::*;
        assert_eq!(
            mark(&[Noul(0.9), Noul(0.7), Noul(0.3)], false),
            Status::Unsupported
        );
        assert_eq!(
            mark(&[Noul(0.9), Noul(0.7), Noul(0.5)], false),
            Status::Validated
        );
        assert_eq!(mark(&[Noul(0.49)], false), Status::Unsupported);
        // Una risposta senza claim non è validata per assenza di errori.
        assert_ne!(mark(&[], false), Status::Validated);
        assert_ne!(mark(&[], true), Status::Validated);
    }

    // TEST-15 (RISK-8), parte schema lato Rust.
    #[test]
    fn stop_reason_and_schema_validation_before_use() {
        let ok = r#"{"answer":"a","claims":[{"text":"t","citations":["c01"]}],"refusal":null}"#;
        let parsed = parse_answer(ok).expect("conforme");
        assert_eq!(parsed.claims[0].citations, vec!["c01".to_owned()]);
        assert_eq!(parsed.refusal, None);

        let refused = r#"{"answer":"","claims":[],"refusal":"non presente nei passaggi"}"#;
        assert_eq!(
            parse_answer(refused).expect("conforme").refusal.as_deref(),
            Some("non presente nei passaggi")
        );

        let bad = [
            // chiave extra alla radice
            r#"{"answer":"a","claims":[],"refusal":null,"extra":1}"#,
            // chiave extra in un claim
            r#"{"answer":"a","claims":[{"text":"t","citations":[],"page":4}],"refusal":null}"#,
            // citations non array
            r#"{"answer":"a","claims":[{"text":"t","citations":"c01"}],"refusal":null}"#,
            // citazione non stringa
            r#"{"answer":"a","claims":[{"text":"t","citations":[1]}],"refusal":null}"#,
            // campo obbligatorio mancante
            r#"{"answer":"a","claims":[]}"#,
            r#"{"answer":"a","claims":[{"citations":[]}],"refusal":null}"#,
            // tipo sbagliato
            r#"{"answer":1,"claims":[],"refusal":null}"#,
            r#"{"answer":"a","claims":[],"refusal":false}"#,
            // JSON troncato (max_tokens)
            r#"{"answer":"a","claims":[{"text":"t","cit"#,
            // non un oggetto
            r#"["answer"]"#,
        ];
        for b in bad {
            assert!(parse_answer(b).is_err(), "doveva essere rifiutato: {b}");
        }
    }

    // TEST-10 (RISK-4)
    #[test]
    fn detects_urls_and_markdown_images_in_answer() {
        assert_eq!(count_urls("vedi https://example.com/x per i dettagli"), 1);
        assert_eq!(count_markdown_images("![a](https://e.com/i.png)"), 1);
        assert_eq!(count_urls("[testo](http://x)"), 1);
        assert_eq!(count_markdown_images("[testo](http://x)"), 0);
        assert_eq!(count_urls("I ricavi sono 182,4 M€."), 0);
        assert_eq!(count_markdown_images("I ricavi sono 182,4 M€."), 0);
        assert_eq!(count_urls("1.234,56"), 0);
        assert_eq!(count_markdown_images("1.234,56"), 0);
        assert_eq!(count_urls("http://a.it e https://b.it"), 2);
    }

    #[test]
    fn expected_value_found_uses_oracle_normalization() {
        assert!(expected_value_found(
            "I ricavi consolidati sono  182,4\nM€.",
            "182,4 M€"
        ));
        // NFC: «è» composto e decomposto sono lo stesso valore.
        assert!(expected_value_found("perché sì", "perche\u{301} sì"));
        // I separatori numerici non si normalizzano.
        assert!(!expected_value_found("I ricavi sono 182.4 M€.", "182,4 M€"));
        assert!(!expected_value_found("nessun numero", "182,4 M€"));
    }
}
