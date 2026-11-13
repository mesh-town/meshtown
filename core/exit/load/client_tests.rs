use std::time::{Duration, Instant};

use super::*;

fn at(t0: Instant, secs: u64) -> Instant {
    t0 + Duration::from_secs(secs)
}

fn feed(a: &mut ExitAnswers, t: Instant, replied: u64, busy: u64) {
    (0..replied).for_each(|_| a.record(Answer::Replied, t));
    (0..busy).for_each(|_| a.record(Answer::Busy, t));
}

/// given: 5 refusals among 20 attempts in the window; then the exit refuses us.
#[test]
fn five_refusals_of_twenty_attempts_enter() {
    let t0 = Instant::now();
    let mut a = ExitAnswers::new(t0);
    feed(&mut a, t0, 15, 5);
    assert_eq!(a.refusing(at(t0, 1)), Some((5, 20)));
}

/// given: 4 refusals among 20 attempts; then it does not.
#[test]
fn four_refusals_do_not_enter() {
    let t0 = Instant::now();
    let mut a = ExitAnswers::new(t0);
    feed(&mut a, t0, 16, 4);
    assert_eq!(a.refusing(at(t0, 1)), None);
}

/// given: it entered; when 120 s pass without a refusal, then it is lifted, not before.
#[test]
fn leaves_after_window_without_refusals() {
    let t0 = Instant::now();
    let mut a = ExitAnswers::new(t0);
    feed(&mut a, t0, 15, 5);
    assert!(a.refusing(at(t0, 100)).is_some());
    feed(&mut a, at(t0, 100), 50, 0);
    assert_eq!(a.refusing(at(t0, 120)), None);
}
