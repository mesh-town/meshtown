use super::*;
use crate::config::config_file::settings::ListenAccess;

/// A file with sections `cfg` and version 1 → `web.access` and the field errors.
fn take(cfg: serde_json::Value) -> (ListenAccess, Vec<FieldError>) {
    let mut config = cfg;
    config["version"] = serde_json::json!(1);
    let text = serde_json::json!({ "config": config }).to_string();
    let p = parse_text(&text).expect("the document parsed");
    (p.settings.process.web_access, p.field_errors)
}

#[test]
fn access_localhost_parsed() {
    let (a, errs) = take(serde_json::json!({"web": {"access": "localhost"}}));
    assert_eq!(a, ListenAccess::Localhost);
    assert!(errs.is_empty(), "extra errors: {errs:?}");
}

#[test]
fn access_bogus_is_field_error() {
    let (a, errs) = take(serde_json::json!({"web": {"access": "anywhere"}}));
    assert_eq!(a, ListenAccess::Localnet, "on error — the default");
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].field_path, "web.access");
    assert!(
        errs[0].message.contains("localhost or localnet"),
        "message: {}",
        errs[0].message
    );
}

#[test]
fn access_missing_defaults_to_localnet() {
    let (a, errs) = take(serde_json::json!({"web": {"enabled": true}}));
    assert_eq!(a, ListenAccess::Localnet);
    assert!(errs.is_empty(), "extra errors: {errs:?}");
}

fn take_socks(cfg: serde_json::Value) -> (ListenAccess, Vec<FieldError>) {
    let mut config = cfg;
    config["version"] = serde_json::json!(1);
    let text = serde_json::json!({ "config": config }).to_string();
    let p = parse_text(&text).expect("the document parsed");
    (p.settings.process.socks_access, p.field_errors)
}

#[test]
fn socks_access_default_is_localhost() {
    let (a, errs) = take_socks(serde_json::json!({}));
    assert_eq!(a, ListenAccess::Localhost);
    assert!(errs.is_empty(), "extra errors: {errs:?}");
}

#[test]
fn socks_access_both_values_parsed() {
    for (text, want) in [
        ("localhost", ListenAccess::Localhost),
        ("localnet", ListenAccess::Localnet),
    ] {
        let (a, errs) = take_socks(serde_json::json!({"socks": {"access": text}}));
        assert_eq!(a, want);
        assert!(errs.is_empty(), "extra errors: {errs:?}");
    }
}

#[test]
fn socks_access_third_value_is_field_error() {
    let (a, errs) = take_socks(serde_json::json!({"socks": {"access": "anywhere"}}));
    assert_eq!(a, ListenAccess::Localhost, "on error — the default");
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].field_path, "socks.access");
    assert!(
        errs[0].message.contains("localhost or localnet"),
        "message: {}",
        errs[0].message
    );
}
