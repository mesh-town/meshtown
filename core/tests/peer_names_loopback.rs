//! The file of the last known member names on disk: a saved name is visible before the first
//! handshake, a fresh one from the handshake wins.
//!
//! What it does: checks the wiring of the peer names file on two `mem` nodes without DHT — reading the names file and
//! `apply_to` before the router starts (`spawn_network`), the write on `MemberEvent::Up`
//! (`remember_member_and_save`).
//! What it doesn't do: it doesn't check the file format and the `PeerNames` bounds (ceilings, record selection) —
//! that is `peer_names_tests.rs`; it doesn't touch DHT or a real network.
//! Links: `meshtown_core::members::peer_names::PeerNames`, `node::start_node`, `NodeConfig::peer_names_path`.

use std::path::PathBuf;
use std::time::Duration;

use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::members::peer_names::PeerNames;
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

/// The path to the names file in the test's temporary directory, its own for each call.
fn scratch_names_path(suffix: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-peer-names-loopback-{}-{nanos}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("test temporary directory");
    dir.join("peer-names.json")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saved_name_shows_before_handshake_and_fresh_name_wins() {
    let key = NetworkKey::generate().unwrap();
    let mut cfg_a = NodeConfig::for_loopback_test(key.clone());
    cfg_a.settings.node_name = Some("厨房🍳".to_string());
    let a = node::start_node(cfg_a).await.unwrap();

    // B's names file already holds a stale name of A.
    let network_id = key.subkeys().network_id_hex();
    let names_path_b = scratch_names_path("b");
    let mut names = PeerNames::load(Some(&names_path_b), &network_id);
    assert!(
        names.remember(a.id, "Old name"),
        "A's name must be accepted by the store before B starts"
    );
    names
        .save()
        .expect("B's names file must be written before the node starts");

    let cfg_b = NodeConfig {
        peer_names_path: Some(names_path_b.clone()),
        ..NodeConfig::for_loopback_test(key)
    };
    let b = node::start_node(cfg_b).await.unwrap();

    // Before the handshake the saved name is visible, and A is not yet a member.
    assert_eq!(
        b.members.name_of(a.id).as_deref(),
        Some("Old name"),
        "the saved name must be visible before the handshake"
    );
    assert!(
        !b.members.is_member(a.id),
        "before introduce A must not be a member of B"
    );

    b.introduce(PeerAddr::new(a.id)).await;

    let became_member = wait_until(Duration::from_secs(10), || b.members.is_member(a.id)).await;
    assert!(became_member, "B did not become a member of A within 10 s");

    assert_eq!(
        b.members.name_of(a.id).as_deref(),
        Some("厨房🍳"),
        "the fresh name from the handshake must win over the saved one"
    );

    let wrote_fresh = wait_until(Duration::from_secs(10), || {
        PeerNames::load(Some(&names_path_b), &network_id).get(a.id) == Some("厨房🍳")
    })
    .await;
    assert!(
        wrote_fresh,
        "B's names file must be updated with the fresh name within 10 s"
    );

    a.shutdown().await;
    b.shutdown().await;
}
