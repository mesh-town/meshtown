//! Golden tests of file parsing on the original code: the exact texts of the
//! `field_errors` messages, value normalization and the list of unknown keys.

use super::*;
use crate::node::net_names;
use serde_json::{json, Value};

mod every_stored_field_bad {
    use super::*;

    #[test]
    fn parse_messages_golden() {
        // given: a file where all 26 stored paths are wrong (type, bound, a 65-character name,
        // a `socks.port` value longer than 40 characters)
        let config = all_bad_config();
        // when: parse_text; then: the (path, message) pairs in path order equal the literal
        let actual = messages_of(config);
        assert_eq!(actual.len(), 26, "one error per path: {actual:?}");
        assert_eq!(actual, expected_messages());
    }
}

mod memory_exit_below_warn {
    use super::*;

    #[test]
    fn parse_memory_exit_below_warn_golden() {
        // given: warn 500, exit 300
        let config = json!({ "node": { "memory_warn_mib": 500, "memory_exit_mib": 300 } });
        // when: parse_text
        let parsed = parse_text(&file_with(config)).expect("the file parses");
        // then: one error with the exact text, exit takes the default 900
        let errors: Vec<(String, String)> = parsed
            .field_errors
            .into_iter()
            .map(|e| (e.field_path, e.message))
            .collect();
        assert_eq!(
            errors,
            vec![e(
                "node.memory_exit_mib",
                "file has 300, expected at least node.memory_warn_mib (500) or 0; using default 900"
            )]
        );
        assert_eq!(parsed.settings.process.node_memory_exit_mib, 900);
        assert_eq!(parsed.settings.process.node_memory_warn_mib, 500);
    }
}

mod ifaces_bad_value {
    use super::*;

    #[test]
    fn parse_ifaces_string_golden() {
        // given: a string instead of a list in socks.ifaces
        let config = json!({ "socks": { "ifaces": "br-lan" } });
        // when: parse_text; then: one error with the text of item 3 and the default []
        assert_eq!(
            messages_of(config),
            vec![e(
                "socks.ifaces",
                "file has \"br-lan\", expected a list of interface names without repeats ([] — automatic, [\"*\"] — no limits); using default []"
            )]
        );
    }
}

mod normalization {
    use super::*;

    #[test]
    fn parse_normalization_golden() {
        // given: a name with spaces at the edges, a key with spaces, null in the "optional" provider_dns field
        // and autostart turned off
        let config = json!({
            "node": { "name": "  Kitchen  " },
            "network": { "key": "  label--key  " },
            "checks": { "provider_dns": null },
            "autostart": { "enabled": false }
        });
        // when: parse_text
        let parsed = parse_text(&file_with(config)).expect("the file parses");
        // then: no errors; the name is stored untrimmed, the key is trimmed, null → None
        let s = parsed.settings;
        assert_eq!(parsed.field_errors, vec![]);
        assert_eq!(s.net.node_name.as_deref(), Some("  Kitchen  "));
        assert_eq!(s.net.network_key.as_deref(), Some("label--key"));
        assert_eq!(s.net.provider_dns, None);
        assert!(!s.process.autostart_enabled);
    }
}

mod unknown_keys {
    use super::*;

    #[test]
    fn parse_unknown_keys_golden() {
        // given: one extra key in each of the 15 sections, an extra section and 6 computed keys
        let config = with_extras(json!({}));
        // when: parse_text
        let parsed = parse_text(&file_with(config)).expect("the file parses");
        // then: the unknown ones are exactly the extra keys and the section; computed ones are not in the list, no errors
        let mut unknown = parsed.unknown_paths;
        unknown.sort();
        assert_eq!(unknown, expected_unknown());
        assert_eq!(parsed.field_errors, vec![]);
    }
}

// ---- setup and helpers ----

/// A file with a single `config` section (version 1), the rest are defaults.
fn file_with(config: Value) -> String {
    let mut config = config;
    config["version"] = json!(1);
    json!({ "rev": 1, "config": config }).to_string()
}

/// Parses a config; the errors as `(path, message)` pairs sorted by path.
fn messages_of(config: Value) -> Vec<(String, String)> {
    let parsed = parse_text(&file_with(config)).expect("the file parses");
    let mut list: Vec<(String, String)> = parsed
        .field_errors
        .into_iter()
        .map(|e| (e.field_path, e.message))
        .collect();
    list.sort_by(|a, b| a.0.cmp(&b.0));
    list
}

/// A config where all 26 stored paths are wrong.
fn all_bad_config() -> Value {
    let long_value = "9".repeat(50);
    json!({
        "node": { "name": "語".repeat(65), "memory_warn_mib": "x", "memory_exit_mib": "x" },
        "network": { "key": 5 },
        "exit": { "enabled": "x", "max_streams": 100001 },
        "socks": { "enabled": "x", "port": long_value },
        "checks": { "enabled": "x", "interval_sec": 1, "doh_server": "nope", "provider_dns": 5 },
        "probe": { "interval_sec": 1, "full_cycle": 1 },
        "route": { "exits": ["bad"] },
        "ports": { "published": 5, "opened": 5 },
        "web": { "enabled": "x", "port": 80, "access": "public" },
        "log": { "ring_lines": 1 },
        "autostart": { "enabled": true },
        "full_tunnel": { "enabled": "x" },
        "net": { "transports": [], "discovery": ["zzz"], "sync": "nope" }
    })
}

fn e(path: &str, message: &str) -> (String, String) {
    (path.to_string(), message.to_string())
}

/// An empty config plus a `zzz` key in each of the 15 sections, the `extra_section` section and 6
/// keys computed by the node (`network.label|masked`, `autostart|full_tunnel.available|reason`).
fn with_extras(base: Value) -> Value {
    let mut config = base;
    for section in crate::config::config_file::fields::section_names() {
        config[section] = json!({ "zzz": 1 });
    }
    config["extra_section"] = json!({ "a": 1 });
    config["network"]["label"] = json!("x");
    config["network"]["masked"] = json!("x");
    for section in ["autostart", "full_tunnel"] {
        config[section]["available"] = json!(false);
        config[section]["reason"] = json!("x");
    }
    config
}

fn expected_unknown() -> Vec<String> {
    let mut list: Vec<String> = crate::config::config_file::fields::section_names()
        .iter()
        .map(|s| format!("config.{s}.zzz"))
        .collect();
    list.push("config.extra_section".to_string());
    list.sort();
    list
}

/// The expected texts for `all_bad_config()`; the names in `net.*` depend on the build features.
fn expected_messages() -> Vec<(String, String)> {
    vec![
        e("autostart.enabled", r#"file has true, expected false (enabling is unavailable in this build); using default false"#),
        e("checks.doh_server", r#"file has "nope", expected a string like address:port; using default 1.1.1.1:443"#),
        e("checks.enabled", r#"file has "x", expected true or false; using default true"#),
        e("checks.interval_sec", r#"file has 1, expected an integer 30…86400; using default 900"#),
        e("checks.provider_dns", r#"file has 5, expected a string like address:port or null; using default null"#),
        e("exit.enabled", r#"file has "x", expected true or false; using default true"#),
        e("exit.max_streams", r#"file has 100001, expected an integer 0…100000; using default 0"#),
        e("full_tunnel.enabled", r#"file has "x", expected false (enabling is unavailable in this build); using default false"#),
        e("log.ring_lines", r#"file has 1, expected an integer 200…20000; using default 2000"#),
        e(
            "net.discovery",
            &format!(
                "file has [\"zzz\"], expected a non-empty list without repeats from: {}; using default {}",
                net_names::discovery().join(", "),
                net_names::default_choice().discovery.join(", ")
            ),
        ),
        e("net.sync", r#"file has "nope", expected one of: mem, mesh; using default mesh"#),
        e(
            "net.transports",
            &format!(
                "file has [], expected a non-empty list without repeats from: {}; using default hyper",
                net_names::transports().join(", ")
            ),
        ),
        e("network.key", r#"file has not a string, expected a non-empty string; using default no key"#),
        e("node.memory_exit_mib", r#"file has "x", expected an integer 0…1048576; using default 900"#),
        e("node.memory_warn_mib", r#"file has "x", expected an integer 0…1048576; using default 400"#),
        e("node.name", r#"file has "語語語語語語語語語語語語語語語語語語語語語語語語語語語語語語語語語語語語語語語, expected a string up to 64 characters, not empty after trimming whitespace; using default no name"#),
        e("ports.opened", r#"file has 5, expected a list of {node, port, proto, local} without repeats of (node, proto, port) and (proto, local), up to 64, proto is "tcp" (default) or "udp"; using default []"#),
        e("ports.published", r#"file has 5, expected a list of publications {port, proto, name, for} without repeated (proto, port), up to 64, proto is "tcp" (default) or "udp", for is "all" or a list of ids; using default []"#),
        e("probe.full_cycle", r#"file has 1, expected true or false; using default false"#),
        e("probe.interval_sec", r#"file has 1, expected an integer 5…86400; using default 300"#),
        e("route.exits", r#"file has ["bad"], expected a list of node ids without repeats, up to 64; using default []"#),
        e("socks.enabled", r#"file has "x", expected true or false; using default true"#),
        e("socks.port", r#"file has "999999999999999999999999999999999999999, expected a port 1024…65535; using default 1080"#),
        e("web.access", r#"file has "public", expected localhost or localnet; using default localnet"#),
        e("web.enabled", r#"file has "x", expected true or false; using default true"#),
        e("web.port", r#"file has 80, expected a port 1024…65535; using default 7777"#),
    ]
}

mod errors_order {
    use super::*;

    #[test]
    fn parse_errors_follow_table_order() {
        // given: node.name and node.memory_exit_mib are both wrong in one file
        let config = json!({ "version": 1, "node": { "name": "  ", "memory_exit_mib": "x" } });
        let text = json!({ "config": config }).to_string();
        // when: parse_text; then: the error order is the table order, memory_exit_mib before name
        let paths: Vec<String> = parse_text(&text)
            .unwrap()
            .field_errors
            .into_iter()
            .map(|e| e.field_path)
            .collect();
        assert_eq!(paths, vec!["node.memory_exit_mib", "node.name"]);
    }
}
