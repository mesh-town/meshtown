//! The fixture of the `GET /api/state` tests about the roles in the network (`network_block_tests.rs`,
//! `network_block_excluded_tests.rs`): a router over one slot whose node reports the roles of a chosen network.

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::Value;

use crate::members::access::authority::{Power, PowerIn};
use crate::members::access::record::Via;
use crate::members::access::tally::{tally, Exclusion, How, Ledger, TallyInput};
use crate::members::access::view::{AccessView, Verdicts};
use crate::members::membership::Members;
use crate::members::network_key::format_managed_key;
use crate::transport::net::{NodeSecret, PeerId};
use crate::web_gui::test_host::{config_with_key, send, slot_handle, test_app, FakeHost, SnapNode};
use crate::web_gui::{ActionError, LogPage, NetRoles, NodeControl, NodeSnapshot, PeerInfo};

/// The node that created a Managed network.
pub(super) const ROOT: u8 = 1;
/// The three nodes of every test network.
pub(super) const NODES: [u8; 3] = [1, 2, 3];

/// How this node left the member list.
#[derive(Clone, Copy)]
pub(super) enum Removal {
    Votes { by: u8 },
    Left,
}

/// The network as one node of it sees it.
pub(super) struct Net {
    me: u8,
    admins: Vec<u8>,
    removal: Option<Removal>,
}

impl Net {
    /// `admins` lists the administrators, the creator included.
    pub(super) fn new(me: u8, admins: &[u8]) -> Net {
        Net {
            me,
            admins: admins.to_vec(),
            removal: None,
        }
    }

    pub(super) fn removed(self, removal: Removal) -> Net {
        Net {
            removal: Some(removal),
            ..self
        }
    }
}

pub(super) struct Fixture {
    router: axum::Router,
}

/// One slot `main` that works with a network, over a node that reports `net`.
pub(super) fn setup(net: Net) -> Fixture {
    let config = config_with_key(Some(&key_text()));
    let node = RolesNode {
        inner: SnapNode::new(snapshot_of(&net)),
        roles: roles_of(&net),
    };
    let main = slot_handle("main", Arc::new(node), config);
    Fixture {
        router: test_app(FakeHost::new(vec![main])).0,
    }
}

impl Fixture {
    pub(super) async fn when_state(&self) -> Value {
        let (status, body) = send(&self.router, Method::GET, "/api/state", "").await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }
}

/// The key the slot works with: it names the creator.
fn key_text() -> String {
    let tail = crate::members::words::generate_tail().expect("a tail");
    format_managed_key(Some("work"), &tail, &peer(ROOT))
}

fn snapshot_of(net: &Net) -> NodeSnapshot {
    let mut snap = NodeSnapshot::default();
    snap.node.id_full = id_full(net.me);
    snap.peers = NODES.iter().map(|&n| peer_info(n, n == net.me)).collect();
    snap.members.me_excluded = net.removal.is_some();
    snap
}

fn peer_info(n: u8, is_self: bool) -> PeerInfo {
    PeerInfo {
        id: id_full(n)[..8].to_string(),
        id_full: id_full(n),
        name: Some(name(n)),
        is_self,
        online: true,
        ..Default::default()
    }
}

/// What the node publishes: the roles of the network, from the verdicts of a real tally.
fn roles_of(net: &Net) -> NetRoles {
    let members = Members::with_access(AccessView::open());
    NODES
        .iter()
        .for_each(|&n| members.set_name(peer(n), name(n)));
    let verdicts = Verdicts {
        me: peer(net.me),
        ledger: Arc::new(ledger_of(net)),
        blocks: Default::default(),
    };
    NetRoles::of(&peer(ROOT), &verdicts, &members)
}

fn ledger_of(net: &Net) -> Ledger {
    let mut ledger = managed_ledger(&net.admins);
    if let Some(removal) = net.removal {
        ledger
            .excluded
            .insert(peer(net.me), exclusion(net.me, removal));
    }
    ledger
}

fn exclusion(me: u8, removal: Removal) -> Exclusion {
    let (how, voter) = match removal {
        Removal::Votes { by } => (How::Votes, by),
        Removal::Left => (How::Left, me),
    };
    Exclusion {
        how,
        at_micros: 1_792_400_000_000_000,
        voters: vec![peer(voter)],
    }
}

/// A network of the creator `ROOT` where everyone is admitted and the `admins` but the creator are granted the role.
fn managed_ledger(admins: &[u8]) -> Ledger {
    let rest = || NODES.iter().filter(|&&n| n != ROOT);
    let mut powers: Vec<PowerIn> = rest()
        .map(|&n| power(Power::Admit(Via::Invite), n, 1))
        .collect();
    let granted = admins.iter().filter(|&&n| n != ROOT);
    powers.extend(granted.map(|&n| power(Power::Grant, n, 2)));
    tally(&TallyInput {
        me: peer(ROOT),
        root: peer(ROOT),
        votes: vec![],
        powers,
    })
}

/// A record by the creator.
fn power(power: Power, subject: u8, at: u64) -> PowerIn {
    PowerIn {
        author: peer(ROOT),
        power,
        subject: peer(subject),
        ts_micros: at * 1_000_000,
    }
}

/// A `SnapNode` that also answers what this node may do in its network.
struct RolesNode {
    inner: Arc<SnapNode>,
    roles: NetRoles,
}

impl NodeControl for RolesNode {
    fn snapshot(&self) -> NodeSnapshot {
        self.inner.snapshot()
    }
    fn log_after(&self, after: u64, limit: usize, net: Option<&str>) -> LogPage {
        self.inner.log_after(after, limit, net)
    }
    fn find_exit(&self, domain: &str) -> Result<(), ActionError> {
        self.inner.find_exit(domain)
    }
    fn check_domain(&self, domain: &str) -> Result<(), ActionError> {
        self.inner.check_domain(domain)
    }
    fn check_all(&self) -> Result<u64, ActionError> {
        self.inner.check_all()
    }
    fn add_domain(&self, domain: &str) -> Result<String, ActionError> {
        self.inner.add_domain(domain)
    }
    fn remove_domain(&self, domain: &str) -> Result<String, ActionError> {
        self.inner.remove_domain(domain)
    }
    fn add_peer_address(&self, line: &str) -> Result<(), String> {
        self.inner.add_peer_address(line)
    }
    fn apply_config(
        &self,
        settings: &crate::config::config_file::Settings,
        paths: &[&'static str],
    ) {
        self.inner.apply_config(settings, paths)
    }
    fn switch_network(&self, key: &str) {
        self.inner.switch_network(key)
    }
    fn stop(&self) {
        self.inner.stop()
    }
    fn net_roles(&self) -> Option<NetRoles> {
        Some(self.roles.clone())
    }
}

fn peer(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

pub(super) fn id_full(n: u8) -> String {
    peer(n).to_string()
}

pub(super) fn name(n: u8) -> String {
    format!("node-{n}")
}
