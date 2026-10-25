//! The membership list across a restart of a node that was excluded or left: on loopback, with the list
//! consulted (`open_list = false`) and the restarted node keeping its lists on disk under the same identity.
//! A node excluded by votes that is pardoned after a restart comes back by itself; a node that left stays out;
//! a restarted node under an exclusion stays out.
//!
//! What it does: two nodes admit each other, one of them is excluded or leaves, restarts with the same key and
//! directory and is introduced again. Every wait runs under an external deadline.
//! What it does not do: no web layer and no more than two nodes. Each test uses its own node keys, because
//! the tests share one process network.

#[path = "common/socks_pair.rs"]
mod common;

use std::path::PathBuf;
use std::time::Duration;

use meshtown_core::members::access::actions::{apply, MemberAction};
use meshtown_core::members::access::record::{ensure, AclKind, AclValue, Via};
use meshtown_core::members::access::report::members_reply;
use meshtown_core::members::access::view::Deny;
use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, NodeConfig, RunningNode};
use meshtown_core::transport::net::NodeSecret;
use meshtown_core::transport::net_mainline::unix_now;
use tokio::time::{sleep, timeout};

use common::{introduce, wait_member};

/// The external deadline of one wait.
const LIMIT: Duration = Duration::from_secs(15);
/// How long a pardoned node may take to return without a manual sync.
const RETURN_LIMIT: Duration = Duration::from_secs(45);
/// How long a node that must stay out is watched.
const WATCH: Duration = Duration::from_secs(12);

mod pardoned_after_a_restart {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_pardoned_node_comes_back_after_a_restart() {
        // given: a connected pair, a excluded b, b knows it and was restarted with the same directory
        let (a, b, cfg) = pair(31, 32).await;
        let b = given_excluded_and_restarted(&a, b, &cfg).await;
        eventually("b still knows its exclusion", || knows_own_exclusion(&b)).await;
        // when: a withdraws the exclusion, and nobody syncs by hand
        act(&a, MemberAction::Unexclude(b.id)).await;
        // then: the nodes are members of each other again and b is no longer excluded
        let back = || a.members.is_member(b.id) && b.members.is_member(a.id);
        eventually_within(RETURN_LIMIT, "a and b are members again", back).await;
        eventually("b is no longer excluded", || !knows_own_exclusion(&b)).await;
        shut(a, b).await;
    }
}

mod staying_out_after_a_restart {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_node_that_left_stays_out_after_a_restart() {
        // given: a connected pair, b left, a sees that b left on its own, b was restarted
        let (a, b, cfg) = pair(33, 34).await;
        act(&b, MemberAction::Leave).await;
        let _ = b.domains.sync_shared_with(a.id).await;
        eventually("a sees that b left", || b_left(&a, &b)).await;
        let b = restart(&cfg, b).await;
        // when: b is introduced to a and given time
        introduce(&b, &a).await;
        sleep(WATCH).await;
        // then: b refuses a as "left" and neither side counts the other as a member
        assert_eq!(b.members.access().check(a.id), Err(Deny::Left));
        then_not_members(&a, &b);
        shut(a, b).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_restarted_node_under_exclusion_stays_out() {
        // given: a connected pair, a excluded b, b knows it and was restarted
        let (a, b, cfg) = pair(35, 36).await;
        let b = given_excluded_and_restarted(&a, b, &cfg).await;
        // when: b is introduced to a and given time
        sleep(WATCH).await;
        // then: b judges a by the ordinary rules, a keeps b excluded, and nobody is a member
        assert_eq!(b.members.access().check(a.id), Ok(()));
        assert_eq!(a.members.access().check(b.id), Err(Deny::Excluded));
        then_not_members(&a, &b);
        shut(a, b).await;
    }
}

// ---- helpers ----

/// A node with a fixed identity (by `tag`) and its own store directory, so that a restart brings it back as it was.
fn restartable_cfg(key: &NetworkKey, tag: u8) -> NodeConfig {
    let mut cfg = NodeConfig::for_loopback_test(key.clone());
    cfg.open_list = false;
    cfg.identity_secret = Some(NodeSecret::from_bytes(&[tag; 32]));
    cfg.docs_store_dir = Some(scratch_store_dir(tag));
    cfg
}

/// A store directory of its own in the temporary directory (only its parent is created).
fn scratch_store_dir(tag: u8) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let root = std::env::temp_dir().join(format!(
        "mesh-access-restart-{}-{nanos}-{tag}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("test temporary directory");
    root.join("docs-store")
}

/// Stops the node and starts a new one with the same config, key and directory.
async fn restart(cfg: &NodeConfig, node: RunningNode) -> RunningNode {
    node.shutdown().await;
    node::start_node(cfg.clone()).await.unwrap()
}

/// A connected pair: `a` is plain, `b` is restartable under the key `tag_b`; the config of `b` comes back too.
async fn pair(tag_a: u8, tag_b: u8) -> (RunningNode, RunningNode, NodeConfig) {
    let key = NetworkKey::generate().unwrap();
    let mut plain = NodeConfig::for_loopback_test(key.clone());
    plain.open_list = false;
    plain.identity_secret = Some(NodeSecret::from_bytes(&[tag_a; 32]));
    let cfg_b = restartable_cfg(&key, tag_b);
    let a = node::start_node(plain).await.unwrap();
    let b = node::start_node(cfg_b.clone()).await.unwrap();
    connected_pair(&a, &b).await;
    (a, b, cfg_b)
}

/// `a` excludes `b`, `b` learns of it, restarts and is introduced to `a` again.
async fn given_excluded_and_restarted(
    a: &RunningNode,
    b: RunningNode,
    cfg: &NodeConfig,
) -> RunningNode {
    act(a, MemberAction::Exclude(b.id, None)).await;
    eventually("b is out", || !a.members.is_member(b.id)).await;
    let _ = b.domains.sync_shared_with(a.id).await;
    eventually("b knows its own exclusion", || knows_own_exclusion(&b)).await;
    let b = restart(cfg, b).await;
    introduce(&b, a).await;
    b
}

/// `b`'s own row says "excluded" (a node always has a row for itself).
fn knows_own_exclusion(b: &RunningNode) -> bool {
    status_of(b, b) == "excluded"
}

/// `a` sees that `b` left on its own rather than being voted out.
fn b_left(a: &RunningNode, b: &RunningNode) -> bool {
    // before the first publication after a restart there is no verdict yet: not seen
    let Some(verdicts) = a.members.access().verdicts() else {
        return false;
    };
    format!("{:?}", verdicts.ledger.how_excluded(&b.id)) == "Some(Left)"
}

fn then_not_members(a: &RunningNode, b: &RunningNode) {
    assert!(!a.members.is_member(b.id), "a counts b as a member");
    assert!(!b.members.is_member(a.id), "b counts a as a member");
}

/// Polls `cond` until it holds or `LIMIT` ends.
async fn eventually(what: &str, cond: impl Fn() -> bool) {
    eventually_within(LIMIT, what, cond).await;
}

/// Polls `cond` until it holds or `limit` ends.
async fn eventually_within(limit: Duration, what: &str, cond: impl Fn() -> bool) {
    let wait = async {
        while !cond() {
            sleep(Duration::from_millis(50)).await;
        }
    };
    timeout(limit, wait)
        .await
        .unwrap_or_else(|_| panic!("did not happen within {limit:?}: {what}"));
}

/// `a` writes its admission of `b` into its own copy of the shared list.
async fn admit(a: &RunningNode, b: &RunningNode) {
    let value = AclValue {
        kind: AclKind::Admit,
        subject: b.id,
        via: Via::None,
        made_unix: unix_now(),
        note: String::new(),
    };
    ensure(a.domains.shared_replica().as_ref(), &value)
        .await
        .unwrap();
}

async fn refresh(a: &RunningNode) {
    timeout(LIMIT, a.access.refresh())
        .await
        .expect("recompute in time")
        .unwrap();
}

fn status_of(a: &RunningNode, who: &RunningNode) -> String {
    // before the first publication after a restart there is no verdict yet: no row
    let Some(v) = a.members.access().verdicts() else {
        return String::new();
    };
    let reply = members_reply(&v, &a.members, &a.members.access().refusals(), None);
    reply
        .members
        .iter()
        .find(|r| r.id_full == who.id.to_string())
        .map(|r| r.status.to_string())
        .unwrap_or_default()
}

/// Mutual admissions, a recompute on each side and a connection between them.
async fn connected_pair(a: &RunningNode, b: &RunningNode) {
    admit(a, b).await;
    admit(b, a).await;
    refresh(a).await;
    refresh(b).await;
    introduce(a, b).await;
    wait_member(a, b.id, LIMIT).await;
    wait_member(b, a.id, LIMIT).await;
}

/// Applies a member action on `a` and requires that it changed the list.
async fn act(a: &RunningNode, action: MemberAction) {
    let changed = timeout(LIMIT, apply(&a.access, action))
        .await
        .expect("the action in time");
    assert_eq!(changed, Ok(true));
}

async fn shut(a: RunningNode, b: RunningNode) {
    a.shutdown().await;
    b.shutdown().await;
}
