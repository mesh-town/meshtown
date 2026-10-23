//! A slot's neighbour cache, written beside the slot file, lets a node find its neighbour with no invite,
//! no DHT and no relay.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, NodeConfig};
use meshtown_core::transport::net::{AddrHint, PeerAddr};
use meshtown_core::transport::net_mainline;
use meshtown_core::transport::peers_cache::{self, peers_cache_path_beside, PeersCache};

async fn wait_until(limit: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if cond() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    cond()
}

fn slot_file() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir()
        .join(format!("mesh-slot-seed-{}-{nanos}", std::process::id()))
        .join("nets")
        .join("slot1");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("config.json")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_slot_finds_its_neighbour_from_the_cache_beside_its_file() {
    let key = NetworkKey::from_bytes([42; 32]);
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    let now = net_mainline::unix_now();
    let file = slot_file();
    let cache_path = peers_cache_path_beside(&file);
    let addr_a = PeerAddr {
        id: a.id,
        hints: vec![AddrHint::Ip(SocketAddr::from(([127, 0, 0, 1], 9)))],
    };
    let mut cache = PeersCache::load(cache_path.as_deref(), &key.subkeys().network_id_hex(), now);
    let entry = peers_cache::entry_for(Some(&addr_a), a.id, None, now);
    assert!(cache.remember(entry, now), "the cache must take the entry");
    cache.save().expect("save");
    let cfg_b = NodeConfig {
        initial_peers: vec![],
        peers_cache_path: cache_path,
        ..NodeConfig::for_loopback_test(key.clone())
    };
    let b = node::start_node(cfg_b).await.unwrap();
    let ok = wait_until(Duration::from_secs(20), || {
        a.members.is_member(b.id) && b.members.is_member(a.id)
    })
    .await;
    assert!(ok, "the nodes must meet within 20 s from the cache alone");
    a.shutdown().await;
    b.shutdown().await;
}
