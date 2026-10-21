//! Lanciatore isolato: limiti per figlio, budget aggregato, cleanup del gruppo (contratto W17, punto 3).
// stub del test-write: da implementare in 01-07
use std::time::Duration;

use crate::job::ChildSpec;
use crate::protocol::ChildOutcome;

/// Limiti per figlio (profilo di `limits.json`).
#[derive(Debug, Clone)]
pub struct Limits {
    pub address_space_bytes: u64,
    pub cpu_seconds: u64,
    pub wall_timeout: Duration,
    pub file_size_bytes: u64,
    pub max_open_files: u64,
    pub out_quota_bytes: u64,
}

/// Budget aggregato di memoria dei figli ammessi.
#[derive(Debug)]
pub struct Budget {
    // stub del test-write: da implementare in 01-07
    _stub: (),
}

impl Budget {
    /// Totale = `memory.max` del cgroup del container; rifiuta `max`.
    pub fn from_cgroup(_parent_reserve_bytes: u64) -> anyhow::Result<Budget> {
        // stub del test-write: da implementare in 01-07
        todo!()
    }

    /// Totale fisso, per i test.
    pub fn fixed(_total_bytes: u64, _parent_reserve_bytes: u64) -> Budget {
        // stub del test-write: da implementare in 01-07
        todo!()
    }
}

/// Rapporto del padre, una riga JSON per job.
#[derive(Debug, Clone)]
pub struct RunReport {
    pub job_id: String,
    pub outcome: ChildOutcome,
    pub exit_signal: Option<i32>,
    pub peak_rss_kb: Option<u64>,
    pub wall_ms: u64,
    pub oom_kill_delta: u64,
    pub leftover_procs: u32,
    pub oom_adj_set: bool,
}

/// Causa di un'uccisione decisa dal padre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillCause {
    Timeout,
    OutputLimit,
    OutQuota,
}

/// Uscita osservata del figlio diretto.
#[derive(Debug, Clone)]
pub struct ExitObservation {
    pub code: Option<i32>,
    pub stdout_line: Option<String>,
    pub parent_kill: Option<KillCause>,
}

/// Lancia il figlio del job sotto limiti, budget e `GroupGuard`.
pub async fn run_child(
    _spec: &ChildSpec,
    _limits: &Limits,
    _budget: &Budget,
) -> anyhow::Result<RunReport> {
    // stub del test-write: da implementare in 01-07
    todo!()
}

/// Classificazione pura dell'uscita del figlio.
pub fn classify(
    _exit: ExitObservation,
    _signal: Option<i32>,
    _oom_delta: u64,
    _stderr_head: &[u8],
) -> ChildOutcome {
    // stub del test-write: da implementare in 01-07
    todo!()
}

#[cfg(test)]
mod tests {
    //! (i) Tabella di `classify`, senza processi né I/O (TEST-5, TEST-6).
    use super::*;
    use crate::protocol::Kind;

    fn obs(
        code: Option<i32>,
        stdout_line: Option<&str>,
        parent_kill: Option<KillCause>,
    ) -> ExitObservation {
        ExitObservation {
            code,
            stdout_line: stdout_line.map(str::to_owned),
            parent_kill,
        }
    }

    fn failed(outcome: &ChildOutcome) -> (Kind, &str) {
        match outcome {
            ChildOutcome::Failed { kind, detail } => (*kind, detail.as_str()),
            other => panic!("atteso failed, ottenuto {other:?}"),
        }
    }

    #[test]
    fn exit_zero_with_valid_ok_line_returns_child_outcome() {
        let line = r#"{"status":"ok","pages":3,"output":"/results/raw/x/j1","data":{"chars":42},"vm_hwm_kb":2048}"#;
        let got = classify(obs(Some(0), Some(line), None), None, 0, b"");
        assert_eq!(
            got,
            ChildOutcome::Ok {
                pages: Some(3),
                output: Some("/results/raw/x/j1".into()),
                data: Some(serde_json::json!({"chars": 42})),
            }
        );
    }

    #[test]
    fn exit_zero_with_valid_failed_line_keeps_child_kind() {
        let line = r#"{"status":"failed","kind":"Encrypted","detail":"encrypt_in_trailer","vm_hwm_kb":10}"#;
        let got = classify(obs(Some(0), Some(line), None), None, 0, b"");
        assert_eq!(failed(&got).0, Kind::Encrypted);
    }

    #[test]
    fn exit_zero_with_valid_quarantined_line_keeps_reasons() {
        let line = r#"{"status":"quarantined","reasons":["javascript","launch"],"vm_hwm_kb":10}"#;
        let got = classify(obs(Some(0), Some(line), None), None, 0, b"");
        assert_eq!(
            got,
            ChildOutcome::Quarantined {
                reasons: vec!["javascript".into(), "launch".into()]
            }
        );
    }

    #[test]
    fn missing_json_line_is_crashed() {
        let got = classify(obs(Some(0), None, None), None, 0, b"");
        assert_eq!(failed(&got).0, Kind::Crashed);
    }

    #[test]
    fn malformed_json_line_is_crashed() {
        for line in [
            r#"{"status":"ok","pages":"#,
            "not json",
            r#"{"status":"exploded"}"#,
        ] {
            let got = classify(obs(Some(0), Some(line), None), None, 0, b"");
            assert_eq!(failed(&got).0, Kind::Crashed, "riga {line:?}");
        }
    }

    #[test]
    fn parent_kill_timeout_is_timeout() {
        let got = classify(obs(None, None, Some(KillCause::Timeout)), Some(9), 0, b"");
        assert_eq!(failed(&got).0, Kind::Timeout);
    }

    #[test]
    fn parent_kill_output_limit_is_output_limit() {
        let got = classify(
            obs(None, None, Some(KillCause::OutputLimit)),
            Some(9),
            0,
            b"",
        );
        assert_eq!(failed(&got).0, Kind::OutputLimit);
    }

    #[test]
    fn parent_kill_out_quota_is_resource_limit_out_quota() {
        let got = classify(obs(None, None, Some(KillCause::OutQuota)), Some(9), 0, b"");
        assert_eq!(failed(&got), (Kind::ResourceLimit, "out_quota"));
    }

    #[test]
    fn parent_kill_is_evaluated_before_signal_and_oom() {
        // SIGKILL del padre con un OOM del cgroup nello stesso intervallo: vince parent_kill.
        let got = classify(obs(None, None, Some(KillCause::Timeout)), Some(9), 1, b"");
        assert_eq!(failed(&got).0, Kind::Timeout);
        let got = classify(obs(None, None, Some(KillCause::OutQuota)), Some(9), 2, b"");
        assert_eq!(failed(&got), (Kind::ResourceLimit, "out_quota"));
    }

    #[test]
    fn sigkill_with_oom_delta_is_resource_limit_cgroup_oom() {
        let got = classify(obs(None, None, None), Some(9), 1, b"");
        assert_eq!(failed(&got), (Kind::ResourceLimit, "cgroup_oom"));
    }

    #[test]
    fn sigkill_without_oom_delta_is_crashed() {
        let got = classify(obs(None, None, None), Some(9), 0, b"");
        assert_eq!(failed(&got).0, Kind::Crashed);
    }

    #[test]
    fn sigxcpu_is_timeout() {
        let got = classify(obs(None, None, None), Some(24), 0, b"");
        assert_eq!(failed(&got).0, Kind::Timeout);
    }

    #[test]
    fn sigabrt_with_rust_alloc_failure_is_resource_limit_rlimit_as() {
        let stderr = b"memory allocation of 268435456 bytes failed\n";
        let got = classify(obs(None, None, None), Some(6), 0, stderr);
        assert_eq!(failed(&got), (Kind::ResourceLimit, "rlimit_as"));
    }

    #[test]
    fn sigabrt_with_bad_alloc_is_resource_limit_rlimit_as() {
        let stderr = b"terminate called after throwing an instance of 'std::bad_alloc'\n  what():  std::bad_alloc\n";
        let got = classify(obs(None, None, None), Some(6), 0, stderr);
        assert_eq!(failed(&got), (Kind::ResourceLimit, "rlimit_as"));
    }

    #[test]
    fn sigabrt_without_alloc_marker_is_crashed() {
        let got = classify(obs(None, None, None), Some(6), 0, b"assertion failed: x\n");
        assert_eq!(failed(&got).0, Kind::Crashed);
    }

    #[test]
    fn sigsegv_is_crashed() {
        let got = classify(obs(None, None, None), Some(11), 0, b"");
        assert_eq!(failed(&got).0, Kind::Crashed);
    }
}
