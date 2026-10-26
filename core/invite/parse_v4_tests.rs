//! Tests of the invite string of format 4 (the body, then the network kind and the
//! creator's node id): layout, parsing and refusals (`parse.rs`).

use std::net::SocketAddr;

use data_encoding::BASE32_NOPAD;

use super::*;
use crate::invite::{build_keyed, INVITE_PREFIX, MAX_INVITE_BYTES, MAX_INVITE_CHARS};
use crate::transport::addr_order::PrimaryIps;
use crate::transport::net::endpoint_shim::{from_endpoint_addr, EndpointAddr};
use crate::transport::net::{NodeSecret, PeerAddr, PeerId};

const T: u64 = 1_760_000_000;
/// The byte after the member records: the network kind of a Managed network.
const KIND_MANAGED: u8 = 1;
/// A kind byte this program doesn't know.
const KIND_UNKNOWN: u8 = 2;
/// The creator's node id takes this many bytes at the very end of the body.
const ROOT_LEN: usize = 32;

mod v4_roundtrip {
    use super::*;

    #[test]
    fn a_managed_invite_parses_with_its_root() {
        // given
        let rig = setup();
        // when
        let parsed = parse(&rig.text).expect("a version 4 invite parses");
        // then
        assert_eq!(parsed.head.root, rig.root);
    }

    #[test]
    fn keeps_the_head_and_the_members() {
        // given
        let rig = setup();
        // when
        let parsed = parse(&rig.text).expect("a version 4 invite parses");
        // then
        assert_eq!(parsed.head.network_id, rig.head.network_id);
        assert_eq!(parsed.head.label, rig.head.label);
        assert_eq!(parsed.head.secret, rig.head.secret);
        assert_eq!(parsed.head.made_unix, T);
        let ids: Vec<PeerId> = parsed.peers.iter().map(|a| a.id).collect();
        assert_eq!(ids, vec![peer(1), peer(2)]);
    }
}

mod v4_layout {
    use super::*;

    #[test]
    fn is_version_four_then_the_body_then_kind_and_creator() {
        // given
        let rig = setup();
        // when
        let body = body_of(&rig.text);
        // then
        let tail = &body[body.len() - ROOT_LEN - 1..];
        assert_eq!(body[0], 4);
        assert_eq!(tail[0], KIND_MANAGED);
        assert_eq!(&tail[1..], rig.root.as_bytes());
    }

    #[test]
    fn keeps_the_invite_prefix() {
        // given / when
        let rig = setup();
        // then
        assert!(rig.text.starts_with(INVITE_PREFIX), "{}", rig.text);
    }
}

mod v4_refusals {
    use super::*;

    #[test]
    fn an_unknown_kind_byte_is_a_different_format() {
        // given
        let rig = setup();
        let mut body = body_of(&rig.text);
        let at = body.len() - ROOT_LEN - 1;
        assert_eq!(body[at], KIND_MANAGED, "the kind byte precedes the creator");
        body[at] = KIND_UNKNOWN;
        // when
        let err = parse(&invite_string(&body)).unwrap_err();
        // then
        let text = format!("{err:#}");
        assert!(
            text.contains("different format (4/2)") && text.contains("update the program"),
            "{text}"
        );
    }

    #[test]
    fn creator_off_the_curve_is_a_damaged_invite() {
        // given: a creator id that is not a curve point
        let off = off_curve_bytes();
        assert!(
            PeerId::from_bytes(&off).is_err(),
            "the test needs an id that `PeerId::from_bytes` refuses"
        );
        let rig = setup();
        let mut body = body_of(&rig.text);
        let from = body.len() - ROOT_LEN;
        body[from..].copy_from_slice(&off);
        // when
        let err = parse(&invite_string(&body)).unwrap_err();
        // then
        let text = format!("{err:#}");
        assert!(
            text.contains("invite is damaged") && text.contains("network creator key is not valid"),
            "{text}"
        );
    }

    #[test]
    fn cut_off_creator_is_a_damaged_invite() {
        // given
        let rig = setup();
        let mut body = body_of(&rig.text);
        body.truncate(body.len() - 10);
        // when
        let err = parse(&invite_string(&body)).unwrap_err();
        // then
        let text = format!("{err:#}");
        assert!(text.contains("invite is damaged"), "{text}");
    }
}

mod v5_different_format {
    use super::*;

    #[test]
    fn is_refused_with_the_format_number() {
        // given
        let rig = setup();
        let mut body = body_of(&rig.text);
        body[0] = 5;
        // when
        let err = parse(&invite_string(&body)).unwrap_err();
        // then
        let text = format!("{err:#}");
        assert!(
            text.contains("different format (5)") && text.contains("update the program"),
            "{text}"
        );
    }
}

mod v4_worst_case_fits {
    use super::*;

    #[test]
    fn five_members_with_relay_and_two_addresses_and_the_longest_label() {
        // given
        let label = "a".repeat(64);
        let members: Vec<PeerAddr> = (1..=5).map(|n| member(n, true)).collect();
        // when
        let text = build_keyed(
            &managed_head(Some(&label)),
            &members[0],
            &members[1..],
            &PrimaryIps::NONE,
        )
        .expect("the worst case fits the byte limit");
        // then
        assert_within_limits(&text);
        let parsed = parse(&text).expect("the worst case parses");
        assert_eq!(parsed.peers.len(), 5);
        assert_eq!(parsed.head.label.as_deref(), Some(label.as_str()));
    }

    #[test]
    fn five_members_without_relay_and_two_addresses() {
        // given
        let members: Vec<PeerAddr> = (1..=5).map(|n| member(n, false)).collect();
        // when
        let text = build_keyed(
            &managed_head(Some(&"a".repeat(64))),
            &members[0],
            &members[1..],
            &PrimaryIps::NONE,
        )
        .expect("the case fits the byte limit");
        // then
        assert_within_limits(&text);
        assert_eq!(parse(&text).expect("it parses").peers.len(), 5);
    }
}

struct Rig {
    head: InviteHead,
    root: PeerId,
    /// The invite string built from `head` and two members.
    text: String,
}

fn peer(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn managed_head(label: Option<&str>) -> InviteHead {
    InviteHead {
        secret: InviteSecret::from_bytes([0x11; 32]),
        network_id: [0xaa, 0xbb, 0xcc, 0xdd],
        label: label.map(str::to_string),
        made_unix: T,
        root: peer(7),
    }
}

/// An invite with two members; `peer(7)` is the creator.
fn setup() -> Rig {
    let head = managed_head(Some("work"));
    let text = build_text(&head);
    Rig {
        head,
        root: peer(7),
        text,
    }
}

fn build_text(head: &InviteHead) -> String {
    build_keyed(
        head,
        &PeerAddr::new(peer(1)),
        &[PeerAddr::new(peer(2))],
        &PrimaryIps::NONE,
    )
    .expect("the invite is built")
}

/// A member with two direct addresses and, if asked, a long relay address.
fn member(n: u8, relay: bool) -> PeerAddr {
    let ip = |s: &str| s.parse::<SocketAddr>().expect("address");
    let mut addr = EndpointAddr::new(peer(n))
        .with_ip_addr(ip("[2001:db8:ffff:ffff:ffff:ffff:ffff:fff1]:65535"))
        .with_ip_addr(ip("[2001:db8:ffff:ffff:ffff:ffff:ffff:fff2]:65535"));
    if relay {
        let labels = ["r".repeat(30), "s".repeat(30), "t".repeat(30)].join(".");
        let url = format!("https://{labels}.relay.example/");
        addr = addr.with_relay_url(url.parse().expect("url"));
    }
    from_endpoint_addr(&addr)
}

fn assert_within_limits(text: &str) {
    assert!(
        text.chars().count() <= MAX_INVITE_CHARS,
        "{} characters",
        text.chars().count()
    );
    assert!(
        body_of(text).len() <= MAX_INVITE_BYTES,
        "{} bytes",
        body_of(text).len()
    );
}

fn body_of(text: &str) -> Vec<u8> {
    let body = text
        .strip_prefix(INVITE_PREFIX)
        .expect("prefix mesh-invite2-");
    BASE32_NOPAD
        .decode(body.to_ascii_uppercase().as_bytes())
        .expect("base32 body")
}

fn invite_string(body: &[u8]) -> String {
    format!(
        "{INVITE_PREFIX}{}",
        BASE32_NOPAD.encode(body).to_ascii_lowercase()
    )
}

/// The first byte pattern `[i; 32]` that is not a curve point.
fn off_curve_bytes() -> [u8; 32] {
    (0..=255u8)
        .map(|i| [i; 32])
        .find(|bytes| PeerId::from_bytes(bytes).is_err())
        .expect("some repeated byte pattern is off the curve")
}
