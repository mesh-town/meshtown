//! Admission by one confirmation on real nodes over loopback, with the list consulted (`open_list = false`):
//! a newcomer vouched for by one member is let in at the others, whatever the size of the network; withdrawing
//! the invitation cuts the newcomer off at the issuer.
//!
//! What it does: nodes share one network key and are tied together with `introduce` or `initial_peers`. Every
//! wait runs under an external deadline (`LIMIT`), so a hang fails the test instead of the run.
//! What it does not do: no web layer; the counting rules themselves belong to the unit tests.

#[path = "common/socks_pair.rs"]
mod common;

use std::time::Duration;

use meshtown::access::record::{ensure, withdraw, AclKind, AclValue, Via};
use meshtown::net::{NodeSecret, PeerAddr, PeerId};
use meshtown::net_mainline::unix_now;
use meshtown::network_key::NetworkKey;
use meshtown::node::{self, NodeConfig, RunningNode};
use tokio::time::{sleep, timeout};

use common::{introduce, wait_member};

/// The external deadline of one wait.
const LIMIT: Duration = Duration::from_secs(20);

async fn start_with(key: &NetworkKey, secret: Option<NodeSecret>, peers: Vec<PeerAddr>) -> RunningNode {
    let mut cfg = NodeConfig::for_loopback_test(key.clone());
    cfg.open_list = false;
    cfg.socks.enabled = false;
    cfg.identity_secret = secret;
    cfg.initial_peers = peers;
    node::start_node(cfg).await.unwrap()
}

async fn start(key: &NetworkKey) -> RunningNode {
    start_with(key, None, Vec::new()).await
}

/// Polls `cond` until it holds or `LIMIT` ends.
async fn eventually(what: &str, cond: impl Fn() -> bool) {
    let wait = async {
        while !cond() {
            sleep(Duration::from_millis(50)).await;
        }
    };
    timeout(LIMIT, wait).await.unwrap_or_else(|_| panic!("did not happen within {LIMIT:?}: {what}"));
}

/// `a` writes its admission of `b`, with the given origin, into its own copy of the shared list.
async fn write(a: &RunningNode, b: PeerId, via: Via) {
    let value = AclValue { kind: AclKind::Admit, subject: b, via, made_unix: unix_now(), note: String::new() };
    ensure(a.domains.shared_replica().as_ref(), &value).await.unwrap();
}

async fn refresh(a: &RunningNode) {
    timeout(LIMIT, a.access.refresh()).await.expect("recompute in time").unwrap();
}

/// Every pair of the nodes admits each other and meets.
async fn mesh(nodes: &[&RunningNode]) {
    for a in nodes {
        for b in nodes.iter().filter(|b| b.id != a.id) {
            write(a, b.id, Via::None).await;
        }
    }
    for a in nodes {
        refresh(a).await;
    }
    for a in nodes {
        for b in nodes.iter().filter(|b| b.id != a.id) {
            introduce(a, b).await;
        }
    }
    for a in nodes {
        for b in nodes.iter().filter(|b| b.id != a.id) {
            wait_member(a, b.id, LIMIT).await;
        }
    }
}

fn peers_of(nodes: &[&RunningNode]) -> Vec<PeerAddr> {
    nodes.iter().map(|n| PeerAddr::new(n.id)).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_vote_admits_in_a_network_of_three() {
    // given: three members and an invitation of a newcomer written by a
    let key = NetworkKey::generate().unwrap();
    let (a, b, c) = (start(&key).await, start(&key).await, start(&key).await);
    mesh(&[&a, &b, &c]).await;
    let secret = NodeSecret::from_bytes(&[7; 32]);
    let n_id = secret.peer_id();
    write(&a, n_id, Via::Invite).await;
    refresh(&a).await;
    // when: the newcomer starts with the three as neighbours
    let n = start_with(&key, Some(secret), peers_of(&[&a, &b, &c])).await;
    // then: the one vote of a lets it in at a, b and c
    wait_member(&a, n.id, LIMIT).await;
    wait_member(&b, n.id, LIMIT).await;
    wait_member(&c, n.id, LIMIT).await;
    for x in [n, a, b, c] {
        x.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn network_of_two_admits_with_one_vote() {
    // given: a network of two and an invitation written by a
    let key = NetworkKey::generate().unwrap();
    let (a, b) = (start(&key).await, start(&key).await);
    mesh(&[&a, &b]).await;
    let secret = NodeSecret::from_bytes(&[8; 32]);
    write(&a, secret.peer_id(), Via::Invite).await;
    refresh(&a).await;
    // when: the newcomer starts
    let n = start_with(&key, Some(secret), peers_of(&[&a, &b])).await;
    // then: b lets it in on the one vote
    wait_member(&b, n.id, LIMIT).await;
    for x in [n, a, b] {
        x.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn withdrawn_invite_cuts_the_newcomer_at_the_issuer() {
    // given: a network of two with the newcomer let in by a's invitation
    let key = NetworkKey::generate().unwrap();
    let (a, b) = (start(&key).await, start(&key).await);
    mesh(&[&a, &b]).await;
    let secret = NodeSecret::from_bytes(&[9; 32]);
    let n_id = secret.peer_id();
    write(&a, n_id, Via::Invite).await;
    refresh(&a).await;
    let n = start_with(&key, Some(secret), peers_of(&[&a, &b])).await;
    wait_member(&a, n.id, LIMIT).await;
    // when: a withdraws the invitation
    assert!(withdraw(a.domains.shared_replica().as_ref(), AclKind::Admit, n_id).await.unwrap());
    refresh(&a).await;
    // then: a no longer lists the newcomer
    eventually("a drops the newcomer", || !a.members.is_member(n.id)).await;
    for x in [n, a, b] {
        x.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn holder_redeem_in_a_network_of_three_needs_no_click() {
    // given: three members; the one that issued the invitation is stopped
    let key = NetworkKey::generate().unwrap();
    let (a, b, c) = (start(&key).await, start(&key).await, start(&key).await);
    mesh(&[&a, &b, &c]).await;
    a.shutdown().await;
    // when: b writes the invitation record for the newcomer, as the holder of the invitation key does
    let secret = NodeSecret::from_bytes(&[10; 32]);
    write(&b, secret.peer_id(), Via::Invite).await;
    refresh(&b).await;
    let n = start_with(&key, Some(secret), peers_of(&[&b, &c])).await;
    // then: b and c let the newcomer in on the one vote
    wait_member(&b, n.id, LIMIT).await;
    wait_member(&c, n.id, LIMIT).await;
    for x in [n, b, c] {
        x.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn holder_redeem_in_a_network_of_two_needs_no_click() {
    // given: a network of two with a stopped
    let key = NetworkKey::generate().unwrap();
    let (a, b) = (start(&key).await, start(&key).await);
    mesh(&[&a, &b]).await;
    a.shutdown().await;
    // when: b writes the invitation record for the newcomer
    let secret = NodeSecret::from_bytes(&[11; 32]);
    write(&b, secret.peer_id(), Via::Invite).await;
    refresh(&b).await;
    let n = start_with(&key, Some(secret), peers_of(&[&b])).await;
    // then: b lets it in
    wait_member(&b, n.id, LIMIT).await;
    n.shutdown().await;
    b.shutdown().await;
}
