//! The wiring of `Inputs.cached_seen` through `collect()` is checked by
//! a real node on loopback: the merge rule in `peers[]` is the job of `build_cache_tests.rs`,
//! here only the wire "cache file on disk → node snapshot".
//! The recipe for the node and the cache file is like `core/tests/peers_cache_loopback.rs::b_finds_a_from_cache_without_dht`,
//! but through the `pub(crate)` API of this library (an integration test outside cannot see it).

use super::*;

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::transport::net::AddrHint;
use crate::transport::net::NodeSecret;

use crate::config::config_file::{IfaceFilter, ListenAccess};
use crate::members::network_key::NetworkKey;
use crate::members::peer_names::PeerNames;
use crate::node::output;
use crate::node::NodeConfig;
use crate::proxy::ProxyFace;
use crate::transport::peers_cache::{entry_for, PeersCache};

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
    let now = crate::transport::net_mainline::unix_now();

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

    let collected = collect(&node, None, &mut DocCache::default()).await;

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
    let now = crate::transport::net_mainline::unix_now();

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

    let collected = collect(&node, None, &mut DocCache::default()).await;

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
    use crate::ports_sharing::{Audience, Proto, Published};
    let key = NetworkKey::generate().unwrap();
    let mut cfg = NodeConfig::for_loopback_test(key);
    cfg.settings.ports_published = vec![Published {
        proto: Proto::Tcp,
        port: 8080,
        name: "web".into(),
        audience: Audience::All,
    }];
    let node = crate::node::start_node(cfg).await.unwrap();

    let collected = collect(&node, None, &mut DocCache::default()).await;

    assert_eq!(collected.snap.ports.published.len(), 1);
    assert_eq!(collected.snap.ports.published[0].port, 8080);
    assert_eq!(collected.snap.ports.published[0].name, "web");

    node.shutdown().await;
}

// assumption (08i): the node address is `invite::own_addr(&node.stack, node.id)`; `crate::node::node_ctl::peer_lines` is a `pub use` in `node_ctl.rs`;
// `NodeSnapshot.transport.self_addresses: Vec<String>` (`web/snapshot.rs`).

/// The node address in the snapshot: `collect` takes it from the transport, not from the `Endpoint`.
mod own_address {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn collect_self_addresses_from_transport() {
        // given: a running node whose transport already knows its own addresses
        let node = loopback_node_with_address().await;
        // when: the snapshot is built
        let collected = collect(&node, None, &mut DocCache::default()).await;
        // then: the node addresses in the snapshot are not empty and equal the transport address lines
        let self_addresses = &collected.snap.transport.self_addresses;
        assert_eq!(
            self_addresses,
            &crate::node::node_ctl::peer_lines(&crate::invite::own_addr(&node.stack, node.id)),
            "self_addresses should come from the node's transport address"
        );
        node.shutdown().await;
    }
}

/// The `socks` block of the snapshot is the published face of the process proxy, not the node's own listener.
mod socks_block {
    use super::*;

    mod face_with_an_address {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn shows_the_proxy_as_enabled_on_that_address() {
            // given: a node whose proxy face points at a loopback port
            let (node, _face) = node_with_face(face_on(41234)).await;
            // when: the snapshot is collected
            let socks = socks_of(&node).await;
            // then: the block shows that address
            assert!(
                socks.enabled,
                "a face with an address means the proxy is on"
            );
            assert_eq!(socks.listen.as_deref(), Some("127.0.0.1:41234"));
            node.shutdown().await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn gives_the_pac_url_on_the_face_port() {
            let (node, _face) = node_with_face(face_on(41234)).await;
            let socks = socks_of(&node).await;
            assert_eq!(
                socks.pac_url.as_deref(),
                Some("http://127.0.0.1:41234/proxy.pac")
            );
            node.shutdown().await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn localhost_access_has_no_lan_addresses() {
            let (node, _face) = node_with_face(face_on(41234)).await;
            let socks = socks_of(&node).await;
            assert_eq!(socks.access, "localhost");
            assert!(socks.lan.is_empty(), "localhost shows no LAN addresses");
            node.shutdown().await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn passes_access_and_ifaces_of_the_face_through() {
            // given: a face in local-network mode over all interfaces
            let face = ProxyFace {
                access: ListenAccess::Localnet,
                ifaces: IfaceFilter::Any,
                ..face_on(41234)
            };
            let (node, _face) = node_with_face(face).await;
            // when / then
            let socks = socks_of(&node).await;
            assert_eq!(socks.access, "localnet");
            assert_eq!(socks.ifaces, Some(vec!["*".to_string()]));
            node.shutdown().await;
        }
    }

    mod face_off {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn shows_the_proxy_as_disabled_with_no_address() {
            // given: a node whose proxy face is off
            let (node, _face) = node_with_face(ProxyFace::OFF).await;
            // when: the snapshot is collected
            let socks = socks_of(&node).await;
            // then: nothing is listening, nothing to show
            assert!(!socks.enabled);
            assert_eq!(socks.listen, None);
            assert_eq!(socks.pac_url, None);
            assert!(socks.lan.is_empty(), "an off proxy has no LAN addresses");
            node.shutdown().await;
        }
    }

    mod face_changes {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn next_snapshot_follows_the_new_face() {
            // given: a node whose face is on and a snapshot taken from it
            let (node, face) = node_with_face(face_on(41234)).await;
            assert!(socks_of(&node).await.enabled, "precondition: face is on");
            // when: the face is switched off and then to another port
            face.send_replace(ProxyFace::OFF);
            let off = socks_of(&node).await;
            face.send_replace(face_on(41999));
            let moved = socks_of(&node).await;
            // then: each snapshot shows the face of its moment
            assert!(!off.enabled, "after the face went off the block is off");
            assert_eq!(off.listen, None);
            assert_eq!(moved.listen.as_deref(), Some("127.0.0.1:41999"));
            assert_eq!(
                moved.pac_url.as_deref(),
                Some("http://127.0.0.1:41999/proxy.pac")
            );
            node.shutdown().await;
        }
    }
}

/// A node on loopback (the `mem` transport); the address is taken from the stack without waiting for an IP.
async fn loopback_node_with_address() -> crate::node::RunningNode {
    let key = NetworkKey::generate().unwrap();
    crate::node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap()
}

/// A proxy face on `127.0.0.1:port` with the default access and interface filter.
fn face_on(port: u16) -> ProxyFace {
    ProxyFace {
        addr: Some(SocketAddr::from(([127, 0, 0, 1], port))),
        access: ListenAccess::Localhost,
        ifaces: IfaceFilter::Auto,
    }
}

/// A loopback node that reads its proxy face from a channel; the sender stays with the test.
async fn node_with_face(
    face: ProxyFace,
) -> (
    crate::node::RunningNode,
    tokio::sync::watch::Sender<ProxyFace>,
) {
    let (tx, rx) = tokio::sync::watch::channel(face);
    let cfg = NodeConfig {
        proxy_face: Some(rx),
        ..NodeConfig::for_loopback_test(NetworkKey::generate().unwrap())
    };
    (crate::node::start_node(cfg).await.unwrap(), tx)
}

/// The `socks` block of a freshly collected snapshot.
async fn socks_of(node: &crate::node::RunningNode) -> crate::web_gui::SocksInfo {
    collect(node, None, &mut DocCache::default())
        .await
        .snap
        .socks
}
