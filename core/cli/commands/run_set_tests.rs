use super::*;
use crate::cli::exit::exit_code;

fn refused(text: &str) -> String {
    let e = set_fields(&[text.to_string()]).unwrap_err();
    assert_eq!(exit_code(&e), std::process::ExitCode::from(2), "{text}");
    e.to_string()
}

#[test]
fn bad_set_is_usage_error() {
    let text = refused("probe.interval_sec=3");
    assert!(text.starts_with("--set probe.interval_sec=3: "), "{text}");
    assert!(text.contains("5") && text.contains("86400"), "{text}");
    assert!(text.ends_with(" run --help"), "{text}");
}

#[test]
fn unknown_and_missing_equals_are_refused() {
    assert!(refused("probe.interval=3").contains("no such setting `probe.interval`"));
    assert!(refused("probe.interval_sec").contains("expected PATH=VALUE"));
}

#[test]
fn set_web_port_hints_flag() {
    assert!(refused("web.port=8000").contains("--web-port"));
}

#[test]
fn set_key_hints_join() {
    let text = refused("network.key=x");
    assert!(text.contains("--key") && text.contains("join"), "{text}");
}

#[test]
fn order_is_kept() {
    let got = set_fields(&["socks.port=2000".into(), "checks.enabled=false".into()]).unwrap();
    let paths: Vec<_> = got.iter().map(FieldSet::path).collect();
    assert_eq!(paths, ["socks.port", "checks.enabled"]);
}
