//! Tests for `PeersCache::forget` (a separate file: `peers_cache_tests.rs` is at the size ceiling).

use std::collections::HashSet;

use super::*;

use crate::net::{NodeSecret, PeerId};
use crate::net::AddrHint;
use std::net::SocketAddr;

fn endpoint_id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn add(cache: &mut PeersCache, n: u8, now: u64) {
    let addr = SocketAddr::from(([127, 0, 0, 1], 10_000 + n as u16));
    let entry = entry_for(None, endpoint_id(n), Some(&AddrHint::Ip(addr)), now);
    assert!(cache.remember(entry, now));
}

#[test]
fn forget_removes_only_listed_and_returns_true() {
    let mut cache = PeersCache::load(None, "net", 1000);
    add(&mut cache, 1, 1000);
    add(&mut cache, 2, 1000);
    let gone: HashSet<PeerId> = [endpoint_id(1)].into_iter().collect();
    assert!(cache.forget(&gone));
    let seen = cache.seen();
    assert_eq!(seen.len(), 1);
    assert!(seen.contains_key(&endpoint_id(2)));
}

#[test]
fn forget_unknown_returns_false() {
    let mut cache = PeersCache::load(None, "net", 1000);
    add(&mut cache, 1, 1000);
    let gone: HashSet<PeerId> = [endpoint_id(9)].into_iter().collect();
    assert!(!cache.forget(&gone));
    assert_eq!(cache.len(), 1);
}
