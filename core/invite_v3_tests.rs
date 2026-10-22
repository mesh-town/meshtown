//! Tests of invite v3 (member records without hints, dialing by key): issuing and parsing.

use std::net::SocketAddr;

use crate::transport::net::{AddrHint, NodeSecret, PeerAddr, PeerId};
use data_encoding::BASE32_NOPAD;

use super::*;
use crate::transport::addr_order::PrimaryIps;

const T: u64 = 1_760_000_000;
/// `T` as postcard varint.
const T_VARINT: [u8; 5] = [0x80, 0xf0, 0x9d, 0xc7, 0x06];

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
    }
}

fn body_of(text: &str) -> Vec<u8> {
    let body = text
        .strip_prefix(INVITE_PREFIX)
        .expect("prefix mesh-invite2-");
    BASE32_NOPAD
        .decode(body.to_ascii_uppercase().as_bytes())
        .unwrap()
}

fn invite_string(body: &[u8]) -> String {
    format!(
        "{INVITE_PREFIX}{}",
        BASE32_NOPAD.encode(body).to_ascii_lowercase()
    )
}

#[test]
fn keyed_build_keeps_key_only_entries() {
    let s = build_keyed(&head(), &with_ip(1), &[key_only(2)], &PrimaryIps::NONE).unwrap();
    let parsed = parse(&s).unwrap();
    let ids: Vec<_> = parsed.peers.iter().map(|a| a.id).collect();
    assert_eq!(ids, vec![id(1), id(2)]);
    assert!(parsed.peers[1].hints.is_empty(), "{:?}", parsed.peers[1]);
}

#[test]
fn keyed_body_starts_with_version_three() {
    let s = build_keyed(&head(), &with_ip(1), &[], &PrimaryIps::NONE).unwrap();
    assert!(s.starts_with("mesh-invite2-"));
    assert_eq!(body_of(&s)[0], 3);
}

#[test]
fn keyed_build_accepts_key_only_entries() {
    // given: nobody has hints (like `hyper`, which dials by key)
    let s = build_keyed(&head(), &key_only(1), &[key_only(2)], &PrimaryIps::NONE).unwrap();
    // then: the invite was built, both records without hints
    let parsed = parse(&s).unwrap();
    assert_eq!(parsed.peers.len(), 2);
    assert!(parsed.peers.iter().all(|p| p.hints.is_empty()));
}

#[test]
fn plain_build_still_drops_key_only() {
    let err = build(&head(), &key_only(1), &[key_only(2)], &PrimaryIps::NONE).unwrap_err();
    assert!(err.to_string().contains("nothing to invite with"), "{err}");
}

#[tokio::test]
async fn make_on_mem_is_v3() {
    // given: a node on `mem`, with no hints at its own address
    let cfg = crate::node::NodeConfig::for_loopback_test(
        crate::members::network_key::NetworkKey::generate().unwrap(),
    );
    let node = crate::node::start_node(cfg).await.unwrap();
    // when: the node builds an invite
    let issued = make(&node.invite_source(), None).unwrap();
    // then: format 3, a record of itself without hints
    assert_eq!(body_of(&issued.text)[0], 3);
    let parsed = parse(&issued.text).unwrap();
    assert_eq!(parsed.peers[0].id, node.id);
    assert!(parsed.peers[0].hints.is_empty(), "{:?}", parsed.peers[0]);
    node.shutdown().await;
}

#[tokio::test]
async fn make_names_listed_members_without_a_connection() {
    use crate::members::access::tally::{Exclusion, How, Ledger};
    use crate::members::access::view::Verdicts;
    use std::collections::{BTreeMap, BTreeSet};
    // given: a node with no connections, whose list admits B and C and has excluded D
    let cfg = crate::node::NodeConfig::for_loopback_test(
        crate::members::network_key::NetworkKey::generate().unwrap(),
    );
    let node = crate::node::start_node(cfg).await.unwrap();
    let src = node.invite_source();
    let excluded = Exclusion {
        how: How::Votes,
        at_micros: 10,
        voters: Vec::new(),
    };
    let ledger = Ledger {
        admitted: [node.id, id(2), id(3), id(4)].into_iter().collect(),
        excluded: BTreeMap::from([(id(4), excluded)]),
        pending: BTreeMap::new(),
        need: 1,
        admissions: BTreeMap::new(),
    };
    let verdicts = Verdicts {
        me: node.id,
        ledger: std::sync::Arc::new(ledger),
        my_votes: BTreeSet::new(),
        blocks: BTreeSet::new(),
        reasons: Default::default(),
    };
    src.members.access().publish(verdicts);
    // when: it builds an invite and the string is parsed
    let issued = make(&src, None).unwrap();
    let parsed = parse(&issued.text).unwrap();
    // then: the string names the issuer, B and C, and not D
    let got: BTreeSet<PeerId> = parsed.peers.iter().map(|a| a.id).collect();
    assert_eq!(got, BTreeSet::from([node.id, id(2), id(3)]));
    node.shutdown().await;
}

#[test]
fn v2_literal_still_parses() {
    // given: a literal of invite v2 (the same as in `invite_golden_tests`)
    const INVITE: &str =
        "mesh-invite2-allk27cqadd4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pahxytkqmaoxlkmadavrbk35ks762pewibz2b3qxf462uyrsllycdjupob2rdiaacad7aaaadia7qd34jvig";
    // when: parsing
    let parsed = parse(INVITE).unwrap();
    // then: version 2, one record with an address
    assert_eq!(body_of(INVITE)[0], 2);
    assert_eq!(parsed.peers.len(), 1);
    assert!(!parsed.peers[0].hints.is_empty());
}

#[test]
fn keyed_invite_golden_body() {
    let mut body = vec![0x03, 0xaa, 0xbb, 0xcc, 0xdd, 0x00];
    body.extend_from_slice(&[0x11; 32]);
    body.extend_from_slice(&T_VARINT);
    body.push(0x01);
    body.extend_from_slice(id(1).as_bytes());
    body.extend_from_slice(&[0x00, 0x00]);
    body.extend_from_slice(&T_VARINT);
    let parsed = parse(&invite_string(&body)).unwrap();
    assert_eq!(parsed.head.network_id, [0xaa, 0xbb, 0xcc, 0xdd]);
    assert_eq!(parsed.head.secret, InviteSecret::from_bytes([0x11; 32]));
    assert_eq!(parsed.head.made_unix, T);
    assert_eq!(parsed.peers.len(), 1);
    assert_eq!(parsed.peers[0].id, id(1));
    assert!(parsed.peers[0].hints.is_empty());
}

#[test]
fn parse_accepts_v2_and_v3_rejects_four() {
    let s = build_keyed(&head(), &with_ip(1), &[], &PrimaryIps::NONE).unwrap();
    let mut body = body_of(&s);
    for (version, ok) in [(2u8, true), (3, true), (4, false)] {
        body[0] = version;
        let got = parse(&invite_string(&body));
        match ok {
            true => assert!(got.is_ok(), "version {version}: {got:?}"),
            false => {
                let err = got.unwrap_err().to_string();
                assert!(err.contains("different format (4)"), "{err}");
            }
        }
    }
}
