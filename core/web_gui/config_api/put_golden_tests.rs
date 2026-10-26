//! Golden tests of `PUT`/`GET /api/config` on the original code: the 422 texts for all
//! editable paths, the lists the API returns, and the internal path lists. The harness
//! (`temp_dir`, `new_state`, `put_body`, `body_json`, `StubNode`) comes from the parent `mod_tests.rs`.

use std::collections::BTreeSet;

use super::*;
use crate::config::config_file::fields;
use crate::transport::net::NodeSecret;

const PUBLISHED: &str = "a list of publications {port, proto, name, for} without repeated (proto, port), up to 64, proto is \"tcp\" (default) or \"udp\", for is \"all\" or a list of ids";
const OPENED: &str =
    "a list of {node, port, proto, local} without repeats of (node, proto, port) and (proto, local), up to 64, proto is \"tcp\" (default) or \"udp\"";
const IFACES: &str =
    "a list of interface names without repeats ([] — automatic, [\"*\"] — no limits)";
const FLAG: &str = "true or false";
const OFF: &str = "false (enabling is unavailable in this build)";

type InvalidCase = (&'static str, Value, String);

fn case(path: &'static str, bad: Value, text: &str) -> InvalidCase {
    (path, bad, text.to_string())
}

/// 16 editable paths: a bad value and the 422 text in `fields[0].message`.
fn invalid_cases() -> Vec<InvalidCase> {
    let mut cases = invalid_front_cases();
    cases.extend(invalid_rest_cases());
    cases
}

fn invalid_front_cases() -> Vec<InvalidCase> {
    vec![
        case(
            "node.name",
            json!("x".repeat(65)),
            "a string up to 64 characters, not empty after trimming whitespace",
        ),
        case("web.access", json!("public"), "localhost or localnet"),
        case("web.ifaces", json!("br-lan"), IFACES),
        case("exit.enabled", json!("x"), FLAG),
        case("socks.enabled", json!("x"), FLAG),
        case("socks.port", json!(80), "a port 1024…65535"),
        case("socks.access", json!("public"), "localhost or localnet"),
        case("socks.ifaces", json!("br-lan"), IFACES),
    ]
}

fn invalid_rest_cases() -> Vec<InvalidCase> {
    vec![
        case("checks.enabled", json!("x"), FLAG),
        case("checks.interval_sec", json!(1), "an integer 30…86400"),
        case(
            "checks.doh_server",
            json!("nope"),
            "a string like address:port",
        ),
        case(
            "checks.provider_dns",
            json!(5),
            "a string like address:port or null",
        ),
        case("probe.interval_sec", json!(1), "an integer 5…86400"),
        case(
            "proxy.net",
            json!("AB"),
            "main or a network id: 6 characters a-z, 2-7",
        ),
        case(
            "route.exits",
            json!(["bad"]),
            "a list of node ids without repeats, up to 64",
        ),
        case("ports.published", json!(5), PUBLISHED),
        case("ports.opened", json!(5), OPENED),
        case("log.ring_lines", json!(1), "an integer 200…20000"),
        case("exit.max_streams", json!(100_001), "an integer 0…100000"),
        case("autostart.enabled", json!("x"), OFF),
        case("full_tunnel.enabled", json!("x"), OFF),
    ]
}

mod put_with_invalid_value {
    use super::*;

    #[test]
    fn put_invalid_text_each_editable() {
        // given: a literal of one triple per path covers the whole list of editable paths
        let cases = invalid_cases();
        let covered: BTreeSet<String> = cases.iter().map(|c| c.0.to_string()).collect();
        assert_eq!(
            covered,
            set_of(&fields::editable_paths()),
            "the literal does not cover EDITABLE"
        );
        for (path, bad, text) in cases {
            // when: a PUT with an invalid value of one path
            let (status, v) = put_one(config_with(path, bad));
            // then: 422 invalid, one error on this path with the exact text
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{path}: {v}");
            assert_eq!(v["error"], "invalid", "{path}: {v}");
            assert_eq!(v["fields"][0]["path"], path, "{path}: {v}");
            assert_eq!(v["fields"][0]["message"], text, "{path}: {v}");
            assert_eq!(v["fields"].as_array().unwrap().len(), 1, "{path}: {v}");
        }
    }
}

mod get_body_lists {
    use super::*;

    #[test]
    fn get_lists_golden() {
        // given: a fresh state; when: GET (`config_get_body`)
        let dir = temp_dir();
        let body = new_state(&dir).config_get_body();
        cleanup(&dir);
        // then: read_only (8) and restart_fields (7) as sets equal the literals
        let read_only = [
            "network.label",
            "network.masked",
            "web.port",
            "web.enabled",
            "autostart.available",
            "autostart.reason",
            "full_tunnel.available",
            "full_tunnel.reason",
        ];
        let restart_fields = [
            "socks.enabled",
            "socks.port",
            "socks.access",
            "socks.ifaces",
            "checks.doh_server",
            "checks.provider_dns",
        ];
        assert_eq!(strings_at(&body, "read_only"), set_of(&read_only));
        assert_eq!(strings_at(&body, "restart_fields"), set_of(&restart_fields));
    }
}

mod put_all_editable_at_once {
    use super::*;

    #[test]
    fn put_changed_and_pending_golden() {
        // given: valid non-default values for 15 paths (without autostart and full_tunnel)
        let id = NodeSecret::from_bytes(&[1u8; 32]).peer_id().to_string();
        let patch = json!({
            "node": { "name": "Kitchen" },
            "web": { "access": "localhost" },
            "exit": { "enabled": false, "max_streams": 7 },
            "socks": { "enabled": false, "port": 1090 },
            "checks": {
                "enabled": false, "interval_sec": 600,
                "doh_server": "9.9.9.9:443", "provider_dns": "8.8.8.8:53"
            },
            "probe": { "interval_sec": 120 },
            "route": { "exits": [id] },
            "ports": {
                "published": [{ "port": 8080, "name": "web", "for": "all" }],
                "opened": [{ "node": id, "port": 80, "local": 8081 }]
            },
            "log": { "ring_lines": 500 }
        });
        // when: one PUT
        let (status, v) = put_one(patch);
        // then: `changed` — all 15, `restart_pending` — exactly the 5 paths that need a restart
        assert_eq!(status, StatusCode::OK, "{v}");
        let changed = [
            "node.name",
            "web.access",
            "exit.enabled",
            "exit.max_streams",
            "socks.enabled",
            "socks.port",
            "checks.enabled",
            "checks.interval_sec",
            "checks.doh_server",
            "checks.provider_dns",
            "probe.interval_sec",
            "route.exits",
            "ports.published",
            "ports.opened",
            "log.ring_lines",
        ];
        let pending = [
            "socks.enabled",
            "socks.port",
            "checks.doh_server",
            "checks.provider_dns",
        ];
        assert_eq!(strings_at(&v, "changed"), set_of(&changed));
        assert_eq!(strings_at(&v, "restart_pending"), set_of(&pending));
    }
}

mod internal_lists {
    use super::*;

    #[test]
    fn internal_lists_golden() {
        // given: the former constants `EDITABLE` (17) and `NEEDS_RESTART` (5)
        let editable = [
            "node.name",
            "web.access",
            "web.ifaces",
            "exit.enabled",
            "socks.enabled",
            "socks.port",
            "socks.access",
            "socks.ifaces",
            "checks.enabled",
            "checks.interval_sec",
            "checks.doh_server",
            "checks.provider_dns",
            "probe.interval_sec",
            "proxy.net",
            "route.exits",
            "ports.published",
            "ports.opened",
            "log.ring_lines",
            "exit.max_streams",
            "autostart.enabled",
            "full_tunnel.enabled",
        ];
        let restart_paths = [
            "socks.enabled",
            "socks.port",
            "socks.access",
            "socks.ifaces",
            "checks.doh_server",
            "checks.provider_dns",
        ];
        // then: the sets equal the literals
        assert_eq!(set_of(&fields::editable_paths()), set_of(&editable));
        assert_eq!(set_of(&fields::restart_paths()), set_of(&restart_paths));
    }
}

// ---- setup and helpers ----

use serde_json::json;

/// `{"a": {"b": value}}` at the path `a.b`.
fn config_with(path: &str, value: Value) -> Value {
    let (section, key) = path.split_once('.').expect("a path like section.key");
    json!({ section: { key: value } })
}

/// A fresh page state, one `PUT` with a patch, the response.
fn put_one(patch: Value) -> (StatusCode, Value) {
    let dir = temp_dir();
    let mut state = new_state(&dir);
    let rev = state.file_report.rev;
    let resp = state.put(&put_body(rev, patch), &StubNode::new());
    let answer = body_json(resp);
    cleanup(&dir);
    answer
}

fn set_of(paths: &[&str]) -> BTreeSet<String> {
    paths.iter().map(|p| p.to_string()).collect()
}

/// The array of strings under the key `key` of the response, as a set.
fn strings_at(body: &Value, key: &str) -> BTreeSet<String> {
    body[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key} is not an array: {body}"))
        .iter()
        .map(|s| s.as_str().expect("string").to_string())
        .collect()
}

mod put_text_rows {
    use super::*;

    #[test]
    fn put_file_only_path_keeps_raw_text() {
        // given: an invalid path that the page does not edit; then: the raw parse message
        let (status, v) = put_one(config_with("probe.full_cycle", json!(5)));
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
        let msg = v["fields"][0]["message"].as_str().unwrap_or_default();
        assert!(msg.starts_with("file has 5, expected"), "{msg}");
    }
}
