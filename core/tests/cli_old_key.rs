//! A settings file whose network key is in an outdated form: the real binary still starts.
//!
//! What it does: runs `meshtown run --config <file>` with `mesh1-...` as the main network key, waits for the
//! page address, and checks that the process is alive, the journal says the key did not parse, the page answers
//! and `POST /api/network/create` makes a network afterwards.
//! What it doesn't do: it doesn't check the other outdated forms (see `cli_errors.rs` for the command line).

#[path = "common/cli_node.rs"]
mod cli_node;
#[path = "common/node_web.rs"]
mod common;

use cli_node::CliNode;

use std::time::Duration;

use meshtown_core::members::network_key::{managed_key_with_label, NetworkKey};
use meshtown_core::transport::net::NodeSecret;

#[test]
fn an_old_key_in_the_file_starts_the_node_without_a_network() {
    // given: a settings file with a key in the outdated three-word form
    let key = format!("mesh1-{}", "a".repeat(52));
    let config = format!(
        r#"{{"config":{{"version":1,"web":{{"access":"localhost"}},"network":{{"key":"{key}"}}}}}}"#
    );
    // when: the node starts
    let mut node = CliNode::start_with_config(&config, &[]);
    let port = node.port;
    let log = node
        .wait_line("network key did not parse: ", Duration::from_secs(10))
        .expect("the journal line about the key");
    // then: the journal says it starts without a network, the process is alive
    assert!(log.contains("; starting without a network"), "{log}");
    assert!(node.child.try_wait().unwrap().is_none(), "exited early");
    let token = std::fs::read_to_string(node.dir.join("web-token"))
        .expect("web-token")
        .trim()
        .to_string();
    // and the page answers, with no network
    let (code, state) = common::api(port, &token, "GET", "/api/state", None);
    assert_eq!(code, 200, "{state}");
    assert!(state["network"].is_null(), "{state}");
    // and a network can be created afterwards
    let body = Some(serde_json::json!({"label": "fresh"}));
    let (code, reply) = common::api(port, &token, "POST", "/api/network/create", body);
    assert_eq!(code, 200, "{reply}");
    let (code, state) = common::api(port, &token, "GET", "/api/state", None);
    assert_eq!(code, 200, "{state}");
    assert!(!state["network"].is_null(), "{state}");
}

#[test]
fn an_old_key_in_the_file_is_reported_as_a_field_error() {
    // given: a settings file with a key in the outdated three-word form
    let key = format!("mesh1-{}", "a".repeat(52));
    let config = format!(
        r#"{{"config":{{"version":1,"web":{{"access":"localhost"}},"network":{{"key":"{key}"}}}}}}"#
    );
    // when: the node starts and the page asks for the settings
    let node = CliNode::start_with_config(&config, &[]);
    let port = node.port;
    let token = read_token(&node);
    let (code, body) = common::api(port, &token, "GET", "/api/config", None);
    // then: the file block names the field that did not parse
    assert_eq!(code, 200, "{body}");
    let errors = body["file"]["field_errors"]
        .as_array()
        .expect("field errors");
    assert!(errors.iter().any(|e| e["path"] == "network.key"), "{body}");
}

#[test]
fn an_old_key_on_the_command_line_adds_no_slot_beside_the_good_main_key() {
    // given: a good key in the settings file and an outdated key on the command line
    let good = good_key();
    let old = format!("mesh1-{}", "a".repeat(52));
    let config = format!(
        r#"{{"config":{{"version":1,"web":{{"access":"localhost"}},"network":{{"key":"{good}"}}}}}}"#
    );
    // when: the node starts
    let mut node = CliNode::start_with_config(&config, &["--key", &old]);
    let port = node.port;
    let token = read_token(&node);
    // then: the process is alive, the journal says no network was added, the good key runs the main network
    assert!(node
        .wait_line("no network added", Duration::from_secs(10))
        .is_some());
    assert!(node.child.try_wait().unwrap().is_none(), "exited early");
    let (code, state) = common::api(port, &token, "GET", "/api/state", None);
    assert_eq!(code, 200, "{state}");
    assert!(!state["network"].is_null(), "{state}");
    let (code, overview) = common::api(port, &token, "GET", "/api/overview", None);
    assert_eq!(code, 200, "{overview}");
    let slots = overview["slots"].as_array().expect("slots");
    assert_eq!(slots.len(), 1, "{overview}");
    assert!(!slots[0]["state"].is_null(), "{overview}");
    assert_eq!(
        overview["nets"].as_array().map(Vec::len),
        Some(1),
        "{overview}"
    );
}

fn read_token(node: &CliNode) -> String {
    node.token()
}

/// A valid Managed network key of a fresh base.
fn good_key() -> String {
    let root = NodeSecret::from_bytes(&[31; 32]).peer_id();
    let base = NetworkKey::generate().unwrap().to_bytes();
    managed_key_with_label(None, &base, &root)
}
