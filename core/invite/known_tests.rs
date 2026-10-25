//! Tests of `with_known` (`known.rs`): member records without hints get the address this node itself proved,
//! and such records survive the invite string.

use std::cell::RefCell;
use std::net::SocketAddr;

use data_encoding::BASE32_NOPAD;

use super::*;
use crate::invite::{
    build_keyed, parse, InviteHead, InviteSecret, INVITE_PREFIX, MAX_INVITE_BYTES, MAX_INVITE_CHARS,
};
use crate::members::network_key::NetKind;
use crate::transport::addr_order::PrimaryIps;
use crate::transport::net::{AddrHint, NodeSecret, PeerAddr, PeerId};

const T: u64 = 1_760_000_000;

mod filling_entries {
    use super::*;

    #[test]
    fn fills_entries_without_hints_and_keeps_order() {
        // given: the issuer with hints, then B and C without; the node proved an address of B only
        let list = vec![with_ip(1, 7), key_only(2), key_only(3)];
        // when
        let got = with_known(list, |p| proven(&[(2, 20)], p));
        // then: B got its address, the order and the number of records are the same
        assert_eq!(got, vec![with_ip(1, 7), with_ip(2, 20), key_only(3)]);
    }

    #[test]
    fn keeps_an_entry_that_has_hints() {
        // given: the issuer has its own hints, and the lookup knows another address for it
        let asked = RefCell::new(Vec::new());
        let list = vec![with_ip(1, 7), key_only(2)];
        // when
        let got = with_known(list, |p| {
            asked.borrow_mut().push(p);
            proven(&[(1, 99), (2, 20)], p)
        });
        // then: the own record is untouched and the lookup was not even asked about it
        assert_eq!(got[0], with_ip(1, 7), "an entry with hints stays as it is");
        assert_eq!(got[1], with_ip(2, 20));
        assert_eq!(
            *asked.borrow(),
            vec![id(2)],
            "only the empty entry is looked up"
        );
    }

    #[test]
    fn keeps_an_entry_the_lookup_knows_nothing_about() {
        // given
        let list = vec![key_only(2)];
        // when
        let got = with_known(list, |_| None);
        // then
        assert_eq!(got, vec![key_only(2)]);
    }

    #[test]
    fn of_an_empty_list_is_empty() {
        // when
        let got = with_known(Vec::new(), |p| proven(&[(2, 20)], p));
        // then
        assert!(got.is_empty());
    }
}

mod build_and_parse {
    use super::*;

    #[test]
    fn member_addresses_round_trip() {
        // given: the issuer, B with a proven address, C without
        let me = with_ip(1, 7);
        let others = with_known(vec![key_only(2), key_only(3)], |p| proven(&[(2, 20)], p));
        // when
        let text =
            build_keyed(&head(), &me, &others, &PrimaryIps::NONE).expect("the invite is built");
        let parsed = parse(&text).expect("the invite is parsed");
        // then
        assert_eq!(parsed.peers.len(), 3);
        assert_eq!(
            parsed.peers[1].hints,
            with_ip(2, 20).hints,
            "B keeps the address"
        );
        assert!(parsed.peers[2].hints.is_empty(), "C stays a key alone");
    }

    #[test]
    fn five_records_with_one_address_each_fit() {
        // given: the maximum number of records, every one with a single IPv4 address
        let all: Vec<PeerAddr> = (1..=5).map(|n| with_ip(n, 100 + n as u16)).collect();
        // when
        let text = build_keyed(&head(), &all[0], &all[1..], &PrimaryIps::NONE)
            .expect("five records with one address each fit");
        // then
        assert!(text.len() <= MAX_INVITE_CHARS, "{} characters", text.len());
        assert!(
            body_len(&text) <= MAX_INVITE_BYTES,
            "{} bytes",
            body_len(&text)
        );
        let parsed = parse(&text).expect("the invite is parsed");
        let got: Vec<PeerAddr> = parsed.peers;
        assert_eq!(got, all);
    }
}

fn id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn key_only(n: u8) -> PeerAddr {
    PeerAddr::new(id(n))
}

/// Node `n` with the documentation address `203.0.113.<last>:4000`.
fn with_ip(n: u8, last: u16) -> PeerAddr {
    let mut addr = key_only(n);
    let sock: SocketAddr = format!("203.0.113.{}:4000", last % 250 + 1)
        .parse()
        .unwrap();
    addr.hints.push(AddrHint::Ip(sock));
    addr
}

/// What a node proved: `(seed, last)` pairs; the same address as `with_ip` gives.
fn proven(table: &[(u8, u16)], peer: PeerId) -> Option<PeerAddr> {
    table
        .iter()
        .find(|(n, _)| id(*n) == peer)
        .map(|(n, last)| with_ip(*n, *last))
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

fn body_len(text: &str) -> usize {
    let body = text.strip_prefix(INVITE_PREFIX).expect("invite prefix");
    BASE32_NOPAD
        .decode(body.to_ascii_uppercase().as_bytes())
        .expect("base32 body")
        .len()
}
