//! `join` with several networks on one node: a second network goes to its own slot, joining a network again leaves
//! no empty slot behind, and a join the node refuses removes the slot it made.

#[path = "common/cli_node.rs"]
mod cli_node;
#[path = "common/join_fixtures.rs"]
mod fixtures;

use cli_node::{cli_in, scratch_dir, stderr, stdout, CliNode};
use fixtures::{key_with_label, stale_invite, unanswered_invite, StubPage};
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

const PRESENT: (&str, &str, u16, &str) = (
    "POST",
    "/api/network/join",
    409,
    r#"{"error":"network_present"}"#,
);
const NEW_SLOT: (&str, &str, u16, &str) = ("POST", "/api/nets", 200, r#"{"slot":"zz9"}"#);
const WAITING_IN_SLOT: (&str, &str, u16, &str) = (
    "POST",
    "/api/net/zz9/network/join",
    200,
    r#"{"waiting":true,"slot":"zz9"}"#,
);
const WAITING_ON_MAIN: (&str, &str, u16, &str) = (
    "POST",
    "/api/network/join",
    200,
    r#"{"waiting":true,"slot":"main"}"#,
);
const REFUSED_STATE: &str =
    r#"{"node":{"invite_join":{"state":"refused","message":"the invite was used"}}}"#;
const WAITING_STATE: &str = r#"{"node":{"invite_join":{"state":"waiting","message":"waiting"}}}"#;

static REFUSED_WHILE_WAITING: [fixtures::Canned; 5] = [
    ("GET", "/api/nets", 200, FULL_MAIN),
    PRESENT,
    NEW_SLOT,
    WAITING_IN_SLOT,
    ("GET", "/api/net/zz9/state", 200, REFUSED_STATE),
];

static WAITING_FOREVER: [fixtures::Canned; 5] = [
    ("GET", "/api/nets", 200, FULL_MAIN),
    PRESENT,
    NEW_SLOT,
    WAITING_IN_SLOT,
    ("GET", "/api/net/zz9/state", 200, WAITING_STATE),
];

/// The join state is gone and the slot holds no network yet: what a join looks like for a moment before the
/// key is saved, and also what a cancelled join looks like.
static VANISHED_WHILE_WAITING: [fixtures::Canned; 5] = [
    ("GET", "/api/nets", 200, FULL_MAIN),
    PRESENT,
    NEW_SLOT,
    WAITING_IN_SLOT,
    ("GET", "/api/net/zz9/state", 200, r#"{"node":{}}"#),
];

static REFUSED_ON_MAIN: [fixtures::Canned; 3] = [
    ("GET", "/api/nets", 200, r#"{"nets":[]}"#),
    WAITING_ON_MAIN,
    ("GET", "/api/net/main/state", 200, REFUSED_STATE),
];

fn join_invite_on(page: &StubPage, dir: &std::path::Path, extra: &[&str]) -> std::process::Output {
    let invite = unanswered_invite(&key_with_label("room-two", 6));
    let mut args = vec!["join", invite.as_str()];
    args.extend_from_slice(extra);
    cli_in(&args, dir, page.port)
}

#[test]
fn refused_while_waiting_drops_new_slot() {
    // given: a new slot accepts the invite, then the node reports the join refused
    let dir = scratch_dir("wait-refused");
    let page = StubPage::start(&dir, &REFUSED_WHILE_WAITING);
    // when
    let out = join_invite_on(&page, &dir, &[]);
    // then: the command fails with the refusal and the slot it made was removed
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("the invite was used"),
        "{}",
        stderr(&out)
    );
    assert!(page.seen(LEAVE), "{:?}", page.requests.lock().unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn vanished_join_keeps_slot() {
    // given: a new slot accepted the invite, then its join state disappears before the network shows up
    let dir = scratch_dir("wait-vanished");
    let page = StubPage::start(&dir, &VANISHED_WHILE_WAITING);
    // when
    let out = join_invite_on(&page, &dir, &[]);
    // then: the command fails, but the slot stays: the join may just have succeeded
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("cancelled"), "{}", stderr(&out));
    assert!(!page.seen(LEAVE), "{:?}", page.requests.lock().unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn timeout_while_waiting_keeps_slot() {
    // given: a new slot accepted the invite and the join is still going on
    let dir = scratch_dir("wait-timeout");
    let page = StubPage::start(&dir, &WAITING_FOREVER);
    // when: the wait runs out
    let out = join_invite_on(&page, &dir, &["--timeout", "1"]);
    // then: the join goes on in the background, so the slot stays
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("still joining"), "{}", stderr(&out));
    assert!(!page.seen(LEAVE), "{:?}", page.requests.lock().unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refused_on_main_tab_drops_nothing() {
    // given: the main tab takes the invite, then the node reports the join refused
    let dir = scratch_dir("wait-main");
    let page = StubPage::start(&dir, &REFUSED_ON_MAIN);
    // when
    let out = join_invite_on(&page, &dir, &[]);
    // then: the command fails and no slot was made or removed
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    let seen = page.requests.lock().unwrap().clone();
    assert!(!seen.iter().any(|r| r.contains("leave")), "{seen:?}");
    assert!(!seen.iter().any(|r| r == "POST /api/nets"), "{seen:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The names in the node's directory, sorted.
fn dir_names(node: &CliNode) -> Vec<String> {
    let entries = std::fs::read_dir(&node.dir).expect("node dir");
    let mut names: Vec<String> = entries
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn join_refused_removes_slot() {
    // given: a node with one network and a snapshot of its directory and settings
    let node = CliNode::start(&[]);
    join_ok(&node, &key_with_label("room-one", 5));
    let (names, config) = (
        dir_names(&node),
        std::fs::read_to_string(node.dir.join("config.json")).expect("settings"),
    );
    // when: an invite the node refuses is joined
    let invite = stale_invite(&key_with_label("room-two", 6));
    let out = node.cli(&["join", &invite]);
    // then: it fails, and neither a slot nor a network was left behind
    assert_ne!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(slots(&node).len(), 1);
    assert_eq!(dir_names(&node), names);
    let after = std::fs::read_to_string(node.dir.join("config.json")).expect("settings");
    assert_eq!(after, config);
}
