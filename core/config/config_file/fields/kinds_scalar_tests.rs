//! Scalar value kinds through file parsing: bounds, `null`, error texts.

use serde_json::{json, Value};

use super::super::value::Kind;

use crate::config::config_file::{parse_text, ProcessSettings, Settings};

/// Returns the parsed settings and (path, message) pairs for a file with a single key `section.key = value`.
fn read(section: &str, key: &str, value: Value) -> (Settings, Vec<(String, String)>) {
    let text = json!({ "config": { "version": 1, section: { key: value } } }).to_string();
    let p = parse_text(&text).expect("the document parsed");
    let errs = p
        .field_errors
        .into_iter()
        .map(|e| (e.field_path, e.message))
        .collect();
    (p.settings, errs)
}

/// Returns the message of the only error.
fn message(section: &str, key: &str, value: Value) -> String {
    let (_, errs) = read(section, key, value);
    assert_eq!(errs.len(), 1, "{section}.{key}: {errs:?}");
    errs[0].1.clone()
}

fn assert_ok(section: &str, key: &str, value: Value) -> Settings {
    let (s, errs) = read(section, key, value);
    assert!(errs.is_empty(), "{section}.{key}: {errs:?}");
    s
}

#[test]
fn flag_takes_bool_only() {
    // given: exit.enabled (default true); with false, it is taken
    assert!(!assert_ok("exit", "enabled", json!(false)).net.exit_enabled);
    // with a string and null, an error with the text and the default true
    let want = r#"file has "x", expected true or false; using default true"#;
    assert_eq!(message("exit", "enabled", json!("x")), want);
    assert!(message("exit", "enabled", json!(null)).contains("expected true or false"));
}

#[test]
fn int_bounds() {
    // given: checks.interval_sec 30…86400, default 900
    assert_eq!(
        assert_ok("checks", "interval_sec", json!(30))
            .net
            .check_interval_sec,
        30
    );
    assert_eq!(
        assert_ok("checks", "interval_sec", json!(86_400))
            .net
            .check_interval_sec,
        86_400
    );
    // one beyond the bounds, negative, fractional, a string, null — an error
    let want = "file has 29, expected an integer 30…86400; using default 900";
    assert_eq!(message("checks", "interval_sec", json!(29)), want);
    for bad in [
        json!(86_401),
        json!(-1),
        json!(1.5),
        json!("30"),
        json!(null),
    ] {
        let (s, errs) = read("checks", "interval_sec", bad);
        assert_eq!(errs.len(), 1);
        assert_eq!(s.net.check_interval_sec, 900);
    }
}

#[test]
fn memory_warn_upper_bound() {
    assert_eq!(
        assert_ok("node", "memory_warn_mib", json!(1_048_576))
            .process
            .node_memory_warn_mib,
        1_048_576
    );
    let want = "file has 1048577, expected an integer 0…1048576; using default 400";
    assert_eq!(message("node", "memory_warn_mib", json!(1_048_577)), want);
}

#[test]
fn port_bounds() {
    // given: socks.port 1024…65535, default from Settings::default()
    assert_eq!(
        assert_ok("socks", "port", json!(1024)).process.socks_port,
        1024
    );
    assert_eq!(
        assert_ok("socks", "port", json!(65_535)).process.socks_port,
        65_535
    );
    let d = Settings::default().process.socks_port;
    let want = format!("file has 1023, expected a port 1024…65535; using default {d}");
    assert_eq!(message("socks", "port", json!(1023)), want);
    for bad in [
        json!(65_536),
        json!(70_000),
        json!(-1),
        json!(80.5),
        json!(null),
    ] {
        assert_eq!(read("socks", "port", bad).1.len(), 1);
    }
}

#[test]
fn off_only_rejects_true() {
    // given: autostart.enabled; false passes, true is an error, default false
    assert!(
        !assert_ok("autostart", "enabled", json!(false))
            .process
            .autostart_enabled
    );
    let (s, errs) = read("autostart", "enabled", json!(true));
    assert!(!s.process.autostart_enabled);
    let want = "file has true, expected false (enabling is unavailable in this build); using default false";
    assert_eq!(errs[0].1, want);
}

#[test]
fn addr_and_opt_addr() {
    let doh = crate::site_checks::doh::DEFAULT_DOH;
    let s = assert_ok("checks", "doh_server", json!("1.2.3.4:53"));
    assert_eq!(s.net.doh_server, "1.2.3.4:53".parse().unwrap());
    let want = format!("file has \"x\", expected a string like address:port; using default {doh}");
    assert_eq!(message("checks", "doh_server", json!("x")), want);
    let s = assert_ok("checks", "provider_dns", json!("[::1]:53"));
    assert_eq!(s.net.provider_dns, Some("[::1]:53".parse().unwrap()));
    assert_eq!(
        assert_ok("checks", "provider_dns", json!(null))
            .net
            .provider_dns,
        None
    );
    let want = "file has 5, expected a string like address:port or null; using default null";
    assert_eq!(message("checks", "provider_dns", json!(5)), want);
}

#[test]
fn choice_parses_and_rejects() {
    use crate::config::config_file::settings::TransportMode;
    let s = assert_ok("transport", "mode", json!("relay_only"));
    assert_eq!(s.net.transport_mode, TransportMode::RelayOnly);
    let want = "file has \"x\", expected direct_and_relay, relay_only or no_relay; using default direct_and_relay";
    assert_eq!(message("transport", "mode", json!("x")), want);
    assert!(
        read("transport", "mode", json!(null)).1.len() == 1,
        "null for Choice is an error"
    );
}

#[test]
fn node_name_limits() {
    let name64 = "語".repeat(64);
    assert_eq!(
        assert_ok("node", "name", json!(name64)).net.node_name,
        Some("語".repeat(64))
    );
    let (s, errs) = read("node", "name", json!("語".repeat(65)));
    assert_eq!(s.net.node_name, None);
    assert!(errs[0].1.ends_with(
        "expected a string up to 64 characters, not empty after trimming whitespace; using default no name"
    ));
    assert_eq!(read("node", "name", json!("   ")).1.len(), 1);
    assert_eq!(read("node", "name", json!(null)).1.len(), 1);
    // the name is stored without trimming
    assert_eq!(
        assert_ok("node", "name", json!("  Kitchen "))
            .net
            .node_name
            .as_deref(),
        Some("  Kitchen ")
    );
}

#[test]
fn net_key_is_trimmed() {
    assert_eq!(
        assert_ok("network", "key", json!("  k1 "))
            .net
            .network_key
            .as_deref(),
        Some("k1")
    );
    let (s, errs) = read("network", "key", json!("   "));
    assert_eq!(s.net.network_key, None);
    assert!(errs[0]
        .1
        .ends_with("expected a non-empty string; using default no key"));
    assert!(!errs[0].1.contains("   "), "the key value is not shown");
}

#[test]
fn memory_exit_without_key_follows_warn() {
    // given: warn 1000, no exit key
    let text = r#"{"config":{"version":1,"node":{"memory_warn_mib":1000}}}"#;
    let p = parse_text(text).unwrap();
    // then: exit is silently raised to warn
    assert_eq!(p.settings.process.node_memory_exit_mib, 1000);
    assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
}

#[test]
fn memory_exit_below_warn_and_zero() {
    let doc = |warn: u64, exit: u64| {
        let node = json!({"memory_warn_mib": warn, "memory_exit_mib": exit});
        parse_text(&json!({"config": {"version": 1, "node": node}}).to_string()).unwrap()
    };
    let p = doc(500, 300);
    assert_eq!(p.settings.process.node_memory_exit_mib, 900);
    assert_eq!(p.field_errors.len(), 1);
    assert_eq!(p.field_errors[0].field_path, "node.memory_exit_mib");
    assert!(p.field_errors[0]
        .message
        .contains("at least node.memory_warn_mib (500) or 0"));
    assert_eq!(doc(500, 0).settings.process.node_memory_exit_mib, 0);
    assert_eq!(doc(0, 100).settings.process.node_memory_exit_mib, 100);
    assert_eq!(doc(500, 500).field_errors, vec![]);
    assert_eq!(doc(500, 1_048_577).field_errors.len(), 1);
}

#[test]
fn expected_of_rows_literal() {
    // given: table rows of different kinds; then: `expected()` equals the literal
    let want = |path: &str| {
        crate::config::config_file::fields::field_by_path(path)
            .expect(path)
            .value
            .expected()
    };
    assert_eq!(
        want("node.name"),
        "a string up to 64 characters, not empty after trimming whitespace"
    );
    assert_eq!(
        want("transport.mode"),
        "direct_and_relay, relay_only or no_relay"
    );
    assert_eq!(want("web.access"), "localhost or localnet");
    assert_eq!(
        want("autostart.enabled"),
        "false (enabling is unavailable in this build)"
    );
    assert_eq!(want("node.memory_exit_mib"), "an integer 0…1048576");
    assert_eq!(want("socks.port"), "a port 1024…65535");
    assert_eq!(want("network.label"), "", "a computed row has no text");
}

#[test]
fn copy_moves_only_own_field() {
    // given: `from` with a different exit threshold; when copying the row; then only it changes
    let row =
        crate::config::config_file::fields::field_by_path("node.memory_exit_mib").expect("row");
    let from = Settings {
        process: ProcessSettings {
            node_memory_exit_mib: 777,
            ..ProcessSettings::default()
        },
        ..Settings::default()
    };
    let mut to = Settings::default();
    row.value.copy_value_between(&from, &mut to);
    assert_eq!(
        to,
        Settings {
            process: ProcessSettings {
                node_memory_exit_mib: 777,
                ..ProcessSettings::default()
            },
            ..Settings::default()
        }
    );
}

#[test]
fn to_json_ipv6() {
    // given: an IPv6 address
    let addr: std::net::SocketAddr = "[::1]:53".parse().unwrap();
    // then: the address is a string with brackets
    assert_eq!(super::OptAddr.to_json(&Some(addr)), Some(json!("[::1]:53")));
    assert_eq!(super::OptAddr.to_json(&None), Some(Value::Null));
}
