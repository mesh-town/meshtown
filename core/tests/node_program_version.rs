//! Integration tests for the member's program version (`program_version`).
//!
//! What it does: on live loopback nodes checks that a node announces its own build version under
//! the `node/version/` key, a peer reads it, a rewrite happens only when the record differs, a bad
//! value and a tombstone are indistinguishable from a missing record, and old readers don't see the key.
//! What it doesn't do: it doesn't parse the version string (`program_version_tests`) or look at the snapshot.
//! The node's background task writes the version by itself, with no switch: tests that write their own record
//! wait for it first (`wait_own_current`), otherwise the task would overwrite the test's record.

use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use bytes::Bytes;
use meshtown::geoip;
use meshtown::net::{PeerAddr, PeerId};
use meshtown::network_key::NetworkKey;
use meshtown::node::{self, NodeConfig, RunningNode};
use meshtown::node_state;
use meshtown::program_version::{self, current_program_version, PROGRAM_VERSION_KEY};
use tokio::time::{sleep, Instant};

/// How long a node takes to write its own version after start (the write is local).
const OWN_WRITE_LIMIT: Duration = Duration::from_secs(10);
/// How long a record takes to reach a peer over loopback.
const SYNC_LIMIT: Duration = Duration::from_secs(20);

/// Program versions by member — what `read_program_versions` returns.
type Versions = HashMap<PeerId, String>;

mod publication_on_start {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn started_node_publishes_own_version() {
        // given: a node after `start`; no records from the test
        let a = start_test_node().await;
        // when: we wait for its own record
        let own = wait_own_version(&a).await;
        // then: the node wrote its build version
        assert_eq!(own.as_deref(), current(), "node did not announce its version");
        a.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn neighbor_sees_started_node_version() {
        // given: A and B know each other
        let (a, b) = start_pair().await;
        // when: B waits for A's version
        let versions = wait_versions(&b, |m| m.contains_key(&a.id)).await;
        // then: B sees A's build version
        assert_version(&versions, a.id, current(), "B did not see A's version");
        shutdown_pair(a, b).await;
    }
}

mod rewrite {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ensure_skips_write_when_record_matches() {
        // given: its own record equals the current version, record time `t`
        let a = start_test_node().await;
        wait_own_current(&a).await;
        let t = own_record_time(&a).await;
        // pause: a record written "now" would get a time later than `t`
        sleep(Duration::from_millis(20)).await;
        // when: ensure with the same version
        let written = ensure(&a, current_program_version()).await;
        // then: nothing was written: `Ok(false)`, the time is unchanged
        assert!(!written, "record matches the version — nothing to write");
        let t_after = own_record_time(&a).await;
        assert_eq!(t_after, t, "record time changed without a write");
        a.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ensure_rewrites_changed_version() {
        // given: A and B know each other, A's record equals the current version
        let (a, b) = start_pair().await;
        wait_own_current(&a).await;
        // when: ensure with another version, then once more with the same one
        let first = ensure(&a, "0.0.2").await;
        let second = ensure(&a, "0.0.2").await;
        // then: the first time it is written, the second time it isn't, the peer eventually sees the new version
        assert!(first, "another version must be written");
        assert!(!second, "a repeated ensure with the same version doesn't write");
        let versions = wait_versions(&b, |m| is_version(m, a.id, "0.0.2")).await;
        assert_version(&versions, a.id, Some("0.0.2"), "B did not see A's new version");
        shutdown_pair(a, b).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ensure_rewrites_garbage_own_record() {
        // given: its own record is a bad value `null`
        let a = start_test_node().await;
        wait_own_current(&a).await;
        write_raw(&a, b"null").await;
        // when: we read its own version, then ensure with the current one
        let own_after_garbage = own_version(&a).await;
        let written = ensure(&a, current_program_version()).await;
        // then: the bad value is not a version, and ensure rewrites it
        assert_eq!(own_after_garbage, None, "a bad record is not a version");
        assert!(written, "a bad record must be rewritten");
        assert_eq!(own_version(&a).await.as_deref(), current());
        a.shutdown().await;
    }
}

mod reading_foreign_value {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn garbage_value_is_dropped_by_reader() {
        // given: B has already seen a valid version of A
        let (a, b) = start_pair().await;
        wait_version_seen(&b, a.id).await;
        // when: A writes a bad value
        write_raw(&a, b"null").await;
        let versions = wait_versions(&b, |m| !m.contains_key(&a.id)).await;
        // then: A disappears from B's map
        assert_version(&versions, a.id, None, "bad value got into B's map");
        shutdown_pair(a, b).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tombstone_clears_version_for_everyone() {
        // given: B saw A's version
        let (a, b) = start_pair().await;
        wait_version_seen(&b, a.id).await;
        // when: A deletes the record
        a.domains.shared_replica().del_own(PROGRAM_VERSION_KEY).await.unwrap();
        let versions = wait_versions(&b, |m| !m.contains_key(&a.id)).await;
        // then: B has no A, A's own record is `None`
        assert_version(&versions, a.id, None, "tombstone did not remove A's version at B");
        assert_eq!(own_version(&a).await, None, "tombstone did not remove the version");
        shutdown_pair(a, b).await;
    }
}

mod old_readers {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn old_readers_unaffected_by_version_key() {
        // given: A's exit flag has already reached B (a stable "before" baseline)
        let (a, b) = start_pair().await;
        let exits_before = wait_exits_of(&b, a.id).await;
        // when: A's version record has reached B
        wait_version_seen(&b, a.id).await;
        // then: old readers don't see it
        let rep = b.domains.shared_replica();
        let list = b.domains.list().await.unwrap();
        let checks = b.checks.check_views().await.unwrap();
        let exits = node_state::read_exit_flags(&*rep).await.unwrap();
        let countries = geoip::read_countries(&*rep).await.unwrap();
        assert!(list.is_empty(), "version got into the domain list: {list:?}");
        assert!(checks.is_empty(), "version got into the checks: {checks:?}");
        assert_eq!(exits, exits_before, "version changed the exit flags");
        assert!(countries.is_empty(), "version got into the countries");
        shutdown_pair(a, b).await;
    }
}

// --- setup and helpers ---

async fn start_test_node() -> RunningNode {
    start_node_in(NetworkKey::generate().unwrap()).await
}

async fn start_node_in(key: NetworkKey) -> RunningNode {
    let config = NodeConfig::for_loopback_test(key);
    node::start_node(config).await.unwrap()
}

/// Two nodes of one network, A knows B's address.
async fn start_pair() -> (RunningNode, RunningNode) {
    let key = NetworkKey::generate().unwrap();
    let a = start_node_in(key.clone()).await;
    let b = start_node_in(key).await;
    introduce(&a, &b).await;
    (a, b)
}

async fn shutdown_pair(a: RunningNode, b: RunningNode) {
    a.shutdown().await;
    b.shutdown().await;
}

/// A copy of the helper from `tests/node_country.rs`: enters `b`'s address into `a` over loopback.
async fn introduce(a: &RunningNode, b: &RunningNode) {
    a.introduce(PeerAddr::new(b.id)).await;
}

/// Polls every 50 ms up to `limit`: the last value read, as soon as `done` returns true
/// or the limit expires (not a guessed `sleep`).
async fn poll<T, F: Future<Output = T>>(
    limit: Duration,
    mut read: impl FnMut() -> F,
    done: impl Fn(&T) -> bool,
) -> T {
    let deadline = Instant::now() + limit;
    loop {
        let value = read().await;
        if done(&value) || Instant::now() >= deadline {
            return value;
        }
        sleep(Duration::from_millis(50)).await;
    }
}

/// The node's own version from its document (`own_program_version`).
async fn own_version(node: &RunningNode) -> Option<String> {
    let own = program_version::own_program_version(&*node.domains.shared_replica()).await;
    own.expect("reading its own version must not fail")
}

/// Waits up to 10 s for its own version to appear; otherwise returns `None`.
async fn wait_own_version(node: &RunningNode) -> Option<String> {
    poll(OWN_WRITE_LIMIT, || own_version(node), Option::is_some).await
}

/// Precondition of the write tests: the node's background task has written the current version and finished.
async fn wait_own_current(node: &RunningNode) {
    let own = wait_own_version(node).await;
    assert_eq!(own.as_deref(), current(), "node did not write its version in time");
}

/// This build's version in the form readers return it (`Option<&str>`).
fn current() -> Option<&'static str> {
    Some(current_program_version())
}

async fn read_versions(node: &RunningNode) -> Versions {
    let rep = node.domains.shared_replica();
    program_version::read_program_versions(&*rep).await.unwrap()
}

/// Waits up to 20 s for the version map at `node` to satisfy `want`; returns the last map.
async fn wait_versions(node: &RunningNode, want: impl Fn(&Versions) -> bool) -> Versions {
    poll(SYNC_LIMIT, || read_versions(node), want).await
}

/// Precondition of the read tests: `node` has seen `other`'s version.
async fn wait_version_seen(node: &RunningNode, other: PeerId) {
    let seen = wait_versions(node, |m| m.contains_key(&other)).await;
    assert!(
        seen.contains_key(&other),
        "precondition: the version did not reach the peer"
    );
}

fn is_version(map: &Versions, id: PeerId, want: &str) -> bool {
    map.get(&id).map(String::as_str) == Some(want)
}

/// The version of member `id` in the map is exactly `want` (`None` — the member is not in the map).
fn assert_version(map: &Versions, id: PeerId, want: Option<&str>, why: &str) {
    let got = map.get(&id).map(String::as_str);
    assert_eq!(got, want, "{why}: {map:?}");
}

/// Waits up to 20 s for `node` to see `other`'s exit flag (by default a node enables the exit).
async fn wait_exits_of(node: &RunningNode, other: PeerId) -> HashMap<PeerId, bool> {
    let read = || async { node_state::read_exit_flags(&*node.domains.shared_replica()).await.unwrap() };
    poll(SYNC_LIMIT, read, |exits| exits.get(&other) == Some(&true)).await
}

/// The time of its own version record in microseconds.
async fn own_record_time(node: &RunningNode) -> u64 {
    let record = node_state::own_value(&*node.domains.shared_replica(), PROGRAM_VERSION_KEY).await;
    record.unwrap().expect("its own version record must exist").1
}

/// `ensure_program_version` of its own node; returns whether a write happened.
async fn ensure(node: &RunningNode, version: &str) -> bool {
    let rep = node.domains.shared_replica();
    let written = program_version::ensure_program_version(&*rep, version).await;
    written.expect("ensure_program_version must not fail")
}

/// Writes arbitrary bytes into its own version record (bypassing `announce_program_version`).
async fn write_raw(node: &RunningNode, bytes: &[u8]) {
    let rep = node.domains.shared_replica();
    let write = rep.set_own(PROGRAM_VERSION_KEY, Bytes::copy_from_slice(bytes));
    write.await.unwrap();
}
