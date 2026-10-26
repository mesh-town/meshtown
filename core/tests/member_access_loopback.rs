//! The membership list on two real nodes over loopback, with the list consulted (`open_list = false`):
//! without admissions the neighbours stay out and the refusal is shown; after the creator's admission they see each
//! other; an exclusion removes a node and withdrawing it brings the node back, also when both sides had
//! already given up dialing each other.
//!
//! What it does: the creator and the member share one Managed network and are tied together with `introduce`. Every wait runs under an
//! external deadline (`LIMIT`), so a hang fails the test instead of the run.
//! What it does not do: no web layer and no more than two nodes.

#[path = "common/invite_nodes.rs"]
mod common;
#[path = "common/managed_net.rs"]
mod managed;
#[path = "common/invite_mesh.rs"]
mod mesh;

use std::time::Duration;

use meshtown_core::members::access::actions::{apply, MemberAction};
use meshtown_core::members::access::record::{ensure, AclKind, AclValue, Via};
use meshtown_core::members::access::report::members_reply;
use meshtown_core::members::access::view::Deny;
use meshtown_core::node::{output, RunningNode};
use meshtown_core::transport::net::{PeerAddr, PeerId};
use meshtown_core::transport::net_mainline::unix_now;
use tokio::time::{sleep, timeout, Instant};

use managed::ManagedNet;

/// `a` dials `b`.
async fn introduce(a: &RunningNode, b: &RunningNode) {
    a.introduce(PeerAddr::new(b.id)).await;
}

/// Waits until the node recognizes the given one as a member.
async fn wait_member(node: &RunningNode, who: PeerId, limit: Duration) {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if node.members.is_member(who) {
            return;
        }
        sleep(Duration::from_millis(50)).await;
    }
    panic!("the handshake did not happen within {limit:?}");
}

/// The external deadline of one wait.
const LIMIT: Duration = Duration::from_secs(15);

/// The seed of the member's permanent key.
const MEMBER_SEED: u8 = 32;

/// The creator and a member that nobody has admitted yet.
async fn pair(net: &ManagedNet) -> (RunningNode, RunningNode) {
    let a = net.start_root(net.base.clone()).await;
    (a, net.start_member(MEMBER_SEED, Vec::new()).await)
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

/// The creator `a` writes its admission of `b` into its own copy of the shared list.
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

/// The creator's admission, a recompute and a connection between the two.
async fn connected_pair(a: &RunningNode, b: &RunningNode) {
    admit(a, b).await;
    refresh(a).await;
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
    // given: the creator and a member with no admission
    let net = ManagedNet::new();
    let (a, b) = pair(&net).await;
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
    // when: the creator writes its admission and recomputes
    admit(&a, &b).await;
    refresh(&a).await;
    introduce(&a, &b).await;
    introduce(&b, &a).await;
    // then: they see each other
    wait_member(&a, b.id, LIMIT).await;
    wait_member(&b, a.id, LIMIT).await;
    shut(a, b).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_exclusion_removes_a_member_and_withdrawing_it_restores() {
    // given: the creator and a member it admitted, connected
    let net = ManagedNet::new();
    let (a, b) = pair(&net).await;
    admit(&a, &b).await;
    refresh(&a).await;
    introduce(&a, &b).await;
    wait_member(&a, b.id, LIMIT).await;
    // when: a excludes b
    let changed = timeout(
        LIMIT,
        apply(
            &a.access,
            MemberAction::Exclude(b.id, Some("test".to_string())),
        ),
    )
    .await;
    assert_eq!(changed.expect("the exclusion in time"), Ok(true));
    // then: b is no longer a member of a and the list shows it as excluded
    eventually("b is out", || !a.members.is_member(b.id)).await;
    assert_eq!(status_of(&a, &b), "excluded");
    // when: the exclusion is withdrawn
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
    let net = ManagedNet::new();
    let (a, b) = pair(&net).await;
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
