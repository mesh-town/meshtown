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
            assert_eq!(
                parse_invite_line("invite Alice"),
                Some(Some("Alice".to_string()))
            );
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

/// `print_dealt_invite`: two journal lines under the `invite` tag, the second is the reach note.
#[test]
fn dealt_invite_logs_notes_and_reach() {
    let first_new_seq = crate::node::output::log_page(0, 1).last_seq;
    let dealt = crate::invite::Dealt {
        text: "the-string".to_string(),
        holders: Vec::new(),
    };
    print_dealt_invite(&dealt);
    let page = crate::node::output::log_page(first_new_seq, 100_000);
    let lines: Vec<String> = page
        .lines
        .into_iter()
        .filter(|l| l.tag == "invite")
        .map(|l| l.text)
        .collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[1], dealt.reach().console_line());
}
