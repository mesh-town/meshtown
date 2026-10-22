//! Tests of the node console (`console.rs`).

use super::*;

/// `parse_invite_line`: `None` — not the `invite` command; `Some(None)` — without "for whom";
/// `Some(Some(text))` — with it (only the console).
mod invite_line_cases {
    use super::*;

    mod bare_command {
        use super::*;

        #[test]
        fn has_no_for_whom() {
            assert_eq!(parse_invite_line("invite"), Some(None));
        }

        #[test]
        fn ignores_spaces_and_line_end_around() {
            assert_eq!(parse_invite_line(" invite "), Some(None));
            assert_eq!(parse_invite_line("  invite \n"), Some(None));
        }
    }

    mod with_for_whom {
        use super::*;

        #[test]
        fn carries_the_text_after_the_command() {
            assert_eq!(parse_invite_line("invite Alice"), Some(Some("Alice".to_string())));
        }

        #[test]
        fn carries_several_words_without_the_edge_spaces() {
            assert_eq!(
                parse_invite_line("  invite   Alice's laptop \n"),
                Some(Some("Alice's laptop".to_string()))
            );
        }
    }

    mod other_lines {
        use super::*;

        #[test]
        fn word_starting_with_invite_is_not_the_command() {
            assert_eq!(parse_invite_line("invites"), None);
            assert_eq!(parse_invite_line("invited Alice"), None);
        }

        #[test]
        fn other_commands_are_not_the_invite() {
            assert_eq!(parse_invite_line("block example.com"), None);
            assert_eq!(parse_invite_line(""), None);
        }
    }
}
