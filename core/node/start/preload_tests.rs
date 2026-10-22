use super::*;
use crate::members::network_key::NetworkKey;
use crate::node::start::boot::boot_endpoint;
use crate::transport::net::PeerAddr;

fn names(sources: &[Arc<dyn Discovery>]) -> Vec<&'static str> {
    sources.iter().map(|d| d.name()).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_cache_no_peers_gives_no_sources() {
    // given: a node without an address cache and without initial peers
    let cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    let boot = boot_endpoint(&cfg).await.unwrap();
    // when: reading the sources
    let (cache, sources) = load_sources(&cfg, &boot);
    // then: the cache is empty, there are no sources
    assert!(cache.is_empty());
    assert!(names(&sources).is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn initial_peer_gives_invite_source() {
    // given: one initial peer
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    let peer = crate::transport::net::PeerId::from_bytes(&[9; 32]).unwrap();
    cfg.initial_peers = vec![PeerAddr::new(peer)];
    let boot = boot_endpoint(&cfg).await.unwrap();
    // when: reading the sources
    let (_, sources) = load_sources(&cfg, &boot);
    // then: the only source is invite
    assert_eq!(names(&sources), vec!["invite"]);
}
