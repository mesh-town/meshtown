//! Several networks on one node, end to end: node A runs under the process host with its own network;
//! node B (another network, or the same one) hands out invites over the in-process test network.
//! What it checks: adding a slot and joining by an invite, the duplicate refusal, leaving a slot and the main
//! network. Links: `core/tests/common/nets_host.rs`, `core/tests/node_web_bootstrap.rs` (single network).

#[path = "common/node_web.rs"]
mod common;
#[path = "common/nets_host.rs"]
mod nets_host;

use std::time::Duration;

use meshtown_core::members::network_key::NetworkKey;
use serde_json::{json, Value};

use common::{api, boot, boot_without_key, teardown, wait_ready, wait_until, Booted};
use nets_host::{boot_host, HostNode};

/// A fresh invite of the network node `b` is in.
fn invite_of(b: &Booted) -> String {
    let (status, body) = api(b.port, &b.token, "GET", "/api/network/invite", None);
    assert_eq!(status, 200, "the invite must be built");
    body["invite"].as_str().expect("invite string").to_string()
}

/// Adds a slot on `a` and returns its id.
fn add_slot(a: &HostNode) -> String {
    let (status, body) = a.post("/api/nets", json!({}));
    assert_eq!(status, 201, "a new slot: {body}");
    body["slot"].as_str().expect("slot id").to_string()
}

/// Enters the network of `invite` in `slot` and waits until the slot lists another online member.
async fn join_slot(a: &HostNode, slot: &str, invite: &str) {
    let path = format!("/api/net/{slot}/network/join");
    let (status, body) = a.post(&path, json!({"key": invite}));
    assert_eq!(status, 202, "join in the slot: {body}");
    let members = format!("/api/net/{slot}/members");
    let ok = wait_until(Duration::from_secs(20), || other_listed(&a.get(&members).1)).await;
    assert!(ok, "the slot never listed another member");
}

/// Whether the member list has a row other than the node's own.
fn other_listed(reply: &Value) -> bool {
    reply["members"]
        .as_array()
        .is_some_and(|rows| rows.iter().any(|r| r["self"] == json!(false)))
}

/// Whether the state page shows another node that is connected right now.
fn other_online_peer(state: &Value) -> bool {
    state["peers"].as_array().is_some_and(|rows| {
        rows.iter()
            .any(|r| r["self"] == json!(false) && r["online"] == json!(true))
    })
}

fn net_ids(a: &HostNode) -> Vec<String> {
    let (_, v) = a.get("/api/nets");
    let nets = v["nets"].as_array().expect("nets array");
    nets.iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect()
}

/// Whether `b` lists a member that left: an excluded row with `how == "left"`.
fn lists_a_member_that_left(b: &Booted) -> bool {
    let (_, v) = api(b.port, &b.token, "GET", "/api/members", None);
    v["members"].as_array().is_some_and(|rows| {
        rows.iter()
            .any(|r| r["self"] == json!(false) && r["excluded"]["how"] == json!("left"))
    })
}

/// A second network enters through a new slot; the same network a second time is refused, and the refused
/// invite still works for a third node.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn second_network_joins_a_new_slot_and_a_duplicate_is_refused() {
    let a = boot_host("dup-a", &NetworkKey::generate().unwrap()).await;
    let b = boot("dup-b", &NetworkKey::generate().unwrap()).await;
    wait_ready(&b).await;

    let slot = add_slot(&a);
    join_slot(&a, &slot, &invite_of(&b)).await;
    assert_eq!(net_ids(&a).len(), 2, "main and the new slot");

    let second = add_slot(&a);
    let again = invite_of(&b);
    let (status, body) = a.post(
        &format!("/api/net/{second}/network/join"),
        json!({"key": again}),
    );
    assert_eq!(status, 409);
    assert_eq!(body["error"], json!("net_already_added"));
    assert_eq!(body["slot"], json!(slot));

    let c = boot_without_key("dup-c").await;
    let (status, _) = api(
        c.port,
        &c.token,
        "POST",
        "/api/network/join",
        Some(json!({"key": again})),
    );
    assert_eq!(status, 202, "the refused invite was not spent");
    let ok = wait_until(Duration::from_secs(20), || {
        let (_, v) = api(c.port, &c.token, "GET", "/api/state", None);
        v["peers"].as_array().is_some_and(|p| {
            p.iter()
                .any(|x| x["self"] == json!(false) && x["online"] == json!(true))
        })
    })
    .await;
    assert!(ok, "the third node did not get in with the refused invite");

    teardown(c, Duration::from_secs(5)).await;
    teardown(b, Duration::from_secs(5)).await;
    a.teardown().await;
}

/// Leaving an added slot drops it from the list and leaves the main network alone.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn leaving_an_added_slot_drops_it_and_keeps_main() {
    let a = boot_host("slot-a", &NetworkKey::generate().unwrap()).await;
    let b = boot("slot-b", &NetworkKey::generate().unwrap()).await;
    wait_ready(&b).await;
    let slot = add_slot(&a);
    join_slot(&a, &slot, &invite_of(&b)).await;

    let path = format!("/api/net/{slot}/members/leave");
    let (status, body) = a.post(&path, json!({"confirm": true}));
    assert_eq!(status, 200, "leave: {body}");

    assert_eq!(net_ids(&a), vec!["main".to_string()]);
    let (_, state) = a.get("/api/state");
    assert_ne!(state["network"], Value::Null, "the main network stays up");

    teardown(b, Duration::from_secs(5)).await;
    a.teardown().await;
}

/// The network the slot was in lists the node as a member that left. The leave notice travels over the
/// slot's link to the other node, so that link must survive every page read before the leave. A network
/// entered at run time counts its list and writes the admission records of a live start for the invite's
/// peers, so the verdicts do not cut the link.
/// Ignored: on a loaded machine the leave notice still sometimes does not reach the other node within the
/// wait (about one run in four at a load of 50 and more; the page of the leaving node reports the leave as
/// announced, the other node keeps listing it as a member), so the test is not yet a reliable check.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "under load the leave notice sometimes does not reach the other node within the wait"]
async fn leaving_an_added_slot_is_seen_by_its_network() {
    let a = boot_host("seen-a", &NetworkKey::generate().unwrap()).await;
    let b = boot("seen-b", &NetworkKey::generate().unwrap()).await;
    wait_ready(&b).await;
    let slot = add_slot(&a);
    let join = format!("/api/net/{slot}/network/join");
    let (status, body) = a.post(&join, json!({"key": invite_of(&b)}));
    assert_eq!(status, 202, "join in the slot: {body}");
    let state = format!("/api/net/{slot}/state");
    let linked = wait_until(Duration::from_secs(20), || other_online_peer(&a.get(&state).1)).await;
    assert!(linked, "the slot never connected to the other node: {}", a.get(&state).1["peers"]);

    let path = format!("/api/net/{slot}/members/leave");
    let (status, body) = a.post(&path, json!({"confirm": true}));
    assert_eq!(status, 200, "leave: {body}");

    let seen = wait_until(Duration::from_secs(10), || lists_a_member_that_left(&b)).await;
    assert!(seen, "the other network never saw the member leave: {body}");

    teardown(b, Duration::from_secs(5)).await;
    a.teardown().await;
}

/// Leaving the main network and entering the same network again by a new invite: the node comes back
/// under a new identity, and the network does not turn it away as a member that left.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn main_network_left_and_entered_again_gets_a_new_peer_id() {
    let key = NetworkKey::generate().unwrap();
    let a = boot_host("main-a", &key).await;
    let b = boot("main-b", &key).await;
    wait_ready(&b).await;
    let (_, before) = a.get("/api/members");
    let old_id = before["me"].as_str().expect("own id").to_string();

    let (status, body) = a.post("/api/members/leave", json!({"confirm": true}));
    assert_eq!(status, 200, "leave: {body}");
    let offline = wait_until(Duration::from_secs(10), || {
        a.get("/api/state").1["node"]["status"] == json!("offline")
    })
    .await;
    assert!(offline, "the node did not wait for a new key after leaving");

    let (status, body) = a.post("/api/network/join", json!({"key": invite_of(&b)}));
    assert_eq!(status, 202, "join after leaving: {body}");
    let ok = wait_until(Duration::from_secs(20), || {
        let (status, v) = a.get("/api/members");
        status == 200 && other_listed(&v)
    })
    .await;
    assert!(ok, "the node did not get back into the network");
    let (_, after) = a.get("/api/members");
    assert_ne!(after["me"], json!(old_id), "a new identity is expected");
    let (_, state) = a.get("/api/state");
    assert_ne!(state["node"]["invite_join"]["state"], json!("refused"));

    teardown(b, Duration::from_secs(5)).await;
    a.teardown().await;
}
