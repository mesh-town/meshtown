//! `join` with several networks on one node: a second network goes to its own slot, joining a network again leaves
//! no empty slot behind, and a join the node refuses removes the slot it made.

#[path = "common/cli_node.rs"]
mod cli_node;
#[path = "common/join_fixtures.rs"]
mod fixtures;

use cli_node::{cli_in, scratch_dir, stderr, stdout, CliNode};
use fixtures::{key_with_label, StubPage};
use serde_json::Value;

fn slots(node: &CliNode) -> Vec<Value> {
    let out = node.cli(&["status", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let reply: Value = serde_json::from_str(stdout(&out).trim()).expect("one JSON line");
    reply["nets"].as_array().expect("nets").clone()
}

fn join_ok(node: &CliNode, key: &str) {
    let out = node.cli(&["join", key]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}

#[test]
fn repeated_key_of_the_second_slot_adds_no_slot() {
    // given: network one in the main slot, network two in a second slot
    let node = CliNode::start(&[]);
    let (one, two) = (key_with_label("room-one", 5), key_with_label("room-two", 6));
    join_ok(&node, &one);
    join_ok(&node, &two);
    let before = slots(&node);
    assert_eq!(before.len(), 2, "{before:?}");
    // when: each key is joined again
    for key in [&two, &one] {
        let out = node.cli(&["join", key]);
        // then: "already", no new slot, no slot without a network
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(
            stderr(&out).contains("already in this network"),
            "{}",
            stderr(&out)
        );
        assert_eq!(slots(&node), before);
    }
    assert!(before
        .iter()
        .all(|s| s["id_hex"].as_str().is_some_and(|h| !h.is_empty())));
}

/// A page whose main tab is taken by another network and whose new slot `zz9` refuses the join.
const FULL_MAIN: &str = r#"{"nets":[{"id":"main","id_hex":"ffffffff"}]}"#;

static ALREADY_IN_SLOT: [fixtures::Canned; 4] = [
    ("GET", "/api/nets", 200, FULL_MAIN),
    (
        "POST",
        "/api/network/join",
        409,
        r#"{"error":"network_present"}"#,
    ),
    ("POST", "/api/nets", 200, r#"{"slot":"zz9"}"#),
    (
        "POST",
        "/api/net/zz9/network/join",
        409,
        r#"{"error":"net_already_added"}"#,
    ),
];

static REFUSED_IN_SLOT: [fixtures::Canned; 4] = [
    ("GET", "/api/nets", 200, FULL_MAIN),
    (
        "POST",
        "/api/network/join",
        409,
        r#"{"error":"network_present"}"#,
    ),
    ("POST", "/api/nets", 200, r#"{"slot":"zz9"}"#),
    (
        "POST",
        "/api/net/zz9/network/join",
        500,
        r#"{"error":"boom","message":"no"}"#,
    ),
];

const LEAVE: &str = "POST /api/net/zz9/members/leave";

#[test]
fn slot_made_for_a_network_already_present_is_removed() {
    // given: a page that accepts the new slot, then reports the network is already there
    let dir = scratch_dir("slot-already");
    let page = StubPage::start(&dir, &ALREADY_IN_SLOT);
    // when
    let out = cli_in(&["join", &key_with_label("room-two", 6)], &dir, page.port);
    // then: "already", exit 0, and the slot was removed
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("already in this network"),
        "{}",
        stderr(&out)
    );
    assert!(page.seen(LEAVE), "{:?}", page.requests.lock().unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn slot_made_for_a_refused_join_is_removed() {
    // given: a page that accepts the new slot, then refuses the join
    let dir = scratch_dir("slot-refused");
    let page = StubPage::start(&dir, &REFUSED_IN_SLOT);
    // when
    let out = cli_in(&["join", &key_with_label("room-two", 6)], &dir, page.port);
    // then: the command fails with the refusal and the slot was removed
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(page.seen(LEAVE), "{:?}", page.requests.lock().unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}
