//! The `mem-guard` thread loop: measure, decide, print.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{
    decide_memory_action, exit_text, stalled_text, MemoryAction, MemoryLimitsKib, Stopper,
    SummaryJob, EXIT_CODE, EXIT_WAIT,
};
use crate::node::output::say;
use crate::node::telemetry;

/// Thread loop: measure, decide, print. Takes nobody's locks: only `rss_kib`,
/// `decide_memory_action`, `say` and `exit`; the summary runs in a separate thread.
pub(super) fn watch(limits: MemoryLimitsKib, sum: &SummaryJob, stop: &Stopper) {
    let mut state = WatchState::new();
    while stop.sleep_tick_unless_stopped() {
        state.tick(limits, sum);
    }
}

/// What the loop remembers between ticks.
struct WatchState {
    prev_rss_kib: u64,
    last_report: Option<Instant>,
    summary_started_at: Instant,
}

impl WatchState {
    fn new() -> Self {
        Self {
            prev_rss_kib: 0,
            last_report: None,
            summary_started_at: Instant::now(),
        }
    }

    fn tick(&mut self, limits: MemoryLimitsKib, sum: &SummaryJob) {
        let rss_kib = telemetry::rss_kib().0;
        let action = self.next_action(rss_kib, limits);
        let summary_done_rx = self.report_action(action, rss_kib, sum);
        exit_on_limit(action, summary_done_rx, rss_kib, limits);
        self.prev_rss_kib = rss_kib;
    }

    /// The decision for this measurement; a non-quiet one stamps the report time.
    fn next_action(&mut self, rss_kib: u64, limits: MemoryLimitsKib) -> MemoryAction {
        let since = self.last_report.map_or(Duration::MAX, |t| t.elapsed());
        let action = decide_memory_action(rss_kib, self.prev_rss_kib, limits, since);
        if action != MemoryAction::Quiet {
            self.last_report = Some(Instant::now());
        }
        action
    }

    /// Starts the summary for a non-quiet action; says so when the previous one still runs.
    fn report_action(
        &mut self,
        action: MemoryAction,
        rss_kib: u64,
        sum: &SummaryJob,
    ) -> Option<mpsc::Receiver<()>> {
        if action == MemoryAction::Quiet {
            return None;
        }
        let started =
            sum.start_summary_thread(rss_kib, self.prev_rss_kib, action == MemoryAction::Report);
        if started.is_some() {
            self.summary_started_at = Instant::now();
        } else {
            let hanging_secs = self.summary_started_at.elapsed().as_secs();
            say(
                "guard",
                stalled_text(rss_kib, self.prev_rss_kib, hanging_secs),
            );
        }
        started
    }
}

/// At the second limit: waits a little for the summary and exits the process.
fn exit_on_limit(
    action: MemoryAction,
    summary_done_rx: Option<mpsc::Receiver<()>>,
    rss_kib: u64,
    limits: MemoryLimitsKib,
) {
    if action != MemoryAction::Exit {
        return;
    }
    if let Some(rx) = summary_done_rx {
        let _ = rx.recv_timeout(EXIT_WAIT);
    }
    say("guard", exit_text(rss_kib, limits));
    std::process::exit(EXIT_CODE);
}

#[cfg(test)]
#[path = "guard_watch_tests.rs"]
mod tests;
