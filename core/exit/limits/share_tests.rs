//! Tests of the per-member share of the exit cap: the reserve, the equal split, the contention window.

use super::super::gate::{Busy, ExitLimits, StreamSlot};
use crate::transport::net::facade::test_support::pid;
use std::time::{Duration, Instant};

fn take(limits: &ExitLimits, member: u8, n: usize, now: Instant) -> Vec<Result<StreamSlot, Busy>> {
    (0..n).map(|_| limits.try_admit(pid(member), now)).collect()
}

fn places(answers: Vec<Result<StreamSlot, Busy>>) -> Vec<StreamSlot> {
    answers.into_iter().map(|a| a.expect("a place")).collect()
}

#[test]
fn alone_member_leaves_a_reserve() {
    // given: a cap of 10
    let limits = ExitLimits::fixed(10);
    let now = Instant::now();

    // when: one member asks for nine places
    let mut answers = take(&limits, 1, 9, now);
    let ninth = answers.pop().expect("nine answers");

    // then: eight are admitted and the ninth hits the reserve
    assert!(answers.iter().all(|a| a.is_ok()), "first eight admitted");
    assert!(matches!(ninth, Err(Busy::Share { held: 8, share: 8 })));
    assert_eq!(limits.live(), 8);
}

#[test]
fn newcomer_gets_a_place_while_one_member_holds_the_rest() {
    // given: member 1 holds eight of ten places
    let limits = ExitLimits::fixed(10);
    let now = Instant::now();
    let _first = places(take(&limits, 1, 8, now));

    // when: member 2 asks three times
    let mut answers = take(&limits, 2, 3, now);
    let third = answers.pop().expect("three answers");

    // then: two places are admitted and the third finds the cap full
    assert!(answers.iter().all(|a| a.is_ok()), "newcomer admitted");
    assert!(matches!(third, Err(Busy::Streams { .. })));
}

#[test]
fn freed_place_goes_to_the_member_below_share() {
    // given: members 1 and 2 hold 8 and 2 places of ten
    let limits = ExitLimits::fixed(10);
    let now = Instant::now();
    let mut first = places(take(&limits, 1, 8, now));
    let _second = places(take(&limits, 2, 2, now));

    // when: member 1 releases one place and both ask
    first.pop();
    let again = limits.try_admit(pid(1), now);
    let newcomer = limits.try_admit(pid(2), now);

    // then: member 1 is above its share of five, member 2 gets the place
    assert!(matches!(again, Err(Busy::Share { held: 7, share: 5 })));
    assert!(newcomer.is_ok(), "the member below its share wins");
}

#[test]
fn refused_member_stays_a_contender_for_the_window() {
    // given: members 1 and 2 hold 8 and 2 places, member 2 was refused
    let limits = ExitLimits::fixed(10);
    let now = Instant::now();
    let mut first = places(take(&limits, 1, 8, now));
    let second = places(take(&limits, 2, 2, now));
    assert!(limits.try_admit(pid(2), now).is_err());

    // when: member 2 leaves and member 1 releases one place
    drop(second);
    first.pop();

    // then: member 1 is still held to the split inside the window and free of it after
    assert!(limits.try_admit(pid(1), now).is_err(), "still contended");
    let later = now + CONTEND_WAIT;
    assert!(limits.try_admit(pid(1), later).is_ok(), "window passed");
}

const CONTEND_WAIT: Duration = Duration::from_secs(11);

#[test]
fn released_member_is_forgotten() {
    // given: two members that held places and one of them was refused
    let limits = ExitLimits::fixed(2);
    let now = Instant::now();
    let held = places(take(&limits, 1, 1, now));
    let other = places(take(&limits, 2, 1, now));
    assert!(limits.try_admit(pid(2), now).is_err());

    // when: everything is released and the window passes
    drop(held);
    drop(other);

    // then: the table keeps the refusal inside the window and nothing after it
    assert_eq!(limits.share_len(now), 1);
    assert_eq!(limits.share_len(now + CONTEND_WAIT), 0);
}

#[test]
fn one_place_limit_admits_one() {
    // given: a cap of one place
    let limits = ExitLimits::fixed(1);
    let now = Instant::now();

    // when: member 1 takes it and asks again, then member 2 asks
    let first = limits.try_admit(pid(1), now);
    let again = limits.try_admit(pid(1), now);
    let other = limits.try_admit(pid(2), now);

    // then: only the first is admitted
    assert!(first.is_ok());
    assert!(matches!(again, Err(Busy::Streams { .. })));
    assert!(matches!(other, Err(Busy::Streams { .. })));
}

#[test]
fn concurrent_admits_keep_the_share() {
    // given: a cap of ten and two threads of one member
    let limits = ExitLimits::fixed(10);
    let now = Instant::now();

    // when: each thread asks fifty times, both starting together
    let start = std::sync::Barrier::new(2);
    let slots: Vec<StreamSlot> = std::thread::scope(|s| {
        let work = || {
            start.wait();
            take(&limits, 1, 50, now)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
        };
        let (a, b) = (s.spawn(work), s.spawn(work));
        let mut all = a.join().expect("thread a");
        all.extend(b.join().expect("thread b"));
        all
    });

    // then: exactly the member's ceiling is held
    assert_eq!(slots.len(), 8);
    assert_eq!(limits.live(), 8);
}
