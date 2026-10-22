//! The wiring of `Inputs.cached_seen` through `collect()` is checked by
//! a real node on loopback: the merge rule in `peers[]` is the job of `build_cache_tests.rs`,
//! here only the wire "cache file on disk → node snapshot".
//! The recipe for the node and the cache file is like `tests/peers_cache_loopback.rs::b_finds_a_from_cache_without_dht`,
//! but through the `pub(crate)` API of this library (an integration test outside cannot see it).

use super::*;

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::net::NodeSecret;
use crate::net::AddrHint;

use crate::network_key::NetworkKey;
use crate::node::NodeConfig;
use crate::output;
use crate::peer_names::PeerNames;
use crate::peers_cache::{entry_for, PeersCache};

/// Path to a cache file in the test's temporary directory, unique for every call.
fn scratch_cache_path(suffix: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-collect-cache-{}-{nanos}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temporary test directory");
    dir.join("peers-cache.json")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collect_shows_cached_peer_offline_after_restart() {
    let key = NetworkKey::generate().unwrap();
    let network_id = key.subkeys().network_id_hex();
    let now = crate::net_mainline::unix_now();

    // B never connected to this node — only a record in the cache file with an address nobody
    // listens on.
    let b_id = NodeSecret::from_bytes(&[7; 32]).peer_id();
    let addr: SocketAddr = "127.0.0.1:9".parse().unwrap();
    let cache_path = scratch_cache_path("restart");
    let mut cache = PeersCache::load(Some(&cache_path), &network_id, now);
    let entry = entry_for(None, b_id, Some(&AddrHint::Ip(addr)), now);
    assert!(
        cache.remember(entry, now),
        "B's record with an address should be accepted by the cache"
    );
    cache.save().expect("the cache file should be written");

    let cfg = NodeConfig {
        peers_cache_path: Some(cache_path),
        ..NodeConfig::for_loopback_test(key)
    };
    let node = crate::node::start_node(cfg).await.unwrap();

    assert!(
        !node.members.is_member(b_id),
        "precondition: B never connected — not a live member"
    );
    assert!(
        !node.telemetry.view().peers.contains_key(&b_id),
        "precondition: B never connected — telemetry does not know it"
    );

    let collected = collect(&node, None, "direct_and_relay", &mut DocCache::default()).await;

    let b_peer = collected
        .snap
        .peers
        .iter()
        .find(|p| p.id == output::short_peer_id(&b_id))
        .expect("B should appear in peers from the on-disk address cache");
    assert!(!b_peer.online, "B is known only from the cache — offline");
    assert_eq!(
        collected.snap.members.count, 0,
        "B from the cache must not count as a live member"
    );

    node.shutdown().await;
}

/// Path to a names file in the test's temporary directory, unique for every call.
fn scratch_names_path(suffix: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-collect-names-{}-{nanos}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temporary test directory");
    dir.join("peer-names.json")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collect_takes_cached_peer_name_from_peer_names_file() {
    let key = NetworkKey::generate().unwrap();
    let network_id = key.subkeys().network_id_hex();
    let now = crate::net_mainline::unix_now();

    // B never connected — known only by an address from the cache (nobody listens) and by the
    // saved name from the names file.
    let b_id = NodeSecret::from_bytes(&[7; 32]).peer_id();
    let addr: SocketAddr = "127.0.0.1:9".parse().unwrap();

    let cache_path = scratch_cache_path("with-names");
    let mut cache = PeersCache::load(Some(&cache_path), &network_id, now);
    let entry = entry_for(None, b_id, Some(&AddrHint::Ip(addr)), now);
    assert!(
        cache.remember(entry, now),
        "B's record with an address should be accepted by the cache"
    );
    cache.save().expect("the cache file should be written");

    let names_path = scratch_names_path("with-names");
    let mut names = PeerNames::load(Some(&names_path), &network_id);
    assert!(
        names.remember(b_id, "Bedroom"),
        "the new name should be accepted"
    );
    names.save().expect("the names file should be written");

    let cfg = NodeConfig {
        peers_cache_path: Some(cache_path),
        peer_names_path: Some(names_path),
        ..NodeConfig::for_loopback_test(key)
    };
    let node = crate::node::start_node(cfg).await.unwrap();

    let collected = collect(&node, None, "direct_and_relay", &mut DocCache::default()).await;

    let b_peer = collected
        .snap
        .peers
        .iter()
        .find(|p| p.id == output::short_peer_id(&b_id))
        .expect("B should appear in peers from the on-disk address cache");
    assert!(
        !b_peer.online,
        "B is known only from the cache and the names file — offline"
    );
    assert_eq!(
        b_peer.name,
        Some("Bedroom".to_string()),
        "the name should come from peer-names.json"
    );

    node.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collect_shows_own_published_port() {
    use crate::ports::{Audience, Published};
    let key = NetworkKey::generate().unwrap();
    let mut cfg = NodeConfig::for_loopback_test(key);
    cfg.settings.ports_published = vec![Published {
        port: 8080,
        name: "web".into(),
        audience: Audience::All,
    }];
    let node = crate::node::start_node(cfg).await.unwrap();

    let collected = collect(&node, None, "direct_and_relay", &mut DocCache::default()).await;

    assert_eq!(collected.snap.ports.published.len(), 1);
    assert_eq!(collected.snap.ports.published[0].port, 8080);
    assert_eq!(collected.snap.ports.published[0].name, "web");

    node.shutdown().await;
}

// assumption (08i): the node address is `invite::own_addr(&node.stack, node.id)`; `crate::node_ctl::peer_lines` is a `pub use` in `node_ctl.rs`;
// `NodeSnapshot.transport.self_addresses: Vec<String>` (`web/snapshot.rs`).

/// The node address in the snapshot: `collect` takes it from the transport, not from the `Endpoint`.
mod own_address {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn collect_self_addresses_from_transport() {
        // given: a running node whose transport already knows its own addresses
        let node = loopback_node_with_address().await;
        // when: the snapshot is built
        let collected = collect(&node, None, "direct_and_relay", &mut DocCache::default()).await;
        // then: the node addresses in the snapshot are not empty and equal the transport address lines
        let self_addresses = &collected.snap.transport.self_addresses;
        assert_eq!(
            self_addresses,
            &crate::node_ctl::peer_lines(&crate::invite::own_addr(&node.stack, node.id)),
            "self_addresses should come from the node's transport address"
        );
        node.shutdown().await;
    }
}

/// A node on loopback (the `mem` transport); the address is taken from the stack without waiting for an IP.
async fn loopback_node_with_address() -> crate::node::RunningNode {
    let key = NetworkKey::generate().unwrap();
    crate::node::start_node(NodeConfig::for_loopback_test(key)).await.unwrap()
}
