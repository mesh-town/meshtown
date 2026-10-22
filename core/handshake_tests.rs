//! Tests of the handshake on the network secret (`handshake.rs`).

use super::*;
use crate::net::{NodeSecret, PeerId};

fn ids() -> (PeerId, PeerId) {
    let a = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    let b = NodeSecret::from_bytes(&[2u8; 32]).peer_id();
    (a, b)
}

#[test]
fn tags_match_for_same_inputs() {
    let (a, b) = ids();
    let key = [3u8; 32];
    let nonce_a = [4u8; 32];
    let nonce_b = [5u8; 32];
    assert_eq!(
        responder_tag(&key, &nonce_a, &nonce_b, &a, &b),
        responder_tag(&key, &nonce_a, &nonce_b, &a, &b)
    );
    assert_eq!(
        initiator_tag(&key, &nonce_a, &nonce_b, &a, &b),
        initiator_tag(&key, &nonce_a, &nonce_b, &a, &b)
    );
}

#[test]
fn tags_differ_by_role_and_ids() {
    let (a, b) = ids();
    let key = [3u8; 32];
    let nonce_a = [4u8; 32];
    let nonce_b = [5u8; 32];
    assert_ne!(
        responder_tag(&key, &nonce_a, &nonce_b, &a, &b),
        initiator_tag(&key, &nonce_a, &nonce_b, &a, &b)
    );
    assert_ne!(
        responder_tag(&key, &nonce_a, &nonce_b, &a, &b),
        responder_tag(&key, &nonce_a, &nonce_b, &b, &a)
    );
}

#[test]
fn tags_differ_by_key() {
    let (a, b) = ids();
    let nonce_a = [4u8; 32];
    let nonce_b = [5u8; 32];
    let key1 = [3u8; 32];
    let key2 = [9u8; 32];
    assert_ne!(
        responder_tag(&key1, &nonce_a, &nonce_b, &a, &b),
        responder_tag(&key2, &nonce_a, &nonce_b, &a, &b)
    );
}

#[test]
fn name_frame_round_trip() {
    for name in ["finch-unwary-4c7d9", "厨房🍳"] {
        let frame = name_frame(name);
        assert_eq!(parse_name_tail(&frame), Some(name.to_string()));
    }
    // Edge of both limits: 64 chars, 256 bytes.
    let long_name = "𝄞".repeat(64);
    assert_eq!(long_name.chars().count(), 64);
    assert_eq!(long_name.len(), 256);
    let frame = name_frame(&long_name);
    assert_eq!(frame.len(), 258);
    assert_eq!(parse_name_tail(&frame), Some(long_name));
}

#[test]
fn name_frame_empty_or_long_is_zero() {
    assert_eq!(name_frame(""), vec![0, 0]);
    let too_long = "é".repeat(129);
    assert_eq!(too_long.len(), 258);
    assert_eq!(name_frame(&too_long), vec![0, 0]);
}

#[test]
fn parse_name_tail_rejects() {
    assert_eq!(parse_name_tail(&[]), None);
    assert_eq!(parse_name_tail(&[0]), None);
    assert_eq!(parse_name_tail(&[0, 0]), None);
    assert_eq!(parse_name_tail(&[0, 5, b'a']), None);
    assert_eq!(parse_name_tail(&[0, 2, 0xff, 0xfe]), None);
    assert_eq!(parse_name_tail(&[1, 1]), None);
    let mut tail = vec![0, 5];
    tail.extend_from_slice("a\nb".as_bytes());
    assert_eq!(parse_name_tail(&tail), None);
}

#[test]
fn parse_name_tail_ignores_trailing() {
    let mut tail = name_frame("厨房🍳");
    tail.extend_from_slice(&[9, 9, 9]);
    assert_eq!(parse_name_tail(&tail), Some("厨房🍳".to_string()));
}
