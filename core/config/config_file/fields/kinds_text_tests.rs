//! The `TextList` kind: normalization of phrase lists, limits, rejections, output, and the
//! `checks.geo_markers` row read from a file.

use serde_json::{json, Value};

use super::super::value::Kind;
use super::{SlotRef, TextList};

use crate::config::config_file::slot_store::NetSlotId;
use crate::config::config_file::{parse_text, Settings};

mod parsing_a_list {
    use super::*;

    mod padded_mixed_case_phrases {
        use super::*;

        #[test]
        fn are_trimmed_lowercased_and_deduplicated() {
            // given: a padded phrase, its lowercase twin and an empty string
            let file_value = json!([" Not Here ", "not here", ""]);
            // when
            let got = parse(file_value);
            // then
            assert_eq!(got, Some(strings(&["not here"])));
        }
    }

    mod repeats_after_normalization {
        use super::*;

        #[test]
        fn keep_the_first_occurrence_in_its_place() {
            let got = parse(json!(["Zeta", "alpha", " ZETA ", "Beta", "ALPHA"]));
            assert_eq!(got, Some(strings(&["zeta", "alpha", "beta"])));
        }
    }

    mod empty_and_blank_phrases {
        use super::*;

        #[test]
        fn are_dropped_without_error() {
            let got = parse(json!(["", "   ", "\t", "kept"]));
            assert_eq!(got, Some(strings(&["kept"])));
        }

        #[test]
        fn alone_give_an_empty_list() {
            assert_eq!(parse(json!(["", "  "])), Some(vec![]));
        }
    }

    mod uppercase_outside_ascii {
        use super::*;

        #[test]
        fn is_lowercased_by_unicode_rules() {
            let got = parse(json!(["ÜNAVAILABLE HÈRE"]));
            assert_eq!(got, Some(strings(&["ünavailable hère"])));
        }
    }

    mod empty_array {
        use super::*;

        #[test]
        fn gives_an_empty_list() {
            assert_eq!(parse(json!([])), Some(vec![]));
        }
    }
}

mod limits {
    use super::*;

    mod thirty_two_phrases {
        use super::*;

        #[test]
        fn are_accepted() {
            let got = parse(distinct_phrases(32)).expect("32 phrases fit");
            assert_eq!(got.len(), 32);
        }
    }

    mod thirty_three_phrases {
        use super::*;

        #[test]
        fn are_rejected() {
            assert_eq!(parse(distinct_phrases(33)), None);
        }
    }

    mod phrase_of_64_characters {
        use super::*;

        #[test]
        fn is_accepted() {
            let phrase = "a".repeat(64);
            assert_eq!(parse(json!([phrase])), Some(vec!["a".repeat(64)]));
        }
    }

    mod phrase_of_65_characters {
        use super::*;

        #[test]
        fn is_rejected() {
            assert_eq!(parse(json!(["a".repeat(65)])), None);
        }

        #[test]
        fn is_rejected_even_next_to_good_phrases() {
            assert_eq!(parse(json!(["fine", "a".repeat(65), "also fine"])), None);
        }
    }

    mod phrase_of_64_cjk_characters {
        use super::*;

        #[test]
        fn is_accepted_because_characters_are_counted_not_bytes() {
            let phrase = "語".repeat(64);
            assert!(phrase.len() > 64, "the phrase is longer than 64 bytes");
            assert_eq!(parse(json!([phrase.clone()])), Some(vec![phrase]));
        }
    }

    mod phrase_of_65_cjk_characters {
        use super::*;

        #[test]
        fn is_rejected() {
            assert_eq!(parse(json!(["語".repeat(65)])), None);
        }
    }

    mod limits_of_the_kind_itself {
        use super::*;

        #[test]
        fn follow_the_struct_fields() {
            // given: a kind with two phrases of three characters
            let small = TextList {
                max_items: 2,
                max_chars: 3,
            };
            // then: the bounds are the fields, not constants
            assert_eq!(
                small.parse(&json!(["abc", "def"])),
                Some(strings(&["abc", "def"]))
            );
            assert_eq!(small.parse(&json!(["abc", "def", "ghi"])), None);
            assert_eq!(small.parse(&json!(["abcd"])), None);
        }
    }
}

mod rejections {
    use super::*;

    mod value_that_is_not_an_array {
        use super::*;

        #[test]
        fn gives_none() {
            for bad in [
                json!("not here"),
                json!(5),
                json!(true),
                json!(null),
                json!({ "a": "b" }),
            ] {
                assert_eq!(parse(bad.clone()), None, "value {bad}");
            }
        }
    }

    mod element_that_is_not_a_string {
        use super::*;

        #[test]
        fn gives_none() {
            for bad in [
                json!(5),
                json!(null),
                json!(false),
                json!(["nested"]),
                json!({ "a": "b" }),
            ] {
                assert_eq!(parse(json!(["fine", bad.clone()])), None, "element {bad}");
            }
        }

        #[test]
        fn rejects_the_whole_list_not_only_the_element() {
            assert_eq!(parse(json!([5, "fine"])), None);
        }
    }
}

mod output {
    use super::*;

    mod to_json {
        use super::*;

        #[test]
        fn gives_an_array_of_strings() {
            let got = list().to_json(&strings(&["not here", "ünavailable hère"]));
            assert_eq!(got, Some(json!(["not here", "ünavailable hère"])));
        }

        #[test]
        fn of_an_empty_list_gives_an_empty_array() {
            assert_eq!(list().to_json(&vec![]), Some(json!([])));
        }
    }

    mod default_text {
        use super::*;

        #[test]
        fn is_empty_brackets() {
            assert_eq!(list().default_text(&vec![]), "[]");
        }
    }

    mod expected {
        use super::*;

        #[test]
        fn names_both_limits() {
            let want = "a list of up to 32 phrases, up to 64 characters each";
            assert_eq!(list().expected(), want);
        }
    }
}

mod row_read_from_a_file {
    use super::*;

    mod list_in_the_file {
        use super::*;

        #[test]
        fn is_normalized_into_settings() {
            // given: a padded list with a repeat and an empty string
            let key = Some(json!([" Not Here ", "not here", "", "Other Phrase"]));
            // when
            let (settings, errs) = read_geo_markers(key);
            // then
            assert_eq!(errs, vec![]);
            assert_eq!(
                settings.net.geo_markers,
                strings(&["not here", "other phrase"])
            );
        }
    }

    mod no_key {
        use super::*;

        #[test]
        fn leaves_the_list_empty() {
            let (settings, errs) = read_geo_markers(None);
            assert_eq!(errs, vec![]);
            assert!(settings.net.geo_markers.is_empty());
        }
    }

    mod value_of_the_wrong_kind {
        use super::*;

        #[test]
        fn gives_one_error_with_the_expectation_and_an_empty_list() {
            let (settings, errs) = read_geo_markers(Some(json!(5)));
            let want = "file has 5, expected a list of up to 32 phrases, up to 64 characters each; using default []";
            assert_eq!(
                errs,
                vec![("checks.geo_markers".to_string(), want.to_string())]
            );
            assert!(settings.net.geo_markers.is_empty());
        }
    }

    mod list_over_the_limit {
        use super::*;

        #[test]
        fn is_dropped_whole() {
            let (settings, errs) = read_geo_markers(Some(distinct_phrases(33)));
            assert_eq!(errs.len(), 1, "{errs:?}");
            assert_eq!(errs[0].0, "checks.geo_markers");
            assert!(
                settings.net.geo_markers.is_empty(),
                "no partial list is kept"
            );
        }
    }
}

mod slot_ref {
    use super::*;

    mod to_json {
        use super::*;

        #[test]
        fn of_main_is_none_so_the_default_is_not_written() {
            assert_eq!(SlotRef.to_json(&NetSlotId::main()), None);
        }

        #[test]
        fn of_another_network_is_its_id_as_a_string() {
            assert_eq!(SlotRef.to_json(&net("ab2c3d")), Some(json!("ab2c3d")));
        }
    }

    mod parse {
        use super::*;

        #[test]
        fn main_gives_the_main_slot() {
            assert_eq!(SlotRef.parse(&json!("main")), Some(NetSlotId::main()));
        }

        #[test]
        fn six_base32_characters_give_that_network() {
            assert_eq!(SlotRef.parse(&json!("ab2c3d")), Some(net("ab2c3d")));
        }

        #[test]
        fn rejects_uppercase_wrong_length_and_other_alphabets() {
            for bad in ["AB", "AB2C3D", "ab2c3", "ab2c3de", "ab1c3d", "", "Main"] {
                assert_eq!(SlotRef.parse(&json!(bad)), None, "text {bad:?}");
            }
        }

        #[test]
        fn rejects_values_that_are_not_strings() {
            for bad in [json!(5), json!(null), json!(true), json!(["main"])] {
                assert_eq!(SlotRef.parse(&bad), None, "value {bad}");
            }
        }
    }

    mod expected {
        use super::*;

        #[test]
        fn names_main_and_the_id_alphabet() {
            assert_eq!(
                SlotRef.expected(),
                "main or a network id: 6 characters a-z, 2-7"
            );
        }
    }

    mod default_text {
        use super::*;

        #[test]
        fn is_the_id_itself() {
            assert_eq!(SlotRef.default_text(&NetSlotId::main()), "main");
        }
    }

    mod row_read_from_a_file {
        use super::*;

        #[test]
        fn network_id_lands_in_process_settings() {
            let (settings, errs) = read_proxy_net(Some(json!("ab2c3d")));
            assert_eq!(errs, vec![]);
            assert_eq!(settings.process.proxy_net, net("ab2c3d"));
        }

        #[test]
        fn no_section_leaves_main() {
            let (settings, errs) = read_proxy_net(None);
            assert_eq!(errs, vec![]);
            assert_eq!(settings.process.proxy_net, NetSlotId::main());
        }

        #[test]
        fn uppercase_id_gives_one_error_with_the_expectation_and_main() {
            let (settings, errs) = read_proxy_net(Some(json!("AB")));
            assert_eq!(errs.len(), 1, "{errs:?}");
            assert_eq!(errs[0].0, "proxy.net");
            let want = "expected main or a network id: 6 characters a-z, 2-7";
            assert!(errs[0].1.contains(want), "message: {}", errs[0].1);
            assert_eq!(settings.process.proxy_net, NetSlotId::main());
        }
    }
}

fn list() -> TextList {
    TextList {
        max_items: 32,
        max_chars: 64,
    }
}

fn parse(file_value: Value) -> Option<Vec<String>> {
    list().parse(&file_value)
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// A JSON array of `n` different non-empty phrases.
fn distinct_phrases(n: usize) -> Value {
    (0..n).map(|i| format!("phrase {i}")).collect()
}

/// Reads a file with the single key `checks.geo_markers` (or without it) and returns the settings
/// and the (path, message) pairs of errors.
fn read_geo_markers(file_value: Option<Value>) -> (Settings, Vec<(String, String)>) {
    let checks = file_value.map_or_else(|| json!({}), |v| json!({ "geo_markers": v }));
    let text = json!({ "config": { "version": 1, "checks": checks } }).to_string();
    let parsed = parse_text(&text).expect("the document parsed");
    let errs = parsed
        .field_errors
        .into_iter()
        .map(|e| (e.field_path, e.message))
        .collect();
    (parsed.settings, errs)
}

fn net(id: &str) -> NetSlotId {
    NetSlotId::parse(id).expect("valid network id")
}

/// Reads a file with the single key `proxy.net` (or without the `proxy` section) and returns the settings
/// and the (path, message) pairs of errors.
fn read_proxy_net(file_value: Option<Value>) -> (Settings, Vec<(String, String)>) {
    let mut config = json!({ "version": 1 });
    if let Some(v) = file_value {
        config["proxy"] = json!({ "net": v });
    }
    let text = json!({ "config": config }).to_string();
    let parsed = parse_text(&text).expect("the document parsed");
    let errs = parsed
        .field_errors
        .into_iter()
        .map(|e| (e.field_path, e.message))
        .collect();
    (parsed.settings, errs)
}
