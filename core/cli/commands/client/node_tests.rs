//! Tests of choosing a network and of changing settings without a node (`node.rs`).

use super::*;
use serde_json::json;

fn two_nets() -> Value {
    json!({"nets": [
        {"id": "main", "label": "Home", "id_hex": "ab12cd34ef"},
        {"id": "n2", "label": "Work", "id_hex": "ab99887766"},
    ]})
}

fn node(nets: Value, explicit_net: bool) -> Node {
    Node {
        target: Target {
            path: "/nowhere/config.json".into(),
            dir: "/nowhere".into(),
            port: 1,
        },
        base: "/api".into(),
        nets,
        explicit_net,
    }
}

fn stale(rev: u64) -> anyhow::Error {
    let body = json!({"error": "stale_rev", "message": "changed", "rev": rev}).to_string();
    anyhow::Error::new(ApiError::from_reply(409, &body))
}

#[test]
fn pick_net_exact_slot() {
    assert_eq!(pick_net(&two_nets(), "n2"), Ok("n2".to_string()));
}

#[test]
fn pick_net_label_ignores_case() {
    assert_eq!(pick_net(&two_nets(), "home"), Ok("main".to_string()));
}

#[test]
fn pick_net_id_hex_prefix_from_4() {
    assert_eq!(pick_net(&two_nets(), "ab12"), Ok("main".to_string()));
}

#[test]
fn pick_net_prefix_of_3_is_refused() {
    let text = pick_net(&two_nets(), "ab1").expect_err("too short");
    assert!(text.starts_with("no network matches \"ab1\""), "{text}");
}

#[test]
fn pick_net_two_matches_lists_networks() {
    let text = pick_net(&two_nets(), "ab").expect_err("too short");
    assert!(text.contains("no network matches"), "{text}");
    let both = json!({"nets": [
        {"id": "a", "label": "Same", "id_hex": "1111"},
        {"id": "b", "label": "same", "id_hex": "2222"},
    ]});
    let text = pick_net(&both, "SAME").expect_err("two matches");
    assert!(
        text.contains("matches several networks: Same (a), same (b)"),
        "{text}"
    );
}

#[test]
fn pick_net_none_lists_networks() {
    let text = pick_net(&two_nets(), "zzzz").expect_err("none");
    assert_eq!(
        text,
        "no network matches \"zzzz\"; networks: Home (main), Work (n2)"
    );
}

#[test]
fn net_path_main_and_slot() {
    let mut n = node(two_nets(), false);
    assert_eq!(n.net_path("network/invite"), "/api/network/invite");
    n.base = "/api/net/n2".into();
    assert_eq!(n.net_path("members/leave"), "/api/net/n2/members/leave");
}

#[test]
fn query_escape_keeps_unreserved_escapes_utf8() {
    assert_eq!(query_escape("Az09-._~"), "Az09-._~");
    assert_eq!(query_escape("a b&c"), "a%20b%26c");
    assert_eq!(query_escape("é"), "%C3%A9");
}

#[test]
fn require_net_two_nets_without_net_is_usage() {
    let e = node(two_nets(), false)
        .require_net("leave")
        .expect_err("usage");
    assert!(e.downcast_ref::<UsageError>().is_some());
    assert!(e.to_string().contains("Home (main), Work (n2)"), "{e}");
}

#[test]
fn require_net_one_net_or_explicit_passes() {
    let one = json!({"nets": [{"id": "main"}]});
    assert!(node(one, false).require_net("leave").is_ok());
    assert!(node(two_nets(), true).require_net("leave").is_ok());
}

#[test]
fn put_config_retries_once_on_stale_rev() {
    let mut calls: Vec<(String, String, Option<Value>)> = Vec::new();
    let mut puts = 0;
    let mut call = |m: &str, p: &str, b: Option<&Value>| -> anyhow::Result<Value> {
        calls.push((m.into(), p.into(), b.cloned()));
        match m {
            "GET" => Ok(json!({"rev": 3})),
            _ if puts == 0 => {
                puts += 1;
                Err(stale(8))
            }
            _ => Ok(json!({"ok": true})),
        }
    };
    let patch = json!({"web": {"access": "localnet"}});
    let done = put_with_retry(&mut call, "/api", &patch).expect("second put succeeds");
    assert_eq!(done, json!({"ok": true}));
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[1].2, Some(json!({"rev": 3, "config": patch})));
    assert_eq!(calls[2].2, Some(json!({"rev": 8, "config": patch})));
    assert_eq!(calls[2].1, "/api/config");
}

#[test]
fn put_config_second_stale_rev_is_error() {
    let mut count = 0;
    let mut call = |m: &str, _: &str, _: Option<&Value>| -> anyhow::Result<Value> {
        count += 1;
        match m {
            "GET" => Ok(json!({"rev": 1})),
            _ => Err(stale(2)),
        }
    };
    let e = put_with_retry(&mut call, "/api", &json!({})).expect_err("two stale answers");
    assert!(e.downcast_ref::<ApiError>().is_some());
    assert_eq!(count, 3);
}

#[test]
fn put_config_without_rev_is_error() {
    let mut call = |_: &str, _: &str, _: Option<&Value>| -> anyhow::Result<Value> { Ok(json!({})) };
    let e = put_with_retry(&mut call, "/api", &json!({})).expect_err("no revision");
    assert_eq!(e.to_string(), "the reply has no settings revision");
}
