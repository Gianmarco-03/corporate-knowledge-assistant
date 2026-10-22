//! Lanciatore isolato: limiti per figlio, budget aggregato, cleanup del gruppo (contratto W17, punto 3).
//!
//! [`run_child`] lancia il bin di un [`ChildSpec`] risolto da `job::resolve` in un processo
//! figlio con i limiti `setrlimit` applicati in `pre_exec`, in un gruppo di processi proprio,
//! con `oom_score_adj` 1000 e una cartella di lavoro creata in modo esclusivo. Il gruppo è
//! ucciso e raccolto, e la cartella rimossa, su ogni uscita: successo, errore, timeout,
//! eccesso di output o di quota, futuro droppato dal chiamante (`GroupGuard`).
//! [`classify`] è la funzione pura che traduce l'uscita osservata in un [`ChildOutcome`].
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::job::{ChildSpec, JobError, JobErrorCode};
use crate::protocol::{ChildOutcome, Kind};

const MIB: u64 = 1024 * 1024;

/// Tetto dello stdout del figlio: oltre, `killpg` ed esito `OutputLimit`.
pub const STDOUT_CAP_BYTES: usize = 1024 * 1024;
/// Byte di stderr trattenuti per la diagnosi; il resto si scarta contandolo.
pub const STDERR_HEAD_BYTES: usize = 4 * 1024;
/// Intervallo della scansione della quota.
pub const QUOTA_INTERVAL: Duration = Duration::from_millis(250);
/// Voci visitate al più da una scansione della quota; oltre vale come quota superata.
pub const QUOTA_MAX_ENTRIES: usize = 10_000;
/// Scadenza del reaping del gruppo dopo `killpg`.
pub const REAP_DEADLINE: Duration = Duration::from_secs(1);

/// Limiti per figlio (profilo di `limits.json`).
///
/// `address_space_bytes` è solo il tetto rigido `RLIMIT_AS` del figlio; la prenotazione nel
/// budget aggregato usa `memory_reservation_bytes` (3 × massimo `VmHWM` benigno, ripasso del
/// 2026-10-21), che non è un tetto rigido per processo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub address_space_bytes: u64,
    pub memory_reservation_bytes: u64,
    pub cpu_seconds: u64,
    pub wall_timeout: Duration,
    pub file_size_bytes: u64,
    pub max_open_files: u64,
    pub out_quota_bytes: u64,
}

/// Contatori di ammissione condivisi fra i permessi.
#[derive(Debug, Default)]
struct AdmitStats {
    active: AtomicU32,
    max: AtomicU32,
}

/// Budget aggregato di memoria dei figli ammessi.
///
/// Semaforo pesato in MiB con capacità `totale − riserva del padre`. Un job prenota
/// `memory_reservation_bytes × (1 + discendenti) + out_quota_bytes` (il tmpfs è memoria del
/// cgroup) e la rilascia solo dopo il cleanup del gruppo. La prenotazione stima la memoria
/// residente con margine 3× sul picco benigno: un figlio ostile può superarla fino al proprio
/// `RLIMIT_AS`, e allora l'OOM del cgroup sceglie un figlio (`oom_score_adj` 1000).
#[derive(Debug)]
pub struct Budget {
    sem: Arc<Semaphore>,
    total_bytes: u64,
    parent_reserve_bytes: u64,
    capacity_mib: u32,
    stats: Arc<AdmitStats>,
}

impl Budget {
    /// Totale = `memory.max` del cgroup del container; rifiuta `max`.
    pub fn from_cgroup(parent_reserve_bytes: u64) -> anyhow::Result<Budget> {
        let raw = std::fs::read_to_string("/sys/fs/cgroup/memory.max")
            .map_err(|e| anyhow::anyhow!("memory.max del cgroup non leggibile: {e}"))?;
        let raw = raw.trim();
        if raw == "max" {
            anyhow::bail!("memory.max del cgroup vale max: il budget aggregato non ha un totale");
        }
        let total: u64 = raw
            .parse()
            .map_err(|e| anyhow::anyhow!("memory.max non numerico ({raw:?}): {e}"))?;
        if total <= parent_reserve_bytes {
            anyhow::bail!(
                "memory.max {total} non supera la riserva del padre {parent_reserve_bytes}"
            );
        }
        Ok(Budget::fixed(total, parent_reserve_bytes))
    }

    /// Totale fisso, per i test.
    pub fn fixed(total_bytes: u64, parent_reserve_bytes: u64) -> Budget {
        let capacity = total_bytes.saturating_sub(parent_reserve_bytes) / MIB;
        let capacity_mib = u32::try_from(capacity).unwrap_or(u32::MAX).min(1 << 28);
        Budget {
            sem: Arc::new(Semaphore::new(capacity_mib as usize)),
            total_bytes,
            parent_reserve_bytes,
            capacity_mib,
            stats: Arc::new(AdmitStats::default()),
        }
    }

    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    pub fn parent_reserve_bytes(&self) -> u64 {
        self.parent_reserve_bytes
    }

    /// Byte prenotabili dai figli: totale − riserva, arrotondato al MiB.
    pub fn capacity_bytes(&self) -> u64 {
        u64::from(self.capacity_mib) * MIB
    }

    /// Prenotazione di un job: `memory_reservation_bytes` × (1 + discendenti) + quota
    /// dell'output.
    pub fn reservation_bytes(spec: &ChildSpec, limits: &Limits) -> u64 {
        limits
            .memory_reservation_bytes
            .saturating_mul(1 + u64::from(spec.descendants))
            .saturating_add(limits.out_quota_bytes)
    }

    /// Massimo di job ammessi insieme da quando il budget esiste.
    pub fn max_admitted(&self) -> u32 {
        self.stats.max.load(Ordering::SeqCst)
    }

    async fn admit(&self, spec: &ChildSpec, limits: &Limits) -> anyhow::Result<Admission> {
        let bytes = Budget::reservation_bytes(spec, limits);
        let mib = bytes.div_ceil(MIB);
        if mib > u64::from(self.capacity_mib) {
            return Err(JobError::new(
                JobErrorCode::BudgetExceeded,
                format!(
                    "prenotazione {mib} MiB oltre il budget {} MiB (totale − riserva)",
                    self.capacity_mib
                ),
            )
            .into());
        }
        let permit = self
            .sem
            .clone()
            .acquire_many_owned(u32::try_from(mib).unwrap_or(u32::MAX))
            .await
            .map_err(|e| anyhow::anyhow!("semaforo del budget chiuso: {e}"))?;
        let now = self.stats.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.stats.max.fetch_max(now, Ordering::SeqCst);
        Ok(Admission {
            _permit: permit,
            stats: self.stats.clone(),
        })
    }
}

/// Permesso del budget: rilasciato quando la guardia del gruppo lo lascia andare.
struct Admission {
    _permit: OwnedSemaphorePermit,
    stats: Arc<AdmitStats>,
}

impl Drop for Admission {
    fn drop(&mut self) {
        self.stats.active.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Rapporto del padre, una riga JSON per job.
#[derive(Debug, Clone, Serialize)]
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
///
/// Errori: `JobError` (`BudgetExceeded`, `PathNotAllowed` con detail `symlink`,
/// `out_dir_exists`, …) quando non parte nulla; un errore di spawn (per esempio una
/// `setrlimit` non applicabile) con l'errno nel messaggio. Ogni esito del figlio, anche un
/// fallimento, è un `RunReport`.
pub async fn run_child(
    spec: &ChildSpec,
    limits: &Limits,
    budget: &Budget,
) -> anyhow::Result<RunReport> {
    if !spec.untampered() {
        anyhow::bail!(
            "ChildSpec di job_id={} modificato dopo resolve: rifiutato",
            spec.job_id
        );
    }
    let admission = budget.admit(spec, limits).await?;
    imp::run(spec, limits, admission).await
}

/// Classificazione pura dell'uscita del figlio.
///
/// Regole, nell'ordine: `parent_kill` prima di tutto (`Timeout` → `Timeout`,
/// `OutputLimit` → `OutputLimit`, `OutQuota` → `ResourceLimit/out_quota`); SIGKILL con
/// `oom_delta` > 0 → `ResourceLimit/cgroup_oom`, senza → `Crashed`; SIGXCPU → `Timeout`;
/// SIGXFSZ → `ResourceLimit/rlimit_fsize`; SIGABRT con un marcatore di allocazione fallita
/// nello stderr → `ResourceLimit/rlimit_as`; ogni altro segnale → `Crashed`; exit 0 con una
/// riga JSON valida → l'esito del figlio; altrimenti `Crashed`.
pub fn classify(
    exit: ExitObservation,
    signal: Option<i32>,
    oom_delta: u64,
    stderr_head: &[u8],
) -> ChildOutcome {
    if let Some(cause) = exit.parent_kill {
        return match cause {
            KillCause::Timeout => ChildOutcome::failed(Kind::Timeout, "wall_timeout"),
            KillCause::OutputLimit => ChildOutcome::failed(Kind::OutputLimit, "stdout_cap"),
            KillCause::OutQuota => ChildOutcome::failed(Kind::ResourceLimit, "out_quota"),
        };
    }
    if let Some(sig) = signal {
        return match sig {
            9 if oom_delta > 0 => ChildOutcome::failed(Kind::ResourceLimit, "cgroup_oom"),
            24 => ChildOutcome::failed(Kind::Timeout, "rlimit_cpu"),
            25 => ChildOutcome::failed(Kind::ResourceLimit, "rlimit_fsize"),
            6 if contains(stderr_head, b"memory allocation of")
                || contains(stderr_head, b"bad_alloc") =>
            {
                ChildOutcome::failed(Kind::ResourceLimit, "rlimit_as")
            }
            s => ChildOutcome::failed(Kind::Crashed, format!("signal {s}")),
        };
    }
    match exit.code {
        Some(0) => exit
            .stdout_line
            .as_deref()
            .and_then(crate::protocol::parse_line)
            .map(|(o, _)| o)
            .unwrap_or_else(|| ChildOutcome::failed(Kind::Crashed, "missing_or_malformed_line")),
        Some(c) => ChildOutcome::failed(Kind::Crashed, format!("exit {c}")),
        None => ChildOutcome::failed(Kind::Crashed, "no_exit_status"),
    }
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// Prima riga non vuota dello stdout del figlio.
#[cfg_attr(not(unix), allow(dead_code))]
fn first_line(stdout: &[u8]) -> Option<String> {
    String::from_utf8_lossy(stdout)
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_owned)
}

#[cfg(not(unix))]
mod imp {
    use super::*;

    pub(super) async fn run(
        spec: &ChildSpec,
        _limits: &Limits,
        _admission: Admission,
    ) -> anyhow::Result<RunReport> {
        anyhow::bail!(
            "run_child di job_id={}: lanciatore isolato solo su Linux (worker o runner)",
            spec.job_id
        )
    }
}

#[cfg(unix)]
mod imp {
    use super::*;

    use std::io;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::process::ExitStatusExt;
    use std::path::{Path, PathBuf};
    use std::process::Stdio;

    use anyhow::Context;
    use nix::errno::Errno;
    use nix::sys::signal::{Signal, killpg};
    use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
    use nix::unistd::Pid;
    use rlimit::Resource;
    use tokio::io::{AsyncRead, AsyncReadExt};
    use tokio::process::Command;
    use tokio::runtime::Handle;
    use tokio::sync::oneshot;
    use tokio::time::{Instant, MissedTickBehavior};

    use crate::job::{ChildBin, Sealed, ancestors_from_root};

    /// `Command` del bin del registro chiuso: nome letterale per variante, cercato solo
    /// nella cartella dei bin ammessi (`PATH` del figlio = `S02_BIN_DIR`, da `resolve`).
    fn command_for(bin: ChildBin) -> Command {
        match bin {
            ChildBin::ParseChild => Command::new("parse-child"),
            ChildBin::Extract => Command::new("extract"),
            ChildBin::Locator => Command::new("locator"),
            ChildBin::Ocr => Command::new("ocr"),
            ChildBin::FakeChild => Command::new("fake-child"),
        }
    }

    /// Limiti da applicare nel figlio, copiati per valore nella closure di `pre_exec`.
    #[derive(Clone, Copy)]
    struct Rlimits {
        address_space: u64,
        cpu: u64,
        file_size: u64,
        open_files: u64,
    }

    /// Marcatore che il figlio scrive sul proprio stderr se non può alzare `oom_score_adj`.
    const OOM_ADJ_MARKER: &[u8] = b"oom_adj_failed errno=";
    /// Uscita del figlio che non ha potuto alzare `oom_score_adj` (non arriva mai all'exec).
    const OOM_ADJ_EXIT: i32 = 125;

    /// errno dell'ultima syscall, senza allocare.
    fn last_errno() -> i32 {
        io::Error::last_os_error().raw_os_error().unwrap_or(0)
    }

    /// Scrive `1000` in `/proc/self/oom_score_adj` (alzarlo non richiede capability).
    fn raise_oom_score_adj() -> Result<(), i32> {
        // SAFETY: open/write/close su un percorso C statico e un buffer statico: syscall
        // async-signal-safe, nessuna allocazione, nessun lock.
        unsafe {
            let fd = nix::libc::open(
                c"/proc/self/oom_score_adj".as_ptr(),
                nix::libc::O_WRONLY | nix::libc::O_CLOEXEC,
            );
            if fd < 0 {
                return Err(last_errno());
            }
            let n = nix::libc::write(fd, b"1000".as_ptr().cast(), 4);
            let res = if n == 4 { Ok(()) } else { Err(last_errno()) };
            nix::libc::close(fd);
            res
        }
    }

    /// `oom_adj_failed errno=<n>` sullo stderr del figlio (già la pipe del padre), formattato
    /// in un buffer sullo stack.
    fn report_oom_failure(errno: i32) {
        let mut buf = [0u8; 40];
        buf[..OOM_ADJ_MARKER.len()].copy_from_slice(OOM_ADJ_MARKER);
        let mut n = OOM_ADJ_MARKER.len();
        let mut digits = [0u8; 10];
        let (mut d, mut v) = (0, errno.unsigned_abs());
        loop {
            digits[d] = b'0' + (v % 10) as u8;
            d += 1;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        while d > 0 {
            d -= 1;
            buf[n] = digits[d];
            n += 1;
        }
        buf[n] = b'\n';
        n += 1;
        // SAFETY: write(2) su un buffer dello stack, async-signal-safe.
        unsafe {
            nix::libc::write(2, buf.as_ptr().cast(), n);
        }
    }

    /// errno riportato dal figlio con [`OOM_ADJ_MARKER`], se lo stderr inizia con esso.
    fn oom_failure_errno(stderr_head: &[u8]) -> Option<i32> {
        let rest = stderr_head.strip_prefix(OOM_ADJ_MARKER)?;
        let digits: Vec<u8> = rest
            .iter()
            .copied()
            .take_while(u8::is_ascii_digit)
            .collect();
        std::str::from_utf8(&digits).ok()?.parse().ok()
    }

    /// Eseguita nel figlio fra `fork` ed `exec`, senza allocazioni né lock:
    /// 1. `oom_score_adj` = 1000, prima che il bin del job esegua una sola istruzione (scriverlo
    ///    dal padre dopo lo spawn perde i figli brevi: un figlio già uscito è uno zombie con la
    ///    voce di `/proc` di root e la scrittura dà `EACCES`). Se fallisce, il figlio scrive il
    ///    marcatore con l'errno sullo stderr ed esce con `_exit` senza mai fare l'exec: il padre
    ///    lo legge, mette `oom_adj_set=false` e chiude il job come `oom_adj_unavailable`;
    /// 2. solo `setrlimit` per AS, CPU, FSIZE, NOFILE, CORE: un limite non applicabile fa
    ///    fallire lo spawn con l'errno, quindi nessun figlio parte senza limiti.
    fn child_setup(r: Rlimits) -> io::Result<()> {
        if let Err(errno) = raise_oom_score_adj() {
            report_oom_failure(errno);
            // SAFETY: _exit è async-signal-safe e non esegue distruttori del processo padre.
            unsafe { nix::libc::_exit(OOM_ADJ_EXIT) };
        }
        apply_rlimits(r)
    }

    /// `setrlimit` dei limiti del job; il primo che fallisce torna con il suo errno.
    fn apply_rlimits(r: Rlimits) -> io::Result<()> {
        let table = [
            (Resource::AS, r.address_space, r.address_space),
            (Resource::CPU, r.cpu, r.cpu.saturating_add(1)),
            (Resource::FSIZE, r.file_size, r.file_size),
            (Resource::NOFILE, r.open_files, r.open_files),
            (Resource::CORE, 0, 0),
        ];
        for (resource, soft, hard) in table {
            if rlimit::setrlimit(resource, soft, hard).is_err() {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    /// Guardia del gruppo del job: su ogni uscita non conclusa da `finish` uccide il gruppo,
    /// lo raccoglie, rimuove la cartella del job e solo allora rilascia il permesso del budget.
    struct GroupGuard {
        pgid: Option<Pid>,
        job_dir: Option<PathBuf>,
        permit: Option<Admission>,
        armed: bool,
    }

    impl GroupGuard {
        /// Cleanup del percorso normale già fatto: rilascia il permesso, niente altro.
        fn finish(mut self) {
            self.armed = false;
            self.permit.take();
        }
    }

    impl Drop for GroupGuard {
        fn drop(&mut self) {
            if !self.armed {
                return;
            }
            if let Some(pgid) = self.pgid {
                let _ = killpg(pgid, Signal::SIGKILL);
            }
            let pgid = self.pgid;
            let job_dir = self.job_dir.take();
            let permit = self.permit.take();
            match Handle::try_current() {
                Ok(handle) => {
                    handle.spawn(async move {
                        if let Some(p) = pgid {
                            reap_group(p).await;
                        }
                        if let Some(d) = job_dir {
                            let _ = tokio::fs::remove_dir_all(&d).await;
                        }
                        drop(permit);
                    });
                }
                Err(_) => {
                    // Spegnimento del processo: reaping best-effort senza attese.
                    if let Some(p) = pgid {
                        for _ in 0..64 {
                            match waitpid(Pid::from_raw(-p.as_raw()), Some(WaitPidFlag::WNOHANG)) {
                                Ok(WaitStatus::StillAlive) | Err(_) => break,
                                Ok(_) => {}
                            }
                        }
                    }
                    if let Some(d) = job_dir {
                        let _ = std::fs::remove_dir_all(d);
                    }
                    drop(permit);
                }
            }
        }
    }

    /// Raccoglie i processi del gruppo (i nipoti orfani tornano al padre subreaper) fino a
    /// `ECHILD`, con `tokio::time::sleep` fra i tentativi e scadenza di 1 s.
    async fn reap_group(pgid: Pid) {
        let end = Instant::now() + REAP_DEADLINE;
        let group = Pid::from_raw(-pgid.as_raw());
        loop {
            match waitpid(group, Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::StillAlive) => {
                    if Instant::now() >= end {
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Ok(_) | Err(Errno::EINTR) => {}
                Err(_) => return,
            }
        }
    }

    fn path_err(detail: &str) -> anyhow::Error {
        JobError::new(JobErrorCode::PathNotAllowed, detail).into()
    }

    /// Nessun componente di `p`, dalla radice, è un symlink.
    fn no_symlink_components(p: &Path) -> anyhow::Result<()> {
        for c in ancestors_from_root(p) {
            match std::fs::symlink_metadata(&c) {
                Ok(m) if m.file_type().is_symlink() => return Err(path_err("symlink")),
                Ok(_) => {}
                Err(_) => return Err(path_err("symlink")),
            }
        }
        Ok(())
    }

    /// Controlli `lstat` subito prima dello spawn e creazione esclusiva di `out_dir` e
    /// cartella del job. Su errore non resta nulla di creato qui.
    fn prepare(s: &Sealed) -> anyhow::Result<()> {
        for (path, identity) in &s.checked {
            no_symlink_components(path)?;
            let m = std::fs::metadata(path).map_err(|_| path_err("symlink"))?;
            if identity.is_some_and(|id| id != (m.dev(), m.ino())) {
                return Err(path_err("symlink"));
            }
        }
        if !std::fs::metadata(&s.bin).is_ok_and(|m| m.is_file()) {
            return Err(
                JobError::new(JobErrorCode::ChildMissing, "bin assente prima dello spawn").into(),
            );
        }

        let mut created_out: Option<&Path> = None;
        if let Some((area_dir, out_dir)) = &s.out {
            no_symlink_components(&s.results_raw)?;
            match std::fs::symlink_metadata(area_dir) {
                Ok(m) if m.file_type().is_symlink() => return Err(path_err("symlink")),
                Ok(m) if !m.is_dir() => return Err(path_err("out_dir_area_not_dir")),
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    match std::fs::create_dir(area_dir) {
                        Ok(()) => {}
                        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                        Err(e) => return Err(e).context("creazione dell'area di out_dir"),
                    }
                    let m = std::fs::symlink_metadata(area_dir)?;
                    if m.file_type().is_symlink() || !m.is_dir() {
                        return Err(path_err("symlink"));
                    }
                }
                Err(e) => return Err(e).context("lstat dell'area di out_dir"),
            }
            match std::fs::create_dir(out_dir) {
                Ok(()) => created_out = Some(out_dir),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    return Err(path_err("out_dir_exists"));
                }
                Err(e) => return Err(e).context("creazione esclusiva di out_dir"),
            }
        }

        let job_root = s.job_dir.parent().unwrap_or(Path::new("/"));
        let made = std::fs::create_dir_all(job_root)
            .context("radice dei job")
            .and_then(|()| match std::fs::create_dir(&s.job_dir) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    Err(path_err("job_dir_exists"))
                }
                Err(e) => Err(e).context("creazione esclusiva della cartella del job"),
            });
        if let Err(e) = made {
            if let Some(o) = created_out {
                let _ = std::fs::remove_dir(o);
            }
            return Err(e);
        }
        Ok(())
    }

    /// Somma dei byte di cartella del job + `out_dir` (nessun symlink seguito), voci visitate
    /// e `VmHWM` del figlio campionato nello stesso giro.
    struct Usage {
        bytes: u64,
        over_entries: bool,
        child_hwm_kb: Option<u64>,
    }

    fn scan_usage(dirs: &[PathBuf], pid: u32) -> Usage {
        let mut bytes = 0u64;
        let mut entries = 0usize;
        let mut stack: Vec<PathBuf> = dirs.to_vec();
        'outer: while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in rd.flatten() {
                entries += 1;
                if entries > QUOTA_MAX_ENTRIES {
                    break 'outer;
                }
                let Ok(m) = std::fs::symlink_metadata(entry.path()) else {
                    continue;
                };
                if m.is_dir() {
                    stack.push(entry.path());
                } else {
                    bytes = bytes.saturating_add(m.len());
                }
            }
        }
        Usage {
            bytes,
            over_entries: entries > QUOTA_MAX_ENTRIES,
            child_hwm_kb: crate::protocol::proc_status_kb(&format!("/proc/{pid}/status"), "VmHWM:"),
        }
    }

    /// Contatore `oom_kill` di `memory.events` del cgroup del container.
    async fn oom_kills() -> Option<u64> {
        let text = tokio::fs::read_to_string("/sys/fs/cgroup/memory.events")
            .await
            .ok()?;
        text.lines()
            .find_map(|l| l.strip_prefix("oom_kill "))
            .and_then(|v| v.trim().parse().ok())
    }

    /// stdout fino al tetto; oltre, segnala `OutputLimit` e smette di leggere.
    async fn read_stdout<R: AsyncRead + Unpin>(mut r: R, overflow: oneshot::Sender<()>) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4096);
        let mut chunk = [0u8; 8192];
        loop {
            match r.read(&mut chunk).await {
                Ok(0) | Err(_) => return buf,
                Ok(n) => {
                    if buf.len() + n > STDOUT_CAP_BYTES {
                        let _ = overflow.send(());
                        return buf;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
            }
        }
    }

    /// stderr: trattiene i primi 4 KiB e scarta il resto contandolo, senza mai bloccare
    /// lo scrittore.
    async fn read_stderr<R: AsyncRead + Unpin>(mut r: R) -> (Vec<u8>, u64) {
        let mut head = Vec::with_capacity(STDERR_HEAD_BYTES);
        let mut dropped = 0u64;
        let mut chunk = [0u8; 8192];
        loop {
            match r.read(&mut chunk).await {
                Ok(0) | Err(_) => return (head, dropped),
                Ok(n) => {
                    let room = STDERR_HEAD_BYTES - head.len();
                    let keep = room.min(n);
                    head.extend_from_slice(&chunk[..keep]);
                    dropped += (n - keep) as u64;
                }
            }
        }
    }

    pub(super) async fn run(
        spec: &ChildSpec,
        limits: &Limits,
        admission: Admission,
    ) -> anyhow::Result<RunReport> {
        let s = spec.sealed.clone();
        let job_id = s.job_id.clone();
        {
            let s = s.clone();
            tokio::task::spawn_blocking(move || prepare(&s))
                .await
                .context("controlli prima dello spawn")??;
        }
        // Da qui la cartella del job esiste: la guardia la rimuove su ogni uscita. È
        // dichiarata prima del figlio, così un drop uccide prima il figlio diretto
        // (kill_on_drop) e poi il gruppo.
        let mut guard = GroupGuard {
            pgid: None,
            job_dir: Some(s.job_dir.clone()),
            permit: Some(admission),
            armed: true,
        };
        let oom_before = oom_kills().await;

        let mut cmd = command_for(s.bin_kind);
        cmd.args(&s.args)
            .env_clear()
            .envs(s.env.iter().map(|(k, v)| (k, v)))
            .current_dir(&s.job_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .process_group(0);
        let rl = Rlimits {
            address_space: limits.address_space_bytes,
            cpu: limits.cpu_seconds,
            file_size: limits.file_size_bytes,
            open_files: limits.max_open_files,
        };
        // SAFETY: la closure gira nel figlio fra fork ed exec e fa solo syscall
        // async-signal-safe (open/write/close, setrlimit, _exit) su valori copiati e buffer
        // statici o sullo stack; non alloca e non prende lock.
        unsafe {
            cmd.pre_exec(move || child_setup(rl));
        }

        let started = Instant::now();
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                if let Some((_, o)) = &s.out {
                    let _ = tokio::fs::remove_dir(o).await;
                }
                let errno = e.raw_os_error().unwrap_or(0);
                return Err(anyhow::Error::new(e).context(format!(
                    "spawn del figlio fallito, nessun processo senza limiti: job_id={job_id} errno={errno}"
                )));
            }
        };
        let pid = child
            .id()
            .context("pid del figlio appena lanciato non disponibile")?;
        let pgid = Pid::from_raw(i32::try_from(pid).context("pid fuori da i32")?);
        guard.pgid = Some(pgid);

        let mut parent_kill: Option<KillCause> = None;
        let kill_group = |cause: KillCause, slot: &mut Option<KillCause>| {
            if slot.is_none() {
                *slot = Some(cause);
            }
            let _ = killpg(pgid, Signal::SIGKILL);
        };

        // Il figlio deve essere la vittima di un OOM del cgroup, non il padre.
        // oom_score_adj l'ha già alzato il figlio prima dell'exec (`child_setup`); un
        // fallimento si riconosce dal marcatore sul suo stderr, a figlio uscito.
        let mut oom_adj_set = true;

        let (overflow_tx, mut overflow_rx) = oneshot::channel();
        let stdout_task = child
            .stdout
            .take()
            .map(|o| tokio::spawn(read_stdout(o, overflow_tx)));
        let stderr_task = child.stderr.take().map(|e| tokio::spawn(read_stderr(e)));

        let mut dirs = vec![s.job_dir.clone()];
        if let Some((_, o)) = &s.out {
            dirs.push(o.clone());
        }
        let deadline = tokio::time::sleep(limits.wall_timeout);
        tokio::pin!(deadline);
        let mut tick = tokio::time::interval(QUOTA_INTERVAL);
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut scan: Option<tokio::task::JoinHandle<Usage>> = None;
        let mut overflow_open = true;
        let mut hwm_sample: Option<u64> = None;

        let status = loop {
            tokio::select! {
                st = child.wait() => break st.context("attesa del figlio")?,
                r = &mut overflow_rx, if overflow_open => {
                    overflow_open = false;
                    if r.is_ok() {
                        kill_group(KillCause::OutputLimit, &mut parent_kill);
                    }
                }
                () = &mut deadline, if parent_kill.is_none() => {
                    kill_group(KillCause::Timeout, &mut parent_kill);
                }
                _ = tick.tick(), if scan.is_none() && parent_kill.is_none() => {
                    let d = dirs.clone();
                    scan = Some(tokio::task::spawn_blocking(move || scan_usage(&d, pid)));
                }
                u = async {
                    match scan.as_mut() {
                        Some(h) => h.await,
                        None => std::future::pending().await,
                    }
                }, if scan.is_some() => {
                    scan = None;
                    if let Ok(u) = u {
                        hwm_sample = u.child_hwm_kb.or(hwm_sample);
                        if parent_kill.is_none()
                            && (u.over_entries || u.bytes > limits.out_quota_bytes)
                        {
                            kill_group(KillCause::OutQuota, &mut parent_kill);
                        }
                    }
                }
            }
        };
        let wall_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

        // Figlio diretto raccolto: uccide e raccoglie il resto del gruppo.
        let _ = killpg(pgid, Signal::SIGKILL);
        reap_group(pgid).await;
        let leftover_procs = u32::from(killpg(pgid, None).is_ok());

        // Ultima scansione a gruppo fermo: un figlio che scrive oltre la quota e termina fra
        // due giri dell'intervallo non passa per riuscito.
        if parent_kill.is_none() && oom_adj_set {
            let d = dirs.clone();
            if let Ok(u) = tokio::task::spawn_blocking(move || scan_usage(&d, pid)).await
                && (u.over_entries || u.bytes > limits.out_quota_bytes)
            {
                parent_kill = Some(KillCause::OutQuota);
            }
        }

        let stdout = match stdout_task {
            Some(t) => join_or_abort(t).await.unwrap_or_default(),
            None => Vec::new(),
        };
        let (stderr_head, _dropped) = match stderr_task {
            Some(t) => join_or_abort(t).await.unwrap_or_default(),
            None => (Vec::new(), 0),
        };
        if let Some(errno) = oom_failure_errno(&stderr_head) {
            eprintln!("oom_adj_failed job_id={job_id} errno={errno}");
            oom_adj_set = false;
        }
        let oom_after = oom_kills().await;
        let oom_kill_delta = match (oom_before, oom_after) {
            (Some(b), Some(a)) => a.saturating_sub(b),
            _ => 0,
        };

        if let Err(e) = tokio::fs::remove_dir_all(&s.job_dir).await
            && e.kind() != io::ErrorKind::NotFound
        {
            eprintln!("job_dir_cleanup_failed job_id={job_id} error={e}");
        }
        guard.finish();

        let stdout_line = first_line(&stdout);
        let child_hwm = stdout_line
            .as_deref()
            .and_then(crate::protocol::parse_line)
            .and_then(|(_, h)| h);
        let signal = status.signal();
        let outcome = if oom_adj_set {
            classify(
                ExitObservation {
                    code: status.code(),
                    stdout_line,
                    parent_kill,
                },
                signal,
                oom_kill_delta,
                &stderr_head,
            )
        } else {
            ChildOutcome::failed(Kind::ResourceLimit, "oom_adj_unavailable")
        };
        Ok(RunReport {
            job_id,
            outcome,
            exit_signal: signal,
            peak_rss_kb: child_hwm.or(hwm_sample),
            wall_ms,
            oom_kill_delta,
            leftover_procs,
            oom_adj_set,
        })
    }

    /// Attende un task di lettura al più 1 s (le pipe si chiudono con il gruppo ucciso).
    async fn join_or_abort<T>(t: tokio::task::JoinHandle<T>) -> Option<T> {
        let abort = t.abort_handle();
        match tokio::time::timeout(REAP_DEADLINE, t).await {
            Ok(Ok(v)) => Some(v),
            _ => {
                abort.abort();
                None
            }
        }
    }
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
