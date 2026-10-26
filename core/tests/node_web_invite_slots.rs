//! Joining by an invite without naming a slot, end to end: node A runs under the process host, node B
//! (another network, or the same one) hands out invites over the in-process test network.
//! What it checks: the slot an invite goes to (a new one, the primary one, the one it came from) and the
//! answer that names it. The redemption itself may not finish in time; only the answers and the slot list
//! are read.

#[path = "common/node_web.rs"]
mod common;
#[path = "common/nets_host.rs"]
mod nets_host;

use std::time::Duration;

use meshtown_core::members::network_key::NetworkKey;
use serde_json::json;

use common::{api, boot_consulting_list, teardown, wait_ready, Booted};
use nets_host::{boot_host, boot_host_empty, HostNode};

const JOIN: &str = "/api/network/join";

/// A fresh invite of the network node `b` is in.
fn invite_of(b: &Booted) -> String {
    let (status, body) = api(b.port, &b.token, "GET", "/api/network/invite", None);
    assert_eq!(status, 200, "the invite must be built");
    body["invite"].as_str().expect("invite string").to_string()
}

fn net_ids(a: &HostNode) -> Vec<String> {
    let (_, v) = a.get("/api/nets");
    let nets = v["nets"].as_array().expect("nets array");
    nets.iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_invite_to_a_new_network_takes_a_new_slot_without_naming_it() {
    let a = boot_host("slots-new-a", &NetworkKey::generate().unwrap()).await;
    let b = boot_consulting_list("slots-new-b", &NetworkKey::generate().unwrap()).await;
    wait_ready(&b).await;
    let before = net_ids(&a).len();

    let (status, body) = a.post(JOIN, json!({"key": invite_of(&b)}));

    assert_eq!(status, 202, "{body}");
    let slot = body["slot"].as_str().expect("the answer names the slot");
    assert_ne!(slot, "main", "{body}");
    assert_eq!(net_ids(&a).len(), before + 1, "one more slot");
    assert!(net_ids(&a).contains(&slot.to_string()));
    teardown(b, Duration::from_secs(5)).await;
    a.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_invite_to_a_network_already_here_names_its_slot() {
    let key = NetworkKey::generate().unwrap();
    let a = boot_host("slots-here-a", &key).await;
    let b = boot_consulting_list("slots-here-b", &key).await;
    wait_ready(&b).await;

    let (status, body) = a.post(JOIN, json!({"key": invite_of(&b)}));

    assert_eq!(status, 409, "{body}");
    assert_eq!(body["error"], json!("net_already_added"));
    assert_eq!(body["slot"], json!("main"));
    assert_eq!(net_ids(&a), vec!["main".to_string()]);
    teardown(b, Duration::from_secs(5)).await;
    a.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_invite_to_a_left_slot_takes_another_new_slot() {
    let a = boot_host("slots-left-a", &NetworkKey::generate().unwrap()).await;
    let b = boot_consulting_list("slots-left-b", &NetworkKey::generate().unwrap()).await;
    wait_ready(&b).await;
    let (status, body) = a.post(JOIN, json!({"key": invite_of(&b)}));
    assert_eq!(status, 202, "{body}");
    let first = body["slot"].as_str().unwrap().to_string();
    // the slot is given up whether or not the redemption has finished
    let leave = format!("/api/net/{first}/members/leave");
    let (status, body) = a.post(&leave, json!({"confirm": true}));
    assert_eq!(status, 200, "leave: {body}");
    assert!(!net_ids(&a).contains(&first), "the slot is gone");

    let (status, body) = a.post(JOIN, json!({"key": invite_of(&b)}));

    assert_eq!(status, 202, "{body}");
    let second = body["slot"].as_str().expect("the answer names the slot");
    assert_ne!(second, first, "{body}");
    teardown(b, Duration::from_secs(5)).await;
    a.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_node_without_a_network_takes_the_invite_into_main() {
    let a = boot_host_empty("slots-empty-a").await;
    let b = boot_consulting_list("slots-empty-b", &NetworkKey::generate().unwrap()).await;
    wait_ready(&b).await;

    let (status, body) = a.post(JOIN, json!({"key": invite_of(&b)}));

    assert_eq!(status, 202, "{body}");
    assert_eq!(body["slot"], json!("main"), "{body}");
    assert_eq!(net_ids(&a), vec!["main".to_string()]);
    teardown(b, Duration::from_secs(5)).await;
    a.teardown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_word_key_of_another_network_is_still_refused_on_main() {
    let a = boot_host("slots-key-a", &NetworkKey::generate().unwrap()).await;
    let other = common::key_text(&NetworkKey::generate().unwrap());

    let (status, body) = a.post(JOIN, json!({"key": other}));

    assert_eq!(status, 409, "{body}");
    assert_eq!(body["error"], json!("network_present"));
    a.teardown().await;
}
