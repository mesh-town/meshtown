//! Tests of dialing through `NetStack` (`node/dial.rs`): `PeerDialer` over the `mem` transport (dial by key). The state of pauses is in
//! `redial_tests.rs`; here only the decision "whether to start a dial" and the failure text.

use super::*;
use crate::members::membership::{MemberContext, MemberEvent, Members};
use crate::node::start::transports_mem::KeyedMem;
use crate::transport::net::contract::{must_finish, must_finish_within, pid, SEED_A, SEED_B};
use crate::transport::net::mem::MemNet;
use crate::transport::net::PeerAddr;
use crate::transport::net::{NetStack, NetStackBuilder, NodeSecret, Transport, AUTH};
use crate::transport::transfer_probe::ProbeSettings;
use portable_atomic::AtomicU64;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

/// How long to wait to see whether a wrong dial or failure shows up that shouldn't.
const QUIET: Duration = Duration::from_millis(300);
/// Time limit for a failure or for the peer to accept the dial.
const FAILURE_LIMIT: Duration = Duration::from_secs(20);

mod transport_closed {
    use super::*;

    #[tokio::test]
    async fn closed_stack_no_redial() {
        // given: node A's stack is closed (stop or network switch)
        let mut fx = setup(Opts {
            transport_closed: true,
        })
        .await;
        // when: member B's address is found
        fx.when_found_b();
        // then: no dial started, no failure came to the channel
        fx.then_no_dial_and_no_failure().await;
    }
}

mod key_only_address {
    use super::*;

    #[tokio::test]
    async fn found_without_hints_dials() {
        // given: peer B is alive in the mem network and accepts; the address has only a key
        let mut fx = setup(Opts::default()).await;
        // when: B's address is found without hints
        fx.when_found_b_key_only();
        // then: B received an incoming connection
        let got = must_finish_within(FAILURE_LIMIT, fx.peer_b.accept()).await;
        assert!(got.is_some(), "the dial by key didn't reach B");
        fx.close().await;
    }
}

mod unreachable_address {
    use super::*;

    #[tokio::test]
    async fn found_unreachable_skipped() {
        // given: B is not in the network (the slot is closed)
        let mut fx = setup(Opts::default()).await;
        fx.peer_b.close().await;
        // when: B's address is found
        fx.when_found_b_key_only();
        // then: no dial and no failure
        fx.then_no_dial_and_no_failure().await;
        fx.close().await;
    }
}

mod down_reason {
    use super::*;

    #[tokio::test]
    async fn down_for_net_change_redials_without_pause() {
        let mut fx = setup(Opts::default()).await;
        fx.dialer.on_peer_down(
            pid(SEED_B),
            &format!("code 4294901763: {}", crate::transport::net::NET_CHANGED),
        );
        let due = fx.dialer.next_redial_at().expect("attempt scheduled");
        assert!(
            due <= Instant::now(),
            "after a network change the retry has no pause"
        );
        fx.close().await;
    }

    #[tokio::test]
    async fn down_for_silence_keeps_first_step() {
        let mut fx = setup(Opts::default()).await;
        fx.dialer
            .on_peer_down(pid(SEED_B), "code 4294901762: no frames for too long");
        let due = fx.dialer.next_redial_at().expect("attempt scheduled");
        assert!(
            due > Instant::now() + Duration::from_secs(1),
            "silence - the usual first pause"
        );
        fx.close().await;
    }
}

mod refused_by_us {
    use super::*;

    #[tokio::test]
    async fn own_refusal_with_access_open_redials_at_once() {
        // given: our list lets B in (the default list is open), so a refusal by it is a stale verdict
        let mut fx = setup(Opts::default()).await;

        // when: a dial of B fails because our own list refused it
        fx.dialer.on_dial_failed(DialFailed {
            peer: pid(SEED_B),
            reason: "refused".into(),
            refusal: DialRefusal::ByUs(Deny::Excluded),
        });

        // then: the retry is due at once, not after the first step of the pause ladder
        let due = fx.dialer.next_redial_at().expect("attempt scheduled");
        assert!(
            due <= Instant::now() + Duration::from_secs(1),
            "the retry must not wait for the ladder"
        );
        fx.close().await;
    }
}

// ---- helpers ----

#[derive(Default)]
struct Opts {
    /// Closes the stack before `on_addr_found`.
    transport_closed: bool,
}

struct Fixture {
    dialer: PeerDialer,
    stack: NetStack,
    /// Peer B's transport in the same `mem` network; accepts the `AUTH` service.
    peer_b: Arc<dyn Transport>,
    members: Members,
    failed_rx: mpsc::UnboundedReceiver<DialFailed>,
    /// The event receiver keeps the channel open; there are no events in these tests.
    _events_rx: mpsc::UnboundedReceiver<MemberEvent>,
}

/// The `PeerDialer` of node A (seed A) over a stack on `mem`; B is the member we look for.
async fn setup(opts: Opts) -> Fixture {
    let net = MemNet::new();
    let secret_a = NodeSecret::from_bytes(&[SEED_A; 32]);
    let a: Arc<dyn Transport> = KeyedMem::new(&net, &secret_a);
    let stack = NetStackBuilder::new(secret_a)
        .transport(a)
        .build()
        .expect("stack built");
    let peer_b = net.transport(&NodeSecret::from_bytes(&[SEED_B; 32]));
    peer_b.serve(&[AUTH]);
    if opts.transport_closed {
        must_finish(stack.close()).await;
    }
    let (ctx, events_rx) = member_context(pid(SEED_A));
    let members = ctx.members.clone();
    let (failed_tx, failed_rx) = mpsc::unbounded_channel();
    let dialer = PeerDialer::new(pid(SEED_A), stack.clone(), ctx, failed_tx);
    Fixture {
        dialer,
        stack,
        peer_b,
        members,
        failed_rx,
        _events_rx: events_rx,
    }
}

fn member_context(me: PeerId) -> (MemberContext, mpsc::UnboundedReceiver<MemberEvent>) {
    let (events, rx) = mpsc::unbounded_channel();
    let probe = ProbeSettings {
        first_delay: Duration::from_secs(3600),
        interval_secs: Arc::new(AtomicU64::new(3600)),
        ok_probe_count: Arc::new(AtomicUsize::new(0)),
        exits: watch::channel(Vec::new()).1,
        full_cycle: false,
    };
    let name = watch::channel("A".to_string()).0;
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

impl Fixture {
    /// B's address with the hints the network knows (a live slot).
    fn when_found_b(&mut self) {
        self.dialer.on_addr_found(PeerAddr::new(pid(SEED_B)));
    }

    /// B's address without a single hint — the key found by `hyperdht`.
    fn when_found_b_key_only(&mut self) {
        self.dialer.on_addr_found(PeerAddr::new(pid(SEED_B)));
    }

    /// No dial started (the "dial in progress" mark is free) and no failure came within `QUIET`.
    async fn then_no_dial_and_no_failure(&mut self) {
        tokio::time::sleep(QUIET).await;
        let failure = self.failed_rx.try_recv();
        assert!(
            failure.is_err(),
            "a dial failure came to the channel: {:?}",
            failure.map(|f| f.reason)
        );
        assert!(
            self.members.try_begin_dial(pid(SEED_B)),
            "B's dial started although the transport is closed"
        );
    }

    async fn close(self) {
        must_finish(self.stack.close()).await;
    }
}
