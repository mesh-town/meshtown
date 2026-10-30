//! Tests of `node_name`: parsing a name from a document record, the node's effective name and the pure rule
//! "which names to put into `Members`" (`names_to_apply`). The network part
//! (publishing, a name change at a peer, refusal of an unfit one, an author that is offline) is checked by
//! `core/tests/node_live_name.rs`.

use std::collections::HashMap;

use crate::transport::net::{NodeSecret, PeerId};

use super::*;
use crate::members::membership::Members;
use crate::members::words::{default_node_name, MAX_NAME_BYTES};

mod parse_node_name {
    use super::*;

    mod good_value {
        use super::*;

        #[test]
        fn plain_name_is_returned_as_is() {
            // given an ordinary name with non-ASCII letters
            let bytes = "Laptop-Zoë".as_bytes();

            // when / then
            assert_eq!(parse_node_name(bytes), Some("Laptop-Zoë".to_string()));
        }

        #[test]
        fn spaces_around_are_trimmed() {
            // given spaces and a tab at the edges
            let bytes = "  name \t".as_bytes();

            // when / then
            assert_eq!(parse_node_name(bytes), Some("name".to_string()));
        }

        #[test]
        fn exactly_max_chars_is_accepted() {
            // given exactly 64 characters (the limit of `trim_and_validate_node_name`)
            let name = "a".repeat(64);

            // when / then
            assert_eq!(parse_node_name(name.as_bytes()), Some(name));
        }

        #[test]
        fn exactly_max_name_bytes_is_accepted() {
            // given 64 four-byte characters: exactly `MAX_NAME_BYTES` bytes, 64 characters
            let name = "😀".repeat(64);
            assert_eq!(
                name.len(),
                MAX_NAME_BYTES,
                "fixture: exactly the byte limit"
            );

            // when / then
            assert_eq!(parse_node_name(name.as_bytes()), Some(name));
        }
    }

    mod bad_value {
        use super::*;

        #[test]
        fn empty_is_rejected() {
            assert_rejected(b"");
        }

        #[test]
        fn only_spaces_is_rejected() {
            assert_rejected(b"   \t ");
        }

        #[test]
        fn control_char_is_rejected() {
            assert_rejected("a\u{7}b".as_bytes());
        }

        #[test]
        fn line_separator_is_rejected() {
            assert_rejected("a\u{2028}b".as_bytes());
        }

        #[test]
        fn not_utf8_is_rejected() {
            assert_rejected(&[0xFF, 0xFE]);
        }

        #[test]
        fn longer_than_max_chars_is_rejected() {
            // given 65 characters
            let name = "a".repeat(65);

            // when / then
            assert_rejected(name.as_bytes());
        }

        #[test]
        fn one_byte_over_limit_is_rejected_even_if_trim_would_fit() {
            // given a space and 64 four-byte characters: `MAX_NAME_BYTES + 1` bytes, after
            // trimming 64 characters — `trim_and_validate_node_name` would let it pass, the byte limit would not
            let name = format!(" {}", "😀".repeat(64));
            assert_eq!(name.len(), MAX_NAME_BYTES + 1, "fixture: one byte more");

            // when / then
            assert_rejected(name.as_bytes());
        }
    }
}

mod effective_node_name {
    use super::*;

    #[test]
    fn no_name_gives_default_by_id() {
        // given there is no name in the config
        let id = peer();

        // when / then
        assert_eq!(
            effective_node_name(None, id),
            default_node_name(id.as_bytes())
        );
    }

    #[test]
    fn empty_name_gives_default_by_id() {
        let id = peer();
        assert_eq!(
            effective_node_name(Some(""), id),
            default_node_name(id.as_bytes())
        );
    }

    #[test]
    fn name_with_control_char_gives_default_by_id() {
        let id = peer();
        assert_eq!(
            effective_node_name(Some("a\u{7}b"), id),
            default_node_name(id.as_bytes())
        );
    }

    #[test]
    fn good_name_is_trimmed() {
        // given a name with spaces at the edges
        let id = peer();

        // when / then
        assert_eq!(effective_node_name(Some(" name "), id), "name");
    }
}

mod names_to_apply {
    use super::*;

    mod own_id {
        use super::*;

        #[test]
        fn own_record_is_not_applied() {
            // given the read has the node's own record
            let mut fx = setup();
            let me = fx.me;

            // when
            let applied = fx.when_read(&[(me, "me")]);

            // then the own name is not in the answer
            assert!(applied.is_empty(), "own name is not applied: {applied:?}");
        }
    }

    mod first_appearance {
        use super::*;

        #[test]
        fn author_not_connected_with_old_name_from_disk_is_applied() {
            // given the author is offline, `Members` has its old name from disk (scenario c)
            let mut fx = setup();
            let a = peer();
            fx.members.set_name(a, "old".to_string());

            // when
            let applied = fx.when_read(&[(a, "new")]);

            // then the name from the document is applied on top
            assert_eq!(applied, vec![(a, "new".to_string())]);
        }

        #[test]
        fn author_connected_with_other_name_is_applied() {
            // given the author is online, the handshake gave another name (scenarios a and b)
            let mut fx = setup();
            let a = peer();
            fx.members.add_verified_conn(a);
            fx.members.set_name(a, "from handshake".to_string());

            // when
            let applied = fx.when_read(&[(a, "from document")]);

            // then the document overrides: it converges to the truth
            assert_eq!(applied, vec![(a, "from document".to_string())]);
        }

        #[test]
        fn author_without_any_name_in_members_is_applied() {
            // given `Members` has no name of the author
            let mut fx = setup();
            let a = peer();

            // when
            let applied = fx.when_read(&[(a, "name")]);

            // then the name is applied
            assert_eq!(applied, vec![(a, "name".to_string())]);
        }

        #[test]
        fn several_authors_are_all_applied() {
            // given two authors for the first time
            let mut fx = setup();
            let (a, b) = (peer(), peer());

            // when
            let applied = fx.when_read(&[(a, "first"), (b, "second")]);

            // then both are applied
            let want = HashMap::from([(a, "first".to_string()), (b, "second".to_string())]);
            assert_eq!(applied.len(), 2, "both authors in the answer: {applied:?}");
            assert_eq!(applied.into_iter().collect::<HashMap<_, _>>(), want);
        }
    }

    mod repeated_read {
        use super::*;

        #[test]
        fn same_name_second_time_gives_nothing() {
            // given the author's name was already read once
            let mut fx = setup();
            let a = peer();
            fx.members.add_verified_conn(a);
            fx.when_read(&[(a, "name")]);

            // when the same name is read again
            let applied = fx.when_read(&[(a, "name")]);

            // then there is nothing to apply
            assert!(
                applied.is_empty(),
                "the same name is applied again: {applied:?}"
            );
        }

        #[test]
        fn changed_name_of_connected_author_is_applied() {
            // given the author is online, its name was already read
            let mut fx = setup();
            let a = peer();
            fx.members.add_verified_conn(a);
            fx.when_read(&[(a, "old")]);

            // when the name changed in the document
            let applied = fx.when_read(&[(a, "new")]);

            // then the new one is applied
            assert_eq!(applied, vec![(a, "new".to_string())]);
        }

        #[test]
        fn name_changed_back_is_applied_again() {
            // given the name changed from "a" to "b" and back
            let mut fx = setup();
            let a = peer();
            fx.when_read(&[(a, "a")]);
            fx.when_read(&[(a, "b")]);

            // when
            let applied = fx.when_read(&[(a, "a")]);

            // then the comparison is with the last read one, not the first
            assert_eq!(applied, vec![(a, "a".to_string())]);
        }
    }

    mod author_gone_from_read {
        use super::*;

        #[test]
        fn nothing_is_applied_and_members_name_is_untouched() {
            // given the author's name was read, `Members` has the same (scenario d)
            let mut fx = setup();
            let a = peer();
            fx.when_read(&[(a, "name")]);
            fx.members.set_name(a, "name".to_string());

            // when the author vanished from the read (no record or garbage)
            let applied = fx.when_read(&[]);

            // then nothing is applied, the name in `Members` stays the same
            assert!(
                applied.is_empty(),
                "a vanished author is touched: {applied:?}"
            );
            assert_eq!(fx.members.name_of(a), Some("name".to_string()));
        }

        #[test]
        fn same_name_after_return_gives_nothing() {
            // given the author was read, then vanished from the read for a tick
            let mut fx = setup();
            let a = peer();
            fx.when_read(&[(a, "name")]);
            fx.when_read(&[]);

            // when it returned with the same name
            let applied = fx.when_read(&[(a, "name")]);

            // then `seen` did not forget the vanishing — there is nothing to apply
            assert!(
                applied.is_empty(),
                "the name was applied again: {applied:?}"
            );
        }
    }
}

// --- setup and helpers ---

struct Fixture {
    me: PeerId,
    members: Members,
    seen: HashMap<PeerId, String>,
}

/// Empty `Members`, empty `seen`, own id — fresh.
fn setup() -> Fixture {
    Fixture {
        me: peer(),
        members: Members::default(),
        seen: HashMap::new(),
    }
}

impl Fixture {
    /// One watcher tick: `names_to_apply` over the read `(author, name)` pairs.
    fn when_read(&mut self, read: &[(PeerId, &str)]) -> Vec<(PeerId, String)> {
        let read = read.iter().map(|(id, n)| (*id, n.to_string())).collect();
        names_to_apply(&mut self.seen, read, self.me, &self.members)
    }
}

/// A new random member id.
fn peer() -> PeerId {
    NodeSecret::generate().peer_id()
}

fn assert_rejected(bytes: &[u8]) {
    assert_eq!(
        parse_node_name(bytes),
        None,
        "unfit name accepted: {bytes:?}"
    );
}

/// `name_attempt_line`: the journal text for the outcome of a publish attempt.
mod name_attempt_line {
    use super::*;

    #[test]
    fn written() {
        let line = name_attempt_line("a", &Ok(true)).expect("a line");
        assert!(line.contains("\"a\""), "{line}");
        assert!(line.contains("written to the network document"), "{line}");
    }

    #[test]
    fn already_current() {
        assert_eq!(name_attempt_line("a", &Ok(false)), None);
    }

    #[test]
    fn failed() {
        let line = name_attempt_line("a", &Err(anyhow::anyhow!("boom"))).expect("a line");
        assert!(line.contains("name not written"), "{line}");
        assert!(line.contains("boom"), "{line}");
    }
}
