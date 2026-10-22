//! The wire "program version → `collect()` → `peers` row" checked with real nodes on loopback
//! (modelled on `collect_country_tests.rs`): the own
//! row shows the build version, not the document record; another member's row shows the version from the shared document.
//! The assembly rule itself (`program_versions[id]` → `version`) is checked by
//! `build_program_version_tests.rs`; here the point is that `collect` inserts its own version and reads
//! the document, not a stub.
//! At startup the node writes its own version itself (no switch, a background task): a test that
//! writes its own value into the document first waits for that write (`wait_own_version`), otherwise the task
//! would overwrite the test's record.

use super::*;

use tokio::time::{sleep, Instant};

use crate::members::network_key::NetworkKey;
use crate::node::program_version::{self, current_program_version};
use crate::node::NodeConfig;

/// The time within which the node writes its own version after startup (the write is local).
const OWN_WRITE_LIMIT: Duration = Duration::from_secs(10);
/// The time within which a record reaches a loopback neighbor.
const SYNC_LIMIT: Duration = Duration::from_secs(20);
/// A version this build cannot have: tells a document record apart from the build version.
const OTHER_VERSION: &str = "9.9.9";

mod own_row {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn collect_own_row_is_build_version_not_document() {
        // given: the node waited for its own record, then the document holds `9.9.9` for it
        let node = start_test_node(NetworkKey::generate().unwrap()).await;
        wait_own_version(&node).await;
        write_version(&node, OTHER_VERSION).await;
        let in_document = program_version::read_program_versions(&*node.domains.shared_replica())
            .await
            .unwrap();
        assert_eq!(
            in_document.get(&node.id).map(String::as_str),
            Some(OTHER_VERSION),
            "precondition: the document holds the own record — not the build version"
        );
        // when: the snapshot is built
        let collected = collect(&node, None, "direct_and_relay", &mut DocCache::default()).await;
        // then: the own row shows the build version, not the document one
        assert_eq!(
            row_version(&collected, &node.id.to_string()),
            Some(current_program_version().to_string()),
            "the own row takes the version from the build constant"
        );
        node.shutdown().await;
    }
}

mod neighbor_row {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn collect_shows_neighbor_version_from_document() {
        // given: A and B know each other; A wrote a version different from the build version and B sees it
        let key = NetworkKey::generate().unwrap();
        let a = start_test_node(key.clone()).await;
        let b = start_test_node(key).await;
        introduce(&a, &b).await;
        wait_own_version(&a).await;
        write_version(&a, OTHER_VERSION).await;
        wait_neighbor_version(&b, &a, OTHER_VERSION).await;
        // when: B builds the snapshot
        let collected = collect_until_row(&b, &a).await;
        // then: A's row shows the version from the document read from the neighbor
        assert_eq!(
            row_version(&collected, &a.id.to_string()),
            Some(OTHER_VERSION.to_string()),
            "collect should read the neighbor's version from the network document"
        );
        a.shutdown().await;
        b.shutdown().await;
    }
}

async fn start_test_node(key: NetworkKey) -> RunningNode {
    crate::node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap()
}

/// Introduces `b` to `a` by one key: `mem` dials by id.
async fn introduce(a: &RunningNode, b: &RunningNode) {
    a.introduce(crate::transport::net::PeerAddr::new(b.id))
        .await;
}

/// Writes the version `version` as the own record of node `node`.
async fn write_version(node: &RunningNode, version: &str) {
    program_version::announce_program_version(&*node.domains.shared_replica(), version)
        .await
        .expect("the version should be written to the network document");
}

/// Waits up to `OWN_WRITE_LIMIT` for the node's background task to write the build version and finish.
async fn wait_own_version(node: &RunningNode) {
    let deadline = Instant::now() + OWN_WRITE_LIMIT;
    loop {
        let own = program_version::own_program_version(&*node.domains.shared_replica())
            .await
            .unwrap();
        if own.as_deref() == Some(current_program_version()) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the node did not write its own version within the limit, have {own:?}"
        );
        sleep(Duration::from_millis(50)).await;
    }
}

/// Polls every 50 ms up to `SYNC_LIMIT`: waits until `node` reads version `want` from `other`.
async fn wait_neighbor_version(node: &RunningNode, other: &RunningNode, want: &str) {
    let deadline = Instant::now() + SYNC_LIMIT;
    loop {
        let seen = program_version::read_program_versions(&*node.domains.shared_replica())
            .await
            .unwrap();
        if seen.get(&other.id).map(String::as_str) == Some(want) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the neighbor did not see version {want}, have {seen:?}"
        );
        sleep(Duration::from_millis(50)).await;
    }
}

/// Collects a `node` snapshot until the `other` row appears in it (up to `SYNC_LIMIT`): a member's
/// row comes from a live connection, not from the document, and may be late.
async fn collect_until_row(node: &RunningNode, other: &RunningNode) -> Collected {
    let deadline = Instant::now() + SYNC_LIMIT;
    loop {
        let collected = collect(node, None, "direct_and_relay", &mut DocCache::default()).await;
        let has_row = collected
            .snap
            .peers
            .iter()
            .any(|p| p.id_full == other.id.to_string());
        if has_row || Instant::now() >= deadline {
            return collected;
        }
        sleep(Duration::from_millis(50)).await;
    }
}

/// The version in the row of the member with full id `id_full`; no row — the test fails.
fn row_version(collected: &Collected, id_full: &str) -> Option<String> {
    let row = collected
        .snap
        .peers
        .iter()
        .find(|p| p.id_full == id_full)
        .unwrap_or_else(|| {
            let known: Vec<_> = collected.snap.peers.iter().map(|p| &p.id_full).collect();
            panic!("no row {id_full} in peers, have {known:?}")
        });
    row.program_version.clone()
}
