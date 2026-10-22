//! Tests of `invite.rs`: secret debug hides bytes.

use super::*;

#[test]
fn plain_debug_prints_only_the_type_name() {
    let secret = InviteSecret::from_bytes([0xC7; 32]);
    assert_eq!(format!("{secret:?}"), "InviteSecret(..)");
}

#[test]
fn pretty_debug_shows_no_bytes() {
    let secret = InviteSecret::from_bytes([0xC7; 32]);
    let text = format!("{secret:#?}");
    assert!(text.contains("InviteSecret"), "{text}");
    assert_no_secret_bytes(&text);
}

#[test]
fn head_debug_keeps_the_secret_masked() {
    let head = head_made_at(NOW);
    let text = format!("{head:?}");
    assert!(text.contains("InviteSecret(..)"), "{text}");
    assert_no_secret_bytes(&text);
}
