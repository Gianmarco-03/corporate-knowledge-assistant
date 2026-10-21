//! Padre del lanciatore: `run`, `batch`, `calibrate` e i driver `hostile`, `budget`, `overhead` (piano 01-07).
use s02_parser::protocol::ChildOutcome;

fn main() {
    // stub del test-write: da implementare in 01-07
    todo!()
}

/// Regola di confronto del driver `hostile`: `(pass, extra_reasons)`.
// stub del test-write: da implementare in 01-07
#[cfg_attr(not(test), allow(dead_code))]
fn hostile_verdict(
    _expected_status: &str,
    _expected: &[String],
    _outcome: &ChildOutcome,
    _oom_adj_set: bool,
) -> (bool, Vec<String>) {
    // stub del test-write: da implementare in 01-07
    todo!()
}

#[cfg(test)]
mod tests {
    //! (j) Tabella di `hostile_verdict` (TEST-4, TEST-6).
    use super::*;
    use s02_parser::protocol::Kind;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    fn ok() -> ChildOutcome {
        ChildOutcome::Ok {
            pages: Some(1),
            output: None,
            data: None,
        }
    }

    fn failed(kind: Kind) -> ChildOutcome {
        ChildOutcome::Failed {
            kind,
            detail: "x".into(),
        }
    }

    fn quarantined(reasons: &[&str]) -> ChildOutcome {
        ChildOutcome::Quarantined {
            reasons: s(reasons),
        }
    }

    #[test]
    fn ok_expected_ok_with_oom_adj_passes() {
        assert_eq!(hostile_verdict("ok", &[], &ok(), true), (true, vec![]));
    }

    #[test]
    fn expected_outcome_without_oom_adj_fails() {
        assert!(!hostile_verdict("ok", &[], &ok(), false).0);
        assert!(!hostile_verdict("failed", &s(&["Corrupt"]), &failed(Kind::Corrupt), false).0);
        assert!(
            !hostile_verdict(
                "quarantined",
                &s(&["javascript"]),
                &quarantined(&["javascript"]),
                false
            )
            .0
        );
    }

    #[test]
    fn failed_kind_among_alternatives_passes() {
        let alt = s(&["Corrupt", "Crashed"]);
        assert!(hostile_verdict("failed", &alt, &failed(Kind::Corrupt), true).0);
        assert!(hostile_verdict("failed", &alt, &failed(Kind::Crashed), true).0);
    }

    #[test]
    fn failed_kind_outside_alternatives_fails() {
        let alt = s(&["Corrupt", "Crashed"]);
        assert!(!hostile_verdict("failed", &alt, &failed(Kind::Timeout), true).0);
    }

    #[test]
    fn quarantined_superset_passes_with_extra_reasons() {
        let (pass, extra) = hostile_verdict(
            "quarantined",
            &s(&["javascript", "launch"]),
            &quarantined(&["launch", "embedded_file", "javascript"]),
            true,
        );
        assert!(pass);
        assert_eq!(extra, s(&["embedded_file"]));
    }

    #[test]
    fn quarantined_missing_expected_reason_fails() {
        let (pass, _) = hostile_verdict(
            "quarantined",
            &s(&["javascript", "launch"]),
            &quarantined(&["javascript"]),
            true,
        );
        assert!(!pass);
    }

    #[test]
    fn different_status_fails() {
        assert!(!hostile_verdict("failed", &s(&["Corrupt"]), &ok(), true).0);
        assert!(
            !hostile_verdict(
                "quarantined",
                &s(&["javascript"]),
                &failed(Kind::Corrupt),
                true
            )
            .0
        );
        assert!(!hostile_verdict("ok", &[], &quarantined(&["javascript"]), true).0);
    }
}
