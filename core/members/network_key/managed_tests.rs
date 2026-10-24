//! Tests of the Managed network key: both text forms, derivation, and that the Peer parser rejects them
//! (`managed.rs`).

// path: `parse_user_input` is reached through the parent re-export, `parse_peer_only` through the sibling
// `input` module (visible to descendants of `network_key`). Adjust if visibility changes.
use super::super::input::parse_peer_only;
use super::*;
use crate::members::network_key::{parse_user_input, BadWord};
use crate::transport::net::NodeSecret;
use data_encoding::{BASE32_NOPAD, HEXLOWER};

/// Random part shared by every golden string.
const TAIL: &str = "abacus-abdomen-0123456789abcdefghjkm";
/// Base32 of the creator's node id, creator = node secret `[7; 32]`.
const ROOT52: &str = "5jfgyy7ctrjavpxvkb5rglwf7gkuo5vox27hxescd3vgsfcg2iwa";
/// The creator's node id itself.
const ROOT_HEX: &str = "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c";
/// What `TAIL` derives to: the key bytes of the Peer network and the base of the Managed one.
const BASE_HEX: &str = "daf2008f0eb0190eb5be32c55e9ad92084b54d4d0b57b5d2786c67933c4efee6";
const BASE_ID: &str = "f704ebff";
const BASE_MESH1: &str = "mesh1-3lzabdyowamq5nn6glcv5gwzecclktknbnl3lutynrtzgpco73ta";
/// Key bytes and id of the Managed network made of `TAIL` and the creator `[7; 32]`.
const KEY_HEX: &str = "356cc856e9ee8cc150202a9e18dbab1a0d6f950ec6b075e76e03602ee4ec8671";
const KEY_ID: &str = "9d815c4a";
/// The same base with the creator `[8; 32]`.
const OTHER_KEY_HEX: &str = "37a95150ca9373b0ab8a87f19568ec158ad852758c0d62a7b929590e978e83a9";
const OTHER_KEY_ID: &str = "1be1fc57";

/// Words form with a label.
const G1: &str = "work--abacus-abdomen-0123456789abcdefghjkm-5jfgyy7ctrjavpxvkb5rglwf7gkuo5vox27hxescd3vgsfcg2iwa";
/// Raw form with a label.
const G2: &str = "work--mesh2-3lzabdyowamq5nn6glcv5gwzecclktknbnl3lutynrtzgpco73toustmmprjyuqkx32va6ytf3c7tfkho2xl5pt3sjbb52tjcrdnela";
/// Raw form without a label.
const G3: &str = "mesh2-3lzabdyowamq5nn6glcv5gwzecclktknbnl3lutynrtzgpco73toustmmprjyuqkx32va6ytf3c7tfkho2xl5pt3sjbb52tjcrdnela";
/// Words form without a label.
const G4: &str =
    "abacus-abdomen-0123456789abcdefghjkm-5jfgyy7ctrjavpxvkb5rglwf7gkuo5vox27hxescd3vgsfcg2iwa";

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

/// The four Peer shapes of the same random part: text, label, read from words.
fn peer_forms() -> Vec<(String, Option<&'static str>, bool)> {
    vec![
        (format!("work--{TAIL}"), Some("work"), true),
        (TAIL.to_string(), None, true),
        (format!("work--{BASE_MESH1}"), Some("work"), false),
        (BASE_MESH1.to_string(), None, false),
    ]
}

fn parse(text: &str) -> NetworkKeyInput {
    parse_user_input(text).unwrap_or_else(|e| panic!("{text}: {e}"))
}

/// The golden Managed network: creator `[7; 32]`, base derived from `TAIL`.
fn assert_managed(input: &NetworkKeyInput, label: Option<&str>, from_words: bool) {
    assert_eq!(input.label.as_deref(), label, "label");
    assert_eq!(input.from_words, from_words, "from_words");
    assert_eq!(input.kind, NetKind::Managed { root: root_of(7) }, "kind");
    assert_eq!(input.kind.root(), Some(root_of(7)), "root");
    assert_eq!(HEXLOWER.encode(&input.base), BASE_HEX, "base");
    assert_eq!(HEXLOWER.encode(&input.key.to_bytes()), KEY_HEX, "key bytes");
    assert_eq!(input.key.subkeys().network_id_hex(), KEY_ID, "network id");
}

fn root_of(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

fn hex32(text: &str) -> [u8; 32] {
    HEXLOWER
        .decode(text.as_bytes())
        .unwrap()
        .try_into()
        .unwrap()
}

fn b32(bytes: &[u8]) -> String {
    BASE32_NOPAD.encode(bytes).to_ascii_lowercase()
}

/// The first byte pattern `[i; 32]` that is not a curve point.
fn off_curve_bytes() -> [u8; 32] {
    (0..=255u8)
        .map(|i| [i; 32])
        .find(|bytes| PeerId::from_bytes(bytes).is_err())
        .expect("some repeated byte pattern is off the curve")
}
