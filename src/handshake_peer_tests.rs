//! Handshake tests on `PeerConn` (`handshake.rs`): both sides are real
//! `initiate`/`respond` over a `mem` pair. Parsing of a broken protocol (a raw initiator) is
//! in the child `handshake_peer_wire_tests.rs`. Pure functions are in `handshake_tests.rs`.

use super::*;
use crate::access::view::AccessView;
use crate::membership::{MemberContext, MemberEvent, Members};
use crate::net::contract::{pid, secret, must_finish, SEED_A, SEED_B, STEP};
use crate::net::mem::conn::MemConn;
use crate::net::{MemNet, PeerConn, PeerId, SplitTransport, Transport, AUTH};
use crate::transfer_probe::ProbeSettings;
use bytes::Bytes;
use std::sync::atomic::AtomicUsize;
use portable_atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

/// Shared network key of the two test nodes; the "foreign" responder uses `OTHER_KEY`.
const KEY: [u8; 32] = [7; 32];
const OTHER_KEY: [u8; 32] = [8; 32];
/// Names: A is the initiator (dials), B is the responder.
const NAME_A: &str = "厨房🍳";
const NAME_B: &str = "Garage";

mod same_key {
    use super::*;

    #[tokio::test]
    async fn mem_pair_handshake_ok_both_sides() {
        // given: A and B with the same network key and different names
        let mut fx = setup(Opts::default()).await;
        // when: A initiates, B responds
        let (initiated, responded) = fx.when_handshaking().await;
        // then: both sides are Ok
        assert!(initiated.is_ok(), "initiate: {initiated:?}");
        assert!(responded.is_ok(), "respond: {responded:?}");
        // then: each side counted the peer as a member and recorded its name from the tail
        let (a, b) = (fx.ini.ctx.own_id, fx.res.ctx.own_id);
        assert!(fx.ini.ctx.members.is_member(b), "A did not add B");
        assert!(fx.res.ctx.members.is_member(a), "B did not add A");
        assert_eq!(fx.ini.ctx.members.name_of(b), Some(NAME_B.to_string()));
        assert_eq!(fx.res.ctx.members.name_of(a), Some(NAME_A.to_string()));
        // then: each side has an Up event with the right peer and its own connection
        fx.ini.then_up_for(b);
        fx.res.then_up_for(a);
    }
}

mod wrong_key {
    use super::*;

    #[tokio::test]
    async fn mem_wrong_key_closes_401() {
        // given: B has a different network key
        let mut fx = setup(Opts { wrong_key: true }).await;
        // when: A initiates, B responds
        let (initiated, responded) = fx.when_handshaking().await;
        // then: both sides Err
        assert!(
            initiated.is_err(),
            "initiate with a foreign key must refuse"
        );
        assert!(responded.is_err(), "respond with a foreign key must refuse");
        // then: A rejected (it checks the responder's tag first) — closed with code 401 and a reason
        let expected = Some((401, Bytes::from_static(b"bad secret")));
        assert_eq!(
            close_info(&fx.ini.conn),
            expected,
            "close code on the rejecting side"
        );
        // then: nobody became a member, no Up events
        assert!(!fx.ini.ctx.members.is_member(fx.res.ctx.own_id), "A added B");
        assert!(!fx.res.ctx.members.is_member(fx.ini.ctx.own_id), "B added A");
        assert!(fx.ini.events.try_recv().is_err(), "A has an event");
        assert!(fx.res.events.try_recv().is_err(), "B has an event");
    }
}

#[path = "handshake_peer_wire_tests.rs"]
mod malformed_initiator;

#[path = "handshake_access_tests.rs"]
mod access_list;

mod as_service {
    use super::*;

    #[tokio::test]
    async fn service_hold_until_close() {
        // given: B serves the incoming connection with the AUTH service, A initiates
        let mut fx = setup(Opts::default()).await;
        let serving = tokio::spawn(handshake_service(fx.res.ctx.clone()).handle(fx.res.conn.clone()));
        must_finish(initiate(&fx.ini.conn, &fx.ini.ctx))
            .await
            .expect("initiate");
        // then: after success B's connection is in the registry
        fx.res.then_conn_tracked().await;
        // when: A closes the connection
        fx.ini.conn.close(0, b"bye");
        // then: the record is gone, B got Up and then Down of peer A, the service finished
        fx.res.then_conn_untracked_after_down(fx.ini.ctx.own_id).await;
        let ended = must_finish(serving).await;
        assert!(ended.is_ok(), "service task failed: {ended:?}");
    }
}

// ---- helpers ----

#[derive(Default)]
struct Opts {
    /// Responder B has a different network key.
    wrong_key: bool,
}

/// One side of the pair: its own connection, member context and the queue of its events.
struct Side {
    conn: Arc<dyn PeerConn>,
    ctx: MemberContext,
    events: mpsc::UnboundedReceiver<MemberEvent>,
}

struct Fixture {
    ini: Side,
    res: Side,
    /// The transports keep the `mem` network alive until the end of the test.
    _nodes: [Arc<SplitTransport>; 2],
}

/// A pair of `mem` connections: A dialed (initiator), B accepted (responder); each has its own context.
async fn setup(opts: Opts) -> Fixture {
    let net = MemNet::new();
    let a = net.transport(&secret(SEED_A));
    let b = net.transport(&secret(SEED_B));
    b.serve(&[AUTH]);
    let (dialed, accepted) = tokio::join!(must_finish(a.connect(&b.local_addr(), AUTH)), must_finish(b.accept()));
    let dialed = dialed.expect("connect A→B gave no connection");
    let (_, accepted) = accepted.expect("accept B is closed, though a connection was awaited");
    let key_b = if opts.wrong_key { OTHER_KEY } else { KEY };
    Fixture {
        ini: side(dialed, member_context(pid(SEED_A), KEY, NAME_A)),
        res: side(accepted, member_context(pid(SEED_B), key_b, NAME_B)),
        _nodes: [a, b],
    }
}

fn side(
    conn: Arc<dyn PeerConn>,
    (ctx, events): (MemberContext, mpsc::UnboundedReceiver<MemberEvent>),
) -> Side {
    Side { conn, ctx, events }
}

/// Member context without live probes: the first probe is in an hour.
fn member_context(
    me: PeerId,
    key: [u8; 32],
    name: &str,
) -> (MemberContext, mpsc::UnboundedReceiver<MemberEvent>) {
    let (events, rx) = mpsc::unbounded_channel();
    let probe = ProbeSettings {
        first_delay: Duration::from_secs(3600),
        interval_secs: Arc::new(AtomicU64::new(3600)),
        ok_probe_count: Arc::new(AtomicUsize::new(0)),
        exits: watch::channel(Vec::new()).1,
        full_cycle: false,
    };
    let name = watch::channel(name.to_string()).0;
    (
        MemberContext {
            own_id: me,
            handshake_key: key,
            members: Members::default(),
            events,
            probe,
            own_name_tx: name,
        },
        rx,
    )
}

/// Like `member_context`, but the members answer to the membership list behind `view`.
fn member_context_with(
    me: PeerId,
    key: [u8; 32],
    name: &str,
    view: AccessView,
) -> (MemberContext, mpsc::UnboundedReceiver<MemberEvent>) {
    let (mut ctx, events) = member_context(me, key, name);
    ctx.members = Members::with_access(view);
    (ctx, events)
}

/// Like `setup` with the same key, but A's list is `ini_view` and B's list is `res_view`.
async fn setup_with_views(ini_view: AccessView, res_view: AccessView) -> Fixture {
    let fx = setup(Opts::default()).await;
    Fixture {
        ini: side(fx.ini.conn.clone(), member_context_with(pid(SEED_A), KEY, NAME_A, ini_view)),
        res: side(fx.res.conn.clone(), member_context_with(pid(SEED_B), KEY, NAME_B, res_view)),
        _nodes: fx._nodes,
    }
}

impl Fixture {
    /// A initiates, B responds — simultaneously, each step at most `STEP`.
    async fn when_handshaking(&self) -> (anyhow::Result<()>, anyhow::Result<()>) {
        tokio::join!(
            must_finish(initiate(&self.ini.conn, &self.ini.ctx)),
            must_finish(respond(&self.res.conn, &self.res.ctx)),
        )
    }

    /// Only the responder runs: the initiator does nothing; wait for the outcome for at most `limit`.
    async fn when_responding_alone(&self, limit: Duration) -> anyhow::Result<()> {
        let done = tokio::time::timeout(limit, respond(&self.res.conn, &self.res.ctx)).await;
        done.expect("respond did not return in time")
    }
}

impl Side {
    /// Waits for Up (already queued) and checks the peer and the connection it carries.
    fn then_up_for(&mut self, peer: PeerId) {
        match self.events.try_recv() {
            Ok(MemberEvent::Up { peer: got, conn }) => {
                assert_eq!(got, peer, "Up named another peer");
                assert_eq!(
                    conn.stable_id(),
                    self.conn.stable_id(),
                    "Up carries a foreign connection"
                );
            }
            Ok(MemberEvent::Down { .. }) => panic!("expected Up, got Down"),
            Err(e) => panic!("expected Up, the queue is empty: {e}"),
        }
    }

    /// The side's connection in `conns_snapshot` (waits up to `STEP`: `hold_conn_until_closed` registers it after Up).
    async fn then_conn_tracked(&self) {
        let tracked = async {
            while self
                .ctx
                .members
                .conns_snapshot()
                .iter()
                .all(|c| c.stable_id() != self.conn.stable_id())
            {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        };
        let done = tokio::time::timeout(STEP, tracked).await;
        assert!(
            done.is_ok(),
            "connection did not reach conns_snapshot within {STEP:?}"
        );
    }

    /// Up arrives, then Down of peer `peer`; by Down the `conns_snapshot` record is already removed.
    async fn then_conn_untracked_after_down(&mut self, peer: PeerId) {
        self.then_up_for(peer);
        match must_finish(self.events.recv()).await {
            Some(MemberEvent::Down { peer: got, .. }) => {
                assert_eq!(got, peer, "Down named another peer")
            }
            Some(MemberEvent::Up { .. }) => panic!("expected Down, got a second Up"),
            None => panic!("expected Down, the event channel is closed"),
        }
        assert!(
            self.ctx.members.conns_snapshot().is_empty(),
            "a connection remained in the registry after Down"
        );
    }
}

/// Code and reason of closing a `mem` connection; `None` — not closed.
fn close_info(conn: &Arc<dyn PeerConn>) -> Option<(u32, Bytes)> {
    let mem = conn.as_any().downcast_ref::<MemConn>();
    mem.expect("connection must be mem").close_info()
}
