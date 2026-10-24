//! Tests of the values and entries the parser ignores (`record.rs`).

use super::support_tests::*;
use super::*;

#[test]
fn ignores_unknown_version() {
    // given: empty bytes, and well-formed bodies under versions 0 and 2
    // when / then: empty is told apart from a wrong version
    assert_eq!(decode(&[]), Err(Ignored::Empty));
    for version in [0, 2, 255] {
        let bytes = raw(Raw {
            version,
            ..Raw::default()
        });
        assert_eq!(
            decode(&bytes),
            Err(Ignored::BadVersion),
            "version {version}"
        );
    }
}

#[test]
fn ignores_unknown_kind() {
    // given: kinds outside 1..=5 and a via outside 0..=3 in otherwise good bodies
    // when / then: the kind and the via are told apart
    for kind in [0, 6, 255] {
        let bytes = raw(Raw {
            kind,
            ..Raw::default()
        });
        assert_eq!(decode(&bytes), Err(Ignored::UnknownKind), "kind {kind}");
    }
    let bytes = raw(Raw {
        via: 4,
        ..Raw::default()
    });
    assert_eq!(decode(&bytes), Err(Ignored::UnknownVia));
}

#[test]
fn ignores_trailing_bytes() {
    // given: a good value with one extra byte
    let mut bytes = encode(&admit_of(base()));
    bytes.push(0);
    // when / then: version 1 allows no tail
    assert_eq!(decode(&bytes), Err(Ignored::TrailingBytes));
}

#[test]
fn ignores_truncated_body() {
    // given: a good value without its last byte, and the version byte alone
    let mut bytes = encode(&admit_of(base()));
    bytes.pop();
    // when / then: the body is cut short
    assert_eq!(decode(&bytes), Err(Ignored::Malformed));
    assert_eq!(decode(&[1]), Err(Ignored::Malformed));
}

#[test]
fn ignores_non_utf8_note() {
    // given: a body whose note bytes are not UTF-8
    let bytes = raw(Raw {
        note: vec![0xFF, 0xFE],
        ..Raw::default()
    });
    // when / then: it is malformed
    assert_eq!(decode(&bytes), Err(Ignored::Malformed));
}

#[test]
fn ignores_note_over_80_chars() {
    // given: notes of 80 and 81 characters, and 81 two-byte characters
    let note = |s: String| {
        raw(Raw {
            note: s.into_bytes(),
            ..Raw::default()
        })
    };
    // when / then: 80 is fine, 81 is too long whatever the byte width
    assert!(decode(&note("a".repeat(80))).is_ok());
    assert!(decode(&note("é".repeat(80))).is_ok());
    assert!(
        decode(&note("a".repeat(64))).is_ok(),
        "an old note of 64 characters must still read"
    );
    assert_eq!(decode(&note("a".repeat(81))), Err(Ignored::NoteTooLong));
    assert_eq!(decode(&note("é".repeat(81))), Err(Ignored::NoteTooLong));
}

#[test]
fn ignores_subject_off_curve() {
    // given: a body whose subject bytes are not a point of the curve
    let bytes = raw(Raw {
        subject: off_curve(),
        ..Raw::default()
    });
    // when / then: the subject is rejected as such, not as a generic postcard error
    assert_eq!(decode(&bytes), Err(Ignored::BadSubject));
}
