use super::*;
use crate::node::guard_sources::GuardSources;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

const LIM: MemoryLimitsKib = MemoryLimitsKib {
    warn_kib: 400 * 1024,
    exit_kib: 900 * 1024,
};

#[test]
fn quiet_tick_keeps_last_report_empty() {
    let mut state = WatchState::new();
    assert_eq!(state.next_action(100 * 1024, LIM), MemoryAction::Quiet);
    assert!(state.last_report.is_none());
}

#[test]
fn report_stamps_last_report() {
    let mut state = WatchState::new();
    assert_eq!(state.next_action(410 * 1024, LIM), MemoryAction::Report);
    assert!(state.last_report.is_some());
}

#[test]
fn non_exit_action_returns() {
    exit_on_limit(MemoryAction::Report, None, 410 * 1024, LIM);
    exit_on_limit(MemoryAction::Quiet, None, 100 * 1024, LIM);
}

#[tokio::test]
async fn quiet_starts_no_summary() {
    let sum = SummaryJob {
        sources: GuardSources::default(),
        runtime: tokio::runtime::Handle::current(),
        is_summary_running: Arc::new(AtomicBool::new(false)),
        dump_dir: PathBuf::new(),
        last_dump_rss_kib: Arc::new(portable_atomic::AtomicU64::new(0)),
    };
    let mut state = WatchState::new();
    assert!(state
        .report_action(MemoryAction::Quiet, 100 * 1024, &sum)
        .is_none());
}
