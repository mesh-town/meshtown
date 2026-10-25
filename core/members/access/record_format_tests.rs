//! Tests of the membership list record format (`record.rs`): value bytes, golden bytes, round trips.

use super::support_tests::*;
use super::*;

// --- encode ---

#[test]
fn admit_value_starts_with_version_one() {
    // given: an admit value
    let value = admit_of(base());
    // when: encoded
    let bytes = encode(&value);
    // then: the first byte is the version, the second the kind
    assert_eq!(bytes[0], 1, "version byte");
    assert_eq!(bytes[1], 1, "kind byte: Admit = 1");
}

#[test]
fn golden_bytes_of_admit_v1() {
    // given: the fixed value of the golden constant
    let value = admit_of(base());
    // when: encoded
    let bytes = encode(&value);
    // then: the bytes equal the hand-laid constant, and the key is prefix plus lowercase hex
    assert_eq!(
        bytes, GOLDEN_ADMIT_V1,
        "any shift of a field, width or version shows here"
    );
    let want_key = format!("acl/admit/58{}", "66".repeat(31));
    assert_eq!(admit_key(&base()), want_key.into_bytes());
}

#[test]
fn golden_bytes_decode_to_the_same_value() {
    // given: the golden bytes of version 1
    // when: decoded
    let decoded = decode(&GOLDEN_ADMIT_V1);
    // then: the value of the constant comes back
    assert_eq!(decoded, Ok(admit_of(base())));
}

#[test]
fn golden_bytes_of_exclude_at_moment_v1() {
    // given: an exclusion vote with the moment mark
    let value = exclude_at_moment_of(base());
    // when: encoded
    let bytes = encode(&value);
    // then: the bytes equal the hand-laid constant: version 1, via 3, no extra bytes
    assert_eq!(
        bytes, GOLDEN_EXCLUDE_AT_MOMENT_V1,
        "the mark is the via byte of version 1, nothing else"
    );
}

// --- decode ---

#[test]
fn roundtrips_every_kind() {
    // given: every kind with every via, and an empty note and a note of 80 two-byte characters
    let notes = [String::new(), "é".repeat(80)];
    for (kind, via, note) in combinations(&notes) {
        let value = AclValue {
            kind,
            subject: base(),
            via,
            made_unix: 1_792_400_000,
            note,
        };
        // when: encoded and decoded
        let back = decode(&encode(&value));
        // then: the same value (the note limit counts characters, not bytes)
        assert_eq!(back, Ok(value.clone()), "{value:?}");
    }
}

#[test]
fn roundtrips_extreme_timestamps() {
    for made_unix in [0, 127, 128, u64::MAX] {
        // given: a value with the timestamp at a varint width edge
        let value = AclValue {
            made_unix,
            ..admit_of(base())
        };
        // when / then: it survives encoding
        assert_eq!(decode(&encode(&value)), Ok(value), "made_unix {made_unix}");
    }
}

#[test]
fn enums_from_u8_reject_unknown_bytes() {
    // given / when / then: known bytes map to their variants, the others to nothing
    assert_eq!(AclKind::from_u8(1), Some(AclKind::Admit));
    assert_eq!(AclKind::from_u8(4), Some(AclKind::Boot));
    assert_eq!((AclKind::from_u8(0), AclKind::from_u8(9)), (None, None));
    assert_eq!(Via::from_u8(0), Some(Via::None));
    assert_eq!(Via::from_u8(3), Some(Via::Witness));
    assert_eq!((Via::from_u8(4), Via::from_u8(255)), (None, None));
}

#[test]
fn exclude_at_moment_decodes_under_v1() {
    // given: the golden bytes of an exclusion vote with the mark: version 1, via 3, no tail
    // when: decoded by the version 1 reader
    let decoded = decode(&GOLDEN_EXCLUDE_AT_MOMENT_V1);
    // then: the value comes back whole, the mark is the via of a plain vote
    assert_eq!(decoded, Ok(exclude_at_moment_of(base())));
    assert_eq!(decoded.map(|v| v.via), Ok(Via::Witness));
}
