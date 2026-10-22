//! The domain list survives a node restart: the `mesh` network replica lies as a file on disk.
//!
//! What it does: through a full node on `mem`, checks that a restart with the same sync directory
//! (`NodeConfig::docs_store_dir`) keeps the domain list and removals, that the replica file is
//! `replica/domains.bin` next to `docs-store/`, and that the former `docs/` and `blobs/` (the iroh-docs
//! store) are absent from the directory.
//! What it doesn't do: doesn't check synchronization between nodes (`domains_loopback.rs`) or the format of
//! the replica file (`sync/replica/file_tests.rs`); the "directory occupied by a file" and "directory occupied
//! by a second node" branches existed only in the iroh-docs adapter, the replica has no analogue (a broken file is the test
//! `domains_broken_starts_empty` in `replica_tests`).
//! Links: `meshtown_core::node::NodeConfig::docs_store_dir`, `meshtown_core::node`.

use std::path::{Path, PathBuf};

use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, NodeConfig, RunningNode};

/// The store root in the test's temporary directory, its own for every call (by process name,
/// time and suffix, so cases don't interfere with each other); it doesn't create the `docs-store` directory itself —
/// only its parent is taken: `replica/` will lie next to it.
fn scratch_store_dir(suffix: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "mesh-domains-persist-{}-{nanos}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("test temporary directory");
    root.join("docs-store")
}

fn persist_cfg(key: &NetworkKey, dir: &Path) -> NodeConfig {
    NodeConfig {
        docs_store_dir: Some(dir.to_path_buf()),
        ..NodeConfig::for_loopback_test(key.clone())
    }
}

/// Stops the node and starts a new one with the same config on the same directory. A failure here is a node defect,
/// the same one that would break a network change.
async fn restart(cfg: &NodeConfig, node: RunningNode) -> RunningNode {
    node.shutdown().await;
    node::start_node(cfg.clone()).await.unwrap()
}

fn names(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn domains_survive_restart_without_peers() {
    let key = NetworkKey::generate().unwrap();
    let dir = scratch_store_dir("restart");
    let cfg = persist_cfg(&key, &dir);

    let node = node::start_node(cfg.clone()).await.unwrap();
    node.domains.add("example.com").await.unwrap();
    node.domains.add("example.org").await.unwrap();

    let node = restart(&cfg, node).await;

    assert_eq!(
        node.domains.list().await.unwrap(),
        names(&["example.com", "example.org"]),
        "the domain list must survive a node restart with the same store directory"
    );
    let root = dir.parent().expect("test root");
    assert!(
        root.join("replica").join("domains.bin").is_file(),
        "the replica must land on disk in replica/domains.bin next to docs-store/"
    );
    assert!(
        !dir.join("docs").exists(),
        "the iroh-docs store (docs/) must not exist"
    );
    assert!(
        !dir.join("blobs").exists(),
        "the blob store (blobs/) must not exist"
    );

    node.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn removal_survives_restart() {
    let key = NetworkKey::generate().unwrap();
    let dir = scratch_store_dir("removal");
    let cfg = persist_cfg(&key, &dir);

    let node = node::start_node(cfg.clone()).await.unwrap();
    node.domains.add("a.example").await.unwrap();
    node.domains.add("b.example").await.unwrap();
    node.domains.remove("a.example").await.unwrap();

    let node = restart(&cfg, node).await;
    assert_eq!(
        node.domains.list().await.unwrap(),
        names(&["b.example"]),
        "the removal from the previous run must survive a restart"
    );

    node.domains.remove("b.example").await.unwrap();
    node.domains.add("c.example").await.unwrap();

    let node = restart(&cfg, node).await;
    assert_eq!(
        node.domains.list().await.unwrap(),
        names(&["c.example"]),
        "the second restart must see the state exactly as it was just before it, not the accumulated history"
    );

    node.shutdown().await;
}
