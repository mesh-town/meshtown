//! Tests for `program_version`: parsing the program version value from the network's shared document
//! and the entry format. The network part (publishing, a peer reading it, overwriting)
//! is checked by `core/tests/node_program_version.rs`.

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

mod report_attempt {
    use super::*;

    mod written {
        use super::*;

        #[test]
        fn finishes_the_task() {
            let f = setup();
            assert!(f.when_reported(Ok(true)));
        }

        #[test]
        fn logs_the_announced_version() {
            let f = setup();
            f.when_reported(Ok(true));
            let want = format!(
                "announced program version {} to peers",
                current_program_version()
            );
            assert!(
                f.logged("docs").contains(&want),
                "log: {:?}",
                f.logged("docs")
            );
        }
    }

    mod already_current {
        use super::*;

        #[test]
        fn finishes_the_task() {
            let f = setup();
            assert!(f.when_reported(Ok(false)));
        }

        #[test]
        fn logs_nothing() {
            let f = setup();
            f.when_reported(Ok(false));
            assert_eq!(f.logged("docs"), Vec::<String>::new());
        }
    }

    mod failed {
        use super::*;

        #[test]
        fn asks_for_a_retry() {
            let f = setup();
            assert!(!f.when_reported(Err(failure())));
        }

        #[test]
        fn logs_the_error_with_its_cause() {
            let f = setup();
            f.when_reported(Err(failure()));
            let lines = f.logged("docs");
            assert!(
                lines
                    .iter()
                    .any(|l| l.starts_with("program version not written: ")
                        && l.contains("outer-marker")
                        && l.contains("inner-marker")),
                "log: {lines:?}"
            );
        }
    }
}

/// Journal lines written after `setup`. The journal is one per process, so the tests of this
/// module assume a process of their own (as `nextest` runs them).
struct Fixture {
    first_new_seq: u64,
}

impl Fixture {
    fn when_reported(&self, attempt: Result<bool>) -> bool {
        report_attempt(attempt)
    }

    /// Texts of the lines with the given tag written since `setup`.
    fn logged(&self, tag: &str) -> Vec<String> {
        let page = crate::node::output::log_page(self.first_new_seq, 100_000);
        let lines = page.lines.into_iter();
        lines.filter(|l| l.tag == tag).map(|l| l.text).collect()
    }
}

fn setup() -> Fixture {
    let first_new_seq = crate::node::output::log_page(0, 1).last_seq;
    Fixture { first_new_seq }
}

fn failure() -> anyhow::Error {
    anyhow::anyhow!("inner-marker").context("outer-marker")
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
