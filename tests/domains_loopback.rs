//! Integration tests of the domain list on the `mem` transport: nodes in a shared in-process network, no DHT.
//!
//! What it does: checks adding, removing and reading on one node; that the list stays on
//! its own node (doesn't reach a neighbor, isn't changed by a removal on a neighbor, isn't caught up by a late
//! node, foreign `list/` records of the shared document aren't shown); that the shared document is
//! still reconciled (a label written into it arrives); that a node of another network doesn't get the shared document.
//! What it doesn't do: doesn't check the DHT or the stdin console.
//! Links: tests `meshtown::domains` through `meshtown::node`.

use std::time::Duration;

use bytes::Bytes;
use meshtown::domains::entry_key;
use meshtown::net::{live, PeerAddr};
use meshtown::network_key::NetworkKey;
use meshtown::node::{self, NodeConfig, RunningNode};
use tokio::time::{sleep, Instant};

fn names(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

async fn introduce(a: &RunningNode, b: &RunningNode) {
    a.introduce(PeerAddr::new(b.id)).await;
}

/// Polls the log every 50 ms until, in lines with the `auth` tag, both handshake refusal
/// markers appear — from the initiator (`a`) and from the responder (`b`) — or the limit expires.
async fn wait_rejected(a: &RunningNode, b: &RunningNode, limit: Duration) -> bool {
    let want_a = format!("handshake with {}: refused", meshtown::output::short_peer_id(&b.id));
    let want_b = format!("incoming from {}: refused", meshtown::output::short_peer_id(&a.id));
    let deadline = Instant::now() + limit;
    loop {
        let lines = meshtown::output::log_page(0, usize::MAX).lines;
        let seen = |want: &str| lines.iter().any(|l| l.tag == "auth" && l.text.starts_with(want));
        if seen(&want_a) && seen(&want_b) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_add_list_remove() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key)).await.unwrap();

    assert_eq!(a.domains.list().await.unwrap(), Vec::<String>::new());
    assert_eq!(a.domains.add("Example.COM").await.unwrap(), "example.com");
    a.domains.add("example.com.org").await.unwrap();
    assert_eq!(
        a.domains.list().await.unwrap(),
        names(&["example.com", "example.com.org"])
    );

    a.domains.remove("example.com").await.unwrap();
    assert_eq!(a.domains.list().await.unwrap(), names(&["example.com.org"]));

    a.domains.add("example.com").await.unwrap();
    assert_eq!(
        a.domains.list().await.unwrap(),
        names(&["example.com", "example.com.org"])
    );
    assert_eq!(a.domains.count().await.unwrap(), 2);

    assert!(a.domains.add("bad_domain.com").await.is_err());
    assert_eq!(a.domains.count().await.unwrap(), 2);
}

/// Waits until the nodes become members of each other (otherwise the shared document reconciliation won't start).
async fn wait_members(a: &RunningNode, b: &RunningNode) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !(a.members.is_member(b.id) && b.members.is_member(a.id)) {
        assert!(Instant::now() < deadline, "the nodes did not become members of each other within 10 s");
        sleep(Duration::from_millis(50)).await;
    }
}

/// Writes a label into the node's shared document: a sign that the shared document reconciliation went through.
async fn put_shared(node: &RunningNode, key: &[u8]) {
    node.domains
        .shared_replica()
        .set_own(key, Bytes::from_static(b"v1"))
        .await
        .unwrap();
}

/// Polls the node's shared document every 50 ms until an entry with the key `key` appears in it
/// or the limit expires. A positive sign: without it, "no domain" could mean "no reconciliation".
async fn wait_shared_key(node: &RunningNode, key: &[u8], limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        let found = node.domains.shared_replica().scan(key).await.unwrap();
        if live(found).iter().any(|e| e.key == key) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn domain_stays_on_own_node() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone())).await.unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key)).await.unwrap();
    introduce(&a, &b).await;
    wait_members(&a, &b).await;

    a.domains.add("example.com").await.unwrap();
    put_shared(&a, b"test/sync/").await;
    assert!(
        wait_shared_key(&b, b"test/sync/", Duration::from_secs(15)).await,
        "the shared document label from A did not reach B within 15 s"
    );
    assert_eq!(b.domains.list().await.unwrap(), Vec::<String>::new());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remove_on_other_peer_keeps_own() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone())).await.unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key)).await.unwrap();
    a.domains.add("example.com").await.unwrap();
    b.domains.add("example.com").await.unwrap();
    b.domains.remove("example.com").await.unwrap();
    put_shared(&b, b"test/remove/").await;
    introduce(&a, &b).await;

    assert!(
        wait_shared_key(&a, b"test/remove/", Duration::from_secs(15)).await,
        "the shared document label from B did not reach A within 15 s"
    );
    assert_eq!(a.domains.list().await.unwrap(), names(&["example.com"]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn late_joiner_gets_shared_not_domains() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone())).await.unwrap();
    a.domains.add("one.example").await.unwrap();
    put_shared(&a, b"test/late/").await;

    let c = node::start_node(NodeConfig::for_loopback_test(key)).await.unwrap();
    introduce(&a, &c).await;
    assert!(
        wait_shared_key(&c, b"test/late/", Duration::from_secs(15)).await,
        "the shared document label did not reach the late node within 15 s"
    );
    assert_eq!(c.domains.list().await.unwrap(), Vec::<String>::new());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn old_node_domain_not_shown() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone())).await.unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key)).await.unwrap();
    introduce(&a, &b).await;
    wait_members(&a, &b).await;

    // Like an old node: the domain is written to `list/` of the shared document.
    put_shared(&a, &entry_key("old.example")).await;
    assert!(
        wait_shared_key(&b, &entry_key("old.example"), Duration::from_secs(15)).await,
        "the `list/` record from A did not reach B within 15 s"
    );
    assert_eq!(b.domains.list().await.unwrap(), Vec::<String>::new());
    let shown = meshtown::output::log_page(0, usize::MAX)
        .lines
        .iter()
        .any(|l| l.tag == "docs" && l.text.contains("old.example"));
    assert!(!shown, "a foreign domain from the shared document ended up in the `docs` log");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn own_namespace_is_private() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone())).await.unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key)).await.unwrap();
    // `mesh` has its own list replica — the fixed id `domains` (shared by all nodes, but it is not
    // handed out to the network: `domain_stays_on_own_node`); it differs from the network's shared replica
    assert_eq!(a.domains.own_replica_id(), b.domains.own_replica_id());
    assert_ne!(a.domains.own_replica_id(), a.domains.shared_replica_id());
    assert_ne!(b.domains.own_replica_id(), b.domains.shared_replica_id());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn other_network_does_not_get_domains() {
    let a = node::start_node(NodeConfig::for_loopback_test(NetworkKey::generate().unwrap()))
        .await
        .unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(NetworkKey::generate().unwrap()))
        .await
        .unwrap();
    a.domains.add("example.com").await.unwrap();
    assert_ne!(a.domains.shared_replica_id(), b.domains.shared_replica_id());
    introduce(&a, &b).await;

    let ok = wait_rejected(&a, &b, Duration::from_secs(10)).await;
    assert!(ok, "the handshake refusal was not written to the log within 10 s");
    assert_eq!(b.domains.list().await.unwrap(), Vec::<String>::new());
}
