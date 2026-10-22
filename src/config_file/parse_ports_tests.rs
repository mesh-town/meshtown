use crate::config_file::parse_text;
use crate::net::{NodeSecret, PeerId};
use crate::ports::types::Audience;

fn id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn doc(ports: &str) -> String {
    format!(r#"{{"config":{{"version":1, "ports": {ports}}}}}"#)
}

/// Parses `ports.published`/`ports.opened` from `body` (the section object).
fn parse(body: &str) -> crate::config_file::ParsedConfigFile {
    parse_text(&doc(body)).expect("the document parsed")
}

#[test]
fn absent_section_gives_empty_lists() {
    let p = parse_text(r#"{"config":{"version":1}}"#).expect("parse");
    assert!(p.settings.net.ports_published.is_empty());
    assert!(p.settings.net.ports_opened.is_empty());
    assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
    assert!(p.unknown_paths.is_empty(), "{:?}", p.unknown_paths);
}

#[test]
fn valid_lists_are_read() {
    let a = id(1);
    let body = format!(
        r#"{{"published":[{{"port":8080,"name":"web","for":"all"}},
              {{"port":22,"name":"ssh","for":["{a}"]}}],
             "opened":[{{"node":"{a}","port":8080,"local":9090}}]}}"#
    );
    let p = parse(&body);
    assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
    assert!(p.unknown_paths.is_empty(), "{:?}", p.unknown_paths);
    let s = p.settings;
    assert_eq!(s.net.ports_published.len(), 2);
    assert_eq!(s.net.ports_published[0].audience, Audience::All);
    assert_eq!(s.net.ports_published[0].name, "web");
    assert_eq!(s.net.ports_published[1].audience, Audience::Nodes(vec![a]));
    assert_eq!(s.net.ports_opened.len(), 1);
    assert_eq!((s.net.ports_opened[0].node, s.net.ports_opened[0].port), (a, 8080));
    assert_eq!(s.net.ports_opened[0].local, 9090);
}

fn assert_rejected(body: &str, path: &str, why: &str) {
    let p = parse(body);
    assert!(p.settings.net.ports_published.is_empty(), "{why}: published");
    assert!(p.settings.net.ports_opened.is_empty(), "{why}: opened");
    assert_eq!(p.field_errors.len(), 1, "{why}: {:?}", p.field_errors);
    assert_eq!(p.field_errors[0].field_path, path, "{why}");
}

#[test]
fn published_without_for_is_rejected_not_all() {
    assert_rejected(
        r#"{"published":[{"port":8080,"name":"web"}]}"#,
        "ports.published",
        "no for",
    );
}

#[test]
fn bad_published_lists_are_rejected_whole() {
    let long = "語".repeat(65);
    let many: Vec<String> = (1..=65)
        .map(|n| format!(r#"{{"port":{n},"name":"","for":"all"}}"#))
        .collect();
    let cases = [
        (
            r#"[{"port":8080,"name":"x","for":"friends"}]"#.to_string(),
            "for friends",
        ),
        (
            r#"[{"port":0,"name":"x","for":"all"}]"#.to_string(),
            "port 0",
        ),
        (
            r#"[{"port":70000,"name":"x","for":"all"}]"#.to_string(),
            "port 70000",
        ),
        (
            r#"[{"port":80,"name":"x","for":"all"},{"port":80,"name":"y","for":"all"}]"#
                .to_string(),
            "duplicate port",
        ),
        (format!("[{}]", many.join(",")), "65 entries"),
        (
            format!(r#"[{{"port":80,"name":"{long}","for":"all"}}]"#),
            "name of 65 characters",
        ),
        (
            r#"[{"port":80,"name":"x","for":["not-an-id"]}]"#.to_string(),
            "broken id in for",
        ),
        (r#""x""#.to_string(), "not a list"),
    ];
    for (list, why) in cases {
        // a valid entry next to it in the other list neither saves nor is spoiled: only published is rejected
        let p = parse(&format!(r#"{{"published":{list}}}"#));
        assert!(p.settings.net.ports_published.is_empty(), "{why}");
        assert_eq!(p.field_errors.len(), 1, "{why}: {:?}", p.field_errors);
        assert_eq!(p.field_errors[0].field_path, "ports.published", "{why}");
    }
}

#[test]
fn bad_opened_lists_are_rejected_whole() {
    let a = id(1);
    let b = id(2);
    let many: Vec<String> = (1..=65)
        .map(|n| format!(r#"{{"node":"{a}","port":{n},"local":{}}}"#, 1000 + n))
        .collect();
    let cases = [
        (
            format!(r#"[{{"node":"{a}","port":0,"local":9000}}]"#),
            "port 0",
        ),
        (
            format!(r#"[{{"node":"{a}","port":70000,"local":9000}}]"#),
            "port 70000",
        ),
        (
            format!(r#"[{{"node":"{a}","port":80,"local":0}}]"#),
            "local 0",
        ),
        (
            format!(
                r#"[{{"node":"{a}","port":80,"local":9000}},{{"node":"{a}","port":80,"local":9001}}]"#
            ),
            "duplicate (node, port)",
        ),
        (
            format!(
                r#"[{{"node":"{a}","port":80,"local":9000}},{{"node":"{b}","port":81,"local":9000}}]"#
            ),
            "duplicate local",
        ),
        (
            r#"[{"node":"not-an-id","port":80,"local":9000}]"#.to_string(),
            "broken node",
        ),
        (format!("[{}]", many.join(",")), "65 entries"),
    ];
    for (list, why) in cases {
        let p = parse(&format!(r#"{{"opened":{list}}}"#));
        assert!(p.settings.net.ports_opened.is_empty(), "{why}");
        assert_eq!(p.field_errors.len(), 1, "{why}: {:?}", p.field_errors);
        assert_eq!(p.field_errors[0].field_path, "ports.opened", "{why}");
    }
}
