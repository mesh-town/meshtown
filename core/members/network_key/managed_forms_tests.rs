//! Tests of the Managed network key: the Peer forms staying unchanged, the old parser rejecting the
//! Managed forms, and the error cases (`managed.rs`).

// path: `parse_user_input` is reached through the parent re-export, `parse_peer_only` through the sibling
// `input` module (visible to descendants of `network_key`). Adjust if visibility changes.
use super::super::input::parse_peer_only;
use super::support::*;
use super::*;
use crate::members::network_key::parse_user_input;
use data_encoding::HEXLOWER;

mod peer_forms_unchanged {
    use super::*;

    #[test]
    fn every_form_is_a_peer_network_with_the_golden_key() {
        for (text, label, from_words) in peer_forms() {
            // when
            let input = parse(&text);
            // then
            assert_eq!(input.kind, NetKind::Peer, "{text}");
            assert_eq!(input.label.as_deref(), label, "{text}");
            assert_eq!(input.from_words, from_words, "{text}");
            assert_eq!(HEXLOWER.encode(&input.key.to_bytes()), BASE_HEX, "{text}");
            assert_eq!(input.key.subkeys().network_id_hex(), BASE_ID, "{text}");
        }
    }

    #[test]
    fn the_base_and_the_reply_secret_are_the_key_itself() {
        for (text, _, _) in peer_forms() {
            // when
            let input = parse(&text);
            // then
            assert_eq!(input.base, input.key.to_bytes(), "{text}");
            assert_eq!(input.reply_secret(), input.key.to_bytes(), "{text}");
        }
    }
}

mod old_parser_rejects_managed {
    use super::*;

    #[test]
    fn every_managed_form_is_an_error_not_a_peer_key() {
        for text in [G1, G2, G3, G4] {
            // when
            let result = parse_peer_only(text);
            // then
            assert!(result.is_err(), "the Peer parser accepted {text}");
        }
    }

    #[test]
    fn the_words_form_fails_on_the_count_of_parts() {
        let err = parse_peer_only(G1).unwrap_err();
        assert!(err.to_string().contains("got 4"), "{err}");
    }

    #[test]
    fn the_raw_form_with_a_label_fails_on_the_count_of_parts() {
        let err = parse_peer_only(G2).unwrap_err();
        assert!(err.to_string().contains("got 2"), "{err}");
    }

    #[test]
    fn the_forms_without_a_label_fail_for_lack_of_a_random_part() {
        for text in [G3, G4] {
            let err = parse_peer_only(text).unwrap_err();
            assert!(err.to_string().contains("no random part"), "{text}: {err}");
        }
    }
}

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
    fn a_fourth_part_of_51_characters_is_the_peer_parser_error() {
        for prefix in ["work--", ""] {
            // given
            let text = format!("{prefix}{TAIL}-{}", &ROOT52[..51]);
            // when
            let ours = parse_user_input(&text).unwrap_err();
            // then
            let theirs = parse_peer_only(&text).unwrap_err();
            assert_eq!(ours.to_string(), theirs.to_string(), "{text}");
        }
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
        let base_b32 = &BASE_MESH1["mesh1-".len()..BASE_MESH1.len() - 1];
        let base_decimal = "218, 242, 0, 143";
        let inputs = [
            G1.to_string(),
            G2.to_string(),
            format!("work--{TAIL}"),
            format!("work--{BASE_MESH1}"),
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
