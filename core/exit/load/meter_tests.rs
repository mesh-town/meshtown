//! Tests of the meter: member counts and pruning.

use super::*;
use crate::transport::net::facade::test_support::pid;
use std::time::Duration;

#[test]
fn member_counts_cover_ten_minutes_and_prune() {
    // given: member 1 has events at 0 s and 100 s, member 2 at 700 s
    let base = Instant::now();
    let at = |secs| base + Duration::from_secs(secs);
    let mut meter = Meter::new(base);
    meter.record(pid(1), Outcome::Admitted, at(0));
    meter.record(pid(1), Outcome::Refused(Reason::Limit), at(100));

    // when: read at 599 s
    let early = meter.members(at(599));

    // then: member 1 is counted over the whole ten minutes
    assert_eq!(early.len(), 1);
    assert_eq!((early[0].admitted, early[0].refused), (1, 1));

    // when: member 2 appears at 700 s and member 1 has been silent for over 600 s
    meter.record(pid(2), Outcome::Admitted, at(700));
    let late = meter.members(at(700));

    // then: only member 2 is left, member 1 is deleted
    assert_eq!(late.len(), 1);
    assert_eq!(late[0].peer, pid(2));
    assert_eq!(meter.member_count(), 1);
}
