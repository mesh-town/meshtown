//! Tests of `invite.rs`: hash is stable and domain separated.

use super::*;

#[test]
fn hash_is_sixty_four_lowercase_hex_chars() {
    let hash = a_secret().hash_hex();
    assert_eq!(hash.len(), 64, "{hash}");
    assert!(
        hash.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')),
        "{hash}"
    );
}

#[test]
fn same_secret_gives_the_same_hash() {
    assert_eq!(a_secret().hash_hex(), a_secret().hash_hex());
}

#[test]
fn secrets_differing_in_one_bit_give_different_hashes() {
    let mut bytes = *a_secret().as_bytes();
    bytes[31] ^= 1;
    let other = InviteSecret::from_bytes(bytes);
    assert_ne!(a_secret().hash_hex(), other.hash_hex());
}

#[test]
fn hash_follows_the_documented_derivation() {
    // The hash in the document must match across all program versions.
    let want = hex(blake3::derive_key(
        "meshtown invite secret v1",
        a_secret().as_bytes(),
    ));
    assert_eq!(a_secret().hash_hex(), want);
    // recompute: `b3sum --derive-key 'meshtown invite secret v1'` over the bytes 0x60..0x7f
    assert_eq!(
        a_secret().hash_hex(),
        "9a4bff2695d8de52541830b1d599b01ceb2b8b1062a5b83f4d2bcd1970ca253d"
    );
}

#[test]
fn hash_is_not_the_plain_blake3_of_the_secret() {
    let plain = hex(*blake3::hash(a_secret().as_bytes()).as_bytes());
    assert_ne!(
        a_secret().hash_hex(),
        plain,
        "hash without domain separation"
    );
}

#[test]
fn hash_is_not_any_network_subkey_made_of_the_same_bytes() {
    // The same 32 bytes as a network key: the invite hash must not match any subkey.
    let bytes = *a_secret().as_bytes();
    let sub = NetworkKey::from_bytes(bytes).subkeys();
    let hash = a_secret().hash_hex();
    for (name, derived) in [
        ("roster_key", sub.roster_key),
        ("handshake_key", sub.handshake_key),
        ("gossip_topic", sub.gossip_topic),
        ("docs_namespace_seed", sub.docs_namespace_seed),
        ("dht_signing_seed", sub.dht_signing_seed),
    ] {
        assert_ne!(hash, hex(derived), "hash matched subkey {name}");
    }
}

#[test]
fn hash_does_not_contain_the_secret() {
    let hash = a_secret().hash_hex();
    assert!(!hash.contains(&hex(*a_secret().as_bytes())), "{hash}");
}

#[test]
fn hash_hex_is_hex_of_hash_bytes() {
    for fill in [0u8, 0x5a, 0xff] {
        let secret = InviteSecret::from_bytes([fill; 32]);
        assert_eq!(secret.hash_hex(), hash_hex_of(&secret.hash_bytes()));
        assert_eq!(secret.hash_hex(), hex(secret.hash_bytes()));
    }
}
