//! Admission by an administrator on real nodes over loopback, with the list consulted (`open_list = false`):
//! a newcomer admitted by the creator is let in at the others, whatever the size of the network; withdrawing
//! the admission cuts the newcomer off at the creator.
//!
//! What it does: the creator and the members share one Managed network and are tied together with
//! `introduce` or `initial_peers`. Every wait runs under an external deadline (`LIMIT`), so a hang fails the
//! test instead of the run.
//! What it does not do: no web layer; the counting rules themselves belong to the unit tests.

#[path = "common/invite_nodes.rs"]
mod common;
#[path = "common/managed_net.rs"]
mod managed;
#[path = "common/invite_mesh.rs"]
mod mesh;

use std::time::Duration;

use meshtown_core::members::access::record::{ensure, withdraw, AclKind, AclValue, Via};
use meshtown_core::node::RunningNode;
use meshtown_core::transport::net::{NodeSecret, PeerAddr, PeerId};
use meshtown_core::transport::net_mainline::unix_now;
use tokio::time::{sleep, timeout, Instant};

use managed::ManagedNet;

/// The external deadline of one wait.
const LIMIT: Duration = Duration::from_secs(20);

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

/// Waits until `node` recognizes `who` as a member.
async fn wait_member(node: &RunningNode, who: PeerId) {
    let deadline = Instant::now() + LIMIT;
    while Instant::now() < deadline {
        if node.members.is_member(who) {
            return;
        }
        sleep(Duration::from_millis(50)).await;
    }
    panic!("the handshake did not happen within {LIMIT:?}");
}

/// The creator writes its admission of `who`, with the given origin, into its own copy of the shared list.
async fn admit(root: &RunningNode, who: PeerId, via: Via) {
    let value = AclValue {
        kind: AclKind::Admit,
        subject: who,
        via,
        made_unix: unix_now(),
        note: String::new(),
    };
    ensure(root.domains.shared_replica().as_ref(), &value)
        .await
        .unwrap();
}

async fn refresh(a: &RunningNode) {
    timeout(LIMIT, a.access.refresh())
        .await
        .expect("recompute in time")
        .unwrap();
}

fn id_of(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

fn peers_of(nodes: &[&RunningNode]) -> Vec<PeerAddr> {
    nodes.iter().map(|n| PeerAddr::new(n.id)).collect()
}

/// The creator and members under `seeds`, all admitted by the creator and tied to it.
async fn network_of(net: &ManagedNet, seeds: &[u8]) -> (RunningNode, Vec<RunningNode>) {
    let root = net.start_root(net.base.clone()).await;
    for seed in seeds {
        admit(&root, id_of(*seed), Via::None).await;
    }
    refresh(&root).await;
    let mut members = Vec::new();
    for seed in seeds {
        members.push(net.start_member(*seed, peers_of(&[&root])).await);
    }
    for member in &members {
        wait_member(&root, member.id).await;
        wait_member(member, root.id).await;
    }
    (root, members)
}

async fn shut_all(nodes: Vec<RunningNode>) {
    for node in nodes {
        node.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_admission_by_the_creator_lets_a_newcomer_in_at_three_members() {
    // given: the creator and two members, and the creator's admission of a newcomer
    let net = ManagedNet::new();
    let (root, members) = network_of(&net, &[5, 6]).await;
    admit(&root, id_of(7), Via::Invite).await;
    refresh(&root).await;
    // when: the newcomer starts with the three as neighbours
    let mut near = peers_of(&[&root]);
    near.extend(members.iter().map(|m| PeerAddr::new(m.id)));
    let n = net.start_member(7, near).await;
    // then: it is let in at the creator and at both members
    wait_member(&root, n.id).await;
    for m in &members {
        wait_member(m, n.id).await;
    }
    shut_all([vec![n, root], members].concat()).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_network_of_two_admits_a_newcomer_on_the_creators_word() {
    // given: the creator and one member, and the creator's admission of a newcomer
    let net = ManagedNet::new();
    let (root, members) = network_of(&net, &[5]).await;
    admit(&root, id_of(8), Via::Invite).await;
    refresh(&root).await;
    // when: the newcomer starts
    let n = net.start_member(8, peers_of(&[&root, &members[0]])).await;
    // then: the member lets it in as well
    wait_member(&members[0], n.id).await;
    shut_all([vec![n, root], members].concat()).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_withdrawn_admission_cuts_the_newcomer_at_the_creator() {
    // given: a newcomer let in by the creator's admission
    let net = ManagedNet::new();
    let (root, members) = network_of(&net, &[5]).await;
    admit(&root, id_of(9), Via::Invite).await;
    refresh(&root).await;
    let n = net.start_member(9, peers_of(&[&root, &members[0]])).await;
    wait_member(&root, n.id).await;
    // when: the creator withdraws the admission
    let withdrawn = withdraw(root.domains.shared_replica().as_ref(), AclKind::Admit, n.id)
        .await
        .unwrap();
    assert!(withdrawn);
    refresh(&root).await;
    // then: the creator no longer lists the newcomer
    eventually("the creator drops the newcomer", || {
        !root.members.is_member(n.id)
    })
    .await;
    shut_all([vec![n, root], members].concat()).await;
}
