//! Tests of `invite.rs`: same network cases.

use super::*;
use crate::network_key::key_with_label;

#[test]
fn same_key_is_the_same_network() {
    assert!(same_network(&head_made_at(NOW), &key().encode()).unwrap());
}

#[test]
fn same_key_with_a_label_is_the_same_network() {
    let text = key_with_label(Some("our-game"), &key());
    assert!(same_network(&head_made_at(NOW), &text).unwrap());
}

#[test]
fn label_of_the_invite_does_not_matter() {
    let mut head = head_made_at(NOW);
    head.label = Some("foreign-label".to_string());
    assert!(same_network(&head, &key().encode()).unwrap());
}

#[test]
fn other_key_is_another_network() {
    let other = NetworkKey::from_bytes([8u8; 32]);
    assert!(!same_network(&head_made_at(NOW), &other.encode()).unwrap());
}

#[test]
fn one_differing_id_byte_is_enough_for_another_network() {
    for byte in 0..4 {
        let mut head = head_made_at(NOW);
        head.network_id[byte] ^= 1;
        assert!(
            !same_network(&head, &key().encode()).unwrap(),
            "byte {byte} of network_id is not compared"
        );
    }
}

#[test]
fn unparseable_key_is_an_error() {
    for bad in ["", "   ", "hello", "mesh1-", "mesh1-!!!!", "mesh1-aaaa"] {
        assert!(
            same_network(&head_made_at(NOW), bad).is_err(),
            "'{bad}' must not parse as a key"
        );
    }
}
