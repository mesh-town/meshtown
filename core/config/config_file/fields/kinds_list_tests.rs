//! List value kinds through file parsing: bounds, repeats, error texts.

use serde_json::{json, Value};

use super::super::value::Kind;

use crate::config::config_file::settings::Settings;
use crate::config::config_file::test_support::test_secret;
use crate::config::config_file::{parse_text, render_config_file, ParsedConfigFile};
use crate::node::net_names;
use crate::ports_sharing::types::{Audience, Opened, Proto, Published};
use crate::transport::net::NodeSecret;

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
    assert!(p.field_errors[0]
        .message
        .ends_with("expected a list of node ids without repeats, up to 64; using default []"));
    assert_eq!(
        parse("route", "exits", json!(["bad"])).field_errors.len(),
        1
    );
}

#[test]
fn peer_list_limit_is_64() {
    let ids = |n: u8| (1..=n).map(id).collect::<Vec<_>>();
    assert_eq!(
        parse("route", "exits", json!(ids(64)))
            .settings
            .net
            .route_exits
            .len(),
        64
    );
    assert_eq!(
        parse("route", "exits", json!(ids(65))).field_errors.len(),
        1
    );
}

#[test]
fn published_list_limit_is_64() {
    let items = |n: u16| {
        (1..=n)
            .map(|p| json!({"port": p, "for": "all"}))
            .collect::<Vec<_>>()
    };
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
        "expected a list of {node, port, proto, local} without repeats of (node, proto, port) and (proto, local), up to 64, proto is \"tcp\" (default) or \"udp\"; using default []"
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
    for bad in [
        json!([]),
        json!([allowed[0], allowed[0]]),
        json!(["nope"]),
        json!("x"),
    ] {
        let p = parse("net", "transports", bad);
        assert_eq!(p.settings.net.net_choice, d);
        assert_eq!(p.field_errors.len(), 1);
        assert!(
            p.field_errors[0].message.ends_with(&want),
            "{}",
            p.field_errors[0].message
        );
    }
    let p = parse("net", "transports", json!([allowed[0]]));
    assert_eq!(
        p.settings.net.net_choice.transports,
        vec![allowed[0].to_string()]
    );
    assert!(p.field_errors.is_empty());
}

#[test]
fn net_sync_unknown_name_is_error() {
    let allowed = net_names::sync();
    let p = parse("net", "sync", json!(allowed[0]));
    assert_eq!(p.settings.net.net_choice.sync, allowed[0]);
    let p = parse("net", "sync", json!("zzz"));
    assert_eq!(
        p.settings.net.net_choice.sync,
        net_names::default_choice().sync
    );
    let want = format!("one of: {}", allowed.join(", "));
    assert!(p.field_errors[0].message.contains(&want));
}

#[test]
fn expected_of_list_rows_literal() {
    // given: list rows; then: `expected()` equals the literal
    let want = |path: &str| {
        crate::config::config_file::fields::field_by_path(path)
            .expect(path)
            .value
            .expected()
    };
    assert_eq!(
        want("route.exits"),
        "a list of node ids without repeats, up to 64"
    );
    assert_eq!(
        want("ports.opened"),
        "a list of {node, port, proto, local} without repeats of (node, proto, port) and (proto, local), up to 64, proto is \"tcp\" (default) or \"udp\""
    );
    assert_eq!(
        want("net.sync"),
        format!("one of: {}", net_names::sync().join(", "))
    );
    let names = net_names::discovery().join(", ");
    assert_eq!(
        want("net.discovery"),
        format!("a non-empty list without repeats from: {names}")
    );
}

#[test]
fn to_json_lists() {
    // given: two nodes; then: an array of strings in the same order
    let ids = vec![id(1).parse().unwrap(), id(2).parse().unwrap()];
    assert_eq!(super::PeerList.to_json(&ids), Some(json!([id(1), id(2)])));
}

const PUBLISHED_TEXT: &str = "a list of publications {port, proto, name, for} without repeated (proto, port), up to 64, proto is \"tcp\" (default) or \"udp\", for is \"all\" or a list of ids";
const OPENED_TEXT: &str = "a list of {node, port, proto, local} without repeats of (node, proto, port) and (proto, local), up to 64, proto is \"tcp\" (default) or \"udp\"";

mod expected_text {
    use super::*;

    #[test]
    fn published_list_names_the_protocol_verbatim() {
        let want = crate::config::config_file::fields::field_by_path("ports.published")
            .expect("ports.published")
            .value
            .expected();
        assert_eq!(want, PUBLISHED_TEXT);
    }

    #[test]
    fn broken_udp_published_list_is_dropped_with_the_verbatim_text() {
        // given: UDP 53 twice
        let item = json!({"port": 53, "proto": "udp", "for": "all"});
        // when
        let p = parse("ports", "published", json!([item, item]));
        // then
        assert!(p.settings.net.ports_published.is_empty());
        assert_eq!(p.field_errors.len(), 1, "{:?}", p.field_errors);
        let tail = format!("expected {PUBLISHED_TEXT}; using default []");
        assert!(
            p.field_errors[0].message.ends_with(&tail),
            "{}",
            p.field_errors[0].message
        );
    }

    #[test]
    fn broken_udp_opened_list_is_dropped_with_the_verbatim_text() {
        // given: one UDP local used twice
        let a = id(1);
        let one = |port: u16| json!({"node": a, "port": port, "proto": "udp", "local": 9000});
        // when
        let p = parse("ports", "opened", json!([one(1), one(2)]));
        // then
        assert!(p.settings.net.ports_opened.is_empty());
        let tail = format!("expected {OPENED_TEXT}; using default []");
        assert!(
            p.field_errors[0].message.ends_with(&tail),
            "{}",
            p.field_errors[0].message
        );
    }

    #[test]
    fn same_local_on_two_protocols_is_not_a_repeat() {
        // given: TCP 53 and UDP 53 on one local number
        let a = id(1);
        let tcp = json!({"node": a, "port": 53, "local": 2053});
        let udp = json!({"node": a, "port": 53, "proto": "udp", "local": 2053});
        // when
        let p = parse("ports", "opened", json!([tcp, udp]));
        // then
        assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
        assert_eq!(p.settings.net.ports_opened.len(), 2);
    }
}

mod written_entries {
    use super::*;

    #[test]
    fn tcp_entry_is_written_without_proto() {
        // given: one TCP publication and one TCP opened port
        let text = when_rendered(
            vec![published(Proto::Tcp, 80)],
            vec![opened(Proto::Tcp, 80, 9000)],
        );
        // then
        assert!(!text.contains("\"proto\""), "{text}");
    }

    #[test]
    fn udp_entry_is_written_with_proto_after_port() {
        // given: one UDP publication and one UDP opened port
        let text = when_rendered(
            vec![published(Proto::Udp, 53)],
            vec![opened(Proto::Udp, 53, 2053)],
        );
        // then: `port, proto, name, for` and `node, port, proto, local`
        assert!(text.contains("\"proto\": \"udp\""), "{text}");
        let ports = &text[text.find("\"ports\"").expect("the ports section")..];
        let (published, opened) = ports.split_at(ports.find("\"opened\"").expect("opened"));
        assert_in_order(published, &["\"port\"", "\"proto\"", "\"name\"", "\"for\""]);
        assert_in_order(opened, &["\"node\"", "\"port\"", "\"proto\"", "\"local\""]);
    }

    #[test]
    fn only_the_udp_entry_of_a_mixed_list_carries_proto() {
        // given: TCP 53 and UDP 53
        let list = vec![published(Proto::Tcp, 53), published(Proto::Udp, 53)];
        // when
        let text = when_rendered(list, vec![]);
        // then
        assert_eq!(text.matches("\"proto\"").count(), 1, "{text}");
    }

    #[test]
    fn written_lists_read_back_unchanged() {
        // given: both protocols on one number
        let pub_list = vec![published(Proto::Tcp, 53), published(Proto::Udp, 53)];
        let open_list = vec![opened(Proto::Tcp, 53, 2053), opened(Proto::Udp, 53, 2053)];
        // when
        let text = when_rendered(pub_list.clone(), open_list.clone());
        let back = parse_text(&text).expect("the document parsed");
        // then
        assert!(back.field_errors.is_empty(), "{:?}", back.field_errors);
        assert_eq!(back.settings.net.ports_published, pub_list);
        assert_eq!(back.settings.net.ports_opened, open_list);
    }
}

fn published(proto: Proto, port: u16) -> Published {
    Published {
        proto,
        port,
        name: "n".into(),
        audience: Audience::All,
    }
}

fn opened(proto: Proto, port: u16, local: u16) -> Opened {
    let node = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    Opened {
        node,
        port,
        proto,
        local,
    }
}

/// The settings file text for the given lists.
fn when_rendered(published: Vec<Published>, opened: Vec<Opened>) -> String {
    let mut settings = Settings::default();
    settings.net.ports_published = published;
    settings.net.ports_opened = opened;
    render_config_file(1, 1_800_000_000, &test_secret(), &settings)
}

fn assert_in_order(text: &str, keys: &[&str]) {
    let at: Vec<_> = keys
        .iter()
        .map(|k| text.find(k).unwrap_or(usize::MAX))
        .collect();
    assert!(
        at.windows(2).all(|w| w[0] < w[1]),
        "keys {keys:?} at {at:?} in {text}"
    );
}
