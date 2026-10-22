//! The string holds no network key: a newcomer gets it from a member, not from the text.

use super::*;

#[test]
fn decoded_body_has_no_run_of_key_bytes() {
    // given
    let net_key = distinctive_key();
    let body = decoded_body(&invite_of_network(&net_key));
    // when / then
    assert!(
        !shares_run(&body, &net_key.to_bytes(), 8),
        "the invite body has a piece of the network key"
    );
}

#[test]
fn decoded_body_has_no_key_text() {
    // given
    let net_key = distinctive_key();
    let body = decoded_body(&invite_of_network(&net_key));
    let key_text = net_key.encode();
    let key_base32 = BASE32_NOPAD.encode(&net_key.to_bytes());
    // then
    for needle in [
        key_text.as_str(),
        key_base32.as_str(),
        &key_base32.to_ascii_lowercase(),
    ] {
        assert!(
            !contains_run(&body, needle.as_bytes()),
            "the invite body records the key as text: {needle}"
        );
    }
}

#[test]
fn string_has_no_key_base32() {
    // given
    let net_key = distinctive_key();
    let s = invite_of_network(&net_key);
    // then
    let key_base32 = BASE32_NOPAD
        .encode(&net_key.to_bytes())
        .to_ascii_lowercase();
    assert!(!s.contains(&key_base32), "the string has the network key in base32");
    assert!(!s.contains(&net_key.encode()), "the string has the network key mesh1-…");
}

#[test]
fn body_carries_secret_and_network_id_instead() {
    // given
    let net_key = distinctive_key();
    let head = head_of_network(&net_key);
    // when
    let body = decoded_body(&invite_of_network(&net_key));
    // then
    assert!(
        contains_run(&body, head.secret.as_bytes()),
        "no secret in the body"
    );
    assert!(
        contains_run(&body, &head.network_id),
        "no network marker in the body"
    );
}

#[test]
fn body_starts_with_version_two_and_network_id() {
    // given
    let net_key = distinctive_key();
    let head = head_of_network(&net_key);
    // when
    let body = decoded_body(&invite_of_network(&net_key));
    // then
    assert_eq!(body[0], 2, "format number");
    assert_eq!(
        body[1..5],
        head.network_id,
        "network marker follows the format number"
    );
}

/// A key without repeated bytes: an 8-byte piece will not match the secret `0xC7…` or the addresses by chance.
fn distinctive_key() -> NetworkKey {
    NetworkKey::from_bytes(core::array::from_fn(|i| 0x21 + (i as u8) * 5))
}

fn head_of_network(net_key: &NetworkKey) -> InviteHead {
    InviteHead {
        network_id: net_key.subkeys().network_id,
        ..head_made_at(NOW)
    }
}

fn invite_of_network(net_key: &NetworkKey) -> String {
    build(
        &head_of_network(net_key),
        &with_relay(1),
        &[with_relay(2)],
        &PrimaryIps::NONE,
    )
    .unwrap()
}

/// Whether `body` has at least one piece of `n` consecutive bytes of `key`.
fn shares_run(body: &[u8], key: &[u8], n: usize) -> bool {
    key.windows(n).any(|piece| contains_run(body, piece))
}

fn contains_run(body: &[u8], piece: &[u8]) -> bool {
    body.windows(piece.len()).any(|w| w == piece)
}
