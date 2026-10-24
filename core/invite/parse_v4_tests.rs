//! Tests of the invite string of format 4 (a Managed network: the body of format 3 plus the network kind and the
//! creator's node id): layout, parsing and refusals (`parse.rs`).

use std::net::SocketAddr;

use data_encoding::BASE32_NOPAD;

use super::*;
use crate::invite::{build_keyed, INVITE_PREFIX, MAX_INVITE_BYTES, MAX_INVITE_CHARS};
use crate::members::network_key::NetKind;
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
    fn keeps_the_kind_and_the_creator() {
        // given
        let rig = setup(Kind::Managed);
        // when
        let parsed = parse(&rig.text).expect("a version 4 invite parses");
        // then
        assert_eq!(parsed.head.kind, NetKind::Managed { root: rig.root });
    }

    #[test]
    fn keeps_the_head_and_the_members() {
        // given
        let rig = setup(Kind::Managed);
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

    #[test]
    fn a_peer_invite_still_parses_as_peer() {
        // given
        let rig = setup(Kind::Peer);
        // when
        let parsed = parse(&rig.text).expect("a version 3 invite parses");
        // then
        assert_eq!(parsed.head.kind, NetKind::Peer);
        assert_eq!(body_of(&rig.text)[0], 3);
    }
}

mod v4_layout {
    use super::*;

    #[test]
    fn is_version_four_then_the_v3_body_then_kind_and_creator() {
        // given: the same head and members issued as Peer (version 3) and as Managed (version 4)
        let managed = setup(Kind::Managed);
        let as_peer = InviteHead {
            kind: NetKind::Peer,
            ..managed.head.clone()
        };
        let v3 = build_text(&as_peer);
        // when
        let v4 = body_of(&managed.text);
        // then
        let mut want = vec![4u8];
        want.extend_from_slice(&body_of(&v3)[1..]);
        want.push(KIND_MANAGED);
        want.extend_from_slice(managed.root.as_bytes());
        assert_eq!(v4, want);
    }

    #[test]
    fn keeps_the_invite_prefix() {
        // given / when
        let rig = setup(Kind::Managed);
        // then
        assert!(rig.text.starts_with(INVITE_PREFIX), "{}", rig.text);
    }
}

mod v4_refusals {
    use super::*;

    #[test]
    fn unknown_kind_asks_to_update() {
        // given
        let rig = setup(Kind::Managed);
        let mut body = body_of(&rig.text);
        let at = body.len() - ROOT_LEN - 1;
        assert_eq!(body[at], KIND_MANAGED, "the kind byte precedes the creator");
        body[at] = KIND_UNKNOWN;
        // when
        let err = parse(&invite_string(&body)).unwrap_err();
        // then
        let text = format!("{err:#}");
        assert!(
            text.contains("unknown network type (2)") && text.contains("update the program"),
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
        let rig = setup(Kind::Managed);
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
        let rig = setup(Kind::Managed);
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
        let rig = setup(Kind::Managed);
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

// assumed: the format-3-only decoder of the previous program can't be called from here (`decode_invite_body`
// takes both versions now), so "version 4 gives `different format (4)` there" is not tested.

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

#[derive(Clone, Copy)]
enum Kind {
    Peer,
    Managed,
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
        kind: NetKind::Managed { root: peer(7) },
    }
}

/// An invite of the given kind with two members; Managed names `peer(7)` as the creator.
fn setup(kind: Kind) -> Rig {
    let mut head = managed_head(Some("work"));
    head.kind = match kind {
        Kind::Peer => NetKind::Peer,
        Kind::Managed => head.kind,
    };
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
