//! The `IfaceNames` kind: three field states, rejections, a parse-render round trip.

use serde_json::{json, Value};

use super::IfaceNames;
use crate::config::config_file::fields::value::Kind;
use crate::config::config_file::IfaceFilter;

#[test]
fn parse_three_states() {
    assert_eq!(IfaceNames.parse(&json!([])), Some(IfaceFilter::Auto));
    assert_eq!(IfaceNames.parse(&Value::Null), Some(IfaceFilter::Auto));
    assert_eq!(IfaceNames.parse(&json!(["*"])), Some(IfaceFilter::Any));
    assert_eq!(
        IfaceNames.parse(&json!(["br-lan"])),
        Some(IfaceFilter::Only(vec!["br-lan".to_string()]))
    );
}

#[test]
fn parse_rejects_wrong_shapes() {
    assert_eq!(IfaceNames.parse(&json!("br-lan")), None);
    assert_eq!(IfaceNames.parse(&json!([1])), None);
    assert_eq!(IfaceNames.parse(&json!(["*", "br-lan"])), None);
}

#[test]
fn to_json_round_trips() {
    for f in [
        IfaceFilter::Auto,
        IfaceFilter::Any,
        IfaceFilter::Only(vec!["a".into(), "b c".into()]),
    ] {
        let j = IfaceNames.to_json(&f).expect("the key is written");
        assert_eq!(IfaceNames.parse(&j), Some(f));
    }
}

#[test]
fn texts() {
    assert_eq!(IfaceNames.default_text(&IfaceFilter::Auto), "[]");
    let e = IfaceNames.expected();
    assert!(e.contains("[]") && e.contains("[\"*\"]"), "{e}");
}

mod exit_iface {
    use super::super::ExitIface;
    use super::*;

    #[test]
    fn null_means_any_network() {
        // given / when
        let parsed = ExitIface.parse(&Value::Null);
        // then: a valid value, and the value is "no interface"
        assert_eq!(parsed, Some(None));
    }

    mod accepts_names_up_to_64_bytes {
        use super::*;

        #[test]
        fn plain_name() {
            assert_eq!(
                ExitIface.parse(&json!("wg0")),
                Some(Some("wg0".to_string()))
            );
        }

        #[test]
        fn name_with_an_inner_space() {
            assert_eq!(
                ExitIface.parse(&json!("Wi-Fi 2")),
                Some(Some("Wi-Fi 2".to_string()))
            );
        }

        #[test]
        fn sixty_four_bytes() {
            let name = "a".repeat(64);
            assert_eq!(ExitIface.parse(&json!(name)), Some(Some(name)));
        }
    }

    mod rejects_bad_names {
        use super::*;

        fn rejected(v: Value) {
            assert_eq!(ExitIface.parse(&v), None, "{v} must be rejected");
        }

        #[test]
        fn empty() {
            rejected(json!(""));
        }

        #[test]
        fn blank() {
            rejected(json!("  "));
        }

        #[test]
        fn sixty_five_bytes() {
            rejected(json!("a".repeat(65)));
        }

        #[test]
        fn control_character_inside() {
            rejected(json!("a\nb"));
        }

        #[test]
        fn star() {
            rejected(json!("*"));
        }

        #[test]
        fn leading_space() {
            rejected(json!(" wg0"));
        }

        #[test]
        fn trailing_space() {
            rejected(json!("wg0 "));
        }

        #[test]
        fn number() {
            rejected(json!(5));
        }

        #[test]
        fn list_of_names() {
            rejected(json!(["wg0"]));
        }
    }

    mod renders {
        use super::*;

        #[test]
        fn no_key_for_any_network() {
            assert_eq!(ExitIface.to_json(&None), None);
        }

        #[test]
        fn the_name_when_set() {
            assert_eq!(
                ExitIface.to_json(&Some("wg0".to_string())),
                Some(json!("wg0"))
            );
        }
    }

    mod texts {
        use super::*;

        #[test]
        fn expected_names_the_limits_and_null() {
            let e = ExitIface.expected();
            assert!(e.contains("1–64 bytes") && e.contains("null"), "{e}");
        }

        #[test]
        fn default_reads_any_network() {
            assert_eq!(ExitIface.default_text(&None), "any network");
        }
    }
}
