//! Retired fields in a settings file: reported in `retired_paths`, never in `unknown_paths`, never an error.

use super::*;

fn parse_config(config: &str) -> ParsedConfigFile {
    let text = format!(r#"{{"config":{{"version":1,{config}}}}}"#);
    parse_text(&text).expect("the file parses")
}

fn parse_mode(value: &str) -> ParsedConfigFile {
    parse_config(&format!(r#""transport":{{"mode":{value}}}"#))
}

fn assert_retired_mode(p: &ParsedConfigFile) {
    assert_eq!(p.retired_paths, vec!["config.transport.mode"]);
    assert_eq!(p.field_errors, vec![]);
    assert!(p.unknown_paths.is_empty(), "{:?}", p.unknown_paths);
}

#[test]
fn retired_mode_direct_and_relay() {
    assert_retired_mode(&parse_mode(r#""direct_and_relay""#));
}

#[test]
fn retired_mode_relay_only() {
    assert_retired_mode(&parse_mode(r#""relay_only""#));
}

#[test]
fn retired_mode_no_relay() {
    assert_retired_mode(&parse_mode(r#""no_relay""#));
}

#[test]
fn retired_mode_garbage_value() {
    assert_retired_mode(&parse_mode(r#""warp""#));
    assert_retired_mode(&parse_mode("17"));
    assert_retired_mode(&parse_mode("null"));
}

#[test]
fn retired_mode_leaves_other_settings_alone() {
    let p = parse_config(r#""transport":{"mode":"no_relay"},"exit":{"enabled":false}"#);
    assert!(!p.settings.net.exit_enabled);
    assert_retired_mode(&p);
}

#[test]
fn retired_and_unknown_are_listed_apart() {
    // given: the retired key, an unknown key in the same section and an unknown section
    let p = parse_config(r#""transport":{"mode":"relay_only","udp_port":4000},"zzz":1"#);
    // then: each list holds its own
    assert_eq!(p.retired_paths, vec!["config.transport.mode"]);
    let mut unknown = p.unknown_paths.clone();
    unknown.sort();
    assert_eq!(unknown, vec!["config.transport.udp_port", "config.zzz"]);
}

#[test]
fn retired_only_exact_path() {
    // given: `transport` without `mode`, and `transport` that is not an object
    let other = parse_config(r#""transport":{"other":1}"#);
    let scalar = parse_config(r#""transport":"relay_only""#);
    // then: both are unknown, nothing is retired
    assert_eq!(other.unknown_paths, vec!["config.transport"]);
    assert!(other.retired_paths.is_empty());
    assert_eq!(scalar.unknown_paths, vec!["config.transport"]);
    assert!(scalar.retired_paths.is_empty());
}

#[test]
fn a_file_without_retired_fields_reports_none() {
    assert!(parse_config(r#""exit":{"enabled":true}"#)
        .retired_paths
        .is_empty());
}
