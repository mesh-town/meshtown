use super::*;
use crate::config::config_file::test_support::test_secret;
use crate::config::config_file::{NetSettings, SecretRead, TransportMode};

const SECRET_B64: &str = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";
const CREATED: u64 = 1_792_749_600;

mod complete_file {
    use super::*;

    #[test]
    fn gives_the_net_block_created_unix_rev_and_secret() {
        // given: a file with the net sections filled in
        let text = envelope(&created_line(), COMPLETE_CONFIG);
        // when: parsed
        let p = parse_ok(&text);
        // then: every value arrives, nothing is reported
        assert_eq!(p.net, complete_net());
        assert_eq!(p.created_unix, CREATED);
        assert_eq!(p.rev, 3);
        assert_eq!(secret_bytes(&p), test_secret().to_bytes());
        assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
        assert!(p.unknown_paths.is_empty(), "{:?}", p.unknown_paths);
        assert!(p.process_paths.is_empty(), "{:?}", p.process_paths);
    }

    #[test]
    fn process_keys_are_listed_not_reported() {
        // given: process keys copied from the main file next to the net keys
        let config = r#"{"version": 1,
            "node": {"name": "kitchen", "memory_warn_mib": 500},
            "socks": {"port": 2000},
            "web": {"port": 8080}}"#;
        // when: parsed
        let p = parse_ok(&envelope(&created_line(), config));
        // then: they are named in `process_paths` only, and the net values still load
        // The paths may be listed bare or with the `config.` prefix that `unknown_paths` use.
        let listed = strings(&p.process_paths);
        assert_eq!(listed.len(), 3, "listed: {listed:?}");
        for key in ["node.memory_warn_mib", "socks.port", "web.port"] {
            let found = listed
                .iter()
                .any(|l| l == key || *l == format!("config.{key}"));
            assert!(found, "{key} is not in {listed:?}");
        }
        assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
        assert!(p.unknown_paths.is_empty(), "{:?}", p.unknown_paths);
        assert_eq!(p.net.node_name.as_deref(), Some("kitchen"));
    }

    #[test]
    fn process_keys_never_reach_the_net_block() {
        // given: a process key only
        let config = r#"{"version": 1, "socks": {"port": 2000}}"#;
        // when: parsed
        let p = parse_ok(&envelope("", config));
        // then: the net block is the default one
        assert_eq!(p.net, NetSettings::default());
    }
}

mod unknown_keys {
    use super::*;

    #[test]
    fn config_key_and_root_key_are_listed() {
        // given: `config.foo` and a root key `bar`
        let config = r#"{"version": 1, "foo": 1}"#;
        let text = envelope(r#""bar": true,"#, config);
        // when: parsed
        let p = parse_ok(&text);
        // then: both are unknown, neither is a process key
        let mut unknown = strings(&p.unknown_paths);
        unknown.sort();
        assert_eq!(unknown, ["bar", "config.foo"]);
        assert!(p.process_paths.is_empty(), "{:?}", p.process_paths);
    }
}

mod created_unix {
    use super::*;

    #[test]
    fn not_a_number_is_zero_with_a_field_error() {
        // given: `created_unix` is a string
        let text = envelope(r#""created_unix": "x","#, r#"{"version": 1}"#);
        // when: parsed
        let p = parse_ok(&text);
        // then: zero and an error that names the key
        assert_eq!(p.created_unix, 0);
        let named: Vec<_> = p
            .field_errors
            .iter()
            .filter(|e| e.field_path == "created_unix")
            .collect();
        assert_eq!(named.len(), 1, "errors: {:?}", p.field_errors);
    }

    #[test]
    fn missing_is_zero_without_an_error() {
        // given: no `created_unix` at all
        let text = envelope("", r#"{"version": 1}"#);
        // when: parsed
        let p = parse_ok(&text);
        // then: zero, nothing reported
        assert_eq!(p.created_unix, 0);
        assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
    }
}

mod not_a_document {
    use super::*;

    #[test]
    fn newer_version_is_refused() {
        // given: `config.version` 2
        let text = envelope("", r#"{"version": 2}"#);
        // when: parsed; then: refused with the reason
        let err = parse_net_text(&text).expect_err("a newer version must not parse");
        assert!(err.contains("newer than this program"), "got: {err}");
    }

    #[test]
    fn missing_config_section_is_refused() {
        // given: an envelope without `config`
        let text = format!(r#"{{"rev": 1, "identity": {{"secret": "{SECRET_B64}"}}}}"#);
        // when: parsed; then: refused
        let err = parse_net_text(&text).expect_err("no config section");
        assert!(err.contains("config"), "got: {err}");
    }

    #[test]
    fn not_an_object_is_refused() {
        // given: a JSON array
        // when: parsed; then: refused
        assert!(parse_net_text("[1, 2]").is_err());
    }

    #[test]
    fn broken_json_is_refused() {
        // given: truncated JSON
        // when: parsed; then: refused
        assert!(parse_net_text(r#"{"rev": 1,"#).is_err());
    }
}

mod byte_order_mark {
    use super::*;

    #[test]
    fn is_stripped() {
        // given: a complete file saved with a BOM
        let text = format!("\u{feff}{}", envelope(&created_line(), COMPLETE_CONFIG));
        // when: parsed
        let p = parse_ok(&text);
        // then: the same result as without it
        assert_eq!(p.net, complete_net());
        assert_eq!(p.created_unix, CREATED);
    }
}

const COMPLETE_CONFIG: &str = r#"{
    "version": 1,
    "node": {"name": "kitchen"},
    "network": {"key": "label--test-key-not-real"},
    "transport": {"mode": "relay_only"},
    "exit": {"enabled": false},
    "probe": {"interval_sec": 120}
}"#;

fn complete_net() -> NetSettings {
    NetSettings {
        node_name: Some("kitchen".to_string()),
        network_key: Some("label--test-key-not-real".to_string()),
        transport_mode: TransportMode::RelayOnly,
        exit_enabled: false,
        probe_interval_sec: 120,
        ..NetSettings::default()
    }
}

fn created_line() -> String {
    format!(r#""created_unix": {CREATED},"#)
}

/// A slot file: `root_extra` is spliced into the envelope (it must end with a comma), `config` is
/// the whole `config` object.
fn envelope(root_extra: &str, config: &str) -> String {
    format!(
        r#"{{"rev": 3, "saved_at": "2027-01-15T08:00:00Z", {root_extra}
        "identity": {{"secret": "{SECRET_B64}"}}, "config": {config}}}"#
    )
}

fn parse_ok(text: &str) -> ParsedNetFile {
    parse_net_text(text).expect("the text is a slot file")
}

fn secret_bytes(p: &ParsedNetFile) -> [u8; 32] {
    match &p.secret {
        SecretRead::Valid(k) => k.to_bytes(),
        other => panic!("expected Valid, got {other:?}"),
    }
}

fn strings<S: ToString>(items: &[S]) -> Vec<String> {
    items.iter().map(ToString::to_string).collect()
}
