//! Tests of `parse_user_input`: Managed forms parse, keys of an older Mesh Town are recognized, garbage is not.

use super::*;
use crate::members::network_key::{format_managed_key, managed_key, test_net};

/// A random part of the older words form.
const TAIL: &str = "finch-unwary-k7m2q9x4rb8vhn3tz01g5";
/// The older machine form of some key.
const MACHINE: &str = "mesh1-xov27y2noadcysy647h6sizm4kqfwjkltwacll7cwsdi3ixguhaa";

fn is_outdated(text: &str) -> bool {
    let err = parse_user_input(text).unwrap_err();
    format!("{err:#}") == LEGACY_KEY_TEXT && err.downcast_ref::<LegacyKey>().is_some()
}

#[test]
fn four_part_words_parse() {
    // given
    let root = test_net("x").root;
    let text = format_managed_key(Some("work"), TAIL, &root);
    // when
    let input = parse_user_input(&text).unwrap();
    // then
    assert_eq!(input.root, root);
    assert_eq!(input.label.as_deref(), Some("work"));
    assert!(input.from_words);
    assert_eq!(
        input.key.to_bytes(),
        managed_key(&input.base, &root).to_bytes()
    );
}

#[test]
fn mesh2_parses_with_its_root() {
    // given
    let net = test_net("work");
    let bare = net.text.rsplit_once("--").unwrap().1.to_string();
    for (text, label) in [(net.text.clone(), Some("work")), (bare, None)] {
        // when
        let input = parse_user_input(&text).unwrap();
        // then
        assert_eq!(input.root, net.root, "{text}");
        assert_eq!(input.label.as_deref(), label, "{text}");
        assert!(!input.from_words, "{text}");
    }
}

#[test]
fn garbage_is_not_called_outdated() {
    for text in ["hello", "x--a-b"] {
        // when
        let err = parse_user_input(text).unwrap_err();
        // then
        assert_ne!(format!("{err:#}"), LEGACY_KEY_TEXT, "{text}");
    }
}

mod an_old_key {
    use super::*;

    #[test]
    fn machine_form_is_outdated() {
        assert!(is_outdated(MACHINE));
    }

    #[test]
    fn labelled_machine_form_is_outdated() {
        assert!(is_outdated(&format!("work--{MACHINE}")));
    }

    #[test]
    fn any_case_and_spaces_is_outdated() {
        assert!(is_outdated(&format!("  {}  ", MACHINE.to_uppercase())));
        assert!(is_outdated(&format!(
            "work--  {}  ",
            MACHINE.to_uppercase()
        )));
    }

    #[test]
    fn word_form_is_outdated() {
        assert!(is_outdated(TAIL));
    }

    #[test]
    fn labelled_word_form_is_outdated() {
        assert!(is_outdated(&format!("work--{TAIL}")));
    }

    #[test]
    fn typo_in_a_word_is_outdated() {
        assert!(is_outdated(&format!(
            "work--{}",
            TAIL.replace("unwary", "unwarz")
        )));
    }
}

mod edges {
    use super::*;

    #[test]
    fn empty_says_not_set() {
        for text in ["", "   "] {
            let err = parse_user_input(text).unwrap_err();
            assert!(err.to_string().contains("is not set"), "{err}");
        }
    }

    #[test]
    fn mesh1_in_the_label_is_not_old() {
        // given
        let net = test_net("mesh1 test");
        // when
        let input = parse_user_input(&net.text).unwrap();
        // then
        assert_eq!(input.root, net.root);
        assert!(!is_outdated(&format!("mesh1 test--{TAIL}-x")));
    }

    #[test]
    fn a_typo_in_a_managed_word_is_a_bad_word() {
        // given
        let root = test_net("x").root;
        let text = format_managed_key(Some("work"), &TAIL.replace("finch", "finck"), &root);
        // when
        let err = parse_user_input(&text).unwrap_err();
        // then
        assert_eq!(err.downcast_ref::<BadWord>().unwrap().word, "finck");
    }
}
