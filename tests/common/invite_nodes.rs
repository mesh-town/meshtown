//! Helpers for end-to-end tests of the one-time invite (`tests/invite_once_loopback.rs`): loopback
//! nodes, issuing a record, joining by a client with short pauses, polling waits with a ceiling.
//! Included via `#[path]`, like the rest of `tests/common/*`.
#![allow(dead_code)] // each test crate takes its own subset

use std::time::Duration;

use meshtown::net_mainline::unix_now;
use meshtown::invite::redeem::{redeem, Progress, RedeemError, RedeemOpts};
use meshtown::invite::store::{issue_record, Ledger, IssueVerdict};
use meshtown::invite::{InviteHead, InviteSecret};
use meshtown::net::{NodeSecret, PeerAddr, PeerId};
use meshtown::net::mem::process_net;
use meshtown::network_key::NetworkKey;
use meshtown::node::{self, NodeConfig, RunningNode};
use tokio::time::{sleep, timeout, Instant};

/// How many seconds the client tries to join in a test where joining must succeed.
pub(crate) const ENTRY_LIMIT_SECS: u64 = 20;
pub(crate) const HOUR: u64 = 3600;
/// The seed of the joining node's permanent key in `redeem_via`.
pub(crate) const OWNER_SEED: u8 = 5;

/// Node A of the network and everything the tests need about it.
pub(crate) struct Rig {
    pub(crate) key: NetworkKey,
    pub(crate) a: RunningNode,
    pub(crate) addr: PeerAddr,
}

pub(crate) async fn setup() -> Rig {
    let key = NetworkKey::generate().unwrap();
    let a = start(&key).await;
    let addr = addr_of(&a);
    Rig { key, a, addr }
}

/// `setup` plus a control: the node admits by the right address. Without it a refusal under someone else's id would prove
/// nothing: a server that doesn't accept ALPN would refuse the same way.
pub(crate) async fn setup_serving() -> Rig {
    let rig = setup().await;
    let control = rig.issue(unix_now()).await;
    rig.redeem(&control).await.expect("control: the server must admit by the right address");
    rig
}

impl Rig {
    /// A new network head of the node; the issue record is written with time `made_unix` (the head is "now").
    pub(crate) async fn issue(&self, made_unix: u64) -> InviteHead {
        let head = fresh_head(&self.key, unix_now());
        record_issue(&self.a, &head, made_unix).await;
        head
    }

    pub(crate) async fn redeem(&self, head: &InviteHead) -> Result<NetworkKey, RedeemError> {
        redeem_via(head, std::slice::from_ref(&self.addr), quick_opts(head, ENTRY_LIMIT_SECS)).await
    }

    pub(crate) fn ledger(&self) -> Ledger {
        Ledger::of(&self.a.domains)
    }

    pub(crate) async fn verdict(&self, head: &InviteHead) -> IssueVerdict {
        self.ledger().verdict(&head.secret.hash_hex(), unix_now()).await
    }
}

pub(crate) async fn start(key: &NetworkKey) -> RunningNode {
    node::start_node(NodeConfig::for_loopback_test(key.clone())).await.unwrap()
}

pub(crate) fn fresh_head(key: &NetworkKey, made_unix: u64) -> InviteHead {
    InviteHead {
        secret: InviteSecret::generate().unwrap(),
        network_id: key.subkeys().network_id,
        label: None,
        made_unix,
    }
}

pub(crate) async fn record_issue(node: &RunningNode, head: &InviteHead, made_unix: u64) {
    let ledger = Ledger::of(&node.domains);
    ledger.issue(&head.secret.hash_hex(), &issue_record(made_unix, None)).await.unwrap();
}

/// A node's address in the `mem` network: its key is given by `KeyedMem`; `id` is any (someone else's id under a peer's address
/// won't be found, but a node with such an id exists in the network — for the check "not the one that was called").
pub(crate) fn loopback_addr(id: PeerId, _node: &RunningNode) -> PeerAddr {
    PeerAddr::new(id)
}

pub(crate) fn addr_of(node: &RunningNode) -> PeerAddr {
    PeerAddr::new(node.id)
}

pub(crate) fn stranger_id() -> PeerId {
    NodeSecret::from_bytes(&[42; 32]).peer_id()
}

/// Pauses of 100–400 ms instead of 5–60 s, the limit is `limit_secs` from now: broken code doesn't hang for a day.
pub(crate) fn quick_opts(head: &InviteHead, limit_secs: u64) -> RedeemOpts {
    RedeemOpts {
        first_pause: Duration::from_millis(100),
        max_pause: Duration::from_millis(400),
        deadline_unix: unix_now() + limit_secs,
        ..RedeemOpts::for_head(head, false)
    }
}

/// The same with a 100 ms pause on all rounds: in a short time several attempts fit.
pub(crate) fn short_opts(head: &InviteHead, limit_secs: u64) -> RedeemOpts {
    RedeemOpts { max_pause: Duration::from_millis(100), ..quick_opts(head, limit_secs) }
}

fn ignore(_: Progress) {}

/// `redeem` with a ceiling in case of a hang: the client's limit must return an answer earlier.
/// The transport is a temporary node in the process-wide `mem` network.
pub(crate) async fn redeem_via(
    head: &InviteHead,
    peers: &[PeerAddr],
    opts: RedeemOpts,
) -> Result<NetworkKey, RedeemError> {
    let transport = node::bind_ephemeral(Some(&process_net())).await.unwrap();
    let owner = NodeSecret::from_bytes(&[OWNER_SEED; 32]);
    let call = redeem(head, peers, &opts, &ignore, transport, &owner);
    timeout(Duration::from_secs(60), call).await.expect("redeem did not return within 60 s")
}

/// Polls the condition every 50 ms until it becomes true or the limit expires.
pub(crate) async fn wait_until(limit: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        sleep(Duration::from_millis(50)).await;
    }
}

/// Waits until the nodes become members of each other (otherwise the shared document reconciliation won't start).
pub(crate) async fn wait_members(a: &RunningNode, b: &RunningNode) {
    let both = || a.members.is_member(b.id) && b.members.is_member(a.id);
    assert!(wait_until(Duration::from_secs(10), both).await, "the nodes did not become members within 10 s");
}

/// Polls the node's verdict on the head's secret until it becomes `want` or the limit expires.
pub(crate) async fn wait_verdict(node: &RunningNode, head: &InviteHead, want: IssueVerdict, secs: u64) -> bool {
    let ledger = Ledger::of(&node.domains);
    let hash = head.secret.hash_hex();
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if ledger.verdict(&hash, unix_now()).await == want {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        sleep(Duration::from_millis(50)).await;
    }
}
