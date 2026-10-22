//! Unit tests of the country lookup schedule: `wait_before_check`, `pause_after_outcome`.
//! The `net_seen` tests left together with the iroh adapter.

use std::time::Duration;

use super::*;

/// Microseconds in a minute — so that time fixtures read as minutes/hours.
const MINUTE_US: u64 = 60_000_000;
const HOUR_US: u64 = 60 * MINUTE_US;

#[test]
fn wait_before_check_no_record_is_zero() {
    assert_eq!(wait_before_check(None, 0), Duration::ZERO);
}

#[test]
fn wait_before_check_thirty_minutes_old_waits_ninety_minutes() {
    let now = 10 * HOUR_US;
    let at = now - 30 * MINUTE_US;
    assert_eq!(
        wait_before_check(Some(at), now),
        Duration::from_secs(90 * 60)
    );
}

#[test]
fn wait_before_check_exactly_ttl_is_zero() {
    let now = 10 * HOUR_US;
    let at = now - 2 * HOUR_US;
    assert_eq!(wait_before_check(Some(at), now), Duration::ZERO);
}

#[test]
fn wait_before_check_older_than_ttl_is_zero() {
    let now = 10 * HOUR_US;
    let at = now - 5 * HOUR_US;
    assert_eq!(wait_before_check(Some(at), now), Duration::ZERO);
}

#[test]
fn wait_before_check_future_write_time_is_zero() {
    let now = 10 * HOUR_US;
    let at = now + HOUR_US;
    assert_eq!(wait_before_check(Some(at), now), Duration::ZERO);
}

#[test]
fn next_wait_matches_outcome() {
    assert_eq!(pause_after_outcome(Outcome::Found), Duration::from_secs(2 * 60 * 60));
    assert_eq!(pause_after_outcome(Outcome::Failed), Duration::from_secs(10 * 60));
    assert_eq!(pause_after_outcome(Outcome::Offline), Duration::from_secs(60));
}

/// The decision "postpone the country lookup": postpone only when the transport itself said "network not seen".
mod net_absent_decision {
    use super::*;

    #[test]
    fn country_check_defers_only_when_net_not_seen() {
        // given: three transport answers about the network — "not seen", "seen", "don't know"
        let (not_seen, seen, unknown) = (info_with(Some(false)), info_with(Some(true)), info_with(None));
        // when: we decide whether to postpone the lookup
        let verdicts = [net_absent(&not_seen), net_absent(&seen), net_absent(&unknown)];
        // then: postpone only on `Some(false)`; "don't know" (None) does not stop the lookup
        assert_eq!(
            verdicts,
            [true, false, false],
            "order: Some(false), Some(true), None"
        );
    }
}

fn info_with(net_seen: Option<bool>) -> NetInfo {
    NetInfo { net_seen, ..NetInfo::default() }
}
