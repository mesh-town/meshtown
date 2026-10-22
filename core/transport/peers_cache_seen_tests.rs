//! Test of `PeersCache::seen()` from `peers_cache.rs` — a separate neighbor file of `mod tests`
//! (`peers_cache_tests.rs` is already at the 404-line ceiling, nothing new is added to it). The helpers
//! `endpoint_id`/`sock_addr` of that file are private to its module — here are our own copies.

use super::*;

use crate::transport::net::AddrHint;
use crate::transport::net::{NodeSecret, PeerId};
use std::net::SocketAddr;

/// The `PeerId` of member `n` — our own copy of the `peers_cache_tests.rs` helper (that one is private to
/// its module).
fn endpoint_id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

/// The address of member `n` — different ports on the local host, so the addresses don't collide.
fn sock_addr(n: u8) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 10_000 + n as u16))
}

#[test]
fn seen_maps_each_entry_to_its_time() {
    let a = endpoint_id(1);
    let b = endpoint_id(2);
    let mut cache = PeersCache::load(None, "net", 1000);
    assert!(cache.remember(
        entry_for(None, a, Some(&AddrHint::Ip(sock_addr(1))), 1000),
        1000
    ));
    assert!(cache.remember(
        entry_for(None, b, Some(&AddrHint::Ip(sock_addr(2))), 2000),
        2000
    ));

    let seen = cache.seen();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen.get(&a), Some(&1000), "A was remembered at moment 1000");
    assert_eq!(seen.get(&b), Some(&2000), "B was remembered at moment 2000");
}

#[test]
fn seen_of_empty_cache_is_empty() {
    let cache = PeersCache::load(None, "net", 1000);
    assert!(cache.seen().is_empty());
}
