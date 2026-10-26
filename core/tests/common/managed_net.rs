//! Helpers for the loopback tests of a Managed network (`core/tests/member_invite_managed.rs`): the key and
//! the creator of the network, a creator node, a member that joined by an invite, redemption under a chosen
//! permanent key, and a wait for the invite verdict as the creator sees it.
//! Included via `#[path]` next to `invite_nodes.rs` and `invite_mesh.rs`, which it uses as `super::common`.
#![allow(dead_code)] // each test crate takes its own subset

use std::collections::BTreeSet;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use meshtown_core::invite::redeem::{redeem, Progress, RedeemError};
use meshtown_core::invite::store::{IssueVerdict, Ledger};
use meshtown_core::invite::{self, InviteHead};
use meshtown_core::members::network_key::{managed_key, managed_key_with_label, NetworkKey};
use meshtown_core::node::{self, NodeConfig, RunningNode};
use meshtown_core::transport::net::mem::process_net;
use meshtown_core::transport::net::{NodeSecret, PeerAddr, PeerId};
use meshtown_core::transport::net_mainline::unix_now;
use tokio::time::{sleep, timeout, Instant};

use super::common::{quick_opts, wait_until, ENTRY_LIMIT_SECS};

/// The seed of the creator's permanent key.
pub(crate) const ROOT_SEED: u8 = 31;
/// How long a wait for a view or a replicated record may take.
pub(crate) const VIEW_LIMIT: Duration = Duration::from_secs(20);

/// The nodes of every test share one in-process network and the same permanent keys, so the tests of a file
/// run one at a time: a `ManagedNet` holds this lock until it is dropped.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// A Managed network: the base its key is derived from, the creator and the derived key.
pub(crate) struct ManagedNet {
    _turn: MutexGuard<'static, ()>,
    pub(crate) base: NetworkKey,
    pub(crate) root_secret: NodeSecret,
    pub(crate) root: PeerId,
    pub(crate) key: NetworkKey,
}

impl ManagedNet {
    pub(crate) fn new() -> ManagedNet {
        let base = NetworkKey::generate().unwrap();
        let root_secret = NodeSecret::from_bytes(&[ROOT_SEED; 32]);
        let root = root_secret.peer_id();
        let key = managed_key(&base.to_bytes(), &root);
        let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        ManagedNet {
            _turn,
            base,
            root_secret,
            root,
            key,
        }
    }

    /// The text of the network key in the raw form, with the label: what a person pastes into the key field.
    pub(crate) fn key_text(&self, label: &str) -> String {
        managed_key_with_label(Some(label), &self.base.to_bytes(), &self.root)
    }

    /// The config of a member started by hand: the permanent key of `seed`, the list consulted.
    pub(crate) fn member_config(&self, seed: u8) -> NodeConfig {
        let mut cfg = managed_config(self.key.clone(), self.root, self.base.clone());
        cfg.identity_secret = Some(NodeSecret::from_bytes(&[seed; 32]));
        cfg
    }

    /// A member under the permanent key of `seed`, with `peers` as the neighbours it dials.
    pub(crate) async fn start_member(&self, seed: u8, peers: Vec<PeerAddr>) -> RunningNode {
        let mut cfg = self.member_config(seed);
        cfg.initial_peers = peers;
        node::start_node(cfg).await.unwrap()
    }

    /// The config of the creator's node; it answers redemptions with `reply` (the base, unless a test wants a lie).
    pub(crate) fn root_config(&self, reply: NetworkKey) -> NodeConfig {
        let mut cfg = managed_config(self.key.clone(), self.root, reply);
        cfg.identity_secret = Some(self.root_secret.clone());
        cfg
    }

    /// The creator's node.
    pub(crate) async fn start_root(&self, reply: NetworkKey) -> RunningNode {
        node::start_node(self.root_config(reply)).await.unwrap()
    }
}

/// A loopback config that consults the membership list and runs the Managed rules.
fn managed_config(key: NetworkKey, root: PeerId, reply: NetworkKey) -> NodeConfig {
    let mut cfg = NodeConfig::for_loopback_test(key);
    cfg.open_list = false;
    cfg.net_root = root;
    cfg.reply_secret = reply;
    cfg
}

/// A node started the way a newcomer starts after a redemption: the key is derived from the raw answer by the
/// invite head, the peers are those of the invite.
pub(crate) async fn start_joined(
    head: &InviteHead,
    answer: &NetworkKey,
    seed: u8,
    peers: Vec<PeerAddr>,
) -> RunningNode {
    let mut cfg = managed_config(head.reply_key(answer), head.root, answer.clone());
    cfg.identity_secret = Some(NodeSecret::from_bytes(&[seed; 32]));
    cfg.initial_peers = peers;
    node::start_node(cfg).await.unwrap()
}

/// `redeem` under the permanent key of `seed`, with a ceiling in case of a hang.
pub(crate) async fn redeem_as(
    seed: u8,
    head: &InviteHead,
    peers: &[PeerAddr],
) -> Result<NetworkKey, RedeemError> {
    let transport = node::bind_ephemeral(Some(&process_net())).await.unwrap();
    let owner = NodeSecret::from_bytes(&[seed; 32]);
    let opts = quick_opts(head, ENTRY_LIMIT_SECS);
    let ignore = |_: Progress| {};
    let call = redeem(head, peers, &opts, &ignore, transport, &owner);
    timeout(Duration::from_secs(60), call)
        .await
        .expect("redeem did not return within 60 s")
}

/// Waits until the node's view lets it issue an invite (it is an admin and the view is published).
pub(crate) async fn wait_can_invite(node: &RunningNode) {
    let can = || invite::make(&super::mesh::source_of(node), None).is_ok();
    assert!(
        wait_until(VIEW_LIMIT, can).await,
        "the node did not become able to invite within {VIEW_LIMIT:?}"
    );
}

/// Waits until `node` is a member of both sides: it sees `other` and `other` sees it.
pub(crate) async fn wait_both_members(node: &RunningNode, other: &RunningNode) {
    let both = || node.members.is_member(other.id) && other.members.is_member(node.id);
    assert!(
        wait_until(VIEW_LIMIT, both).await,
        "the nodes did not become members of each other within {VIEW_LIMIT:?}"
    );
}

/// Polls the verdict `node` gives on the head's secret, counting only the records of `admins`.
pub(crate) async fn wait_verdict_among(
    node: &RunningNode,
    head: &InviteHead,
    admins: &[PeerId],
    want: IssueVerdict,
) -> bool {
    let set: BTreeSet<PeerId> = admins.iter().copied().collect();
    let ledger = Ledger::of(&node.domains);
    let hash = head.secret.hash_hex();
    let deadline = Instant::now() + VIEW_LIMIT;
    loop {
        if ledger.verdict(&hash, unix_now(), Some(&set)).await == want {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        sleep(Duration::from_millis(50)).await;
    }
}
