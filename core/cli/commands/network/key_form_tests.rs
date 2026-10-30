use super::*;
use crate::members::network_key::format_managed_key;
use crate::members::words::generate_tail;
use crate::transport::net::NodeSecret;

fn words_key() -> String {
    let root = NodeSecret::from_bytes(&[4; 32]).peer_id();
    format_managed_key(Some("our-game"), &generate_tail().unwrap(), &root)
}

#[test]
fn words_and_raw_are_one_network() {
    // given: a key in words
    let words = words_key();
    // when: it is turned into the machine form
    let raw = raw_form(&words).expect("raw form");
    // then: it parses to the same key bytes, label and root
    let a = parse_user_input(&words).unwrap();
    let b = parse_user_input(&raw).unwrap();
    assert!(raw.starts_with("our-game--mesh2-"), "{raw}");
    assert_eq!(a.key.to_bytes(), b.key.to_bytes());
    assert_eq!(a.label, b.label);
    assert_eq!(a.root, b.root);
}

#[test]
fn raw_of_raw_is_unchanged() {
    let raw = raw_form(&words_key()).unwrap();
    assert_eq!(raw_form(&raw).unwrap(), raw);
}

#[test]
fn garbage_is_refused() {
    assert!(raw_form("not a key at all").is_err());
}

#[test]
fn legacy_mesh1_key_is_refused_with_its_text() {
    let err = raw_form("mesh1-abcdefghijklmnopqrstuvwxyz234567").unwrap_err();
    assert!(
        err.downcast_ref::<crate::members::network_key::LegacyKey>()
            .is_some(),
        "{err:#}"
    );
}
