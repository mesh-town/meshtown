//! A string of the earlier kind (`mesh-invite1-`, network key inside): recognized but not accepted.

use super::*;

/// The T1 refusal text verbatim.
const T1: &str = "This invite is of the old format (mesh-invite1-): it contained the network key, such invites are no longer accepted. Ask a network member for a new invite — from the updated program.";

mod parse_of_it {
    use super::*;

    #[test]
    fn refuses_with_the_old_format_text() {
        // given
        let old = old_format_string();
        // when
        let err = parse(&old).unwrap_err();
        // then
        assert_eq!(format!("{err:#}"), T1);
    }

    #[test]
    fn refuses_with_the_same_text_in_upper_case_with_spaces() {
        // given
        let old = format!("  {}\n", old_format_string().to_uppercase());
        // when
        let err = parse(&old).unwrap_err();
        // then
        assert_eq!(format!("{err:#}"), T1);
    }

    #[test]
    fn does_not_say_expired_or_unreadable() {
        // given / when
        let err = parse(&old_format_string()).unwrap_err();
        // then
        let text = format!("{err:#}");
        assert!(
            !text.contains("unreadable"),
            "damage text instead of T1: {text}"
        );
        assert!(!text.contains("more than a day ago"), "T2 text instead of T1: {text}");
    }
}

mod looks_like_invite_of_it {
    use super::*;

    #[test]
    fn is_true_for_the_old_prefix() {
        assert!(looks_like_invite(&old_format_string()));
    }

    #[test]
    fn is_true_ignoring_case_and_spaces() {
        assert!(looks_like_invite(&format!(
            "  {}  ",
            old_format_string().to_uppercase()
        )));
    }

    #[test]
    fn is_true_for_both_prefixes() {
        assert!(looks_like_invite(OLD_INVITE_PREFIX));
        assert!(looks_like_invite(INVITE_PREFIX));
        assert_ne!(OLD_INVITE_PREFIX, INVITE_PREFIX);
    }
}

/// A plausible body of the earlier kind: version 1, 32 bytes of key, empty label, time.
fn old_format_string() -> String {
    let mut body = vec![1u8];
    body.extend_from_slice(&[9u8; 32]);
    body.extend_from_slice(&[0, 0x80, 0x80, 0x80, 0x80, 0x07, 0]);
    format!(
        "{OLD_INVITE_PREFIX}{}",
        BASE32_NOPAD.encode(&body).to_ascii_lowercase()
    )
}
