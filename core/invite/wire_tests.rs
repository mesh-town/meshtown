//! Tests of the frames of the invite key-issuing protocol (`wire.rs`).
//!
//! The legacy request is 33 bytes `[V][secret 32]`, the response is 34 bytes `[V][code][key 32 or zeros]`;
//! the codes are fixed by the protocol. The owner request (kind 3) is covered by `wire_owner_tests.rs`.

use super::*;
use crate::invite::store::IssueVerdict;
use crate::invite::InviteSecret;
use crate::members::network_key::NetworkKey;

mod constants {
    use super::*;

    #[test]
    fn protocol_name_and_frame_sizes_are_fixed() {
        assert_eq!(INVITE_ALPN, b"meshtown/invite/1");
        assert!(INVITE_ALPN.starts_with(b"meshtown/") && INVITE_ALPN.ends_with(b"/1"));
        assert_eq!(REDEEM_FRAME_KIND, 1);
        assert_eq!(REQUEST_LEN, 33);
        assert_eq!(REPLY_LEN, 34);
    }
}

mod request {
    use super::*;

    mod round_trip {
        use super::*;

        #[test]
        fn gives_the_same_secret() {
            let secret = a_secret();
            let frame = encode_invite_request(&secret);
            assert_eq!(parse_invite_request(&frame), Some(secret));
        }

        #[test]
        fn different_secrets_give_different_frames() {
            let other = InviteSecret::from_bytes([0x11; 32]);
            assert_ne!(
                encode_invite_request(&a_secret()),
                encode_invite_request(&other)
            );
        }
    }

    mod frame_layout {
        use super::*;

        #[test]
        fn version_byte_then_secret_bytes() {
            let secret = a_secret();
            let frame = encode_invite_request(&secret);
            assert_eq!(frame.len(), REQUEST_LEN);
            assert_eq!(frame[0], REDEEM_FRAME_KIND);
            assert_eq!(&frame[1..], secret.as_bytes().as_slice());
        }
    }

    mod foreign_version {
        use super::*;

        #[test]
        fn version_two_is_rejected() {
            let frame = request_with_version(2);
            assert_eq!(parse_invite_request(&frame), None);
        }

        #[test]
        fn version_zero_is_rejected() {
            let frame = request_with_version(0);
            assert_eq!(parse_invite_request(&frame), None);
        }

        #[test]
        fn version_255_is_rejected() {
            let frame = request_with_version(255);
            assert_eq!(parse_invite_request(&frame), None);
        }
    }
}

mod legacy_request {
    use super::*;

    #[test]
    fn legacy_request_kind_stays_1() {
        // given: the owner kind is new; the old request must stay byte for byte what an old client sends
        let frame = encode_invite_request(&a_secret());
        // then: kind 1, 33 bytes, and the owner kind is not mistaken for it
        assert_eq!(REDEEM_FRAME_KIND, 1);
        assert_eq!((frame[0], frame.len()), (1, 33));
        assert_ne!(OWNER_FRAME_KIND, REDEEM_FRAME_KIND);
    }

    #[test]
    fn legacy_parser_rejects_the_first_bytes_of_an_owner_frame() {
        // given: the first 33 bytes of an owner frame (what an old server reads)
        let owner = encode_owner_request(
            &a_secret(),
            crate::transport::net::contract::pid(7),
            &[0xC3; 64],
        );
        let mut head = [0u8; REQUEST_LEN];
        head.copy_from_slice(&owner[..REQUEST_LEN]);
        // when / then: the old parser answers None, so an old server says BadRequest
        assert_eq!(parse_invite_request(&head), None);
    }
}

mod reply {
    use super::*;

    mod ok_code {
        use super::*;

        #[test]
        fn frame_is_version_code_zero_then_key() {
            let frame = encode_invite_reply(RedeemCode::Ok, Some(&a_key()));
            assert_eq!(frame.len(), REPLY_LEN);
            assert_eq!(frame[0], REDEEM_FRAME_KIND);
            assert_eq!(frame[1], 0);
            assert_eq!(&frame[2..], a_key_bytes().as_slice());
        }

        #[test]
        fn carries_the_key_through_a_round_trip() {
            let frame = encode_invite_reply(RedeemCode::Ok, Some(&a_key()));
            assert_eq!(key_in(parse_invite_reply(&frame)), Some(a_key_bytes()));
        }

        #[test]
        fn without_a_key_the_tail_is_zero() {
            let frame = encode_invite_reply(RedeemCode::Ok, None);
            assert_eq!(&frame[2..], [0u8; 32].as_slice());
        }
    }

    mod refusal_codes {
        use super::*;

        #[test]
        fn used_has_thirty_two_zeros_in_the_tail() {
            let frame = encode_invite_reply(RedeemCode::Used, None);
            assert_eq!(frame[0], REDEEM_FRAME_KIND);
            assert_eq!(frame[1], 3);
            assert_eq!(&frame[2..], [0u8; 32].as_slice());
        }

        #[test]
        fn key_is_never_put_into_a_refusal() {
            // Leak insurance: even if the caller passed a key, a refusal sends zeros.
            for (code, number) in refusal_codes() {
                let frame = encode_invite_reply(code, Some(&a_key()));
                assert_eq!(
                    &frame[2..],
                    [0u8; 32].as_slice(),
                    "code {number} leaked the key in the tail"
                );
            }
        }

        #[test]
        fn every_refusal_survives_a_round_trip() {
            for (code, number) in refusal_codes() {
                let frame = encode_invite_reply(code, None);
                assert_eq!(
                    refusal_in(parse_invite_reply(&frame)),
                    Some(code),
                    "code {number}"
                );
            }
        }
    }

    mod malformed_frame {
        use super::*;

        #[test]
        fn unknown_code_ten_is_rejected() {
            let frame = raw_reply(REDEEM_FRAME_KIND, 10, [0; 32]);
            assert!(parse_invite_reply(&frame).is_none());
        }

        #[test]
        fn unknown_code_with_a_key_in_the_tail_is_rejected() {
            let frame = raw_reply(REDEEM_FRAME_KIND, 200, a_key_bytes());
            assert!(parse_invite_reply(&frame).is_none());
        }

        #[test]
        fn ok_with_a_zero_key_is_rejected() {
            let frame = raw_reply(REDEEM_FRAME_KIND, 0, [0; 32]);
            assert!(parse_invite_reply(&frame).is_none());
        }

        #[test]
        fn ok_written_without_a_key_is_rejected_on_read() {
            let frame = encode_invite_reply(RedeemCode::Ok, None);
            assert!(parse_invite_reply(&frame).is_none());
        }

        #[test]
        fn foreign_version_is_rejected_even_with_a_valid_key() {
            let frame = raw_reply(2, 0, a_key_bytes());
            assert!(parse_invite_reply(&frame).is_none());
        }

        #[test]
        fn foreign_version_is_rejected_for_a_refusal_too() {
            let frame = raw_reply(0, 3, [0; 32]);
            assert!(parse_invite_reply(&frame).is_none());
        }
    }
}

fn a_secret() -> InviteSecret {
    InviteSecret::from_bytes(core::array::from_fn(|i| 0xA0 + i as u8))
}

fn a_key_bytes() -> [u8; 32] {
    core::array::from_fn(|i| 1 + i as u8)
}

fn a_key() -> NetworkKey {
    NetworkKey::from_bytes(a_key_bytes())
}

fn request_with_version(version: u8) -> [u8; REQUEST_LEN] {
    let mut frame = encode_invite_request(&a_secret());
    frame[0] = version;
    frame
}

fn raw_reply(version: u8, code: u8, tail: [u8; 32]) -> [u8; REPLY_LEN] {
    let mut frame = [0u8; REPLY_LEN];
    frame[0] = version;
    frame[1] = code;
    frame[2..].copy_from_slice(&tail);
    frame
}

/// The key from a `Key` response, if the response is exactly that.
fn key_in(reply: Option<RedeemReply>) -> Option<[u8; 32]> {
    match reply {
        Some(RedeemReply::Key(key)) => Some(key.to_bytes()),
        _ => None,
    }
}

/// The code from a `Refused` response, if the response is exactly that.
fn refusal_in(reply: Option<RedeemReply>) -> Option<RedeemCode> {
    match reply {
        Some(RedeemReply::Refused(code)) => Some(code),
        _ => None,
    }
}

fn all_codes() -> [(RedeemCode, u8); 9] {
    [
        (RedeemCode::Ok, 0),
        (RedeemCode::NotReady, 1),
        (RedeemCode::Busy, 2),
        (RedeemCode::Used, 3),
        (RedeemCode::Expired, 4),
        (RedeemCode::Unknown, 5),
        (RedeemCode::BadRequest, 6),
        (RedeemCode::Internal, 7),
        (RedeemCode::Excluded, 8),
    ]
}

/// All codes except `Ok`: their response has no key.
fn refusal_codes() -> impl Iterator<Item = (RedeemCode, u8)> {
    all_codes().into_iter().filter(|(_, number)| *number != 0)
}

fn all_verdicts() -> [IssueVerdict; 5] {
    [
        IssueVerdict::Grant,
        IssueVerdict::Used,
        IssueVerdict::Expired,
        IssueVerdict::Unknown,
        IssueVerdict::NotReady,
    ]
}

#[path = "wire_managed_tests.rs"]
mod managed;
#[path = "wire_owner_tests.rs"]
mod owner_frame;
