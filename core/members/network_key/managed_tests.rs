//! Tests of the Managed network key: both text forms, derivation, and the golden strings (`managed.rs`).

// path: `parse_user_input` is reached through the parent re-export, `parse_peer_only` through the sibling
// `input` module (visible to descendants of `network_key`). Adjust if visibility changes.
use super::support::*;
use super::*;
use crate::members::network_key::{parse_user_input, BadWord};
use data_encoding::{BASE32_NOPAD, HEXLOWER};

mod managed_golden_words {
    use super::*;

    #[test]
    fn label_kind_base_key_and_id_are_the_golden_ones() {
        // when
        let input = parse(G1);
        // then
        assert_managed(&input, Some("work"), true);
    }

    #[test]
    fn a_typo_in_a_word_names_the_word() {
        // given
        let typo = G1.replace("abacus", "abacuz");
        // when
        let err = parse_user_input(&typo).unwrap_err();
        // then
        let bad = err.downcast_ref::<BadWord>().expect("a bad word error");
        assert_eq!(bad.word, "abacuz");
    }
}

mod managed_golden_raw {
    use super::*;

    #[test]
    fn label_kind_base_key_and_id_are_the_golden_ones() {
        // when
        let input = parse(G2);
        // then
        assert_managed(&input, Some("work"), false);
    }

    #[test]
    fn upper_case_is_read_the_same() {
        // when
        let input = parse(&G2.to_uppercase());
        // then
        assert_managed(&input, Some("work"), false);
    }
}

mod managed_golden_unlabelled {
    use super::*;

    #[test]
    fn raw_form_has_no_label_and_the_same_key() {
        // when
        let input = parse(G3);
        // then
        assert_managed(&input, None, false);
    }

    #[test]
    fn words_form_has_no_label_and_the_same_key() {
        // when
        let input = parse(G4);
        // then
        assert_managed(&input, None, true);
    }
}

mod forms_agree {
    use super::*;

    #[test]
    fn writers_produce_exactly_the_golden_strings() {
        // given
        let root = root_of(7);
        let base = hex32(BASE_HEX);
        // when / then
        assert_eq!(format_managed_key(Some("work"), TAIL, &root), G1);
        assert_eq!(managed_key_with_label(Some("work"), &base, &root), G2);
        assert_eq!(managed_key_with_label(None, &base, &root), G3);
        assert_eq!(format_managed_key(None, TAIL, &root), G4);
    }

    #[test]
    fn both_forms_parse_to_one_key() {
        // when
        let (words, raw) = (parse(G1), parse(G2));
        // then
        assert_eq!(words.key.to_bytes(), raw.key.to_bytes());
        assert_eq!(words.base, raw.base);
        assert_eq!(words.kind, raw.kind);
    }

    #[test]
    fn the_key_derives_from_base_and_creator() {
        // when
        let key = managed_key(&hex32(BASE_HEX), &root_of(7));
        // then
        assert_eq!(HEXLOWER.encode(&key.to_bytes()), KEY_HEX);
    }

    #[test]
    fn the_golden_creator_is_the_node_id_of_seed_seven() {
        assert_eq!(HEXLOWER.encode(root_of(7).as_bytes()), ROOT_HEX);
        assert_eq!(
            BASE32_NOPAD
                .encode(root_of(7).as_bytes())
                .to_ascii_lowercase(),
            ROOT52
        );
    }
}

mod other_root_other_network {
    use super::*;

    #[test]
    fn the_words_form_with_another_creator_is_another_network() {
        // given
        let other = root_of(8);
        let text = format!("work--{TAIL}-{}", b32(other.as_bytes()));
        // when
        let input = parse(&text);
        // then
        assert_eq!(input.kind, NetKind::Managed { root: other });
        assert_eq!(HEXLOWER.encode(&input.key.to_bytes()), OTHER_KEY_HEX);
        assert_eq!(input.key.subkeys().network_id_hex(), OTHER_KEY_ID);
    }

    #[test]
    fn the_base_stays_the_same_and_only_the_creator_changes() {
        // given
        let text = format!("work--{TAIL}-{}", b32(root_of(8).as_bytes()));
        // when
        let input = parse(&text);
        // then
        assert_eq!(HEXLOWER.encode(&input.base), BASE_HEX);
        assert_ne!(input.key.to_bytes(), parse(G1).key.to_bytes());
    }

    #[test]
    fn the_raw_form_with_another_creator_agrees_with_the_words_form() {
        // given
        let other = root_of(8);
        // when
        let raw = parse(&managed_key_with_label(
            Some("work"),
            &hex32(BASE_HEX),
            &other,
        ));
        // then
        assert_eq!(HEXLOWER.encode(&raw.key.to_bytes()), OTHER_KEY_HEX);
    }
}
