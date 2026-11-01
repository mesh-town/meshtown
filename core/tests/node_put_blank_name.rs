//! `PUT /api/config` on a live node: a blank node name is replaced, `null` removes the name, a long name is refused.

#[path = "common/cli_node.rs"]
mod cli_node;
#[path = "common/node_web.rs"]
mod common;

use cli_node::CliNode;
use serde_json::{json, Value};

fn clip(body: &Value) -> String {
    body.to_string().chars().take(200).collect()
}

fn get(port: u16, token: &str) -> Value {
    let (code, body) = common::api(port, token, "GET", "/api/config", None);
    assert_eq!(code, 200, "GET: {}", clip(&body));
    body
}

fn put(port: u16, token: &str, name: Value) -> (u16, Value) {
    let snapshot = get(port, token);
    let mut config = snapshot["config"].clone();
    config["node"]["name"] = name;
    let doc = json!({"rev": snapshot["rev"], "config": config});
    let (code, body) = common::api(port, token, "PUT", "/api/config", Some(doc));
    println!("PUT status={code} body={}", clip(&body));
    (code, body)
}

fn name_now(port: u16, token: &str) -> Value {
    get(port, token)["config"]["node"]["name"].clone()
}

#[test]
fn put_blank_node_name_on_a_live_node() {
    let node = CliNode::start(&[]);
    let (port, token) = (node.port, node.token());
    let mut failures: Vec<String> = Vec::new();
    // an untouched snapshot goes back as it is
    let snapshot = get(port, &token);
    let doc = json!({"rev": snapshot["rev"], "config": snapshot["config"]});
    let (code, body) = common::api(port, &token, "PUT", "/api/config", Some(doc));
    println!("PUT same status={code} body={}", clip(&body));
    if code != 200 {
        failures.push(format!("same document: {code} {}", clip(&body)));
    }
    // empty and blank names are replaced by a generated one
    for blank in ["", "   "] {
        let (code, body) = put(port, &token, json!(blank));
        if code != 200 {
            failures.push(format!("name {blank:?}: {code} {}", clip(&body)));
        }
        let now = name_now(port, &token);
        if !now.as_str().is_some_and(|s| !s.trim().is_empty()) {
            failures.push(format!("name {blank:?} left {now}"));
        }
    }
    // null removes the name: the page then shows none
    let (code, body) = put(port, &token, Value::Null);
    if code != 200 {
        failures.push(format!("null: {code} {}", clip(&body)));
    }
    if body["changed"] != json!(["node.name"]) {
        failures.push(format!("null: changed is {}", body["changed"]));
    }
    let after = name_now(port, &token);
    if !after.is_null() {
        failures.push(format!("null left the name {after}"));
    }
    // 65 characters are refused
    let (code, body) = put(port, &token, json!("x".repeat(65)));
    if code != 422 {
        failures.push(format!("65 chars: {code} {}", clip(&body)));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
