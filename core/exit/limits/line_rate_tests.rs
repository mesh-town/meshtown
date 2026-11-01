//! Tests of the keyed limiter of repeated log lines: a window and a held-back count per key.

use super::*;

const SEC: Duration = Duration::from_secs(1);

#[test]
fn keys_do_not_share_the_window() {
    // given: key 1 has had its line and one event held back
    let limiter = KeyedLineRate::<u8>::default();
    let t0 = Instant::now();
    assert_eq!(limiter.due(1, t0), Some(0));
    assert_eq!(limiter.due(1, t0 + SEC), None);

    // when: key 2 comes in the same window
    let second = limiter.due(2, t0 + 2 * SEC);

    // then: it gets its own first line, and key 1 still reports its own tail
    assert_eq!(second, Some(0));
    assert_eq!(limiter.due(1, t0 + 10 * SEC), Some(1));
}

#[test]
fn idle_keys_are_forgotten() {
    // given: 100 keys that each had a line and held nothing back
    let limiter = KeyedLineRate::<u32>::default();
    let t0 = Instant::now();
    (0..100).for_each(|k| assert_eq!(limiter.due(k, t0), Some(0)));
    assert_eq!(limiter.len(), 100);

    // when: a new key comes after the window
    limiter.due(1000, t0 + 11 * SEC);

    // then: the idle ones are gone
    assert_eq!(limiter.len(), 1);
}

#[test]
fn held_tail_is_kept_until_it_is_reported() {
    // given: 100 keys, one of them with a held-back event
    let limiter = KeyedLineRate::<u32>::default();
    let t0 = Instant::now();
    (0..100).for_each(|k| assert_eq!(limiter.due(k, t0), Some(0)));
    assert_eq!(limiter.due(7, t0 + SEC), None);

    // when: a new key comes after the window and prunes
    limiter.due(1000, t0 + 11 * SEC);

    // then: the key with the tail is still there and reports it
    assert_eq!(limiter.len(), 2);
    assert_eq!(limiter.due(7, t0 + 12 * SEC), Some(1));
}
