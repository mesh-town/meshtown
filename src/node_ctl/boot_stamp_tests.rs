//! Tests of the lap stamp (`boot_stamp.rs`).

use super::*;
use std::time::{Duration, Instant};

#[test]
fn new_stamp_id_is_eight_hex_digits() {
    let s = BootStamp::new();
    assert_eq!(s.id().len(), 8);
    assert!(s.id().chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn id_is_stable_for_one_stamp() {
    let s = BootStamp::new();
    assert_eq!(s.id(), s.id());
}

#[test]
fn two_stamps_have_different_ids() {
    assert_ne!(BootStamp::new().id(), BootStamp::new().id());
}

#[test]
fn fresh_stamp_uptime_is_near_zero() {
    assert!(BootStamp::new().uptime_sec() < 2);
}

#[test]
fn uptime_counts_from_since() {
    let since = Instant::now().checked_sub(Duration::from_secs(5)).unwrap();
    let s = BootStamp {
        id: "x".into(),
        since,
    };
    assert!((5..7).contains(&s.uptime_sec()));
}

#[test]
fn backdated_stamp_is_older() {
    assert!(BootStamp::backdated(100).uptime_sec() >= 100);
}
