//! Integration tests of the node's live name (`node_name`).
//!
//! What it does: on live nodes on loopback checks that the node writes its current name under the key
//! `node/name/`, a name change (`RunningNode::set_node_name`) reaches a peer's `Members` within 10 s without
//! reconnecting, an invalid name and a junk record don't reach the peer, the name of a node without a connection
//! comes from the document, and the handshake carries the current name, not the startup name.
//! What it doesn't do: it doesn't parse the name (`node_name_tests`), doesn't go through the web page and the snapshot
//! (`node_ctl::state_tests`, a manual run). The helpers are copies from `exit_flag.rs`,
//! `node_program_version.rs` and `handshake_compat.rs`: we don't pull in the shared `core/tests/common/`.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::time::Duration;

use bytes::Bytes;
use meshtown::handshake::parse_name_tail;
use meshtown::net::{PeerAddr, PeerId};
use meshtown::network_key::NetworkKey;
use meshtown::node::{self, NodeConfig, RunningNode};
use meshtown::node_name::{self, NODE_NAME_KEY};
use meshtown::node_state;
use meshtown::program_version;
use meshtown::words::default_node_name;
use tokio::time::{sleep, timeout, Instant};

#[path = "common/raw_handshake.rs"]
mod raw;

/// The time of the first document reconciliation between a pair of nodes over loopback.
const SYNC_LIMIT: Duration = Duration::from_secs(20);
/// The task's boundary: a name change reaches the peer within 10 s after the pair reconciles.
const LIVE_LIMIT: Duration = Duration::from_secs(10);
/// More than the observer's polling period (2 s): in that time it would have managed to set the name from the document.
const WATCH_MARGIN: Duration = Duration::from_secs(3);

/// Names per member — what `read_node_names` returns.
type Names = HashMap<PeerId, String>;

// fails if the node doesn't write the name to `node/name/` (no publisher) or the key shares readers with neighboring
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn publication() {
    // given A and B know each other
    let (a, b) = start_pair().await;

    // when B waits for A's name record
    let names = wait_names(&b, SYNC_LIMIT, |m| m.contains_key(&a.id)).await;

    // then B's document holds the name that A calls itself
    assert_name(names.get(&a.id), &a.current_name(), "A's name at B");
    // and the name record did not add authors to the other readers (exit flag, version, domain list)
    let extra = extra_authors(&b, &[a.id, b.id]).await;
    assert!(extra.is_empty(), "extra authors: {extra:?}");
    let list = b.domains.list().await.unwrap();
    assert!(list.is_empty(), "the name landed in the domain list: {list:?}");
    shutdown_pair(a, b).await;
}

// fails without the observer: the name in B's `Members` stays from the handshake, there is no reconnect
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_change() {
    // given the pair reconciled: B sees A's name record
    let (a, b) = start_pair().await;
    wait_name_seen(&b, a.id).await;

    // when A changes the name without a restart
    a.set_node_name(Some("new-name"));

    // then B calls A by the new name within 10 s
    let name = wait_member_name(&b, a.id, LIVE_LIMIT, "new-name").await;
    assert_name(name.as_ref(), "new-name", "B did not learn A's new name within 10 s");
    let own = a.current_name();
    assert_name(Some(&own), "new-name", "A's own name");
    shutdown_pair(a, b).await;
}

// fails if an invalid name goes to the peer as is (no `effective_node_name` in `set_node_name`
// or `parse_node_name` in the reader)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejected_name() {
    // given B already sees a valid name of A (the startup name on loopback is the default anyway — without this
    // step the test would be green on any code)
    let (a, b) = start_pair().await;
    wait_name_seen(&b, a.id).await;
    a.set_node_name(Some("name-1"));
    let first = wait_member_name(&b, a.id, LIVE_LIMIT, "name-1").await;
    assert_name(first.as_ref(), "name-1", "precondition: B did not see name-1");

    // when A saves a name with a control character
    a.set_node_name(Some("a\u{7}b"));

    // then B has the default name both in the document and in `Members`
    let default = default_node_name(a.id.as_bytes());
    let name = wait_member_name(&b, a.id, LIVE_LIMIT, &default).await;
    let names = wait_names(&b, LIVE_LIMIT, |m| m.get(&a.id) == Some(&default)).await;
    assert_name(name.as_ref(), &default, "A's invalid name in B's Members");
    assert_name(names.get(&a.id), &default, "A's invalid name in B's document");
    let own = a.current_name();
    assert_name(Some(&own), &default, "A's own name");
    shutdown_pair(a, b).await;
}

// fails if junk in `node/name/` reaches the name map (no `parse_node_name` rejection)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn garbage_dropped() {
    // given A reads B's name from the document, A's `Members` holds B's name from the handshake
    let (a, b) = start_pair().await;
    wait_name_seen(&a, b.id).await;
    let before = a.members.name_of(b.id);
    assert!(before.is_some(), "precondition: A doesn't know B's name");

    // when B writes a value with a control character over its own record
    let garbage = Bytes::from("a\u{7}b");
    let rep = b.domains.shared_replica();
    rep.set_own(NODE_NAME_KEY, garbage).await.unwrap();

    // then A's reader drops the record: B is gone from the name map
    let names = wait_names(&a, SYNC_LIMIT, |m| !m.contains_key(&b.id)).await;
    assert!(!names.contains_key(&b.id), "junk in the map: {names:?}");
    // and the observer didn't touch B's name over a polling tick (the read is repeated: the record could have been in flight
    // without a blob — then the reader skipped it for a different reason)
    sleep(WATCH_MARGIN).await;
    let names = read_names(&a).await;
    assert!(!names.contains_key(&b.id), "junk in the map later: {names:?}");
    assert_eq!(a.members.name_of(b.id), before, "B's name changed");
    shutdown_pair(a, b).await;
}

// fails if the name of an author without a connection is taken only from the handshake or from disk (no reading
// of the document by the observer)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_author() {
    // given A called itself "name-a", the record reached B's document, then A is shut down
    let (a, b) = start_pair().await;
    a.set_node_name(Some("name-a"));
    let names = wait_names(&b, SYNC_LIMIT, |m| {
        m.get(&a.id).map(String::as_str) == Some("name-a")
    });
    assert_name(names.await.get(&a.id), "name-a", "A's record did not reach B");
    let (a_id, key) = (a.id, a.network_key.clone());
    a.shutdown().await;

    // when C starts and meets only B
    let c = start_node_in(key).await;
    introduce(&c, &b).await;

    // then C learns A's name from the document, though there is no connection to A
    let name = wait_member_name(&c, a_id, SYNC_LIMIT, "name-a").await;
    assert_name(name.as_ref(), "name-a", "C did not get A's name from the document");
    assert!(!c.members.is_member(a_id), "C is online with the shut-down A");
    b.shutdown().await;
    c.shutdown().await;
}

// fails if the handshake takes the startup name rather than the current one from `MemberContext.own_name_tx`
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handshake_sends_current_name() {
    // given A changed its name after startup; next to it only raw endpoints, they have no network document
    let key = NetworkKey::generate().unwrap();
    let mut cfg = NodeConfig::for_loopback_test(key.clone());
    cfg.probe_first_delay = Duration::from_secs(3600); // a raw member won't answer the probe
    let a = node::start_node(cfg).await.unwrap();
    a.set_node_name(Some("new-name"));

    // when a raw initiator comes to A
    let got = raw::raw_initiate_to(a.id, &key).await;

    // then A as the responder named itself by the new name
    assert_name(got.as_ref(), "new-name", "responder A: wrong name");

    // when A itself dials a raw responder
    let (responder, mut tails) = raw::RawResponder::spawn(&key);
    a.introduce(PeerAddr::new(responder.id)).await;

    // then A as the initiator sent the new name in the tail
    let tail = timeout(SYNC_LIMIT, tails.recv())
        .await
        .expect("A did not call within 20 s");
    let sent = parse_name_tail(&tail.expect("the tail channel is closed"));
    assert_name(sent.as_ref(), "new-name", "initiator A: wrong name");
    responder.stop().await;
    a.shutdown().await;
}

// --- setup and helpers ---

async fn start_node_in(key: NetworkKey) -> RunningNode {
    node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap()
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

/// A copy of the helper from `core/tests/exit_flag.rs`: introduces the address of `b` into `a` (in the `mem` network — by key).
async fn introduce(a: &RunningNode, b: &RunningNode) {
    a.introduce(PeerAddr::new(b.id)).await;
}

/// Polls every 50 ms up to `limit`: the last value read, as soon as `done` returns true
/// or the time runs out (a copy from `core/tests/node_program_version.rs`).
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

async fn read_names(node: &RunningNode) -> Names {
    let rep = node.domains.shared_replica();
    node_name::read_node_names(&*rep).await.unwrap()
}

/// Waits up to `limit` until the name map at `node` satisfies `want`; returns the last map.
async fn wait_names(node: &RunningNode, limit: Duration, want: impl Fn(&Names) -> bool) -> Names {
    poll(limit, || read_names(node), want).await
}

/// Precondition: `other`'s name record reached `node`'s document (the pair reconciled).
async fn wait_name_seen(node: &RunningNode, other: PeerId) {
    let names = wait_names(node, SYNC_LIMIT, |m| m.contains_key(&other)).await;
    assert!(names.contains_key(&other), "the name record didn't arrive");
}

async fn member_name(node: &RunningNode, id: PeerId) -> Option<String> {
    node.members.name_of(id)
}

/// Waits up to `limit` until `node` names the member `id` by `want`; returns the last name.
async fn wait_member_name(
    node: &RunningNode,
    id: PeerId,
    limit: Duration,
    want: &str,
) -> Option<String> {
    poll(
        limit,
        || member_name(node, id),
        |n| n.as_deref() == Some(want),
    )
    .await
}

/// The name `got` is exactly `want` (`None` — no name).
fn assert_name(got: Option<&String>, want: &str, why: &str) {
    assert_eq!(got.map(String::as_str), Some(want), "{why}");
}

/// Authors in reads unrelated to the name (exit flags, program versions), except `known`.
async fn extra_authors(node: &RunningNode, known: &[PeerId]) -> Vec<PeerId> {
    let rep = node.domains.shared_replica();
    let exits = node_state::read_exit_flags(&*rep).await.unwrap();
    let versions = program_version::read_program_versions(&*rep).await.unwrap();
    let all: HashSet<_> = exits.into_keys().chain(versions.into_keys()).collect();
    all.into_iter().filter(|id| !known.contains(id)).collect()
}
