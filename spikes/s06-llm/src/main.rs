//! Spike S-06 (A-04): Haiku 5.5 e Sonnet 5.5 con structured output e citazioni
//! per claim, routing per complessità deciso da JEV, fallback a Sonnet senza
//! JEV (risposta `degraded`, mai `validated`), tenuta alla prompt injection.

// Gli stub del test-write non sono ancora chiamati dalla CLI: l'esecutore di
// 01-12 li collega e toglie queste allow.
#[allow(dead_code)]
mod anthropic;
#[allow(dead_code)]
mod context;
#[allow(dead_code)]
mod jev;
#[allow(dead_code)]
mod validate;

use context::{Acl, OracleQuery};
use serde::{Deserialize, Serialize};

/// `spikes/data/s06/selection.json`: solo id e semi, nessun valore atteso.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub seed: u64,
    pub fallback_seed: u64,
    pub ids: Vec<String>,
    pub fallback_ids: Vec<String>,
}

/// Modello generativo della matrice di D-31.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Haiku,
    Sonnet,
}

/// Riga di `llm.csv` con la marca `no_answer` dell'oracolo (join con queries.json).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleRow {
    pub model: Model,
    pub query_id: String,
    pub rep: u32,
    pub label: jev::Complexity,
    pub no_answer: bool,
    pub json_valid: bool,
    pub claims_n: u64,
    pub claims_supported: u64,
}

/// Tassi aggregati sui claim (somma `claims_supported` / somma `claims_n`);
/// `None` con denominatore nullo.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuleNumbers {
    pub haiku_complex: Option<f64>,
    pub sonnet_complex: Option<f64>,
    pub haiku_simple: Option<f64>,
}

/// Esito della regola di routing di D-31.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RoutingVerdict {
    /// (a) e (b) vere: Haiku resta il modello delle `simple`.
    HaikuForSimple(RuleNumbers),
    /// Almeno una condizione falsa o un denominatore nullo: la scelta torna all'utente.
    BackToUser(RuleNumbers),
}

/// Riga di `fallback.csv`.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackRow {
    pub query_id: String,
    pub jev_error: String,
    pub model_used: String,
    pub status: validate::Status,
}

/// Selezione deterministica dai due semi fra le query ratificate (tre assi
/// verdi): 30 `ids` con ≥ 12 simple, ≥ 12 complex, ≥ 4 no_answer, ≥ 3
/// injection-case; 15 `fallback_ids` ⊂ `ids` dal secondo seme.
#[allow(dead_code)]
pub fn select(queries_json: &str, seed: u64, fallback_seed: u64) -> anyhow::Result<Selection> {
    // stub del test-write: da implementare in 01-12
    let _ = (queries_json, seed, fallback_seed);
    todo!()
}

/// Query di `ids` su cui gira Sonnet (D-31): `complex` o `no_answer`, mai le
/// `simple` con risposta. Ordine di `ids`.
#[allow(dead_code)]
pub fn sonnet_jobs(queries: &[OracleQuery], ids: &[String]) -> Vec<String> {
    // stub del test-write: da implementare in 01-12
    let _ = (queries, ids);
    todo!()
}

/// Regola di routing di D-31 sulle sole righe rep 2..3 con `json_valid`:
/// (a) sulle `complex` non `no_answer`, sullo stesso insieme di query per i due
/// modelli, tasso Haiku ≥ tasso Sonnet − 0,05; (b) tasso Haiku sulle `simple` ≥ 0,85.
#[allow(dead_code)]
pub fn routing_rule(rows: &[RuleRow]) -> RoutingVerdict {
    // stub del test-write: da implementare in 01-12
    let _ = rows;
    todo!()
}

/// Percorso `fallback`: per ogni `fallback_ids`, JEV (irraggiungibile) non
/// decide, la generazione usa `complex_model` e la risposta è `degraded`, mai
/// `validated`. Contesto con `assert_visible` prima del prompt.
#[allow(dead_code)]
pub async fn run_fallback(
    llm: &anthropic::Client,
    complex_model: &str,
    jev: &jev::Client,
    acl: &Acl,
    queries: &[OracleQuery],
    selection: &Selection,
) -> anyhow::Result<Vec<FallbackRow>> {
    // stub del test-write: da implementare in 01-12
    let _ = (llm, complex_model, jev, acl, queries, selection);
    todo!()
}

fn main() {}

/// Server HTTP/1.1 minimo su loopback per i test: una richiesta per
/// connessione, risposte prese in ordine dallo script (l'ultima si ripete).
#[cfg(test)]
pub(crate) mod fake_http {
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[derive(Clone)]
    pub(crate) struct Reply {
        status: u16,
        body: String,
    }

    impl Reply {
        pub(crate) fn ok(body: String) -> Reply {
            Reply { status: 200, body }
        }

        pub(crate) fn status(status: u16, body: &str) -> Reply {
            Reply {
                status,
                body: body.to_owned(),
            }
        }
    }

    #[derive(Clone)]
    pub(crate) struct Recorded {
        pub(crate) head: String,
        pub(crate) body: serde_json::Value,
    }

    pub(crate) struct FakeServer {
        pub(crate) base_url: String,
        log: Arc<Mutex<Vec<Recorded>>>,
    }

    impl FakeServer {
        pub(crate) async fn start(script: Vec<Reply>) -> FakeServer {
            assert!(!script.is_empty(), "script non vuoto");
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let port = listener.local_addr().expect("addr").port();
            let base_url = format!("http://127.0.0.1:{port}");
            let log: Arc<Mutex<Vec<Recorded>>> = Arc::default();
            let log_srv = Arc::clone(&log);
            tokio::spawn(async move {
                loop {
                    let Ok((mut sock, _)) = listener.accept().await else {
                        return;
                    };
                    let log = Arc::clone(&log_srv);
                    let script = script.clone();
                    tokio::spawn(async move {
                        let mut buf = Vec::new();
                        let mut chunk = [0u8; 8192];
                        let head_end = loop {
                            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                                break pos + 4;
                            }
                            match sock.read(&mut chunk).await {
                                Ok(0) | Err(_) => return,
                                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                            }
                        };
                        let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
                        let len = head
                            .lines()
                            .find_map(|l| {
                                let (k, v) = l.split_once(':')?;
                                k.trim()
                                    .eq_ignore_ascii_case("content-length")
                                    .then(|| v.trim().parse::<usize>().ok())?
                            })
                            .unwrap_or(0);
                        while buf.len() < head_end + len {
                            match sock.read(&mut chunk).await {
                                Ok(0) | Err(_) => return,
                                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                            }
                        }
                        let body = serde_json::from_slice(&buf[head_end..head_end + len])
                            .unwrap_or(serde_json::Value::Null);
                        let idx = {
                            let mut log = log.lock().expect("log");
                            log.push(Recorded { head, body });
                            log.len() - 1
                        };
                        let reply = script
                            .get(idx)
                            .or_else(|| script.last())
                            .cloned()
                            .expect("script non vuoto");
                        let out = format!(
                            "HTTP/1.1 {} Test\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                            reply.status,
                            reply.body.len(),
                            reply.body
                        );
                        let _ = sock.write_all(out.as_bytes()).await;
                        let _ = sock.shutdown().await;
                    });
                }
            });
            FakeServer { base_url, log }
        }

        pub(crate) fn count(&self) -> usize {
            self.log.lock().expect("log").len()
        }

        pub(crate) fn requests(&self) -> Vec<Recorded> {
            self.log.lock().expect("log").clone()
        }
    }

    /// URL di una porta loopback chiusa: si apre un listener, se ne legge la
    /// porta e lo si chiude.
    pub(crate) async fn closed_port_url() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        drop(listener);
        format!("http://127.0.0.1:{port}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anthropic::tests::{SONNET, message_body};
    use crate::context::tests::{acl, oracle};
    use crate::fake_http::{FakeServer, Reply, closed_port_url};
    use jev::Complexity::{Complex, Simple};
    use std::collections::BTreeSet;
    use std::time::{Duration, Instant};

    fn queries_json() -> String {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/corpus/oracles/queries.json"
        );
        std::fs::read_to_string(path).expect("queries.json degli oracoli")
    }

    // TEST-21 (RISK-14, RISK-12) e forma di selection.json (TEST-19, parte unit).
    #[test]
    fn selection_is_deterministic_from_seeds() {
        let raw = queries_json();
        let all: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap();
        let ratified: BTreeSet<String> = all
            .iter()
            .filter(|q| {
                q["guardrail"]["user"] == "ratified" && q["guardrail"]["gpt"] == "CONFIRMED"
            })
            .map(|q| q["oracle_id"].as_str().unwrap().to_owned())
            .collect();
        let by_id = |id: &str| all.iter().find(|q| q["oracle_id"] == id).unwrap().clone();

        let a = select(&raw, 42, 7).expect("selezione");
        let b = select(&raw, 42, 7).expect("selezione");
        assert_eq!(a, b, "stessi semi, stessa selezione nello stesso ordine");
        assert_eq!((a.seed, a.fallback_seed), (42, 7));

        assert_eq!(a.ids.len(), 30);
        let ids: BTreeSet<&String> = a.ids.iter().collect();
        assert_eq!(ids.len(), 30, "id univoci");
        assert!(
            a.ids.iter().all(|id| ratified.contains(id)),
            "solo query ratificate"
        );

        let count = |f: &dyn Fn(&serde_json::Value) -> bool| {
            a.ids.iter().filter(|id| f(&by_id(id))).count()
        };
        assert!(count(&|q| q["complexity"] == "simple" && q["no_answer"] == false) >= 12);
        assert!(count(&|q| q["complexity"] == "complex") >= 12);
        assert!(count(&|q| q["no_answer"] == true) >= 4);
        assert!(count(&|q| q["injection_case"].is_string()) >= 3);

        assert_eq!(a.fallback_ids.len(), 15);
        let fb: BTreeSet<&String> = a.fallback_ids.iter().collect();
        assert_eq!(fb.len(), 15, "fallback_ids univoci");
        assert!(
            a.fallback_ids.iter().all(|id| a.ids.contains(id)),
            "fallback_ids ⊂ ids"
        );

        let c = select(&raw, 42, 8).expect("selezione");
        assert_eq!(c.ids, a.ids, "il seme di fallback non cambia gli ids");
        assert_ne!(
            c.fallback_ids, a.fallback_ids,
            "il seme di fallback è usato"
        );

        // Solo id e semi: nessun valore atteso nel file.
        let v = serde_json::to_value(&a).unwrap();
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys.len(), 4, "{keys:?}");
        assert!(!v.to_string().contains("expected"));
    }

    // TEST-20 (RISK-14, RISK-9)
    #[test]
    fn sonnet_matrix_excludes_simple() {
        let queries = vec![
            oracle("Q-S1", "guest", "simple", false, &[]),
            oracle("Q-S2", "guest", "simple", false, &[]),
            oracle("Q-C1", "guest", "complex", false, &[]),
            oracle("Q-NA", "guest", "simple", true, &[]),
            oracle("Q-FUORI", "guest", "complex", false, &[]),
        ];
        let ids: Vec<String> = ["Q-S1", "Q-C1", "Q-S2", "Q-NA"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(
            sonnet_jobs(&queries, &ids),
            vec!["Q-C1".to_owned(), "Q-NA".to_owned()]
        );
    }

    fn rows(model: Model, qid: &str, label: jev::Complexity, n: u64, s: u64) -> Vec<RuleRow> {
        (2..=3)
            .map(|rep| RuleRow {
                model,
                query_id: qid.to_owned(),
                rep,
                label,
                no_answer: false,
                json_valid: true,
                claims_n: n,
                claims_supported: s,
            })
            .collect()
    }

    fn fixture(h_complex: (u64, u64), s_complex: (u64, u64), h_simple: (u64, u64)) -> Vec<RuleRow> {
        let mut r = rows(Model::Haiku, "Q-C1", Complex, h_complex.1, h_complex.0);
        r.extend(rows(
            Model::Sonnet,
            "Q-C1",
            Complex,
            s_complex.1,
            s_complex.0,
        ));
        r.extend(rows(Model::Haiku, "Q-S1", Simple, h_simple.1, h_simple.0));
        r
    }

    fn approx(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|x| (x - b).abs() < 1e-9)
    }

    // TEST-23 (RISK-13): entrambi i rami della regola come funzione pura.
    #[test]
    fn routing_rule_both_branches() {
        // Entrambe vere.
        let v = routing_rule(&fixture((90, 100), (90, 100), (90, 100)));
        let RoutingVerdict::HaikuForSimple(n) = v else {
            panic!("atteso HaikuForSimple, ottenuto {v:?}");
        };
        assert!(approx(n.haiku_complex, 0.9) && approx(n.sonnet_complex, 0.9));
        assert!(approx(n.haiku_simple, 0.9));

        // Haiku migliore di Sonnet sulle complex va bene.
        assert!(matches!(
            routing_rule(&fixture((95, 100), (80, 100), (90, 100))),
            RoutingVerdict::HaikuForSimple(_)
        ));

        // (a) falsa con 0,06 di scarto.
        let v = routing_rule(&fixture((84, 100), (90, 100), (95, 100)));
        let RoutingVerdict::BackToUser(n) = v else {
            panic!("atteso BackToUser, ottenuto {v:?}");
        };
        assert!(approx(n.haiku_complex, 0.84) && approx(n.sonnet_complex, 0.9));

        // (b) falsa con 0,84.
        assert!(matches!(
            routing_rule(&fixture((90, 100), (90, 100), (84, 100))),
            RoutingVerdict::BackToUser(_)
        ));

        // Confine esatto: 0,05 e 0,85 passano.
        for (h, s) in [((80, 100), (85, 100)), ((75, 100), (80, 100))] {
            let v = routing_rule(&fixture(h, s, (85, 100)));
            assert!(
                matches!(v, RoutingVerdict::HaikuForSimple(_)),
                "{h:?} {s:?}: {v:?}"
            );
        }
    }

    // TEST-23 (RISK-13): denominatore nullo → torna all'utente.
    #[test]
    fn routing_rule_zero_denominator_goes_back_to_user() {
        let v = routing_rule(&fixture((0, 0), (90, 100), (90, 100)));
        assert!(matches!(v, RoutingVerdict::BackToUser(_)), "{v:?}");
        let v = routing_rule(&fixture((90, 100), (0, 0), (90, 100)));
        assert!(matches!(v, RoutingVerdict::BackToUser(_)), "{v:?}");
        let v = routing_rule(&fixture((90, 100), (90, 100), (0, 0)));
        let RoutingVerdict::BackToUser(n) = v else {
            panic!("atteso BackToUser, ottenuto {v:?}");
        };
        assert_eq!(n.haiku_simple, None);
        // Nessuna riga simple.
        let mut r = rows(Model::Haiku, "Q-C1", Complex, 10, 9);
        r.extend(rows(Model::Sonnet, "Q-C1", Complex, 10, 9));
        assert!(matches!(routing_rule(&r), RoutingVerdict::BackToUser(_)));
        assert!(matches!(routing_rule(&[]), RoutingVerdict::BackToUser(_)));
    }

    // TEST-23 (RISK-13): filtri e insiemi della regola.
    #[test]
    fn routing_rule_filters_reps_validity_no_answer_and_aligns_query_sets() {
        let mut r = fixture((90, 100), (90, 100), (90, 100));
        // rep 1 (scartata) e json non valido non contano.
        for (rep, valid) in [(1, true), (2, false)] {
            r.push(RuleRow {
                model: Model::Haiku,
                query_id: "Q-S1".to_owned(),
                rep,
                label: Simple,
                no_answer: false,
                json_valid: valid,
                claims_n: 1000,
                claims_supported: 0,
            });
        }
        // Una no_answer etichettata complex è fuori dal confronto (a).
        for (model, s) in [(Model::Haiku, 0), (Model::Sonnet, 100)] {
            for mut row in rows(model, "Q-NA", Complex, 100, s) {
                row.no_answer = true;
                r.push(row);
            }
        }
        // Una complex che solo Haiku ha generato è fuori dall'insieme comune.
        r.extend(rows(Model::Haiku, "Q-SOLO-HAIKU", Complex, 100, 0));

        let v = routing_rule(&r);
        let RoutingVerdict::HaikuForSimple(n) = v else {
            panic!("atteso HaikuForSimple, ottenuto {v:?}");
        };
        assert!(approx(n.haiku_complex, 0.9), "{n:?}");
        assert!(approx(n.sonnet_complex, 0.9), "{n:?}");
        assert!(approx(n.haiku_simple, 0.9), "{n:?}");
    }

    // TEST-5 (RISK-2) e RISK-1 sul percorso completo.
    #[tokio::test]
    async fn fallback_end_to_end_closed_port() {
        let answer = r#"{"answer":"I ricavi sono 182,4 M€.","claims":[{"text":"Ricavi 182,4 M€","citations":["c01"]},{"text":"Fonte FAQ","citations":["c01"]}],"refusal":null}"#;
        let llm_server = FakeServer::start(vec![Reply::ok(message_body(
            SONNET, "end_turn", answer, 500,
        ))])
        .await;
        let llm = anthropic::Client::new(anthropic::tests::config(&llm_server.base_url, 1.0))
            .expect("client anthropic");
        let jev_url = closed_port_url().await;
        let jev = jev::Client::new(jev::tests::config(&jev_url), acl()).expect("client jev");

        let queries = vec![
            oracle(
                "Q-1",
                "guest",
                "simple",
                false,
                &[("syn-06-faq-public", "2.1", "Ricavi 182,4 M€")],
            ),
            oracle(
                "Q-2",
                "employee",
                "complex",
                false,
                &[("syn-01-budget-interno", "4.1", "EBITDA 30,4")],
            ),
            oracle(
                "Q-3",
                "guest",
                "complex",
                false,
                &[("syn-06-faq-public", "2.2", "Dipendenti 1.240")],
            ),
            oracle(
                "Q-R",
                "admin",
                "simple",
                false,
                &[("syn-03-verbale-cda-restricted", "1", "VIETATO-CDA")],
            ),
        ];
        let selection = Selection {
            seed: 1,
            fallback_seed: 2,
            ids: vec!["Q-1".to_owned(), "Q-2".to_owned(), "Q-3".to_owned()],
            fallback_ids: vec!["Q-1".to_owned(), "Q-3".to_owned()],
        };

        let t = Instant::now();
        let out = run_fallback(&llm, SONNET, &jev, &acl(), &queries, &selection)
            .await
            .expect("fallback");
        assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());

        let ids: Vec<&str> = out.iter().map(|r| r.query_id.as_str()).collect();
        assert_eq!(ids, vec!["Q-1", "Q-3"], "solo i fallback_ids");
        for row in &out {
            assert_eq!(row.model_used, SONNET);
            assert_eq!(row.status, validate::Status::Degraded, "{row:?}");
            assert_ne!(row.status, validate::Status::Validated);
            assert!(!row.jev_error.is_empty(), "{row:?}");
        }

        let rec = llm_server.requests();
        assert_eq!(rec.len(), 2, "una generazione per fallback_id");
        for r in &rec {
            assert_eq!(r.body["model"], SONNET);
            assert!(!r.body.to_string().contains("VIETATO-CDA"));
        }
    }
}
