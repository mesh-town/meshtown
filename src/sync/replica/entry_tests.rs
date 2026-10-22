//! Tests for `entry.rs`: the bytes to sign, `seal`/`check`, the `wins` order.

use super::*;
use crate::net::id::NodeSecret;

fn secret(n: u8) -> NodeSecret {
    NodeSecret::from_bytes(&[n; 32])
}

fn rid(n: u8) -> ReplicaId {
    ReplicaId([n; 32])
}

fn sealed(n: u8, key: &[u8], value: &[u8], ts: u64) -> WireEntry {
    seal(&secret(n), &rid(1), key, value, ts)
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).expect("hex"))
        .collect()
}

mod signed_bytes {
    use super::*;

    #[test]
    fn layout_is_fixed() {
        // given: a fixed replica, author (a key from RFC 8032), key, value, ts_micros
        let author_hex = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
        let author = PeerId::from_bytes(&unhex(author_hex).try_into().unwrap()).unwrap();
        // when: we build the bytes to sign
        let got = signed_bytes(
            &rid(0x11),
            &author,
            b"node/name/",
            b"alpha",
            0x0102030405060708,
        );
        // then: they equal the reference (prefix, BE fields) — changing the format breaks the signatures of the whole network
        let want = "6d657368746f776e207265706c69636120656e747279207631001111111111111111111111111111111111111111111111111111111111111111d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a01020304050607080000000a6e6f64652f6e616d652f616c706861";
        assert_eq!(got, unhex(want));
    }
}

mod check {
    use super::*;

    #[test]
    fn sealed_passes() {
        let w = sealed(5, b"k", b"v", 10);
        assert_eq!(check(&rid(1), &w).ok(), Some(secret(5).peer_id()));
    }

    #[test]
    fn rejects_each_change() {
        let w = sealed(5, b"k", b"v", 10);
        let mut value = w.clone();
        value.value = b"w".to_vec();
        let mut ts = w.clone();
        ts.ts_micros = 11;
        let mut key = w.clone();
        key.key = b"j".to_vec();
        for bad in [value, ts, key] {
            assert!(matches!(check(&rid(1), &bad), Err(Reject::BadSignature)));
        }
        assert!(matches!(check(&rid(2), &w), Err(Reject::BadSignature)));
    }

    #[test]
    fn limits() {
        let long_key = sealed(5, &[b'k'; MAX_KEY_BYTES + 1], b"v", 1);
        assert!(matches!(check(&rid(1), &long_key), Err(Reject::KeyTooLong)));
        let long_value = sealed(5, b"k", &[0u8; 4097], 1);
        assert!(matches!(
            check(&rid(1), &long_value),
            Err(Reject::ValueTooLong)
        ));
        let at_limit = sealed(5, &[b'k'; MAX_KEY_BYTES], &[0u8; 4096], 1);
        assert!(check(&rid(1), &at_limit).is_ok());
        let mut off_curve = sealed(5, b"k", b"v", 1);
        off_curve.author = (0..=255u8)
            .map(|i| [i; 32])
            .find(|b| PeerId::from_bytes(b).is_err())
            .expect("a point not on the curve exists");
        assert!(matches!(check(&rid(1), &off_curve), Err(Reject::BadAuthor)));
    }
}

mod wins {
    use super::*;

    #[test]
    fn newer_ts_wins() {
        let old = sealed(5, b"k", b"zzz", 10);
        let new = sealed(5, b"k", b"a", 11);
        assert!(wins(&new, &old));
        assert!(!wins(&old, &new));
    }

    #[test]
    fn equal_ts_greater_value_wins_both_ways() {
        let a = sealed(5, b"k", b"a", 10);
        let b = sealed(5, b"k", b"b", 10);
        assert!(wins(&a, &b) != wins(&b, &a), "exactly one of the two");
        assert!(wins(&b, &a));
    }

    #[test]
    fn identical_never_wins() {
        let a = sealed(5, b"k", b"a", 10);
        assert!(!wins(&a, &a.clone()));
    }
}

mod to_entry {
    use super::*;

    #[test]
    fn copies_fields() {
        let w = sealed(5, b"k", b"v", 10);
        let e = to_entry(secret(5).peer_id(), &w);
        assert_eq!(
            (e.key.as_slice(), e.value.as_ref(), e.ts_micros),
            (&b"k"[..], &b"v"[..], 10)
        );
        assert_eq!(e.author, secret(5).peer_id());
    }
}

#[test]
fn protocol_strings_are_meshtown_v1() {
    assert_eq!(SIGN_CONTEXT, b"meshtown replica entry v1\0".as_slice());
}
