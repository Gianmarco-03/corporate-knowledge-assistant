//! Client TEI (`POST /embed`, URL da `TEI_EMBED_URL`): embedding dei documenti e della query
//! separati, istruzione asimmetrica solo sulla query.
//!
//! STUB test-write 01-10: i costruttori del corpo della richiesta sono `todo!()`; `Client`
//! lo scrive l'esecutore e deve usare questi costruttori.
use crate::Model;

/// Corpo di `POST /embed` per un lotto di documenti: mai istruzioni.
pub fn documents_body(_texts: &[&str]) -> serde_json::Value {
    todo!("documents_body: implementazione del piano 01-10")
}

/// Corpo di `POST /embed` per una query: istruzione per modello se `with_instruction`.
pub fn query_body(_model: Model, _query: &str, _with_instruction: bool) -> serde_json::Value {
    todo!("query_body: implementazione del piano 01-10")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const Q: &str = "Qual è l'EBITDA previsto dal budget 2026?";

    fn inputs_str(body: &Value) -> &str {
        body.get("inputs")
            .and_then(Value::as_str)
            .expect("inputs della query come stringa")
    }

    fn assert_flags(body: &Value) {
        assert_eq!(
            body.get("normalize"),
            Some(&Value::Bool(true)),
            "normalize: true"
        );
        assert_eq!(
            body.get("truncate"),
            Some(&Value::Bool(true)),
            "truncate: true"
        );
    }

    // TEST-11 (RISK-7): istruzione per modello sulla query, mai sui documenti, flag efficace.
    #[test]
    fn query_instruction_per_model_and_flag() {
        // Qwen3: istruzione via prompt_name oppure prefisso «Instruct: …\nQuery: <q>».
        let qwen = query_body(Model::Qwen3, Q, true);
        assert_flags(&qwen);
        let has_prompt_name = qwen
            .get("prompt_name")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty());
        let inp = inputs_str(&qwen);
        let has_prefix = inp.starts_with("Instruct: ")
            && inp.ends_with(&format!("\nQuery: {Q}"))
            && inp.len() > Q.len();
        assert!(
            has_prompt_name || has_prefix,
            "Qwen3 senza istruzione di query: {qwen}"
        );
        if has_prompt_name {
            assert_eq!(inp, Q, "con prompt_name il testo resta la query");
        }

        // e5-instruct: prefisso manuale «Instruct: …\nQuery: <q>».
        let e5 = query_body(Model::E5, Q, true);
        assert_flags(&e5);
        let inp = inputs_str(&e5);
        assert!(inp.starts_with("Instruct: "), "e5 senza prefisso: {inp:?}");
        assert!(
            inp.ends_with(&format!("\nQuery: {Q}")),
            "e5 prefisso malformato: {inp:?}"
        );
        assert!(e5.get("prompt_name").is_none());

        // bge-m3: nessun prefisso.
        let bge = query_body(Model::BgeM3, Q, true);
        assert_flags(&bge);
        assert_eq!(inputs_str(&bge), Q);
        assert!(bge.get("prompt_name").is_none());

        // Flag disattivato: nessun modello ha istruzione.
        for m in [Model::Qwen3, Model::BgeM3, Model::E5] {
            let b = query_body(m, Q, false);
            assert_flags(&b);
            assert_eq!(inputs_str(&b), Q, "{m:?} con --no-instruction");
            assert!(
                b.get("prompt_name").is_none(),
                "{m:?} con --no-instruction ha prompt_name"
            );
        }
    }

    // TEST-11 (RISK-7): embed_documents non aggiunge mai istruzioni.
    #[test]
    fn documents_body_never_has_instruction() {
        let docs = [
            "Ricavi consolidati 182,4 M€",
            "Instruct: non è un'istruzione nostra",
        ];
        let b = documents_body(&docs);
        assert_flags(&b);
        assert!(b.get("prompt_name").is_none());
        let arr = b
            .get("inputs")
            .and_then(Value::as_array)
            .expect("inputs dei documenti come array");
        let got: Vec<&str> = arr.iter().map(|v| v.as_str().expect("stringa")).collect();
        assert_eq!(got, docs, "i testi dei documenti passano invariati");
    }
}
