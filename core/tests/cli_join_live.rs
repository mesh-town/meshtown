//! `join` on a running node: what the label, `status` and the slot list show, joining the same network again, and an
//! invite nobody answers.
//! What it doesn't do: an invite answered by a member (needs a network between two processes; the invite tests
//! cover the exchange in one process).

#[path = "common/cli_node.rs"]
mod cli_node;
#[path = "common/join_fixtures.rs"]
mod fixtures;

use std::time::Duration;

use cli_node::{stderr, stdout, CliNode};
use fixtures::{key_with_label, unanswered_invite};
use serde_json::Value;

/// The slots the node reports, from `status --json`.
fn slots(node: &CliNode) -> Vec<Value> {
    let out = node.cli(&["status", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let reply: Value = serde_json::from_str(stdout(&out).trim()).expect("one JSON line");
    reply["nets"].as_array().expect("nets").clone()
}

#[test]
fn joined_network_is_shown_by_status() {
    // given: a node without a network
    let node = CliNode::start(&[]);
    let key = key_with_label("our-room", 9);
    // when
    let joined = node.cli(&["join", &key]);
    // then: the label on stdout; the plain and the JSON status name the network
    assert_eq!(joined.status.code(), Some(0), "{}", stderr(&joined));
    assert_eq!(stdout(&joined).trim(), "our-room");
    let plain = node.cli(&["status"]);
    assert!(stdout(&plain).contains("our-room"), "{}", stdout(&plain));
    assert!(
        !stderr(&plain).contains("Not in a network"),
        "{}",
        stderr(&plain)
    );
    let list = slots(&node);
    assert_eq!(list.len(), 1, "{list:?}");
    assert_eq!(list[0]["id"], "main");
    assert!(list[0]["id_hex"].as_str().is_some_and(|h| !h.is_empty()));
}

#[test]
fn same_key_again_changes_nothing() {
    // given: a node in the network of the key, settled
    let mut node = CliNode::start(&[]);
    let key = key_with_label("our-room", 9);
    assert_eq!(node.cli(&["join", &key]).status.code(), Some(0));
    let before = slots(&node);
    let restarts = node.count_lines("restarting the node", Duration::from_secs(1));
    // when: the same key is joined again
    let out = node.cli(&["join", &key]);
    // then: "already", the slots are the same and the node did not restart
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("already in this network"),
        "{}",
        stderr(&out)
    );
    assert!(stdout(&out).trim().is_empty(), "{}", stdout(&out));
    assert_eq!(slots(&node), before);
    let after = node.count_lines("restarting the node", Duration::from_secs(2));
    assert_eq!(after, restarts, "the node restarted");
}

#[test]
fn unanswered_invite_waits_then_gives_up() {
    // given: a node without a network and an invite whose issuer is not there
    let node = CliNode::start(&[]);
    let invite = unanswered_invite(&key_with_label("our-room", 9));
    // when
    let out = node.cli(&["join", &invite, "--timeout", "2"]);
    // then: it waited, then said the join goes on in the background; nothing on stdout
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("Waiting for network members"),
        "{}",
        stderr(&out)
    );
    assert!(stderr(&out).contains("still joining"), "{}", stderr(&out));
    assert!(stdout(&out).trim().is_empty(), "{}", stdout(&out));
}
