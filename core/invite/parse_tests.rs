//! Tests of the invite versions the parser refuses by their first byte (`parse.rs`).

use data_encoding::BASE32_NOPAD;

use super::*;
use crate::invite::{build_keyed, INVITE_PREFIX};
use crate::transport::addr_order::PrimaryIps;
use crate::transport::net::{NodeSecret, PeerAddr, PeerId};

fn peer(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

/// A valid current invite string.
fn current() -> String {
    let head = InviteHead {
        secret: InviteSecret::from_bytes([0x11; 32]),
        network_id: [0xaa, 0xbb, 0xcc, 0xdd],
        label: None,
        made_unix: 1_760_000_000,
        root: peer(7),
    };
    build_keyed(&head, &PeerAddr::new(peer(1)), &[], &PrimaryIps::NONE)
        .expect("the invite is built")
}

/// The string with its first body byte replaced by `version`; with `tail` the body is that byte and `tail`.
fn with_version(version: u8, tail: Option<&[u8]>) -> String {
    let text = current();
    let body = text.strip_prefix(INVITE_PREFIX).expect("prefix");
    let mut bytes = BASE32_NOPAD
        .decode(body.to_ascii_uppercase().as_bytes())
        .expect("base32 body");
    if let Some(tail) = tail {
        bytes.truncate(1);
        bytes.extend_from_slice(tail);
    }
    bytes[0] = version;
    format!(
        "{INVITE_PREFIX}{}",
        BASE32_NOPAD.encode(&bytes).to_ascii_lowercase()
    )
}

#[test]
fn a_version_3_invite_is_outdated() {
    // given: a string whose version byte is 3
    let text = with_version(3, None);
    // when
    let err = parse(&text).unwrap_err();
    // then
    assert_eq!(format!("{err:#}"), TEXT_V3_INVITE);
}

#[test]
fn a_version_3_byte_with_garbage_is_still_outdated() {
    // given: the byte 3 and then bytes that no format reads
    let text = with_version(3, Some(&[0xff; 7]));
    // when
    let err = parse(&text).unwrap_err();
    // then
    assert_eq!(format!("{err:#}"), TEXT_V3_INVITE);
}

#[test]
fn a_version_2_invite_keeps_its_text() {
    // given
    let text = with_version(2, None);
    // when
    let err = parse(&text).unwrap_err();
    // then
    assert_eq!(format!("{err:#}"), TEXT_V2_INVITE);
}

#[test]
fn a_version_5_is_a_different_format() {
    // given
    let text = with_version(5, None);
    // when
    let err = parse(&text).unwrap_err();
    // then
    let text = format!("{err:#}");
    assert!(
        text.contains("different format (5)") && text.contains("update the program"),
        "{text}"
    );
}
