//! Integration tests of the exit flag on other nodes (`node_state`).
//!
//! What it does: checks truly live iroh-docs synchronization between two nodes on
//! loopback — the flag switch reaches the neighbor, a node disabled at startup announces
//! `false` (not absence), and the key itself is invisible to old readers (`list/`, `chk/`).
//! What it doesn't do: doesn't check the HTTP page or the node snapshot — that is `tests/node_web.rs` and
//! `node_ctl::snapshot::exit_tests`.
//! Links: `meshtown::node_state`, `meshtown::node::RunningNode`.

use std::collections::HashMap;
use std::time::Duration;

use meshtown::net::{PeerAddr, PeerId};
use meshtown::network_key::NetworkKey;
use meshtown::node::{self, NodeConfig, RunningNode};
use meshtown::node_state;
use tokio::time::{sleep, Instant};

/// A copy of the helper from `core/tests/checks_loopback.rs`: introduces the address of `b` to `a` over loopback.
async fn introduce(a: &RunningNode, b: &RunningNode) {
    a.introduce(PeerAddr::new(b.id)).await;
}

/// Polls every 200 ms up to `limit`: reads the exit flags of `node` and returns the last map
/// as soon as `want` returns true or the deadline passes (the same technique as `wait_views` in
/// `core/tests/checks_loopback.rs`).
async fn wait_exits(
    node: &RunningNode,
    limit: Duration,
    want: impl Fn(&HashMap<PeerId, bool>) -> bool,
) -> HashMap<PeerId, bool> {
    let deadline = Instant::now() + limit;
    loop {
        let exits = node_state::read_exit_flags(&*node.domains.shared_replica()).await.unwrap();
        if want(&exits) || Instant::now() >= deadline {
            return exits;
        }
        sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exit_flag_follows_toggle() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    introduce(&a, &b).await;

    let exits = wait_exits(&b, Duration::from_secs(20), |e| e.get(&a.id) == Some(&true)).await;
    assert_eq!(
        exits.get(&a.id),
        Some(&true),
        "A's exit is on by default — B must see true"
    );

    a.set_exit_enabled(false);
    let exits = wait_exits(&b, Duration::from_secs(20), |e| {
        e.get(&a.id) == Some(&false)
    })
    .await;
    assert_eq!(
        exits.get(&a.id),
        Some(&false),
        "turning off A's exit must reach B as false"
    );

    a.set_exit_enabled(true);
    let exits = wait_exits(&b, Duration::from_secs(20), |e| e.get(&a.id) == Some(&true)).await;
    assert_eq!(
        exits.get(&a.id),
        Some(&true),
        "turning A's exit back on must again reach B"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exit_flag_off_at_start_reads_false() {
    let key = NetworkKey::generate().unwrap();
    let mut cfg_a = NodeConfig::for_loopback_test(key.clone());
    cfg_a.settings.exit_enabled = false;
    let a = node::start_node(cfg_a).await.unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    introduce(&a, &b).await;

    let exits = wait_exits(&b, Duration::from_secs(20), |e| {
        e.get(&a.id) == Some(&false)
    })
    .await;
    assert_eq!(
        exits.get(&a.id),
        Some(&false),
        "A started with the exit off — B must see exactly false, not absence"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exit_key_invisible_to_old_readers() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    introduce(&a, &b).await;

    wait_exits(&b, Duration::from_secs(20), |e| e.get(&a.id) == Some(&true)).await;

    assert!(
        b.domains.list().await.unwrap().is_empty(),
        "the exit flag must not fall under reading the domain list"
    );
    assert!(
        b.checks.check_views().await.unwrap().is_empty(),
        "the exit flag must not fall under reading the checks"
    );

    for prefix in [b"list/".as_slice(), b"chk/".as_slice()] {
        let raw = b.domains.shared_replica().scan(prefix).await.unwrap();
        assert!(
            raw.is_empty(),
            "a raw request by the prefix {:?} must not fall under reading the exit flag",
            prefix,
        );
    }
}
