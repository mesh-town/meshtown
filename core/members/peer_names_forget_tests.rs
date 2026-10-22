//! Tests of `PeerNames::forget` (a separate file: `peer_names_tests.rs` is at the ceiling).

use std::collections::HashSet;

use crate::transport::net::{NodeSecret, PeerId};

use super::*;

fn endpoint_id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

#[test]
fn forget_removes_only_listed_and_returns_true() {
    let mut names = PeerNames::load(None, "net");
    assert!(names.remember(endpoint_id(1), "Kitchen"));
    assert!(names.remember(endpoint_id(2), "Balcony"));
    let gone: HashSet<PeerId> = [endpoint_id(1)].into_iter().collect();
    assert!(names.forget(&gone));
    assert_eq!(names.get(endpoint_id(1)), None);
    assert_eq!(names.get(endpoint_id(2)), Some("Balcony"));
    assert_eq!(names.len(), 1);
}

#[test]
fn forget_unknown_returns_false() {
    let mut names = PeerNames::load(None, "net");
    assert!(names.remember(endpoint_id(1), "Kitchen"));
    let gone: HashSet<PeerId> = [endpoint_id(9)].into_iter().collect();
    assert!(!names.forget(&gone));
    assert_eq!(names.len(), 1);
}
