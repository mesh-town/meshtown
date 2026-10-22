//! Tests of `invite.rs`: key text cases.

use super::*;
use crate::network_key::key_with_label;

#[test]
fn without_a_label_it_is_the_plain_key() {
    assert_eq!(head_made_at(NOW).key_text(&key()), key().encode());
}

#[test]
fn with_a_label_it_is_label_dashes_key_and_parses_back() {
    let mut head = head_made_at(NOW);
    head.label = Some("our-game".to_string());
    let text = head.key_text(&key());
    assert_eq!(text, key_with_label(Some("our-game"), &key()));
    let back = crate::network_key::parse_user_input(&text).unwrap();
    assert_eq!(back.label.as_deref(), Some("our-game"));
    assert_eq!(back.key.encode(), key().encode());
}
