//! Tests for `net/id.rs`: `PeerId` and `NodeSecret` (skeleton — `tests.md`).
//! Ids and secrets are built only through the public signatures: `NodeSecret::from_bytes`,
//! `PeerId::from_bytes`, `str::parse::<PeerId>()`.

use super::*;
use ed25519_dalek::VerifyingKey;
use std::collections::HashSet;

/// RFC 8032 vector, section 7.1, TEST 1: the private seed and the public key.
const RFC_SEED_HEX: &str = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
const RFC_PUBLIC_HEX: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";

#[path = "id_parse_tests.rs"]
mod parse_and_serde;

mod key_derivation {
    use super::*;

    #[test]
    fn rfc8032_test1_seed_gives_the_published_public_key() {
        // given: the seed of RFC 8032 vector #1
        let seed = hex32(RFC_SEED_HEX);
        // when: we derive the id from the secret
        let id = NodeSecret::from_bytes(&seed).peer_id();
        // then: it is the published public key
        assert_eq!(
            to_hex(id.as_bytes()),
            RFC_PUBLIC_HEX,
            "the id must be the ed25519 public key for the seed"
        );
    }

    #[test]
    fn same_seed_gives_the_same_id() {
        assert_eq!(id_of(7), id_of(7), "key derivation is deterministic");
    }

    #[test]
    fn different_seeds_give_different_ids() {
        assert_ne!(id_of(1), id_of(2), "different seeds cannot give one id");
    }
}

mod sign {
    use super::*;

    #[test]
    fn own_signature_verifies() {
        let secret = NodeSecret::from_bytes(&distinct_seed());
        let sig = secret.sign(b"message");
        assert!(secret.peer_id().verify(b"message", &sig));
    }

    #[test]
    fn other_key_rejected() {
        let a = NodeSecret::from_bytes(&[1u8; 32]);
        let b = NodeSecret::from_bytes(&[2u8; 32]);
        let sig = a.sign(b"message");
        assert!(!b.peer_id().verify(b"message", &sig));
    }

    #[test]
    fn changed_message_rejected() {
        let a = NodeSecret::from_bytes(&[1u8; 32]);
        let sig = a.sign(b"message");
        assert!(!a.peer_id().verify(b"messagE", &sig));
    }
}

mod secret {
    use super::*;

    #[test]
    fn to_bytes_returns_the_seed_it_was_built_from() {
        let seed = hex32(RFC_SEED_HEX);
        assert_eq!(NodeSecret::from_bytes(&seed).to_bytes(), seed);
    }

    #[test]
    fn generate_gives_different_secrets() {
        let first = NodeSecret::generate().to_bytes();
        let second = NodeSecret::generate().to_bytes();
        assert_ne!(first, second, "two generate() calls must not coincide");
    }

    #[test]
    fn generated_secret_gives_an_id_on_the_curve() {
        let id = NodeSecret::generate().peer_id();
        let checked = PeerId::from_bytes(id.as_bytes());
        assert_eq!(
            checked.ok(),
            Some(id),
            "an id from generate() passes from_bytes"
        );
    }

    #[test]
    fn clone_keeps_the_same_seed_and_id() {
        let secret = NodeSecret::from_bytes(&distinct_seed());
        let copy = secret.clone();
        assert_eq!(copy.to_bytes(), secret.to_bytes());
        assert_eq!(copy.peer_id(), secret.peer_id());
    }

    #[test]
    fn memory_is_zeroed_after_drop() {
        // given: the secret in a manual slot, to read its memory after destruction
        let seed = distinct_seed();
        assert_eq!(
            std::mem::size_of::<NodeSecret>(),
            32,
            "test assumption: the secret is exactly the seed"
        );
        let mut slot = std::mem::MaybeUninit::new(NodeSecret::from_bytes(&seed));
        let raw = slot.as_mut_ptr() as *const [u8; 32];
        // SAFETY: the slot is alive until the end of the function, the size is checked above; volatile read.
        let before = unsafe { std::ptr::read_volatile(raw) };
        assert_eq!(
            before, seed,
            "test assumption: the seed lies in the 32 bytes of the secret"
        );
        // when: the value is destroyed
        // SAFETY: the slot is initialized and no longer used as a value.
        unsafe { slot.assume_init_drop() };
        // then: the bytes are wiped with zeros
        // SAFETY: the slot's memory on the stack stays valid.
        let after = unsafe { std::ptr::read_volatile(raw) };
        assert_eq!(after, [0u8; 32], "Drop must wipe the seed (zeroize)");
    }

    mod debug_output {
        use super::*;

        #[test]
        fn is_a_fixed_masked_marker() {
            let text = format!("{:?}", NodeSecret::from_bytes(&distinct_seed()));
            assert_eq!(text, "NodeSecret(..)");
        }

        #[test]
        fn has_no_seed_byte_in_hex() {
            // given: a seed of distinct bytes, none of which matches the marker letters
            let seed = distinct_seed();
            let secret = NodeSecret::from_bytes(&seed);
            // when: we print in the plain and the "pretty" form
            for text in [format!("{secret:?}"), format!("{secret:#?}")] {
                let text = text.to_lowercase();
                // then: not a single seed byte as two hex digits
                for byte in seed {
                    let pair = format!("{byte:02x}");
                    assert!(
                        !text.contains(&pair),
                        "a seed byte {pair} was found in the secret's Debug: {text}"
                    );
                }
            }
        }
    }
}

mod text_form {
    use super::*;

    #[test]
    fn display_is_64_lowercase_hex_chars() {
        for id in sample_ids() {
            let text = id.to_string();
            assert_eq!(text, to_hex(id.as_bytes()), "Display = hex of the id bytes");
            assert_eq!(text.len(), 64);
            assert!(
                text.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')),
                "only lowercase hex digits: {text}"
            );
        }
    }

    #[test]
    fn fmt_short_is_the_first_10_chars_of_display() {
        for id in sample_ids() {
            assert_eq!(id.fmt_short(), id.to_string()[..10]);
        }
    }

    #[test]
    fn fmt_short_of_the_rfc_key_is_d75a980182() {
        let id = id_from_hex(RFC_PUBLIC_HEX);
        assert_eq!(id.fmt_short(), "d75a980182");
    }

    #[test]
    fn debug_wraps_the_full_hex_in_peerid() {
        let id = id_from_hex(RFC_PUBLIC_HEX);
        assert_eq!(format!("{id:?}"), format!("PeerId({RFC_PUBLIC_HEX})"));
    }

    #[test]
    fn as_ref_gives_the_same_bytes_as_as_bytes() {
        let id = id_of(3);
        let bytes: &[u8] = id.as_ref();
        assert_eq!(bytes, id.as_bytes().as_slice());
    }

    #[test]
    fn from_bytes_keeps_the_bytes_of_a_valid_point() {
        let public = hex32(RFC_PUBLIC_HEX);
        let id = PeerId::from_bytes(&public).expect("the RFC public key lies on the curve");
        assert_eq!(id.as_bytes(), &public);
    }
}

mod equality_and_order {
    use super::*;

    #[test]
    fn ids_built_from_equal_bytes_are_equal_and_hash_alike() {
        let set: HashSet<PeerId> = [id_of(5), id_of(5), id_of(6)].into_iter().collect();
        assert_eq!(set.len(), 2, "equal ids collapse in the set");
    }

    #[test]
    fn order_of_ids_follows_the_order_of_their_bytes() {
        let ids: Vec<PeerId> = (1..=8).map(id_of).collect();
        for a in &ids {
            for b in &ids {
                let want = a.as_bytes().cmp(b.as_bytes());
                assert_eq!(a.cmp(b), want, "Ord by bytes: {a} versus {b}");
                assert_eq!(
                    a.partial_cmp(b),
                    Some(want),
                    "PartialOrd is consistent with Ord"
                );
            }
        }
    }
}

mod error_text {
    use super::*;

    #[test]
    fn every_variant_has_a_non_empty_text() {
        for err in [
            IdError::Hex,
            IdError::Base32,
            IdError::Length,
            IdError::NotOnCurve,
        ] {
            assert!(!err.to_string().is_empty(), "{err:?} has an empty Display");
        }
    }

    #[test]
    fn id_error_is_a_std_error() {
        fn takes_error<E: std::error::Error>(_: &E) {}
        takes_error(&IdError::Hex);
    }
}

// --- setup and helpers ---

/// Node id with seed `[n; 32]`.
fn id_of(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

/// Five different ids for "for any id" checks.
fn sample_ids() -> Vec<PeerId> {
    (1..=5).map(id_of).collect()
}

/// A seed of distinct bytes 0x11..=0xea, among which there are no `de` and `ec`
/// (the two hex digit pairs that occur in the word "NodeSecret").
fn distinct_seed() -> [u8; 32] {
    std::array::from_fn(|i| 0x11 + (i as u8) * 7)
}

fn id_from_hex(text: &str) -> PeerId {
    PeerId::from_bytes(&hex32(text)).expect("in the test the point is surely on the curve")
}

/// The first pattern `[i; 32]`, i = 0..=255, that the ed25519 library rejects as a point.
fn first_off_curve_pattern() -> Option<[u8; 32]> {
    (0..=255u8)
        .map(|i| [i; 32])
        .find(|bytes| VerifyingKey::from_bytes(bytes).is_err())
}

fn off_curve_bytes() -> [u8; 32] {
    first_off_curve_pattern()
        .expect("a point off the curve is needed, see some_repeated_byte_pattern_*")
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex32(text: &str) -> [u8; 32] {
    assert_eq!(text.len(), 64, "the test expects 64 hex digits");
    std::array::from_fn(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).expect("hex"))
}

fn base32_nopad_upper(bytes: &[u8]) -> String {
    data_encoding::BASE32_NOPAD.encode(bytes)
}
