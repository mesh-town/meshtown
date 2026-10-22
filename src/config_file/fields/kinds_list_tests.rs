//! List value kinds through file parsing: bounds, repeats, error texts.

use serde_json::{json, Value};

use super::super::value::Kind;

use crate::config_file::{parse_text, ParsedConfigFile};
use crate::net::NodeSecret;
use crate::node::net_names;

fn id(n: u8) -> String {
    NodeSecret::from_bytes(&[n; 32]).peer_id().to_string()
}

fn parse(section: &str, key: &str, value: Value) -> ParsedConfigFile {
    let text = json!({ "config": { "version": 1, section: { key: value } } }).to_string();
    parse_text(&text).expect("the document parsed")
}

#[test]
fn peer_list_keeps_order_and_rejects_duplicate() {
    let (a, b) = (id(1), id(2));
    let p = parse("route", "exits", json!([b, a]));
    assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
    assert_eq!(p.settings.net.route_exits.len(), 2);
    assert_eq!(p.settings.net.route_exits[0].to_string(), b);
    let p = parse("route", "exits", json!([a, a]));
    assert!(p.settings.net.route_exits.is_empty());
    assert_eq!(p.field_errors.len(), 1);
    assert!(p.field_errors[0].message.ends_with(
        "expected a list of node ids without repeats, up to 64; using default []"
    ));
    assert_eq!(parse("route", "exits", json!(["bad"])).field_errors.len(), 1);
}

#[test]
fn peer_list_limit_is_64() {
    let ids = |n: u8| (1..=n).map(id).collect::<Vec<_>>();
    assert_eq!(parse("route", "exits", json!(ids(64))).settings.net.route_exits.len(), 64);
    assert_eq!(parse("route", "exits", json!(ids(65))).field_errors.len(), 1);
}

#[test]
fn published_list_limit_is_64() {
    let items = |n: u16| (1..=n).map(|p| json!({"port": p, "for": "all"})).collect::<Vec<_>>();
    let p = parse("ports", "published", json!(items(64)));
    assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
    assert_eq!(p.settings.net.ports_published.len(), 64);
    let p = parse("ports", "published", json!(items(65)));
    assert!(p.settings.net.ports_published.is_empty());
    assert_eq!(p.field_errors.len(), 1);
    assert_eq!(p.field_errors[0].field_path, "ports.published");
}

#[test]
fn opened_list_rejects_repeated_local() {
    let a = id(1);
    let one = |port: u16, local: u16| json!({"node": a, "port": port, "local": local});
    let p = parse("ports", "opened", json!([one(1, 9000), one(2, 9001)]));
    assert_eq!(p.settings.net.ports_opened.len(), 2);
    let p = parse("ports", "opened", json!([one(1, 9000), one(2, 9000)]));
    assert!(p.settings.net.ports_opened.is_empty());
    assert!(p.field_errors[0].message.ends_with(
        "expected a list of {node, port, local} without repeats of (node, port) and local, up to 64; using default []"
    ));
}

#[test]
fn net_names_empty_and_duplicate_rejected() {
    let d = net_names::default_choice();
    let allowed = net_names::transports();
    let want = format!(
        "expected a non-empty list without repeats from: {}; using default {}",
        allowed.join(", "),
        d.transports.join(", ")
    );
    for bad in [json!([]), json!([allowed[0], allowed[0]]), json!(["nope"]), json!("x")] {
        let p = parse("net", "transports", bad);
        assert_eq!(p.settings.net.net_choice, d);
        assert_eq!(p.field_errors.len(), 1);
        assert!(p.field_errors[0].message.ends_with(&want), "{}", p.field_errors[0].message);
    }
    let p = parse("net", "transports", json!([allowed[0]]));
    assert_eq!(p.settings.net.net_choice.transports, vec![allowed[0].to_string()]);
    assert!(p.field_errors.is_empty());
}

#[test]
fn net_sync_unknown_name_is_error() {
    let allowed = net_names::sync();
    let p = parse("net", "sync", json!(allowed[0]));
    assert_eq!(p.settings.net.net_choice.sync, allowed[0]);
    let p = parse("net", "sync", json!("zzz"));
    assert_eq!(p.settings.net.net_choice.sync, net_names::default_choice().sync);
    let want = format!("one of: {}", allowed.join(", "));
    assert!(p.field_errors[0].message.contains(&want));
}

#[test]
fn expected_of_list_rows_literal() {
    // given: list rows; then: `expected()` equals the literal
    let want = |path: &str| crate::config_file::fields::field_by_path(path).expect(path).value.expected();
    assert_eq!(want("route.exits"), "a list of node ids without repeats, up to 64");
    assert_eq!(
        want("ports.opened"),
        "a list of {node, port, local} without repeats of (node, port) and local, up to 64"
    );
    assert_eq!(want("net.sync"), format!("one of: {}", net_names::sync().join(", ")));
    let names = net_names::discovery().join(", ");
    assert_eq!(want("net.discovery"), format!("a non-empty list without repeats from: {names}"));
}

#[test]
fn to_json_lists() {
    // given: two nodes; then: an array of strings in the same order
    let ids = vec![id(1).parse().unwrap(), id(2).parse().unwrap()];
    assert_eq!(super::PeerList.to_json(&ids), Some(json!([id(1), id(2)])));
}
