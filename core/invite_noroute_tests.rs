//! How many direct addresses an invite record keeps, depending on whether the default route is known.

use std::net::SocketAddr;

use crate::transport::net::{AddrHint, NodeSecret, PeerAddr, PeerId, RelayRef};

use super::*;
use crate::transport::addr_order::PrimaryIps;

const T: u64 = 1_760_000_000;
const PRIVATE: [&str; 4] = [
    "192.168.1.5:4000",
    "192.168.7.3:4000",
    "172.16.0.9:4000",
    "192.168.64.2:4000",
];

fn id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn head() -> InviteHead {
    InviteHead {
        secret: InviteSecret::from_bytes([0x11; 32]),
        network_id: [0xaa, 0xbb, 0xcc, 0xdd],
        label: None,
        made_unix: T,
        root: id(31),
    }
}

fn member(n: u8, addrs: &[&str]) -> PeerAddr {
    let mut addr = PeerAddr::new(id(n));
    for a in addrs {
        addr.hints
            .push(AddrHint::Ip(a.parse::<SocketAddr>().unwrap()));
    }
    addr
}

fn hints_after(primary: &PrimaryIps, addrs: &[&str]) -> Vec<AddrHint> {
    let text = build_keyed(&head(), &member(1, addrs), &[], primary).unwrap();
    parse(&text).unwrap().peers[0].hints.clone()
}

#[test]
fn no_route_keeps_four_direct_addresses() {
    assert_eq!(hints_after(&PrimaryIps::NONE, &PRIVATE).len(), 4);
}

#[test]
fn known_v4_route_keeps_two() {
    let primary = PrimaryIps {
        v4: Some("192.168.64.2".parse().unwrap()),
        v6: None,
    };
    let hints = hints_after(&primary, &PRIVATE);
    assert_eq!(hints.len(), 2);
    let route = AddrHint::Ip("192.168.64.2:4000".parse().unwrap());
    assert!(
        hints.contains(&route),
        "the route address (last in the input) must be kept: {hints:?}"
    );
}

#[test]
fn known_v6_route_keeps_two() {
    let primary = PrimaryIps {
        v4: None,
        v6: Some("2001:db8::1".parse().unwrap()),
    };
    assert_eq!(hints_after(&primary, &PRIVATE).len(), 2);
}

#[test]
fn relay_record_has_no_direct_addresses() {
    let mut addr = member(1, &PRIVATE);
    addr.hints.push(AddrHint::Relay(RelayRef::Url(
        "https://relay.example/".into(),
    )));
    let text = build_keyed(&head(), &addr, &[], &PrimaryIps::NONE).unwrap();
    let hints = parse(&text).unwrap().peers[0].hints.clone();
    assert!(
        hints.iter().all(|h| !matches!(h, AddrHint::Ip(_))),
        "{hints:?}"
    );
}

#[test]
fn five_members_with_four_addresses_fit_the_limits() {
    let me = member(1, &PRIVATE);
    let others: Vec<PeerAddr> = (2..=5).map(|n| member(n, &PRIVATE)).collect();
    let text = build_keyed(&head(), &me, &others, &PrimaryIps::NONE).unwrap();
    assert!(text.len() <= MAX_INVITE_CHARS, "{} chars", text.len());
    let parsed = parse(&text).unwrap();
    assert_eq!(parsed.peers.len(), 5);
    assert!(parsed.peers.iter().all(|p| p.hints.len() == 4));
}
