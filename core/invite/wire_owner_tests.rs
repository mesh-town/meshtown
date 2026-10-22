//! Tests of the owner request frame (`wire.rs`, kind 3): the newcomer's permanent id and the proof of owning it.
//!
//! The frame is 129 bytes `[3][invite secret 32][owner id 32][signature 64]`; the reply is the usual
//! 34-byte frame. The signature itself is checked by `proof_tests.rs`; here only bytes and parsing.

use super::*;
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;

mod layout {
    use super::*;

    #[test]
    fn owner_request_layout_is_kind_secret_owner_sig() {
        // given: a secret, an owner and a signature with recognizable bytes
        let (secret, owner, sig) = (a_secret(), pid(7), a_sig());
        // when
        let frame = encode_owner_request(&secret, owner, &sig);
        // then: kind, secret, owner id, signature — in this order, and nothing else
        assert_eq!(frame.len(), OWNER_REQUEST_LEN);
        assert_eq!(OWNER_REQUEST_LEN, 129);
        assert_eq!(frame[0], OWNER_FRAME_KIND);
        assert_eq!(OWNER_FRAME_KIND, 3);
        assert_eq!(&frame[1..33], secret.as_bytes().as_slice(), "secret");
        assert_eq!(&frame[33..65], owner.as_bytes().as_slice(), "owner id");
        assert_eq!(&frame[65..129], sig.as_slice(), "signature");
    }

    #[test]
    fn owner_kind_differs_from_legacy_and_hold() {
        assert_ne!(OWNER_FRAME_KIND, REDEEM_FRAME_KIND);
        assert_ne!(OWNER_FRAME_KIND, crate::invite::hold_wire::HOLD_FRAME_KIND);
    }
}

mod parsing {
    use super::*;

    #[test]
    fn owner_request_roundtrips() {
        // given
        let (secret, owner, sig) = (a_secret(), pid(7), a_sig());
        let frame = encode_owner_request(&secret, owner, &sig);
        // when
        let req = parse_owner_request(&frame).expect("a well-formed owner frame parses");
        // then: all three fields come back
        assert_eq!(req.secret, secret);
        assert_eq!(req.owner, owner);
        assert_eq!(req.sig, sig);
    }

    #[test]
    fn owner_request_of_other_kind_is_none() {
        for kind in [0u8, 1, 2, 4, 255] {
            // given: a good frame with another kind in the first byte
            let mut frame = encode_owner_request(&a_secret(), pid(7), &a_sig());
            frame[0] = kind;
            // when / then
            assert!(
                parse_owner_request(&frame).is_none(),
                "kind {kind} was accepted as an owner frame"
            );
        }
    }

    #[test]
    fn owner_request_with_non_ed25519_owner_is_none() {
        // given: owner bytes that are not a point of the curve
        let mut frame = encode_owner_request(&a_secret(), pid(7), &a_sig());
        frame[33..65].copy_from_slice(&off_curve());
        // when / then
        assert!(
            parse_owner_request(&frame).is_none(),
            "an owner that is not an ed25519 key was accepted"
        );
    }

    #[test]
    fn signature_is_not_checked_by_the_parser() {
        // given: a signature that cannot be valid; checking it is the proof's job, not the parser's
        let frame = encode_owner_request(&a_secret(), pid(7), &[0u8; 64]);
        // when / then
        assert!(parse_owner_request(&frame).is_some());
    }
}

fn a_sig() -> [u8; 64] {
    core::array::from_fn(|i| 0xC0u8.wrapping_add(i as u8))
}

/// The first `[i; 32]` that is not a point of the curve.
fn off_curve() -> [u8; 32] {
    let bytes = (0..=255u8)
        .map(|i| [i; 32])
        .find(|b| PeerId::from_bytes(b).is_err());
    bytes.expect("some [i; 32] must be off the curve")
}
