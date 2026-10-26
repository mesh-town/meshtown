//! Tests of forgetting members cut by the prune (`Redial::forget`) and the long pause after a series of failures
//! (`redial_pause` from attempt 9). Synchronous, time is `Instant::now()` plus manual shifts.

use std::time::Duration;

use crate::transport::net::{NodeSecret, PeerId};
use tokio::time::Instant;

use super::*;

fn id(byte: u8) -> PeerId {
    NodeSecret::from_bytes(&[byte; 32]).peer_id()
}

mod pause {
    use super::*;

    #[test]
    fn attempt_8_still_capped_at_75_s() {
        assert_eq!(redial_pause(8, 0), Duration::from_secs(60));
        assert_eq!(redial_pause(8, u32::MAX), Duration::from_secs(75));
    }

    #[test]
    fn long_pause_from_attempt_9() {
        for attempt in [9, 10, 50, u32::MAX] {
            assert_eq!(
                redial_pause(attempt, 0),
                Duration::from_secs(600),
                "attempt {attempt}, rnd 0"
            );
            assert_eq!(
                redial_pause(attempt, u32::MAX),
                Duration::from_secs(750),
                "attempt {attempt}, rnd max"
            );
        }
    }

    #[test]
    fn down_restarts_series_with_short_pause() {
        let mut redial = Redial::default();
        let (_, long) = (0..9)
            .map(|_| redial.failed(id(1), Instant::now(), 0, false))
            .last()
            .unwrap();
        assert_eq!(long, Duration::from_secs(600));
        assert_eq!(
            redial.down(id(1), Instant::now(), 0),
            Duration::from_secs(5)
        );
    }
}

mod unlisted {
    use super::*;

    fn pause_after(failures: u32, unlisted: bool) -> Duration {
        let mut redial = Redial::default();
        let last = (0..failures)
            .map(|_| redial.failed(id(1), Instant::now(), 0, unlisted))
            .last();
        last.unwrap().1
    }

    #[test]
    fn unlisted_refusal_keeps_pause_at_cap() {
        for failures in [9, 20] {
            assert_eq!(
                pause_after(failures, true),
                REDIAL_CAP,
                "{failures} refusals as not admitted"
            );
        }
    }

    #[test]
    fn other_failures_still_get_long_pause() {
        assert_eq!(pause_after(9, false), REDIAL_LONG);
    }
}

/// A refusal that is not `Unlisted` (the member was removed from the list) gets the ordinary ladder: the pause
/// grows as for any failure and, after `REDIAL_LONG_AFTER` attempts, goes to `REDIAL_LONG`. Driven through
/// `PeerDialer::on_dial_failed`, which decides what the refusal means for the pause.
mod refusals {
    use super::dial_support::{setup, Fixture};
    use super::*;
    use crate::members::access::view::Deny;

    #[tokio::test]
    async fn unlisted_refusal_never_reaches_long_pause() {
        // the contrast: the same series of refusals as "not admitted yet" stays at the cap
        let mut fx = setup().await;
        fx.when_refused_times(Deny::Unlisted, REDIAL_LONG_AFTER + 5);
        fx.then_next_pause_between(REDIAL_CAP, REDIAL_CAP + REDIAL_CAP / 4);
        fx.close().await;
    }

    impl Fixture {
        fn when_refused_times(&mut self, deny: Deny, times: u32) {
            for _ in 0..times {
                self.dialer.on_dial_failed(crate::node::dial::DialFailed {
                    peer: self.peer,
                    reason: "refused".to_string(),
                    refusal: crate::node::dial::DialRefusal::ByPeer(deny),
                });
            }
        }

        fn then_next_pause_between(&self, min: Duration, max: Duration) {
            let due = self.dialer.next_redial_at().expect("a retry is scheduled");
            let pause = due.saturating_duration_since(Instant::now());
            // the pause was computed a moment ago, so allow a little slack below the minimum
            let slack = Duration::from_secs(5);
            assert!(
                pause + slack >= min && pause <= max,
                "pause {pause:?} outside [{min:?} - 5 s, {max:?}]"
            );
        }
    }
}

// ---- helpers ----

/// A `PeerDialer` over the `mem` transport; only its retry bookkeeping is used here.
mod dial_support {
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;
    use std::time::Duration;

    use portable_atomic::AtomicU64;
    use tokio::sync::{mpsc, watch};

    use crate::members::membership::{MemberContext, MemberEvent, Members};
    use crate::node::dial::{DialFailed, PeerDialer};
    use crate::node::start::transports_mem::KeyedMem;
    use crate::transport::net::contract::{must_finish, pid, SEED_A, SEED_B};
    use crate::transport::net::mem::MemNet;
    use crate::transport::net::{NetStack, NetStackBuilder, NodeSecret, PeerId, Transport};
    use crate::transport::transfer_probe::ProbeSettings;

    pub(super) struct Fixture {
        pub(super) dialer: PeerDialer,
        pub(super) peer: PeerId,
        stack: NetStack,
        _events_rx: mpsc::UnboundedReceiver<MemberEvent>,
        _failed_rx: mpsc::UnboundedReceiver<DialFailed>,
    }

    pub(super) async fn setup() -> Fixture {
        let net = MemNet::new();
        let secret_a = NodeSecret::from_bytes(&[SEED_A; 32]);
        let a: Arc<dyn Transport> = KeyedMem::new(&net, &secret_a);
        let stack = NetStackBuilder::new(secret_a)
            .transport(a)
            .build()
            .expect("stack built");
        let (events, events_rx) = mpsc::unbounded_channel();
        let probe = ProbeSettings {
            first_delay: Duration::from_secs(3600),
            interval_secs: Arc::new(AtomicU64::new(3600)),
            ok_probe_count: Arc::new(AtomicUsize::new(0)),
            exits: watch::channel(Vec::new()).1,
            full_cycle: false,
        };
        let ctx = MemberContext {
            own_id: pid(SEED_A),
            handshake_key: [7; 32],
            members: Members::default(),
            events,
            probe,
            own_name_tx: watch::channel("A".to_string()).0,
        };
        let (failed_tx, failed_rx) = mpsc::unbounded_channel();
        let dialer = PeerDialer::new(pid(SEED_A), stack.clone(), ctx, failed_tx);
        Fixture {
            dialer,
            peer: pid(SEED_B),
            stack,
            _events_rx: events_rx,
            _failed_rx: failed_rx,
        }
    }

    impl Fixture {
        pub(super) async fn close(self) {
            must_finish(self.stack.close()).await;
        }
    }
}
