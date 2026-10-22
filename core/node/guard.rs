//! The node's memory guard: a separate OS thread `mem-guard` measures the process RSS once a second.
//!
//! Why: an exit node can grow by hundreds of MB in a minute and die of OOM, while `[status]`
//! is printed every 60 s and doesn't catch this. An OS thread, not a tokio task: when the runtime chokes it
//! keeps working. Above threshold 1 it prints a `[guard]` summary (what holds the memory), above
//! threshold 2 — the summary once and `exit(3)`, before the computer goes into swap.
//! The `GuardHandle` handle lives in `RunningNode`: `shutdown` stops the thread, the tick wait is interrupted at once.
//! Links: `Members` (connections and paths), `Checks` (running checks), `telemetry::rss_kib`.

use portable_atomic::AtomicU64;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use tokio::runtime::Handle;

use crate::members::membership::Members;
use crate::node::alloc::{heap_prof_dump, heap_prof_state};
use crate::node::output::{say, short_peer_id};
use crate::node::telemetry;
use crate::site_checks::checks::Checks;
use crate::transport::net::PeerConn;

/// Measurement period.
const TICK: Duration = Duration::from_secs(1);
/// How often to repeat the summary while memory is above threshold 1.
const REPEAT: Duration = Duration::from_secs(10);
/// How long to wait for the summary before exiting.
const EXIT_WAIT: Duration = Duration::from_secs(2);
/// Step for a repeated heap profile dump, KiB of RSS since the previous dump.
const DUMP_STEP_KIB: u64 = 200 * 1024;
/// Exit code when threshold 2 is exceeded.
const EXIT_CODE: i32 = 3;

/// Thresholds in KiB; `0` — the threshold is off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MemoryLimitsKib {
    pub warn_kib: u64,
    pub exit_kib: u64,
}

impl MemoryLimitsKib {
    /// Thresholds from the settings in MiB.
    pub fn from_mib(warn_mib: u64, exit_mib: u64) -> MemoryLimitsKib {
        MemoryLimitsKib {
            warn_kib: warn_mib * 1024,
            exit_kib: exit_mib * 1024,
        }
    }
}

/// What to do given the measurement result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MemoryAction {
    Quiet,
    Report,
    Exit,
}

/// Decision for the current RSS. `prev_rss_kib` is the previous measurement (0 — none), `since_report` is how much
/// time has passed since the previous summary (`Duration::MAX` — no summaries yet). RSS `0` — no data.
pub(crate) fn decide_memory_action(
    rss_kib: u64,
    prev_rss_kib: u64,
    limits: MemoryLimitsKib,
    since_report: Duration,
) -> MemoryAction {
    if rss_kib == 0 {
        return MemoryAction::Quiet;
    }
    if limits.exit_kib > 0 && rss_kib >= limits.exit_kib {
        return MemoryAction::Exit;
    }
    let above = limits.warn_kib > 0 && rss_kib >= limits.warn_kib;
    let crossed = limits.warn_kib > 0 && prev_rss_kib < limits.warn_kib;
    match (above, crossed || since_report >= REPEAT) {
        (true, true) => MemoryAction::Report,
        _ => MemoryAction::Quiet,
    }
}

/// Whether a heap profile dump is needed: `last_dump_rss_kib` is the RSS of the previous dump (`0` — no dumps yet); the first
/// summary gets a dump, then one for every `DUMP_STEP_KIB` of growth since the previous.
pub(crate) fn dump_due(rss_kib: u64, last_dump_rss_kib: u64) -> bool {
    last_dump_rss_kib == 0 || rss_kib >= last_dump_rss_kib + DUMP_STEP_KIB
}

/// Dump path: `<directory>/heap-<unix-seconds>-<rss_mib>.prof`.
pub(crate) fn dump_path(dir: &std::path::Path, unix_secs: u64, rss_kib: u64) -> PathBuf {
    dir.join(format!("heap-{unix_secs}-{}.prof", rss_kib / 1024))
}

/// Summary header: memory, growth per second, runtime load, checks, connections.
pub(crate) fn header_text(
    rss_kib: u64,
    prev_rss_kib: u64,
    tasks: usize,
    workers: usize,
    checks: usize,
    conns: &str,
) -> String {
    let growth = rss_kib as i64 - prev_rss_kib as i64;
    format!("memory {rss_kib} KiB ({growth:+} KiB/s), runtime tasks {tasks}, runtime threads {workers}, checks {checks}, {conns}")
}

/// Exit line.
pub(crate) fn exit_text(rss_kib: u64, limits: MemoryLimitsKib) -> String {
    format!(
        "memory {rss_kib} KiB ≥ {} MiB — exit (code {EXIT_CODE})",
        limits.exit_kib / 1024
    )
}

/// Line instead of the summary when the previous one is still hanging (no locks: written from the guard thread).
pub(crate) fn stalled_text(rss_kib: u64, prev_rss_kib: u64, hanging_secs: u64) -> String {
    let growth = rss_kib as i64 - prev_rss_kib as i64;
    format!("memory {rss_kib} KiB ({growth:+} KiB/s), the previous summary didn't finish in {hanging_secs} s")
}

/// Everything the summary needs; the `mem-guard` thread doesn't get it — it holds only the measurement.
#[derive(Clone)]
struct SummaryJob {
    members: Members,
    checks: Checks,
    runtime: Handle,
    is_summary_running: Arc<AtomicBool>,
    /// Directory of heap profile dumps.
    dump_dir: PathBuf,
    /// `0` — no dumps yet.
    last_dump_rss_kib: Arc<AtomicU64>,
}

impl SummaryJob {
    /// Starts the summary in the `mem-guard-report` thread; `None` — the previous one is still running (or the thread
    /// didn't start). The receiver comes alive when the summary is printed.
    fn start_summary_thread(
        &self,
        rss_kib: u64,
        prev_rss_kib: u64,
        dump: bool,
    ) -> Option<mpsc::Receiver<()>> {
        if self.is_summary_running.swap(true, Ordering::SeqCst) {
            return None;
        }
        let (tx, rx) = mpsc::channel();
        let me = self.clone();
        let run = move || {
            // The dump goes before the summary: the summary takes connection locks and may hang, the dump can't.
            if dump {
                me.heap_dump(rss_kib);
            }
            print_summary(rss_kib, prev_rss_kib, &me.members, &me.checks, &me.runtime);
            me.is_summary_running.store(false, Ordering::SeqCst);
            let _ = tx.send(());
        };
        match std::thread::Builder::new()
            .name("mem-guard-report".into())
            .spawn(run)
        {
            Ok(_) => Some(rx),
            Err(e) => {
                self.is_summary_running.store(false, Ordering::SeqCst);
                say("guard", format!("summary thread not started: {e}"));
                None
            }
        }
    }
}

impl SummaryJob {
    /// Heap profile dump before the summary, if it's time (`dump_due`); in the summary thread, not the measurement one.
    fn heap_dump(&self, rss_kib: u64) {
        if !dump_due(rss_kib, self.last_dump_rss_kib.load(Ordering::SeqCst)) {
            return;
        }
        self.last_dump_rss_kib.store(rss_kib, Ordering::SeqCst);
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let path = dump_path(&self.dump_dir, secs, rss_kib);
        let shown = path.display();
        match heap_prof_dump(&path.to_string_lossy()) {
            Ok(()) => say("guard", format!("heap profile written: {shown}")),
            Err(e) => say("guard", format!("heap profile unavailable: {e}")),
        }
    }
}

/// Handle of the `mem-guard` thread: on `stop` the thread exits at once, without waiting for the tick to end.
#[derive(Clone)]
pub(crate) struct GuardHandle {
    stop: mpsc::Sender<()>,
    /// Liveness token of the thread: while the thread is alive, `strong_count > 0`.
    #[cfg(test)]
    alive: std::sync::Weak<()>,
}

impl GuardHandle {
    /// Asks the thread to exit; repeating it and calling from a copy are harmless.
    pub(crate) fn stop(&self) {
        let _ = self.stop.send(());
    }

    /// The thread has already exited (tests only).
    #[cfg(test)]
    pub(crate) fn is_finished(&self) -> bool {
        self.alive.strong_count() == 0
    }
}

/// Receiving side of the stop: replaces `sleep(TICK)` in the thread loop.
struct Stopper(mpsc::Receiver<()>);

impl Stopper {
    /// Waits for a tick; `true` — keep measuring, `false` — a command came or all handles are dropped
    /// (the node was taken apart without `shutdown`, nobody left to guard).
    fn sleep_tick_unless_stopped(&self) -> bool {
        matches!(self.0.recv_timeout(TICK), Err(RecvTimeoutError::Timeout))
    }
}

/// Starts the `mem-guard` thread with a stop channel; `None` — the thread didn't start.
fn start_thread(run: impl FnOnce(Stopper) + Send + 'static) -> Option<GuardHandle> {
    let (tx, rx) = mpsc::channel();
    let token = Arc::new(());
    #[cfg(test)]
    let alive = Arc::downgrade(&token);
    let body = move || {
        let _alive = token;
        run(Stopper(rx));
    };
    match std::thread::Builder::new()
        .name("mem-guard".into())
        .spawn(body)
    {
        Ok(_) => Some(GuardHandle {
            stop: tx,
            #[cfg(test)]
            alive,
        }),
        Err(e) => {
            say("guard", format!("guard thread not started: {e}"));
            None
        }
    }
}

/// Starts the `mem-guard` thread; both thresholds `0` — doesn't start it (`None`).
pub(crate) fn start_memory_guard(
    limits: MemoryLimitsKib,
    members: Members,
    checks: Checks,
    handle: Handle,
    dump_dir: PathBuf,
) -> Option<GuardHandle> {
    if limits.warn_kib == 0 && limits.exit_kib == 0 {
        return None;
    }
    say("guard", format!("heap profile: {}", heap_prof_state()));
    let last_dump_rss_kib = Arc::new(AtomicU64::new(0));
    let sum = SummaryJob {
        members,
        checks,
        runtime: handle,
        is_summary_running: Arc::new(AtomicBool::new(false)),
        dump_dir,
        last_dump_rss_kib,
    };
    start_thread(move |stop| watch(limits, &sum, &stop))
}

/// Thread loop: measure, decide, print. Takes nobody's locks: only `rss_kib`,
/// `decide_memory_action`, `say` and `exit`; the summary runs in a separate thread.
fn watch(limits: MemoryLimitsKib, sum: &SummaryJob, stop: &Stopper) {
    let mut prev_rss_kib = 0;
    let mut last_report: Option<Instant> = None;
    let mut summary_started_at = Instant::now();
    loop {
        if !stop.sleep_tick_unless_stopped() {
            return;
        }
        let rss_kib = telemetry::rss_kib().0;
        let since = last_report.map_or(Duration::MAX, |t| t.elapsed());
        let action = decide_memory_action(rss_kib, prev_rss_kib, limits, since);
        if action != MemoryAction::Quiet {
            last_report = Some(Instant::now());
        }
        let summary_done_rx = match action {
            MemoryAction::Quiet => None,
            _ => sum.start_summary_thread(rss_kib, prev_rss_kib, action == MemoryAction::Report),
        };
        if summary_done_rx.is_some() {
            summary_started_at = Instant::now();
        } else if action != MemoryAction::Quiet {
            say(
                "guard",
                stalled_text(
                    rss_kib,
                    prev_rss_kib,
                    summary_started_at.elapsed().as_secs(),
                ),
            );
        }
        if action == MemoryAction::Exit {
            if let Some(rx) = summary_done_rx {
                let _ = rx.recv_timeout(EXIT_WAIT);
            }
            say("guard", exit_text(rss_kib, limits));
            std::process::exit(EXIT_CODE);
        }
        prev_rss_kib = rss_kib;
    }
}

/// Summary: the header and a line for every path of every member connection.
fn print_summary(
    rss_kib: u64,
    prev_rss_kib: u64,
    members: &Members,
    checks: &Checks,
    handle: &Handle,
) {
    let metrics = handle.metrics();
    let head = header_text(
        rss_kib,
        prev_rss_kib,
        metrics.num_alive_tasks(),
        metrics.num_workers(),
        checks.domains_in_progress().len(),
        &members.conns_text(),
    );
    say("guard", head);
    for conn in members.conns_snapshot() {
        peer_lines(&*conn);
    }
}

/// `[guard]` lines for every connection path; a transport without paths — no lines.
fn peer_lines(conn: &dyn PeerConn) {
    let peer = short_peer_id(&conn.peer());
    for line in conn.path_lines() {
        say("guard", format!("peer {peer}: {line}"));
    }
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
