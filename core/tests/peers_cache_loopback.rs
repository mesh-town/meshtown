//! The members' address cache on disk: written when a connection comes up, read at node start.
//!
//! What it does: checks the wiring of the peers cache on two `mem` nodes without DHT — the cache write on
//! `MemberEvent::Up` (`dispatch_member_event`/`remember_peer_and_save`) and the cache read in `spawn_network`
//! and its serving by the `cache` source (`PeersCacheSource`) through the stack's `DiscoveryMux`.
//! What it doesn't do: it doesn't check the file format and the `PeersCache` bounds (ceilings, TTL,
//! `entry_for`/`touch`) — that is `peers_cache_tests.rs`; it doesn't touch DHT or a real network.
//! Links: `meshtown_core::transport::peers_cache::PeersCache`, `node::start_node`, `NodeConfig::peers_cache_path`.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, NodeConfig};
use meshtown_core::transport::net::{AddrHint, PeerAddr};
use meshtown_core::transport::peers_cache::{self, PeersCache};
use tokio::time::sleep;

#[path = "common/wait.rs"]
mod wait;
use wait::wait_until;

/// The path to the cache file in the test's temporary directory, its own for each call (by process name,
/// time and suffix, so that cases don't interfere with each other).
fn scratch_cache_path(suffix: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-peers-cache-loopback-{}-{nanos}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("test temporary directory");
    dir.join("peers-cache.json")
}

/// On `mem` the path has no IP hint (`peer_path_tests::record_peer_up_on_mem_gives_no_hint`), and
/// the cache rejects a record without an address: after a connection comes up, B's cache file is empty. The write from
/// a path hint is checked by the `event_loop`/`peers_cache` unit tests, not by this file.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn b_has_nothing_to_remember_about_a_on_mem() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();

    let cache_path_b = scratch_cache_path("b");
    let cfg_b = NodeConfig {
        initial_peers: vec![PeerAddr::new(a.id)],
        peers_cache_path: Some(cache_path_b.clone()),
        ..NodeConfig::for_loopback_test(key.clone())
    };
    let b = node::start_node(cfg_b).await.unwrap();

    let ok = wait_until(Duration::from_secs(20), || {
        a.members.is_member(b.id) && b.members.is_member(a.id)
    })
    .await;
    assert!(
        ok,
        "the nodes did not become members of each other within 20 s"
    );

    // control "the phenomenon was in the window": the member events are up, now let them reach the cache
    sleep(Duration::from_secs(2)).await;
    let network_id = key.subkeys().network_id_hex();
    let cache = PeersCache::load(
        Some(&cache_path_b),
        &network_id,
        meshtown_core::transport::net_mainline::unix_now(),
    );
    assert!(
        cache.addrs().is_empty(),
        "a `mem` path has no address, so the cache must be empty: {:?}",
        cache.addrs()
    );

    a.shutdown().await;
    b.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn b_finds_a_from_cache_without_dht() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();

    // The cache accepts only records with an address: the IP hint is fictitious, on `mem` a peer is found
    // by key, the hint is needed only so that the record gets into the file.
    let addr_a = PeerAddr {
        id: a.id,
        hints: vec![AddrHint::Ip(SocketAddr::from(([127, 0, 0, 1], 9)))],
    };

    let network_id = key.subkeys().network_id_hex();
    let now = meshtown_core::transport::net_mainline::unix_now();
    let cache_path_b2 = scratch_cache_path("b2");
    let mut cache = PeersCache::load(Some(&cache_path_b2), &network_id, now);
    let entry = peers_cache::entry_for(Some(&addr_a), a.id, None, now);
    assert!(
        cache.remember(entry, now),
        "A's record with an address must be accepted by the cache"
    );
    cache.save().expect("the cache file must be written");

    let cfg_b2 = NodeConfig {
        initial_peers: vec![],
        peers_cache_path: Some(cache_path_b2),
        ..NodeConfig::for_loopback_test(key.clone())
    };
    let b2 = node::start_node(cfg_b2).await.unwrap();

    let ok = wait_until(Duration::from_secs(20), || {
        a.members.is_member(b2.id) && b2.members.is_member(a.id)
    })
    .await;
    assert!(
        ok,
        "node B2 did not find A by the cache within 20 s without DHT, relay and invite"
    );

    a.shutdown().await;
    b2.shutdown().await;
}
