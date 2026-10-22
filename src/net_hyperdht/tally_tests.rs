use super::*;

#[test]
fn counts_rounds_and_errors() {
    let t0 = Instant::now();
    let mut t = Tally::new(t0, Duration::from_secs(600));
    t.record_round_ok(2, t0);
    t.record_round_failure(&NetError::Timeout, t0);
    t.record_round_failure(&NetError::Timeout, t0);
    t.record_round_ok(3, t0);
    assert_eq!(t.counts(), (4, 2));
    assert!(!t.failing);
}

#[test]
fn tick_resets_summary_clock_only_after_period() {
    let t0 = Instant::now();
    let mut t = Tally::new(t0, Duration::from_secs(600));
    t.log_summary_if_due(t0 + Duration::from_secs(10));
    assert_eq!(t.last_summary, t0);
    t.log_summary_if_due(t0 + Duration::from_secs(601));
    assert_eq!(t.last_summary, t0 + Duration::from_secs(601));
}
