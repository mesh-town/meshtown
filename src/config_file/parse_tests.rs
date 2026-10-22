use super::*;
use crate::config_file::settings::Settings;
use crate::config_file::NetSettings;
use crate::config_file::test_support::{parsed, scratch, secret_of, zero_secret_b64};
use crate::config_file::FILE_NAME;
use crate::net::NodeSecret;

/// The example from CONFIG-FILE.md section 4 with a real secret of 32 zero bytes.
const EXAMPLE: &str = r#"{
  "rev": 8,
  "saved_at": "2026-09-24T19:41:07Z",
  "identity": { "secret": "SECRET" },
  "config": {
    "version": 1,
    "node":      { "name": "finch-unwary-4c7d9" },
    "network":   { "key": "our-gaming-room--finch-unwary-k7m2q9x4rb8vhn3tzd6w5" },
    "transport": { "mode": "direct_and_relay" },
    "exit":      { "enabled": true },
    "socks":     { "enabled": true, "port": 1080 },
    "checks":    { "enabled": true, "interval_sec": 900,
                   "doh_server": "1.1.1.1:443", "provider_dns": null },
    "probe":     { "interval_sec": 300 },
    "web":       { "enabled": true, "port": 7777 },
    "log":       { "ring_lines": 2000 }
  }
}"#;

#[test]
fn full_example_parses() {
    let secret_b64 = zero_secret_b64();
    assert_eq!(secret_b64.len(), 44);
    let text = EXAMPLE.replace("SECRET", &secret_b64);
    let p = parse_text(&text).expect("the CONFIG-FILE.md example should parse");
    assert_eq!(p.rev, 8);
    match &p.secret {
        SecretRead::Valid(k) => assert_eq!(k.to_bytes(), [0u8; 32]),
        other => panic!("expected Valid, got {other:?}"),
    }
    assert_eq!(p.field_errors, vec![]);
    assert!(p.unknown_paths.is_empty(), "unknown fields: {:?}", p.unknown_paths);
    let want = Settings {
        net: NetSettings {
            node_name: Some("finch-unwary-4c7d9".to_string()),
            network_key: Some("our-gaming-room--finch-unwary-k7m2q9x4rb8vhn3tzd6w5".to_string()),
            ..NetSettings::default()
        },
        ..Settings::default()
    };
    assert_eq!(p.settings, want);
}

#[test]
fn empty_config_gives_defaults() {
    let p = parse_text(r#"{"config":{"version":1}}"#).expect("parse");
    assert_eq!(p.settings, Settings::default());
    assert!(matches!(p.secret, SecretRead::Missing));
    assert_eq!(p.field_errors, vec![]);
}

#[test]
fn bad_field_only_that_field() {
    let text = r#"{"rev": 3, "config": {"version": 1,
        "socks": {"port": 80, "enabled": false},
        "checks": {"interval_sec": "x", "enabled": false},
        "probe": {"interval_sec": 120}}}"#;
    let p = parse_text(text).expect("parse");
    let d = Settings::default();
    assert_eq!(p.settings.process.socks_port, d.process.socks_port);
    assert_eq!(p.settings.net.check_interval_sec, d.net.check_interval_sec);
    assert!(
        !p.settings.process.socks_enabled,
        "the other fields of the section are taken from the file"
    );
    assert!(
        !p.settings.net.checks_enabled,
        "the other fields of the section are taken from the file"
    );
    assert_eq!(p.settings.net.probe_interval_sec, 120);
    let paths: Vec<&str> = p.field_errors.iter().map(|e| e.field_path.as_str()).collect();
    assert_eq!(paths, vec!["socks.port", "checks.interval_sec"]);
}

#[test]
fn version_newer_is_invalid() {
    let err = parse_text(r#"{"config":{"version":2}}"#).unwrap_err();
    assert!(err.contains("newer"), "{err}");
}

#[test]
fn version_missing_is_invalid() {
    assert!(parse_text(r#"{"config":{}}"#).is_err());
    assert!(parse_text(r#"{"rev":1}"#).is_err());
    assert!(parse_text(r#"{"config":{"version":0}}"#).is_err());
}

#[test]
fn not_json_is_invalid() {
    let err = parse_text("{ this is not json").unwrap_err();
    assert!(err.contains("Couldn't parse JSON"), "{err}");
    assert!(parse_text("[1,2]").is_err());
}

#[test]
fn secret_bad_base64() {
    let text = r#"{"identity": {"secret": "!!!junk-value"}, "config": {"version": 1}}"#;
    let p = parse_text(text).expect("parse");
    assert!(
        matches!(&p.secret, SecretRead::Bad(w) if w == "not base64"),
        "{:?}",
        p.secret
    );
    assert_eq!(p.field_errors.len(), 1);
    assert_eq!(p.field_errors[0].field_path, "identity.secret");
    assert!(
        !p.field_errors[0].message.contains("junk-value"),
        "the secret value in the message"
    );
}

#[test]
fn secret_wrong_length() {
    let b64 = data_encoding::BASE64.encode(&[1u8; 16]);
    let text = format!(r#"{{"identity": {{"secret": "{b64}"}}, "config": {{"version": 1}}}}"#);
    let p = parse_text(&text).expect("parse");
    assert!(
        matches!(&p.secret, SecretRead::Bad(w) if w == "16 bytes instead of 32"),
        "{:?}",
        p.secret
    );
    assert_eq!(p.field_errors.len(), 1);
    assert_eq!(p.field_errors[0].field_path, "identity.secret");
    assert!(
        !p.field_errors[0].message.contains(&b64),
        "the secret value in the message"
    );
}

#[test]
fn unknown_fields_listed() {
    let text = r#"{"config": {"version":1, "zzz": 1, "socks": {"port": 1080, "q": 2}}}"#;
    let mut p = parse_text(text).expect("parse");
    p.unknown_paths.sort();
    assert_eq!(p.unknown_paths, vec!["config.socks.q", "config.zzz"]);
    assert_eq!(p.field_errors, vec![]);
}

#[test]
fn computed_fields_ignored_silently() {
    let text = r#"{"config": {"version":1,
        "network": {"label": "our-gaming-room", "masked": "our--••••"},
        "autostart": {"enabled": false, "available": false, "reason": "later"},
        "full_tunnel": {"available": false, "reason": "later"}}}"#;
    let p = parse_text(text).expect("parse");
    assert_eq!(p.field_errors, vec![]);
    assert!(p.unknown_paths.is_empty(), "unknown fields: {:?}", p.unknown_paths);
}

#[test]
fn autostart_true_rejected() {
    let text = r#"{"config": {"version":1, "autostart": {"enabled": true}}}"#;
    let p = parse_text(text).expect("parse");
    assert!(!p.settings.process.autostart_enabled);
    assert_eq!(p.field_errors.len(), 1);
    assert_eq!(p.field_errors[0].field_path, "autostart.enabled");
}

#[test]
fn name_rules() {
    let name_doc =
        |name: &str| format!(r#"{{"config": {{"version":1, "node": {{"name": "{name}"}}}}}}"#);
    let p = parse_text(&name_doc("  ")).expect("parse");
    assert_eq!(p.settings.net.node_name, None);
    assert_eq!(p.field_errors.len(), 1);
    assert_eq!(p.field_errors[0].field_path, "node.name");

    let long = "語".repeat(65);
    let p = parse_text(&name_doc(&long)).expect("parse");
    assert_eq!(p.settings.net.node_name, None);
    assert_eq!(p.field_errors.len(), 1);

    let edge = "語".repeat(64);
    let p = parse_text(&name_doc(&edge)).expect("parse");
    assert_eq!(p.settings.net.node_name, Some(edge));
    assert_eq!(p.field_errors, vec![]);

    let p = parse_text(&name_doc("赤い淵🦊")).expect("parse");
    assert_eq!(p.settings.net.node_name.as_deref(), Some("赤い淵🦊"));
    assert_eq!(p.field_errors, vec![]);
}

#[path = "parse_route_tests.rs"]
mod route;

#[test]
fn load_absent() {
    let dir = scratch("absent");
    assert!(matches!(load_config_file(&dir.join(FILE_NAME)), LoadedConfigFile::Absent));
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn load_unreadable() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("unreadable");
    let file = dir.join(FILE_NAME);
    std::fs::write(&file, r#"{"config":{"version":1}}"#).expect("write the test file");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000))
        .expect("test file permissions");
    if std::fs::read(&file).is_ok() {
        // Running as root: permissions don't stop it, nothing to check.
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let got = load_config_file(&file);
    let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(matches!(got, LoadedConfigFile::Unreadable(_)), "got {got:?}");
}

/// UTF-8 with a BOM (how Windows Notepad and some editors save) parses the same as without one.
#[test]
fn bom_at_start_is_skipped() {
    let plain = EXAMPLE.replace("SECRET", &zero_secret_b64());
    let with_bom = format!("\u{feff}{plain}");
    let want = parse_text(&plain).expect("parse without BOM");
    let got = parse_text(&with_bom).expect("parse with BOM");
    assert_eq!(got.rev, want.rev);
    assert_eq!(got.settings, want.settings);
    assert_eq!(got.field_errors, want.field_errors);
    assert_eq!(got.unknown_paths, want.unknown_paths);
    assert_eq!(secret_of(&got), secret_of(&want));
    // The same text through a file: `load_config_file` reads the bytes and hands the text to `parse_text`.
    let dir = scratch("bom");
    let file = dir.join(FILE_NAME);
    std::fs::write(&file, with_bom.as_bytes()).expect("write the test file");
    let from_file = parsed(&file);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(from_file.rev, want.rev);
    assert_eq!(from_file.settings, want.settings);
}

/// `identity.secret` is not a string (or `identity` is not an object) — the secret is damaged, but the rest of the
/// document is read.
#[test]
fn identity_secret_non_string_is_bad() {
    for identity in [r#"{"secret": null}"#, r#"{"secret": 5}"#, "7"] {
        let text = format!(
            r#"{{"identity": {identity}, "config": {{"version": 1, "socks": {{"port": 2000}}}}}}"#
        );
        let p = parse_text(&text)
            .unwrap_or_else(|e| panic!("identity = {identity}: the document did not parse: {e}"));
        assert!(
            matches!(&p.secret, SecretRead::Bad(_)),
            "identity = {identity}: {:?}",
            p.secret
        );
        assert_eq!(
            p.settings.process.socks_port, 2000,
            "identity = {identity}: the rest of the document was not read"
        );
        let paths: Vec<&str> = p.field_errors.iter().map(|e| e.field_path.as_str()).collect();
        assert_eq!(paths, vec!["identity.secret"], "identity = {identity}");
    }
}

#[path = "parse_memory_tests.rs"]
mod memory;

mod geo_markers_field {
    use super::*;

    mod value_of_the_wrong_kind {
        use super::*;

        #[test]
        fn error_names_the_field_path() {
            // given: a file with a number where a list of phrases is expected
            let text = r#"{"config": {"version": 1, "checks": {"geo_markers": 5}}}"#;
            // when
            let p = parse_text(text).expect("parse");
            // then: one error that names the field, and the list stays empty
            let paths: Vec<&str> = p.field_errors.iter().map(|e| e.field_path.as_str()).collect();
            assert_eq!(paths, vec!["checks.geo_markers"]);
            assert!(p.settings.net.geo_markers.is_empty());
        }
    }
}
