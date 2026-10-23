//! Strings of earlier formats (`mesh-invite1-` with the network key inside, and format 2): recognized but not accepted.

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
        assert!(
            !text.contains("more than a day ago"),
            "T2 text instead of T1: {text}"
        );
    }
}

/// A string of format 2: the version byte is checked before the body is read.
const V2_INVITE: &str = "mesh-invite2-allk27cqadd4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pahxytkqmaoxlkmadavrbk35ks762pewibz2b3qxf462uyrsllycdjupob2rdiaacad7aaaadia7qd34jvig";

mod parse_of_a_version_two_body {
    use super::*;

    #[test]
    fn refuses_with_the_older_format_text() {
        // given / when
        let err = parse(V2_INVITE).unwrap_err();
        // then
        assert_eq!(format!("{err:#}"), TEXT_V2_INVITE);
    }

    #[test]
    fn refuses_with_the_same_text_in_upper_case_with_spaces() {
        // given
        let spaced = format!("  {}\n", V2_INVITE.to_uppercase());
        // when
        let err = parse(&spaced).unwrap_err();
        // then
        assert_eq!(format!("{err:#}"), TEXT_V2_INVITE);
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
