//! Integration tests on the `mem` transport: two nodes in a shared in-process network, no DHT, direct introduction.
//!
//! What it does: checks that nodes with the same network key become members of each other,
//! with different keys they do not, that a `mem` network node does not go to the DHT, that after the handshake a gossip peer appears and the probe passes
//! (the `[probe]` log line carries the path of the chosen connection), that after the
//! handshake each one knows the peer's name.
//! What it does not do: does not check the DHT or real sockets — only the node logic on `mem`.
//! Links: checks `meshtown_core::node` as a whole (and through it almost all other modules).

use std::sync::atomic::Ordering;
use std::time::Duration;

use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, NodeConfig};
use meshtown_core::transport::net::PeerAddr;
use tokio::time::{sleep, Instant};

/// Polls the condition every 50 ms until it becomes true or the limit expires.
async fn wait_until(limit: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return cond();
        }
        sleep(Duration::from_millis(50)).await;
    }
}

/// Polls the log every 50 ms until the lines tagged `auth` contain both refusal marks
/// of the handshake — from the initiator (`a`) and from the responder (`b`) — or the limit expires.
async fn wait_rejected(a: &node::RunningNode, b: &node::RunningNode, limit: Duration) -> bool {
    let want_a = format!(
        "handshake with {}: refused",
        meshtown_core::node::output::short_peer_id(&b.id)
    );
    let want_b = format!(
        "incoming from {}: refused",
        meshtown_core::node::output::short_peer_id(&a.id)
    );
    let deadline = Instant::now() + limit;
    loop {
        let lines = meshtown_core::node::output::log_page(0, usize::MAX).lines;
        let seen = |want: &str| {
            lines
                .iter()
                .any(|l| l.tag == "auth" && l.text.starts_with(want))
        };
        if seen(&want_a) && seen(&want_b) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        sleep(Duration::from_millis(50)).await;
    }
}

/// A peer's address in the `mem` network: the key is everything, `KeyedMem` appends the slot.
async fn introduce(a: &node::RunningNode, b: &node::RunningNode) {
    a.introduce(PeerAddr::new(b.id)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn node_runs_on_mem() {
    let key = NetworkKey::generate().unwrap();
    let running = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    let snap = running.stack.snapshot();
    assert_eq!(snap.transport_names, vec!["mem"]);
    // the `invite`/`cache` names from the config become sources only on an invite or a cache;
    // a fresh node has no sources, and certainly no `hyperdht`: it does not go to the DHT
    assert!(
        snap.discovery_names.is_empty(),
        "discovery sources: {:?}",
        snap.discovery_names
    );
    running.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn same_key_peers_become_members() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();

    introduce(&a, &b).await;

    let ok = wait_until(Duration::from_secs(10), || {
        a.members.is_member(b.id) && b.members.is_member(a.id)
    })
    .await;
    assert!(
        ok,
        "the nodes did not become members of each other within 10 s"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wrong_key_peers_are_rejected() {
    let key_a = NetworkKey::generate().unwrap();
    let key_b = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key_a))
        .await
        .unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key_b))
        .await
        .unwrap();

    introduce(&a, &b).await;

    let ok = wait_rejected(&a, &b, Duration::from_secs(10)).await;
    assert!(ok, "the handshake refusal was not logged within 10 s");
    assert!(!a.members.is_member(b.id));
    assert!(!b.members.is_member(a.id));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gossip_neighbors_after_handshake() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();

    introduce(&a, &b).await;

    let ok = wait_until(Duration::from_secs(15), || {
        a.gossip_neighbor_count.load(Ordering::Relaxed) >= 1
            && b.gossip_neighbor_count.load(Ordering::Relaxed) >= 1
    })
    .await;
    assert!(ok, "gossip peers did not appear within 15 s");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transfer_probe_over_loopback() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();

    introduce(&a, &b).await;

    let ok = wait_until(Duration::from_secs(20), || {
        a.ok_probe_count.load(Ordering::Relaxed) >= 1
            && b.ok_probe_count.load(Ordering::Relaxed) >= 1
    })
    .await;
    assert!(ok, "the volume probe did not pass within 20 s");

    // The counter grows before the `[probe]` line is printed to the log — wait separately.
    let logged = wait_until(Duration::from_secs(2), || {
        meshtown_core::node::output::log_page(0, usize::MAX)
            .lines
            .iter()
            .any(|l| {
                l.tag == "probe"
                    && l.text.starts_with("-> ")
                    && l.text.contains("acknowledged in")
                    && l.text.contains("path: ")
            })
    })
    .await;
    assert!(
        logged,
        "a `[probe]` line with an acknowledgment and a path did not appear in the log within 2 s"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn peers_learn_each_others_names() {
    let key = NetworkKey::generate().unwrap();
    let mut cfg_a = NodeConfig::for_loopback_test(key.clone());
    cfg_a.settings.node_name = Some("厨房🍳".to_string());
    let mut cfg_b = NodeConfig::for_loopback_test(key);
    cfg_b.settings.node_name = Some("Bedroom".to_string());
    let a = node::start_node(cfg_a).await.unwrap();
    let b = node::start_node(cfg_b).await.unwrap();

    introduce(&a, &b).await;

    let ok = wait_until(Duration::from_secs(10), || {
        a.members.is_member(b.id) && b.members.is_member(a.id)
    })
    .await;
    assert!(
        ok,
        "the nodes did not become members of each other within 10 s"
    );

    assert_eq!(a.members.name_of(b.id).as_deref(), Some("Bedroom"));
    assert_eq!(b.members.name_of(a.id).as_deref(), Some("厨房🍳"));
    assert_eq!(a.name_at_start, "厨房🍳");
    assert_eq!(b.name_at_start, "Bedroom");

    a.shutdown().await;
    b.shutdown().await;
}
