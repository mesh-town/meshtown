//! The vote counters of the snapshot (`collect.rs`): with no published verdicts they are empty,
//! `collect` never forces a recompute, and `overlay_members` reads the counters live.

use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::config::config_file::Settings;
use crate::members::access::tally::{Exclusion, How, Ledger, PendingVote};
use crate::members::access::view::Verdicts;
use crate::members::network_key::NetworkKey;
use crate::node::node_ctl::snapshot::DocCache;
use crate::node::node_ctl::NodeCtl;
use crate::node::NodeConfig;
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;
use crate::web_gui::NodeControl;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_verdicts_means_no_votes() {
    // given: a node whose list was never recomputed (the loopback preset leaves the view open)
    let node = crate::node::start_node(NodeConfig::for_loopback_test(
        NetworkKey::generate().unwrap(),
    ))
    .await
    .unwrap();
    assert!(
        node.access.view.verdicts().is_none(),
        "precondition: nothing is published"
    );
    // when: the snapshot is collected
    let collected = collect(&node, None, "direct_and_relay", &mut DocCache::default()).await;
    // then: both counters are empty and the snapshot published nothing
    assert_eq!(collected.snap.members.voting, 0);
    assert!(!collected.snap.members.me_excluded);
    assert!(
        node.access.view.verdicts().is_none(),
        "collect must not recompute"
    );
    node.shutdown().await;
}

async fn loopback_node() -> crate::node::RunningNode {
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    cfg.socks.enabled = false;
    crate::node::start_node(cfg).await.unwrap()
}

fn verdicts_of(me: PeerId, ledger: Ledger) -> Verdicts {
    Verdicts {
        me,
        ledger: Arc::new(ledger),
        my_votes: BTreeSet::new(),
        blocks: BTreeSet::new(),
        reasons: BTreeMap::new(),
    }
}

/// A list of me, a proposer and a subject; the proposer's removal vote on the subject waits for my confirmation.
fn one_vote_waiting(me: PeerId) -> Verdicts {
    let (proposer, subject) = (pid(2), pid(3));
    let admitted = BTreeSet::from([me, proposer, subject]);
    let pending = BTreeMap::from([(
        subject,
        PendingVote {
            voters: vec![proposer],
            need: 2,
        },
    )]);
    verdicts_of(
        me,
        Ledger {
            admitted,
            excluded: BTreeMap::new(),
            pending,
            need: 2,
            admissions: BTreeMap::new(),
            forgets: Default::default(),
            admit_unix: Default::default(),
            forget_marks: Default::default(),
        },
    )
}

fn excluding(me: PeerId) -> Verdicts {
    let excluded = BTreeMap::from([(
        me,
        Exclusion {
            how: How::Votes,
            at_micros: 1,
            voters: vec![pid(2)],
        },
    )]);
    verdicts_of(
        me,
        Ledger {
            admitted: BTreeSet::from([me, pid(2)]),
            excluded,
            pending: BTreeMap::new(),
            need: 1,
            admissions: BTreeMap::new(),
            forgets: Default::default(),
            admit_unix: Default::default(),
            forget_marks: Default::default(),
        },
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collect_counts_the_votes_awaiting_this_node() {
    // given: a node whose published list holds a vote proposed by another member
    let node = loopback_node().await;
    node.access.view.publish(one_vote_waiting(node.id));
    // when: the snapshot is collected
    let collected = collect(&node, None, "direct_and_relay", &mut DocCache::default()).await;
    // then: the vote is counted and the node is not excluded
    assert_eq!(collected.snap.members.voting, 1);
    assert!(!collected.snap.members.me_excluded);
    node.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collect_flags_this_node_as_excluded() {
    // given: a node whose published list excludes it
    let node = loopback_node().await;
    node.access.view.publish(excluding(node.id));
    // when: the snapshot is collected
    let collected = collect(&node, None, "direct_and_relay", &mut DocCache::default()).await;
    // then: the flag is up
    assert!(collected.snap.members.me_excluded);
    node.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn overlay_members_shows_votes_newer_than_the_collected_snapshot() {
    // given: a snapshot collected before any list was published
    let node = loopback_node().await;
    let mut snap = collect(&node, None, "direct_and_relay", &mut DocCache::default())
        .await
        .snap;
    assert_eq!(snap.members.voting, 0);
    // when: a vote appears and the old snapshot is overlaid with the live block
    node.access.view.publish(one_vote_waiting(node.id));
    overlay_members(&mut snap, &node);
    // then: the vote is shown
    assert_eq!(snap.members.voting, 1);
    node.shutdown().await;
}

#[tokio::test]
async fn snapshot_right_after_attach_already_shows_the_vote() {
    // given: a controller with no collected snapshot and a node whose list holds a waiting vote
    let node = loopback_node().await;
    node.access.view.publish(one_vote_waiting(node.id));
    let ctl = NodeCtl::new(&Settings::default());
    // when: the node is attached and the snapshot is read at once (the background collection has not run)
    ctl.attach(&node);
    let snap = ctl.snapshot();
    // then: the vote is in the block
    assert_eq!(snap.members.voting, 1);
    node.shutdown().await;
}

/// Me and a voter are admitted; `x` asks to join with the voter's confirmation, `me_asks` adds my own request.
fn requests_waiting(me: PeerId, me_asks: bool) -> Verdicts {
    let (voter, x) = (pid(2), pid(9));
    let request = |voters: Vec<PeerId>| crate::members::access::tally::Admission {
        voters,
        need: 2,
        label: None,
    };
    let mut admissions = BTreeMap::from([(x, request(vec![voter]))]);
    if me_asks {
        admissions.insert(me, request(vec![]));
    }
    let admitted = BTreeSet::from([me, voter]);
    verdicts_of(
        me,
        Ledger {
            admitted,
            excluded: BTreeMap::new(),
            pending: BTreeMap::new(),
            need: 2,
            admissions,
            forgets: Default::default(),
            admit_unix: Default::default(),
            forget_marks: Default::default(),
        },
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collect_counts_admitting() {
    // given: a published list with one request that I have not confirmed
    let node = loopback_node().await;
    node.access.view.publish(requests_waiting(node.id, false));
    // when: the snapshot is collected
    let collected = collect(&node, None, "direct_and_relay", &mut DocCache::default()).await;
    // then: the request is counted and I am not waiting
    assert_eq!(collected.snap.members.admitting, 1);
    assert!(!collected.snap.members.me_pending);
    node.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collect_flags_me_pending() {
    // given: my own request is on the list and another node has refused me as not admitted
    let node = loopback_node().await;
    node.access.view.publish(requests_waiting(node.id, true));
    node.access.view.note_refusal(
        pid(2),
        crate::members::access::view::Side::Them,
        crate::members::access::view::Deny::Unlisted,
    );
    // when: the snapshot is collected
    let collected = collect(&node, None, "direct_and_relay", &mut DocCache::default()).await;
    // then: the flag is up
    assert!(collected.snap.members.me_pending);
    node.shutdown().await;
}
