//! Tests of the HOLD frames (`hold_wire.rs`): signature with the network key, length, compatibility with an old server.

use super::*;
use crate::invite::wire::{encode_invite_reply, parse_invite_request, REQUEST_LEN};

fn key() -> NetworkKey {
    NetworkKey::from_bytes([7u8; 32])
}

const HASH: [u8; 32] = [0x21; 32];
const MADE: u64 = 1_790_000_000;

#[test]
fn round_trip_gives_the_same_hash_and_time() {
    let frame = hold_frame(&key(), &HASH, MADE);
    let req = parse_hold(&key(), &frame).expect("the frame parses");
    assert_eq!(
        req,
        HoldReq {
            hash: HASH,
            made_unix: MADE
        }
    );
}

#[test]
fn is_73_bytes_starting_with_version_2() {
    let frame = hold_frame(&key(), &HASH, MADE);
    assert_eq!(frame.len(), 73);
    assert_eq!(frame[0], 2);
}

#[test]
fn wrong_key_is_rejected() {
    let frame = hold_frame(&key(), &HASH, MADE);
    let other = NetworkKey::from_bytes([8u8; 32]);
    assert!(parse_hold(&other, &frame).is_none());
}

#[test]
fn flipped_byte_in_hash_time_or_mac_is_rejected() {
    let frame = hold_frame(&key(), &HASH, MADE);
    for at in [1usize, 40, 41, 72] {
        let mut bad = frame;
        bad[at] ^= 1;
        assert!(parse_hold(&key(), &bad).is_none(), "byte {at}");
    }
}

#[test]
fn old_server_sees_bad_request_in_the_first_33_bytes() {
    let frame = hold_frame(&key(), &HASH, MADE);
    let mut head = [0u8; REQUEST_LEN];
    head.copy_from_slice(&frame[..REQUEST_LEN]);
    assert!(parse_invite_request(&head).is_none());
}

#[test]
fn ok_reply_round_trips() {
    let reply = hold_reply(RedeemCode::Ok);
    assert_eq!(reply[0], 2);
    assert_eq!(parse_hold_reply(&reply), Some(RedeemCode::Ok));
}

#[test]
fn redeem_reply_of_version_1_is_not_a_hold_answer() {
    let reply = encode_invite_reply(RedeemCode::BadRequest, None);
    assert_eq!(parse_hold_reply(&reply), None);
}

#[test]
fn nonzero_tail_is_rejected() {
    let mut reply = hold_reply(RedeemCode::Ok);
    reply[33] = 1;
    assert_eq!(parse_hold_reply(&reply), None);
}

#[test]
fn unknown_code_is_rejected() {
    let mut reply = hold_reply(RedeemCode::Ok);
    reply[1] = 200;
    assert_eq!(parse_hold_reply(&reply), None);
}
