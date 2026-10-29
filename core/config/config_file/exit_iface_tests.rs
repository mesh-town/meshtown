//! The `exit.interface` field in the settings file: a missing key means any network, a bad value
//! falls back to it with a message, a good name survives a write and a read.

use serde_json::{json, Value};

use crate::config::config_file::test_support::test_secret;
use crate::config::config_file::{parse_text, render_config_file, NetSettings, Settings};

mod absent_field_is_any_network {
    use super::*;

    #[test]
    fn reads_as_none_without_errors() {
        // given: a file with an `exit` section but no `interface` key
        let text = file_with(json!({"exit": {"enabled": true}}));
        // when
        let parsed = parse_text(&text).expect("the file parses");
        // then
        assert_eq!(parsed.settings.net.exit_interface, None);
        assert!(parsed.field_errors.is_empty(), "{:?}", parsed.field_errors);
    }

    #[test]
    fn default_settings_render_no_interface_key() {
        // given: default settings
        let settings = Settings::default();
        // when
        let doc = rendered(&settings);
        // then: the key is not written
        assert!(doc["config"]["exit"].get("interface").is_none(), "{doc}");
    }

    #[test]
    fn default_net_settings_hold_none() {
        // when / then
        assert_eq!(NetSettings::default().exit_interface, None);
    }
}

mod good_value {
    use super::*;

    #[test]
    fn reads_as_the_name() {
        // given
        let text = file_with(json!({"exit": {"interface": "Wi-Fi 2"}}));
        // when
        let parsed = parse_text(&text).expect("the file parses");
        // then
        assert_eq!(
            parsed.settings.net.exit_interface.as_deref(),
            Some("Wi-Fi 2")
        );
        assert!(parsed.field_errors.is_empty(), "{:?}", parsed.field_errors);
    }

    #[test]
    fn is_written_and_read_back() {
        // given: settings where the interface is set
        let mut settings = Settings::default();
        settings.net.exit_interface = Some("wg0".to_string());
        // when: rendered and parsed again
        let text = render_config_file(1, 0, &test_secret(), &settings);
        let parsed = parse_text(&text).expect("the file parses");
        // then
        assert_eq!(rendered(&settings)["config"]["exit"]["interface"], "wg0");
        assert_eq!(parsed.settings.net.exit_interface.as_deref(), Some("wg0"));
    }
}

mod bad_file_value_falls_back {
    use super::*;

    #[test]
    fn a_number_gives_none_and_one_message() {
        // given
        let text = file_with(json!({"exit": {"interface": 5}}));
        // when
        let parsed = parse_text(&text).expect("the file parses");
        // then
        assert_eq!(parsed.settings.net.exit_interface, None);
        assert_eq!(
            messages(&parsed.field_errors),
            vec![("exit.interface", MESSAGE_FOR_5)]
        );
    }

    #[test]
    fn a_name_with_a_control_character_gives_none() {
        // given
        let text = file_with(json!({"exit": {"interface": "a\nb"}}));
        // when
        let parsed = parse_text(&text).expect("the file parses");
        // then
        assert_eq!(parsed.settings.net.exit_interface, None);
        assert_eq!(parsed.field_errors.len(), 1, "{:?}", parsed.field_errors);
        assert!(
            parsed.field_errors[0].message.ends_with(TAIL),
            "{:?}",
            parsed.field_errors
        );
    }

    #[test]
    fn a_name_over_64_bytes_gives_none() {
        // given
        let text = file_with(json!({"exit": {"interface": "a".repeat(65)}}));
        // when
        let parsed = parse_text(&text).expect("the file parses");
        // then
        assert_eq!(parsed.settings.net.exit_interface, None);
        assert_eq!(parsed.field_errors.len(), 1, "{:?}", parsed.field_errors);
    }

    #[test]
    fn other_fields_of_the_file_are_kept() {
        // given: a bad interface next to a good field
        let text = file_with(json!({"exit": {"interface": 5, "max_streams": 7}}));
        // when
        let parsed = parse_text(&text).expect("the file parses");
        // then
        assert_eq!(parsed.settings.process.exit_max_streams, 7);
    }
}

const TAIL: &str = "expected an interface name (1–64 bytes, no control characters) or null — any network; using default any network";
const MESSAGE_FOR_5: &str = "file has 5, expected an interface name (1–64 bytes, no control characters) or null — any network; using default any network";

fn file_with(config: Value) -> String {
    let mut config = config;
    config["version"] = json!(1);
    json!({ "rev": 1, "config": config }).to_string()
}

/// The text a write produces, as a document.
fn rendered(settings: &Settings) -> Value {
    let text = render_config_file(1, 0, &test_secret(), settings);
    serde_json::from_str(&text).expect("the file is JSON")
}

fn messages(errors: &[crate::config::config_file::FieldError]) -> Vec<(&str, &str)> {
    errors
        .iter()
        .map(|e| (e.field_path.as_str(), e.message.as_str()))
        .collect()
}
