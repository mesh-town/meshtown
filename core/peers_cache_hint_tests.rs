//! Tests for building a cache entry from a node's address hints (`entry_for`, `PeersCache::addrs`):
//! hints meaningless to the cache are skipped, the order and the ceiling stay as before.

use std::net::SocketAddr;

use super::*;
use crate::net::{AddrHint, NodeSecret, PeerAddr, PeerId, RelayRef};
use crate::net_mainline::record::RosterEntry;

fn peer(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn sock(text: &str) -> SocketAddr {
    text.parse().expect("address")
}

fn url(text: &str) -> AddrHint {
    AddrHint::Relay(RelayRef::Url(text.to_string()))
}

#[test]
fn selected_relay_peer_and_opaque_are_skipped() {
    let known = PeerAddr { id: peer(1), hints: vec![AddrHint::Ip(sock("127.0.0.1:1000"))] };
    let base = entry_for(Some(&known), peer(1), None, 1000);
    let by_peer = AddrHint::Relay(RelayRef::Peer(peer(2)));
    let opaque = AddrHint::Opaque { kind: 7, bytes: vec![1, 2] };
    assert_eq!(entry_for(Some(&known), peer(1), Some(&by_peer), 1000), base);
    assert_eq!(entry_for(Some(&known), peer(1), Some(&opaque), 1000), base);
}

#[test]
fn known_first_url_relay_wins_and_peer_relay_is_ignored() {
    let known = PeerAddr {
        id: peer(1),
        hints: vec![
            AddrHint::Relay(RelayRef::Peer(peer(2))),
            url("https://b.example/"),
            url("https://a.example/"),
        ],
    };
    let entry = entry_for(Some(&known), peer(1), None, 1000);
    assert_eq!(entry.relay, Some("https://b.example/".to_string()));
}

#[test]
fn v4_before_v6_and_cap() {
    let v6_x = sock("[2001:db8::10]:4000");
    let ips = ["[2001:db8::1]:4000", "10.0.0.1:1", "[2001:db8::2]:4000", "10.0.0.2:1", "10.0.0.3:1"];
    let known = PeerAddr {
        id: peer(1),
        hints: ips.iter().map(|t| AddrHint::Ip(sock(t))).collect(),
    };
    let entry = entry_for(Some(&known), peer(1), Some(&AddrHint::Ip(v6_x)), 1000);
    let want = vec![v6_x, sock("10.0.0.1:1"), sock("10.0.0.2:1"), sock("10.0.0.3:1")];
    assert_eq!(entry.addrs, want);
    assert_eq!(entry.addrs.len(), MAX_ADDRS);
}

#[test]
fn addrs_skips_bad_relay_record() {
    let good = RosterEntry { id: *peer(2).as_bytes(), relay: None, addrs: vec![sock("127.0.0.1:1")], seen_unix: 900 };
    let bad = RosterEntry { id: *peer(1).as_bytes(), relay: Some("not a url".to_string()), addrs: vec![], seen_unix: 1000 };
    let mut cache = PeersCache::load(None, "net", 1000);
    assert!(cache.remember(bad, 1000));
    assert!(cache.remember(good, 900));
    let ids: Vec<PeerId> = cache.addrs().iter().map(|a| a.id).collect();
    assert_eq!(ids, vec![peer(2)]);
    assert!(cache.seen().contains_key(&peer(1)));
}
