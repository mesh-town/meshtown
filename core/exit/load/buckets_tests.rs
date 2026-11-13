//! Tests of the bucket ring.

use super::*;
use crate::exit::load::counts::MemberCounts;
use std::time::Duration;

#[test]
fn ring_forgets_events_older_than_ten_minutes() {
    // given: one refusal in the first bucket
    let mut ring: Ring<MemberCounts> = Ring::new();
    ring.at(0).refused += 1;

    // when: the sum is taken at 599 s and at 600 s
    let at = |secs| {
        let origin = Instant::now();
        index_of(origin, origin + Duration::from_secs(secs))
    };
    let young = ring.total(at(599), SLOTS);
    let old = ring.total(at(600), SLOTS);

    // then: the event is there at 599 s and gone at 600 s
    assert_eq!(young.refused, 1, "still inside ten minutes");
    assert_eq!(old.refused, 0, "older than ten minutes");
}
