//! Tests of forgetting members cut by the prune (`Redial::forget`) and the long pause after a series of failures
//! (`redial_pause` from attempt 9). Synchronous, time is `Instant::now()` plus manual shifts.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Duration;

use crate::transport::net::{AddrHint, NodeSecret, PeerAddr, PeerId};
use tokio::time::Instant;

use super::*;

fn id(byte: u8) -> PeerId {
    NodeSecret::from_bytes(&[byte; 32]).peer_id()
}

fn addr(byte: u8) -> PeerAddr {
    let socket: SocketAddr = format!("203.0.113.{byte}:4001").parse().unwrap();
    PeerAddr {
        id: id(byte),
        hints: vec![AddrHint::Ip(socket)],
    }
}

/// A `Redial` where two ids, 1 and 2, wait for the first pause (rnd 0).
fn two_waiting(now: Instant) -> Redial {
    let mut redial = Redial::default();
    for b in [1u8, 2] {
        redial.found(addr(b));
        redial.started(id(b));
        redial.failed(id(b), now, 0, false);
    }
    redial
}

fn set(bytes: &[u8]) -> HashSet<PeerId> {
    bytes.iter().map(|b| id(*b)).collect()
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

mod forget {
    use super::*;

    #[test]
    fn forgotten_is_not_due_and_others_stay() {
        let now = Instant::now();
        let mut redial = two_waiting(now);
        redial.forget(&set(&[1]));
        let due = redial.take_due(now + Duration::from_secs(10));
        assert_eq!(due.iter().map(|d| d.0.id).collect::<Vec<_>>(), vec![id(2)]);
    }

    #[test]
    fn next_due_none_when_only_forgotten_waited() {
        let now = Instant::now();
        let mut redial = Redial::default();
        redial.found(addr(1));
        redial.started(id(1));
        redial.failed(id(1), now, 0, false);
        assert!(redial.next_due().is_some());
        redial.forget(&set(&[1]));
        assert_eq!(redial.next_due(), None);
    }

    #[test]
    fn found_after_forget_starts_from_attempt_1() {
        let now = Instant::now();
        let mut redial = Redial::default();
        redial.found(addr(1));
        for _ in 0..5 {
            redial.failed(id(1), now, 0, false);
        }
        redial.forget(&set(&[1]));
        assert_eq!(redial.found(addr(1)), Some((addr(1), 1)));
    }

    #[test]
    fn failure_of_dialing_forgotten_recreates_record() {
        // the dial was going to the cut member: its outcome creates the entry again (allowed), the prune will remove it again
        let now = Instant::now();
        let mut redial = Redial::default();
        redial.found(addr(1));
        redial.started(id(1));
        redial.forget(&set(&[1]));
        let (attempt, _) = redial.failed(id(1), now, 0, false);
        assert_eq!(attempt, 1);
        assert!(redial.next_due().is_some());
        redial.forget(&set(&[1]));
        assert_eq!(redial.next_due(), None);
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
mod removed_refusal {
    use super::dial_support::{setup, Fixture};
    use super::*;
    use crate::members::access::view::Deny;

    #[tokio::test]
    async fn removed_refusal_uses_long_ladder() {
        // given: a dialer whose peer refuses every dial as removed
        let mut fx = setup().await;
        // when: the refusal repeats past the ordinary attempts
        fx.when_refused_times(Deny::Forgotten, REDIAL_LONG_AFTER + 1);
        // then: the pause before the next attempt is the long one
        fx.then_next_pause_between(REDIAL_LONG, REDIAL_LONG + REDIAL_LONG / 4);
        fx.close().await;
    }

    #[tokio::test]
    async fn removed_by_me_refusal_uses_long_ladder() {
        let mut fx = setup().await;
        fx.when_refused_times(Deny::RemovedByMe, REDIAL_LONG_AFTER + 1);
        fx.then_next_pause_between(REDIAL_LONG, REDIAL_LONG + REDIAL_LONG / 4);
        fx.close().await;
    }

    #[tokio::test]
    async fn removed_refusal_keeps_ordinary_pause_for_first_attempts() {
        // given / when: exactly `REDIAL_LONG_AFTER` refusals as removed
        let mut fx = setup().await;
        fx.when_refused_times(Deny::Forgotten, REDIAL_LONG_AFTER);
        // then: the pause is still capped by the ordinary ladder (with the random addition)
        fx.then_next_pause_between(REDIAL_CAP, REDIAL_CAP + REDIAL_CAP / 4);
        fx.close().await;
    }

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
                    refusal: Some(deny),
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
