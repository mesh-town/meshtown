//! Tests of network members and gossip events (`membership.rs`).

use super::*;
use crate::net::NodeSecret;

#[test]
fn ids_lists_current_members() {
    let members = Members::default();
    let a = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    let b = NodeSecret::from_bytes(&[2u8; 32]).peer_id();
    assert!(members.ids().is_empty());
    members.add_verified_conn(a);
    members.add_verified_conn(b);
    members.add_verified_conn(b);
    assert_eq!(members.ids(), [a, b].into_iter().collect::<HashSet<_>>());
    // The second connection to b is still alive, the member stays.
    members.remove_verified_conn(b);
    assert_eq!(members.ids(), [a, b].into_iter().collect::<HashSet<_>>());
    members.remove_verified_conn(b);
    assert_eq!(members.ids(), [a].into_iter().collect::<HashSet<_>>());
}

#[test]
fn names_survive_leave() {
    let members = Members::default();
    let id = NodeSecret::from_bytes(&[3u8; 32]).peer_id();
    assert_eq!(members.peer_label(id), short_peer_id(&id));
    members.set_name(id, "Kitchen".to_string());
    members.add_verified_conn(id);
    members.remove_verified_conn(id);
    assert_eq!(members.name_of(id), Some("Kitchen".to_string()));
    assert!(members.peer_label(id).ends_with("\"Kitchen\""));
}

#[test]
fn names_copies_known_names() {
    let members = Members::default();
    let a = NodeSecret::from_bytes(&[4u8; 32]).peer_id();
    let b = NodeSecret::from_bytes(&[5u8; 32]).peer_id();
    members.set_name(a, "Kitchen".to_string());
    let names = members.names();
    assert_eq!(names.get(&a), Some(&"Kitchen".to_string()));
    assert!(!names.contains_key(&b));
}

// ---- connection bookkeeping and the drop reason on PeerConn ----

use crate::net::contract::{secret, must_finish, SEED_A, SEED_B, STEP};
use crate::net::{MemNet, PeerAddr, PeerConn, SplitTransport, Transport, AUTH};
use crate::transfer_probe::ProbeSettings;
use std::sync::atomic::AtomicUsize;
use portable_atomic::AtomicU64;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

mod two_conns_to_one_peer {
    use super::*;

    #[tokio::test]
    async fn two_conns_one_peer_two_keys() {
        // given: two `mem` connections A→B (one peer)
        let pair = dial_two_conns().await;
        let members = Members::default();
        // when: both are registered
        members.track_conn(&pair.first);
        members.track_conn(&pair.second);
        // then: the snapshot has two different u64 keys
        let keys: HashSet<u64> = members
            .conns_snapshot()
            .iter()
            .map(|c| c.stable_id())
            .collect();
        assert_eq!(keys.len(), 2, "stable_id keys: {keys:?}");
        // then: the status line counts both connections
        let text = members.conns_text();
        let rest = text
            .strip_prefix("connections 2, paths ")
            .unwrap_or_else(|| panic!("line: {text}"));
        rest.parse::<usize>().unwrap_or_else(|_| panic!("line: {text}"));
        pair.close().await;
    }

    #[tokio::test]
    async fn untrack_removes_only_own_conn() {
        // given: two `mem` connections to one peer, both registered
        let pair = dial_two_conns().await;
        let members = Members::default();
        members.track_conn(&pair.first);
        members.track_conn(&pair.second);
        // when: the first is closed and unregistered
        pair.first.close(0, b"done");
        members.untrack_conn(&pair.first);
        // then: the second remains registered, and only it
        let left = members.conns_snapshot();
        assert_eq!(left.len(), 1, "{} connections registered", left.len());
        assert_eq!(
            left[0].stable_id(),
            pair.second.stable_id(),
            "the wrong connection remained"
        );
        assert!(
            members.conns_text().starts_with("connections 1, "),
            "line: {}",
            members.conns_text()
        );
        pair.close().await;
    }
}

mod down_reason {
    use super::*;

    #[tokio::test]
    async fn down_reason_on_mem_is_empty() {
        // given: a mem pair; B holds the connection, A is its member
        let mem = mem_pair().await;
        let (ctx, mut events) = member_context(mem.near.peer());
        let peer = mem.far.peer();
        ctx.members.add_verified_conn(peer);
        let holding = tokio::spawn(hold_conn_until_closed(mem.far.clone(), peer, ctx.clone()));
        tracked(&ctx.members, 1).await;
        // when: A closes the connection
        mem.near.close(401, b"bad secret");
        // then: the reason is empty, Down came anyway
        assert_eq!(down_reason(&mut events).await, "", "drop reason of mem");
        must_finish(holding).await.expect("hold");
    }
}

#[path = "membership_access_tests.rs"]
mod access_verdicts;

// ---- helpers ----

/// Member context without live probes (the first probe is in an hour); `me` is the id of the context owner.
fn member_context(me: PeerId) -> (MemberContext, mpsc::UnboundedReceiver<MemberEvent>) {
    let (events, rx) = mpsc::unbounded_channel();
    let probe = ProbeSettings {
        first_delay: Duration::from_secs(3600),
        interval_secs: Arc::new(AtomicU64::new(3600)),
        ok_probe_count: Arc::new(AtomicUsize::new(0)),
        exits: watch::channel(Vec::new()).1,
        full_cycle: false,
    };
    let name = watch::channel("node".to_string()).0;
    (
        MemberContext {
            own_id: me,
            handshake_key: [7; 32],
            members: Members::default(),
            events,
            probe,
            own_name_tx: name,
        },
        rx,
    )
}

/// Waits up to `STEP` until `count` connections are registered.
async fn tracked(members: &Members, count: usize) {
    let polling = async {
        while members.conns_snapshot().len() != count {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let done = tokio::time::timeout(STEP, polling).await;
    assert!(
        done.is_ok(),
        "registered connections did not reach {count} within {STEP:?}"
    );
}

/// Waits for a `Down` event and returns its reason; `Up` or a closed channel is a panic.
async fn down_reason(events: &mut mpsc::UnboundedReceiver<MemberEvent>) -> String {
    match must_finish(events.recv()).await {
        Some(MemberEvent::Down { reason, .. }) => reason,
        Some(MemberEvent::Up { .. }) => panic!("expected Down, got Up"),
        None => panic!("expected Down, the event channel is closed"),
    }
}

/// Two `mem` connections A→B on one pair of nodes; the B sides and nodes live until `close`.
struct TwoConns {
    first: Arc<dyn PeerConn>,
    second: Arc<dyn PeerConn>,
    /// Accepted ends: the last handle would close the connection.
    _accepted: Vec<Arc<dyn PeerConn>>,
    nodes: [Arc<SplitTransport>; 2],
}

async fn dial_two_conns() -> TwoConns {
    let net = MemNet::new();
    let a = net.transport(&secret(SEED_A));
    let b = net.transport(&secret(SEED_B));
    b.serve(&[AUTH]);
    let addr = b.local_addr();
    let (first, far_first) = dial_once(&a, &b, &addr).await;
    let (second, far_second) = dial_once(&a, &b, &addr).await;
    TwoConns {
        first,
        second,
        _accepted: vec![far_first, far_second],
        nodes: [a, b],
    }
}

/// One connection A→B: the dialed and the accepted ends.
async fn dial_once(
    a: &SplitTransport,
    b: &SplitTransport,
    addr: &PeerAddr,
) -> (Arc<dyn PeerConn>, Arc<dyn PeerConn>) {
    let (dialed, accepted) = tokio::join!(must_finish(a.connect(addr, AUTH)), must_finish(b.accept()));
    let (_, far) = accepted.expect("accept B is closed, though a connection was awaited");
    (dialed.expect("connect A→B gave no connection"), far)
}

impl TwoConns {
    async fn close(self) {
        self.first.close(0, b"done");
        self.second.close(0, b"done");
        for node in &self.nodes {
            must_finish(node.close()).await;
        }
    }
}

/// A pair of `mem` connections: `near` dialed (its `peer()` is B), `far` accepted (its `peer()` is A).
struct MemPair {
    near: Arc<dyn PeerConn>,
    far: Arc<dyn PeerConn>,
    _nodes: [Arc<SplitTransport>; 2],
}

async fn mem_pair() -> MemPair {
    let net = MemNet::new();
    let a = net.transport(&secret(SEED_A));
    let b = net.transport(&secret(SEED_B));
    b.serve(&[AUTH]);
    let (dialed, accepted) = tokio::join!(must_finish(a.connect(&b.local_addr(), AUTH)), must_finish(b.accept()));
    let (_, far) = accepted.expect("accept mem is closed");
    MemPair {
        near: dialed.expect("connect mem gave no connection"),
        far,
        _nodes: [a, b],
    }
}
