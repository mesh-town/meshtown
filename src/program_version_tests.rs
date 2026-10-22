//! Tests for `program_version`: parsing the program version value from the network's shared document
//! and the entry format. The network part (publishing, a peer reading it, overwriting)
//! is checked by `tests/node_program_version.rs`.

use super::*;

mod parse {
    use super::*;

    mod valid_value {
        use super::*;

        #[test]
        fn accepts_release_prerelease_and_build() {
            // given
            let cases = ["0.1.0", "1.12.3-rc1", "2.0.0+build.5"];

            // when / then
            for version in cases {
                assert_parses_as_is(version.as_bytes());
            }
        }

        #[test]
        fn accepts_single_digit() {
            // given: the minimal valid length — one character
            assert_parses_as_is(b"7");
        }

        #[test]
        fn accepts_exactly_max_len() {
            // given: exactly 32 characters: `1.` and 30 digits
            let version = format!("1.{}", "0".repeat(30));
            assert_eq!(
                version.len(),
                32,
                "the test fixture must be exactly 32 characters"
            );

            // when / then
            assert_parses_as_is(version.as_bytes());
        }
    }

    mod invalid_value {
        use super::*;

        #[test]
        fn rejects_empty() {
            assert_all_rejected(&[b""]);
        }

        #[test]
        fn rejects_longer_than_max_len() {
            // given: 33 valid characters: `1.` and 31 digits
            let version = format!("1.{}", "0".repeat(31));
            assert_eq!(
                version.len(),
                33,
                "the test fixture must be exactly 33 characters"
            );

            // when / then
            assert_all_rejected(&[version.as_bytes()]);
        }

        #[test]
        fn rejects_space_and_foreign_chars() {
            assert_all_rejected(&[
                b"0.1 .0",
                b"0.1.0/",
                b"0.1.0;",
                b"0.1.0\n",
                "0.1.é".as_bytes(),
            ]);
        }

        #[test]
        fn rejects_non_utf8() {
            assert_all_rejected(&[&[0xFF, 0xFE]]);
        }

        #[test]
        fn rejects_nul_bytes() {
            assert_all_rejected(&[b"0.1.0\0", b"\0", b"\x000.1.0"]);
        }

        #[test]
        fn rejects_not_starting_with_digit() {
            // `null` is exactly what an invalid node writes instead of a version
            assert_all_rejected(&[b"null", b"v1.0.0", b"-1", b".1", b"+1"]);
        }
    }
}

mod current_build {
    use super::*;

    #[test]
    fn current_passes_own_parser() {
        // given: the version of this build; otherwise the node would announce a value that peers discard
        let current = current_program_version();

        // when / then
        assert_parses_as_is(current.as_bytes());
    }

    #[test]
    fn current_equals_cargo_package_version() {
        assert_eq!(current_program_version(), env!("CARGO_PKG_VERSION"));
    }
}

mod wire_format {
    use super::*;

    #[test]
    fn document_key_is_node_version() {
        // network format: old builds read only their own keys, a foreign one must not match
        assert_eq!(PROGRAM_VERSION_KEY, b"node/version/");
    }

    #[test]
    fn max_len_is_32() {
        assert_eq!(MAX_PROGRAM_VERSION_LEN, 32);
    }
}

/// A valid value is returned as the same string.
fn assert_parses_as_is(bytes: &[u8]) {
    let want = String::from_utf8(bytes.to_vec()).expect("a valid version is UTF-8");
    assert_eq!(
        parse_program_version(bytes),
        Some(want),
        "{bytes:?} must parse as is"
    );
}

/// Every value is invalid: the reader discards it, the member is left without a version.
fn assert_all_rejected(cases: &[&[u8]]) {
    for bytes in cases {
        assert_eq!(
            parse_program_version(bytes),
            None,
            "{bytes:?} must be discarded"
        );
    }
}
