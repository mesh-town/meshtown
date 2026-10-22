//! Tests of building and parsing the invite string (`invite.rs`).

use std::net::{Ipv6Addr, SocketAddr};

use crate::net::endpoint_shim::from_endpoint_addr;
use crate::net::endpoint_shim::{EndpointAddr, RelayUrl};
use crate::net::{AddrHint, NodeSecret, PeerAddr, PeerId, RelayRef};
use data_encoding::{BASE32_NOPAD, BASE32_NOPAD_NOCASE};

use super::*;
use crate::addr_order::PrimaryIps;

const NOW: u64 = 1_790_000_000;

fn id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn raw_relay(n: u8) -> EndpointAddr {
    let relay: RelayUrl = "https://euc1-1.relay.n0.iroh.link./".parse().unwrap();
    EndpointAddr::new(id(n)).with_relay_url(relay)
}

fn with_relay(n: u8) -> PeerAddr {
    from_endpoint_addr(&raw_relay(n))
}

fn ipv6_pair(n: u8) -> PeerAddr {
    let a = SocketAddr::from((
        Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, n as u16),
        50000,
    ));
    let b = SocketAddr::from((
        Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, n as u16),
        50001,
    ));
    from_endpoint_addr(&EndpointAddr::new(id(n)).with_ip_addr(a).with_ip_addr(b))
}

fn key() -> NetworkKey {
    NetworkKey::from_bytes([9u8; 32])
}

#[test]
fn round_trip() {
    let head = head_made_at(NOW);
    let me = with_relay(1);
    let others = [with_relay(2)];
    let s = build(&head, &me, &others, &PrimaryIps::NONE).unwrap();
    assert!(s.starts_with(INVITE_PREFIX));
    let parsed = parse(&s).unwrap();
    assert_eq!(parsed.head.secret, head.secret);
    assert_eq!(parsed.head.network_id, key().subkeys().network_id);
    assert_eq!(parsed.head.label, None);
    assert_eq!(parsed.head.made_unix, NOW);
    let ids: Vec<_> = parsed.peers.iter().map(|a| a.id).collect();
    assert_eq!(ids, vec![id(1), id(2)]);
    assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
}

#[test]
fn label_round_trip() {
    let me = with_relay(1);
    let others = [with_relay(2)];
    let head = head_with_label("our-game");
    let s = build(&head, &me, &others, &PrimaryIps::NONE).unwrap();
    let parsed = parse(&s).unwrap();
    assert_eq!(parsed.head.label.as_deref(), Some("our-game"));
    assert_eq!(parsed.head.secret, head.secret);
    assert_eq!(parsed.head.network_id, head.network_id);
}

#[test]
fn me_first_and_at_most_five() {
    let me = with_relay(1);
    let others: Vec<PeerAddr> = [2u8, 1, 3, 4, 5, 6, 7]
        .iter()
        .map(|&n| with_relay(n))
        .collect();
    let s = build(&head_made_at(NOW), &me, &others, &PrimaryIps::NONE).unwrap();
    let parsed = parse(&s).unwrap();
    let ids: Vec<_> = parsed.peers.iter().map(|a| a.id).collect();
    assert_eq!(ids, vec![id(1), id(2), id(3), id(4), id(5)]);
}

// The lengths come from a run: the previous 677/692/1122 plus 4 bytes of the network marker (key and secret are both
// 32 bytes) make 684/698/1129 characters with the same number of members; if the format changes, the numbers change.
#[test]
fn lengths_match_plan() {
    let me = with_relay(1);
    let others: Vec<PeerAddr> = (2..=5).map(with_relay).collect();
    let s = build(&head_made_at(NOW), &me, &others, &PrimaryIps::NONE).unwrap();
    assert_eq!(s.chars().count(), 684, "{s}");

    let s = build(
        &head_with_label("our-game"),
        &me,
        &others,
        &PrimaryIps::NONE,
    )
    .unwrap();
    assert_eq!(s.chars().count(), 698, "{s}");

    let me6 = ipv6_pair(1);
    let others6: Vec<PeerAddr> = (2..=5).map(ipv6_pair).collect();
    let s = build(
        &head_with_label(&"𝄞".repeat(64)),
        &me6,
        &others6,
        &PrimaryIps::NONE,
    )
    .unwrap();
    assert_eq!(s.chars().count(), 1129, "{s}");
}

#[test]
fn relay_drops_direct_addrs() {
    let me =
        from_endpoint_addr(&raw_relay(1).with_ip_addr(SocketAddr::from(([10, 0, 0, 1], 4000))));
    let neighbor = EndpointAddr::new(id(2))
        .with_ip_addr(SocketAddr::from(([10, 0, 0, 2], 4001)))
        .with_ip_addr(SocketAddr::from(([10, 0, 0, 3], 4002)))
        .with_ip_addr(SocketAddr::from(([10, 0, 0, 4], 4003)));
    let neighbor = from_endpoint_addr(&neighbor);
    let s = build(&head_made_at(NOW), &me, &[neighbor], &PrimaryIps::NONE).unwrap();
    let parsed = parse(&s).unwrap();
    assert_eq!(parsed.peers.len(), 2);
    assert_eq!(
        crate::net::endpoint_shim::to_endpoint_addr(&parsed.peers[0])
            .relay_urls()
            .count(),
        1
    );
    assert_eq!(
        crate::net::endpoint_shim::to_endpoint_addr(&parsed.peers[0])
            .ip_addrs()
            .count(),
        0
    );
    assert_eq!(
        crate::net::endpoint_shim::to_endpoint_addr(&parsed.peers[1])
            .relay_urls()
            .count(),
        0
    );
    assert_eq!(
        crate::net::endpoint_shim::to_endpoint_addr(&parsed.peers[1])
            .ip_addrs()
            .count(),
        2
    );
}

#[test]
fn nothing_to_invite() {
    let me = PeerAddr::new(id(1));
    let err = format!(
        "{:#}",
        build(&head_made_at(NOW), &me, &[], &PrimaryIps::NONE).unwrap_err()
    );
    assert!(err.contains("nothing to invite with"), "{err}");
}

#[test]
fn build_with_only_peer_relay_and_opaque_has_nothing_to_invite() {
    let mut me = PeerAddr::new(id(1));
    me.hints.push(AddrHint::Relay(RelayRef::Peer(id(2))));
    me.hints.push(AddrHint::Opaque {
        kind: 7,
        bytes: vec![1, 2, 3],
    });
    let err = build(&head_made_at(NOW), &me, &[], &PrimaryIps::NONE).unwrap_err();
    assert_eq!(
        format!("{err:#}"),
        "the node has no address of its own and no members yet — nothing to invite with; wait a minute after start"
    );
}

#[test]
fn parse_rejects_garbage() {
    for bad in ["", "   \n"] {
        let err = format!("{:#}", parse(bad).unwrap_err());
        assert!(err.contains("is empty"), "{err}");
    }
    let long = "a".repeat(4001);
    let err = format!("{:#}", parse(&long).unwrap_err());
    assert!(err.contains("4000"), "{err}");

    let err = format!("{:#}", parse("hello").unwrap_err());
    assert!(err.contains("mesh-invite2-"), "{err}");

    let err = format!("{:#}", parse("mesh-invite2-").unwrap_err());
    assert!(err.contains("nothing after"), "{err}");

    let err = format!("{:#}", parse("mesh-invite2-!!!!").unwrap_err());
    assert!(err.contains("foreign characters"), "{err}");

    let real = build(
        &head_made_at(NOW),
        &with_relay(1),
        &[with_relay(2)],
        &PrimaryIps::NONE,
    )
    .unwrap();
    let body = &real[INVITE_PREFIX.len()..];
    let mut bytes = BASE32_NOPAD_NOCASE.decode(body.as_bytes()).unwrap();
    // version 3 is now legal (a key invite); "foreign format" is 4
    bytes[0] = 4;
    let corrupted = format!(
        "{INVITE_PREFIX}{}",
        BASE32_NOPAD.encode(&bytes).to_ascii_lowercase()
    );
    let err = format!("{:#}", parse(&corrupted).unwrap_err());
    assert!(
        err.contains("update the program") && err.contains('4'),
        "{err}"
    );

    bytes[0] = 2;
    bytes.truncate(10);
    let truncated = format!(
        "{INVITE_PREFIX}{}",
        BASE32_NOPAD.encode(&bytes).to_ascii_lowercase()
    );
    let err = format!("{:#}", parse(&truncated).unwrap_err());
    assert!(err.contains("unreadable"), "{err}");
}

#[test]
fn parse_tolerates_case_and_spaces() {
    let head = head_made_at(NOW);
    let real = build(&head, &with_relay(1), &[with_relay(2)], &PrimaryIps::NONE).unwrap();
    let upper = real.to_uppercase();
    let mut spaced = String::new();
    for (i, c) in upper.chars().enumerate() {
        spaced.push(c);
        if (i + 1) % 40 == 0 {
            spaced.push('\n');
            spaced.push(' ');
        }
    }
    let parsed = parse(&spaced).unwrap();
    assert_eq!(parsed.head.secret, head.secret);
    assert_eq!(parsed.head.network_id, head.network_id);
}

#[test]
fn looks_like_invite_ignores_case_and_spaces() {
    assert!(looks_like_invite(&format!("  {}abc  ", INVITE_PREFIX)));
    assert!(looks_like_invite(&format!(
        "{}ABC",
        INVITE_PREFIX.to_uppercase()
    )));
    assert!(!looks_like_invite("mesh1-abc"));
    assert!(!looks_like_invite(""));
    assert!(!looks_like_invite("mesh-invite"));
    assert!(
        !looks_like_invite("mesh-invite3-abc"),
        "unknown format number — not an invite"
    );
}

#[path = "invite_old_format_tests.rs"]
mod old_format_invite;

#[path = "invite_no_key_tests.rs"]
mod invite_carries_no_network_key;

#[path = "invite_order_tests.rs"]
mod direct_addrs_order;

// One-time invite: secret, hash, freshness, network of the key.

#[path = "invite_secret_tests.rs"]
mod invite_secret;

#[path = "invite_debug_tests.rs"]
mod secret_debug_hides_bytes;

#[path = "invite_hash_tests.rs"]
mod hash_is_stable_and_domain_separated;

#[path = "invite_fresh_tests.rs"]
mod check_fresh_cases;

#[path = "invite_same_network_tests.rs"]
mod same_network_cases;

#[path = "invite_key_text_tests.rs"]
mod key_text_cases;

const HOUR: u64 = 3_600;

fn a_secret() -> InviteSecret {
    InviteSecret::from_bytes(core::array::from_fn(|i| 0x60 + i as u8))
}

/// Head of an invite of the network `key()`, built at the moment `made_unix`.
fn head_made_at(made_unix: u64) -> InviteHead {
    InviteHead {
        secret: InviteSecret::from_bytes([0xC7; 32]),
        network_id: key().subkeys().network_id,
        label: None,
        made_unix,
    }
}

/// Head of an invite of the network `key()` with a label.
fn head_with_label(label: &str) -> InviteHead {
    InviteHead {
        label: Some(label.to_string()),
        ..head_made_at(NOW)
    }
}

/// Body of the invite string after base32 — the bytes `postcard` reads.
fn decoded_body(invite: &str) -> Vec<u8> {
    assert!(invite.starts_with(INVITE_PREFIX), "{invite}");
    BASE32_NOPAD_NOCASE
        .decode(&invite.as_bytes()[INVITE_PREFIX.len()..])
        .unwrap()
}

fn hex(bytes: [u8; 32]) -> String {
    blake3::Hash::from_bytes(bytes).to_hex().to_string()
}

/// The text has no byte of the secret `0xC7`, neither in decimal (199) nor in hexadecimal form.
fn assert_no_secret_bytes(text: &str) {
    for needle in ["199", "c7", "C7"] {
        assert!(
            !text.contains(needle),
            "the debug output has '{needle}': {text}"
        );
    }
}
