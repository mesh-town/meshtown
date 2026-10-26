//! Tests of the Managed network key: the error cases and the printed form (`managed.rs`).

use super::support::*;
use super::*;
use crate::members::network_key::{parse_user_input, LegacyKey};
use data_encoding::HEXLOWER;

mod managed_errors {
    use super::*;

    #[test]
    fn a_raw_key_of_the_wrong_length_names_the_length() {
        // given
        let text = format!("work--mesh2-{}", b32(&[9u8; 63]));
        // when
        let err = parse_user_input(&text).unwrap_err();
        // then
        assert!(err.to_string().contains("64 bytes, got 63"), "{err}");
    }

    #[test]
    fn a_raw_key_with_a_character_outside_base32_does_not_decode() {
        // given
        let body = G2.rsplit_once("mesh2-").unwrap().1;
        let broken = format!("work--mesh2-{}1{}", &body[..10], &body[11..]);
        // when
        let err = parse_user_input(&broken).unwrap_err();
        // then
        assert!(
            err.to_string().contains("does not decode from base32"),
            "{err}"
        );
    }

    #[test]
    fn a_creator_off_the_curve_is_a_damaged_key_in_both_forms() {
        // given
        let off = off_curve_bytes();
        assert!(
            PeerId::from_bytes(&off).is_err(),
            "the fixture must be off the curve"
        );
        let mut material = hex32(BASE_HEX).to_vec();
        material.extend_from_slice(&off);
        let forms = [
            format!("work--mesh2-{}", b32(&material)),
            format!("work--{TAIL}-{}", b32(&off)),
        ];
        for text in forms {
            // when
            let err = parse_user_input(&text).unwrap_err();
            // then
            assert!(
                err.to_string()
                    .contains("creator part of the key is damaged"),
                "{text}: {err}"
            );
        }
    }

    #[test]
    fn a_fourth_part_of_51_characters_is_neither_a_key_nor_an_outdated_one() {
        // given
        let labelled = format!("work--{TAIL}-{}", &ROOT52[..51]);
        let bare = format!("{TAIL}-{}", &ROOT52[..51]);
        // when
        let labelled_err = parse_user_input(&labelled).unwrap_err();
        let bare_err = parse_user_input(&bare).unwrap_err();
        // then
        assert!(labelled_err.to_string().contains("got 4"), "{labelled_err}");
        assert!(
            bare_err.to_string().contains("only the label"),
            "{bare_err}"
        );
        assert!(labelled_err.downcast_ref::<LegacyKey>().is_none());
        assert!(bare_err.downcast_ref::<LegacyKey>().is_none());
    }
}

mod one_label_separator {
    use super::*;

    #[test]
    fn labelled_forms_have_exactly_one_double_hyphen() {
        for text in [G1, G2] {
            assert_eq!(text.matches("--").count(), 1, "{text}");
        }
    }

    #[test]
    fn unlabelled_forms_have_none() {
        for text in [G3, G4] {
            assert_eq!(text.matches("--").count(), 0, "{text}");
        }
    }
}

mod debug_hides_base {
    use super::*;

    #[test]
    fn the_printed_input_shows_neither_the_base_nor_the_key_bytes() {
        let base_words = BASE_HEX;
        let base_all = b32(&hex32(BASE_HEX));
        let base_b32 = &base_all[..base_all.len() - 1];
        let base_decimal = "218, 242, 0, 143";
        let inputs = [
            G1.to_string(),
            G2.to_string(),
            G3.to_string(),
            G4.to_string(),
        ];
        for text in &inputs {
            // when
            let printed = format!("{:?}", parse(text));
            // then
            for secret in [base_words, base_b32, base_decimal, KEY_HEX] {
                assert!(
                    !printed.contains(secret),
                    "{text}: the output holds {secret}: {printed}"
                );
            }
        }
    }
}

mod reply_secret_managed_is_base {
    use super::*;

    #[test]
    fn it_is_the_base_not_the_derived_key() {
        for text in [G1, G2, G3, G4] {
            // when
            let input = parse(text);
            // then
            assert_eq!(HEXLOWER.encode(&input.reply_secret()), BASE_HEX, "{text}");
            assert_ne!(input.reply_secret(), input.key.to_bytes(), "{text}");
        }
    }
}
