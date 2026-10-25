//! The membership list on two real nodes over loopback, with the list consulted (`open_list = false`):
//! without admissions the neighbours stay out and the refusal is shown; after mutual admissions they see each
//! other; an exclusion vote removes a node and withdrawing it brings the node back, also when both sides had
//! already given up dialing each other.
//!
//! What it does: both nodes share one network key and are tied together with `introduce`. Every wait runs under an
//! external deadline (`LIMIT`), so a hang fails the test instead of the run.
//! What it does not do: no web layer and no more than two nodes (the vote threshold is the job of the unit tests).

#[path = "common/socks_pair.rs"]
mod common;

use std::time::Duration;

use meshtown_core::members::access::actions::{apply, MemberAction};
use meshtown_core::members::access::record::{ensure, AclKind, AclValue, Via};
use meshtown_core::members::access::report::members_reply;
use meshtown_core::members::access::view::Deny;
use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, output, NodeConfig, RunningNode};
use meshtown_core::transport::net_mainline::unix_now;
use tokio::time::{sleep, timeout};

use common::{introduce, wait_member};

/// The external deadline of one wait.
const LIMIT: Duration = Duration::from_secs(15);

async fn start(key: &NetworkKey) -> RunningNode {
    let mut cfg = NodeConfig::for_loopback_test(key.clone());
    cfg.open_list = false;
    node::start_node(cfg).await.unwrap()
}

async fn pair() -> (RunningNode, RunningNode) {
    let key = NetworkKey::generate().unwrap();
    (start(&key).await, start(&key).await)
}

/// Polls `cond` until it holds or `LIMIT` ends.
async fn eventually(what: &str, cond: impl Fn() -> bool) {
    let wait = async {
        while !cond() {
            sleep(Duration::from_millis(50)).await;
        }
    };
    timeout(LIMIT, wait)
        .await
        .unwrap_or_else(|_| panic!("did not happen within {LIMIT:?}: {what}"));
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
    let v = a.members.access().verdicts().expect("published");
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

/// Fails unless the log has a line with `text` after the sequence mark `since`.
fn assert_logged(since: u64, text: &str) {
    let page = output::log_page(since, 5000);
    assert!(
        page.lines.iter().any(|l| l.text.contains(text)),
        "the log must have a line with {text:?}: {:?}",
        page.lines
    );
}

async fn shut(a: RunningNode, b: RunningNode) {
    a.shutdown().await;
    b.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unlisted_neighbours_are_refused_and_listed_ones_connect() {
    // given: two nodes with no admissions
    let (a, b) = pair().await;
    // when: they are introduced
    introduce(&a, &b).await;
    // then: a records the refusal of b as unlisted and does not count b as a member
    let refused = || {
        a.members
            .access()
            .refusals()
            .iter()
            .any(|(p, r)| *p == b.id && r.deny == Deny::Unlisted)
    };
    eventually("a refuses b as unlisted", refused).await;
    assert!(!a.members.is_member(b.id));
    // when: both write their admissions and recompute
    admit(&a, &b).await;
    admit(&b, &a).await;
    refresh(&a).await;
    refresh(&b).await;
    introduce(&a, &b).await;
    introduce(&b, &a).await;
    // then: they see each other
    wait_member(&a, b.id, LIMIT).await;
    wait_member(&b, a.id, LIMIT).await;
    shut(a, b).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_exclusion_vote_removes_a_member_and_withdrawing_it_restores() {
    // given: two nodes that admitted each other and are connected
    let (a, b) = pair().await;
    admit(&a, &b).await;
    admit(&b, &a).await;
    refresh(&a).await;
    refresh(&b).await;
    introduce(&a, &b).await;
    wait_member(&a, b.id, LIMIT).await;
    // when: a votes to exclude b
    let changed = timeout(
        LIMIT,
        apply(
            &a.access,
            MemberAction::Exclude(b.id, Some("test".to_string())),
        ),
    )
    .await;
    assert_eq!(changed.expect("the vote in time"), Ok(true));
    // then: b is no longer a member of a and the list shows it as excluded
    eventually("b is out", || !a.members.is_member(b.id)).await;
    assert_eq!(status_of(&a, &b), "excluded");
    // when: the vote is withdrawn
    let back = timeout(LIMIT, apply(&a.access, MemberAction::Unexclude(b.id))).await;
    assert_eq!(back.expect("the withdrawal in time"), Ok(true));
    // then: b is listed again
    assert_eq!(status_of(&a, &b), "member");
    // when: b gets the withdrawal while the first redial is still waiting for its term
    let _ = b.domains.sync_shared_with(a.id).await;
    // then: both sides are members of each other again
    wait_member(&a, b.id, LIMIT).await;
    wait_member(&b, a.id, LIMIT).await;
    shut(a, b).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unexcluded_member_comes_back_without_a_restart() {
    // given: a and b are members of each other, a excludes b, and b has learned that it was excluded
    let (a, b) = pair().await;
    connected_pair(&a, &b).await;
    act(&a, MemberAction::Exclude(b.id, None)).await;
    eventually("b is out", || !a.members.is_member(b.id)).await;
    let _ = b.domains.sync_shared_with(a.id).await;
    // and: it keeps judging its neighbours by the ordinary rules, so a stays allowed for it
    eventually("b knows its own exclusion", || {
        status_of(&b, &b) == "excluded" && b.members.access().check(a.id) == Ok(())
    })
    .await;
    // and: both redial terms have passed, so neither side has a dial in flight or waiting
    sleep(Duration::from_secs(10)).await;
    let before = output::log_summary(0).last_seq;
    // when: a withdraws the exclusion and b receives the record
    act(&a, MemberAction::Unexclude(b.id)).await;
    let _ = b.domains.sync_shared_with(a.id).await;
    // then: the connection returns by itself, without a restart and without a new `introduce`
    eventually("a and b are members of each other again", || {
        a.members.is_member(b.id) && b.members.is_member(a.id)
    })
    .await;
    assert_logged(
        before,
        &format!("{}: access allowed again", output::short_peer_id(&b.id)),
    );
    shut(a, b).await;
}
