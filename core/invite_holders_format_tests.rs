//! Tests of the holders string format: the record order changes, the format does not.

use std::net::SocketAddr;

use super::*;
use crate::transport::addr_order::PrimaryIps;
use crate::transport::net::{AddrHint, NodeSecret};

const T: u64 = 1_760_000_000;

fn id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn key_only(n: u8) -> PeerAddr {
    PeerAddr::new(id(n))
}

fn with_ip(n: u8) -> PeerAddr {
    let mut addr = key_only(n);
    let sock: SocketAddr = "203.0.113.7:4000".parse().unwrap();
    addr.hints.push(AddrHint::Ip(sock));
    addr
}

fn head() -> InviteHead {
    InviteHead {
        secret: InviteSecret::from_bytes([0x11; 32]),
        network_id: [0xaa, 0xbb, 0xcc, 0xdd],
        label: None,
        made_unix: T,
        kind: NetKind::Peer,
    }
}

#[test]
fn v3_body_without_holders_is_accepted() {
    // given: v3 with a single creator
    let s = build_keyed(&head(), &with_ip(1), &[], &PrimaryIps::NONE).unwrap();
    // then: parsing gives one record
    let parsed = parse(&s).unwrap();
    assert_eq!(parsed.peers.len(), 1);
    assert_eq!(parsed.peers[0].id, id(1));
}

#[test]
fn v3_with_key_only_holders_first_keeps_order_and_hints() {
    // given: two holders without hints, a creator with a hint, one other
    let (h1, h2, me, o) = (key_only(1), key_only(2), with_ip(3), key_only(4));
    // when: holders first
    let s = build_keyed(&head(), &h1, &[h2, me, o], &PrimaryIps::NONE).unwrap();
    // then: order is kept, holders have no hints, the creator has one
    let peers = parse(&s).unwrap().peers;
    let ids: Vec<PeerId> = peers.iter().map(|a| a.id).collect();
    assert_eq!(ids, vec![id(1), id(2), id(3), id(4)]);
    assert!(peers[0].hints.is_empty() && peers[1].hints.is_empty());
    assert!(!peers[2].hints.is_empty());
}
