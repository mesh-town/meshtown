//! Tests for the duplicate cache: the first time, an entry's life, eviction, id by content.

use super::{msg_id, Seen};
use std::time::{Duration, Instant};

const LIFE: Duration = Duration::from_secs(300);

fn id(n: u8) -> [u8; 32] {
    [n; 32]
}

/// given: an empty cache → when: the same id twice → then: first `true`, then `false`.
#[test]
fn first_time_then_repeat() {
    let mut seen = Seen::new(8, LIFE);
    let now = Instant::now();

    assert!(seen.remember_if_new(id(1), now));
    assert!(!seen.remember_if_new(id(1), now));
}

/// given: an id is remembered → when: `life` and another 1 ms have passed → then: new again.
#[test]
fn forgets_after_life() {
    let mut seen = Seen::new(8, LIFE);
    let now = Instant::now();
    seen.remember_if_new(id(1), now);

    let later = now + LIFE + Duration::from_millis(1);

    assert!(seen.remember_if_new(id(1), later));
}

/// given: cap 2, three ids → when: we ask for the first and third → then: the first is evicted, the third is remembered.
#[test]
fn evicts_oldest_over_cap() {
    let mut seen = Seen::new(2, LIFE);
    let now = Instant::now();
    seen.remember_if_new(id(1), now);
    seen.remember_if_new(id(2), now);
    seen.remember_if_new(id(3), now);

    assert!(seen.remember_if_new(id(1), now), "the first is evicted");
    assert!(!seen.remember_if_new(id(3), now), "the third is remembered");
}

/// given: two bodies → when: `msg_id` → then: equal bodies give an equal id, different ones a different id, id = blake3 of the body.
#[test]
fn msg_id_is_content_hash() {
    assert_eq!(msg_id(b"abc"), msg_id(b"abc"));
    assert_ne!(msg_id(b"abc"), msg_id(b"abd"));
    assert_eq!(msg_id(b"abc"), *blake3::hash(b"abc").as_bytes());
}
