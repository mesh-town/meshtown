//! Tests of waking idle ids after a denial is lifted (`Redial::wake_idle`): which states are woken, what
//! the woken id keeps (address, failure count) and what is left alone. Synchronous, time is `Instant::now()`
//! plus manual shifts.

use std::cell::Cell;
use std::net::SocketAddr;
use std::time::Duration;

use crate::transport::net::{AddrHint, NodeSecret, PeerAddr, PeerId};
use tokio::time::Instant;

use super::*;

mod idle_peer {
    use super::*;

    #[test]
    fn idle_peer_allowed_again_is_due_now() {
        // given: an id with a known address that came up and went idle
        let now = Instant::now();
        let (mut redial, a) = idle_with_address();

        // when: it is woken
        let woken = redial.wake_idle(now, |_| true);

        // then: it is reported, and its first attempt at the same address matures at once
        assert_eq!(woken, vec![a.id], "the idle id must be woken");
        assert_eq!(
            redial.take_due(now),
            vec![(a, 1)],
            "the woken id must be due now, attempt 1"
        );
    }

    #[test]
    fn idle_peer_without_address_dials_by_key() {
        // given: an id that only came up as an incoming connection: no address is remembered
        let now = Instant::now();
        let mut redial = Redial::default();
        redial.up(id(1));

        // when: it is woken
        redial.wake_idle(now, |_| true);

        // then: the dial goes by the bare key
        assert_eq!(redial.take_due(now), vec![(PeerAddr::new(id(1)), 1)]);
    }

    #[test]
    fn idle_peer_still_denied_stays_idle() {
        // given: an idle id whose denial still holds
        let now = Instant::now();
        let (mut redial, _a) = idle_with_address();

        // when: the predicate refuses it
        let woken = redial.wake_idle(now, |_| false);

        // then: nothing is woken and nothing is scheduled
        assert!(woken.is_empty(), "woken: {woken:?}");
        assert_eq!(redial.next_due(), None, "an idle id has no deadline");
        assert!(redial.take_due(now + Duration::from_secs(3600)).is_empty());
    }

    #[test]
    fn only_the_allowed_one_of_two_idle_ids_is_woken() {
        // given: two idle ids
        let now = Instant::now();
        let mut redial = Redial::default();
        redial.up(id(1));
        redial.up(id(2));

        // when: the predicate lets in the second one only
        let woken = redial.wake_idle(now, |p| p == id(2));

        // then: only the second one is due
        assert_eq!(woken, vec![id(2)]);
        assert_eq!(redial.take_due(now), vec![(PeerAddr::new(id(2)), 1)]);
    }
}

mod peer_not_idle {
    use super::*;

    #[test]
    fn waiting_peer_keeps_its_deadline() {
        // given: an id that vanished and waits for its first pause (rnd 0)
        let now = Instant::now();
        let mut redial = Redial::default();
        redial.down(id(1), now, 0);

        // when: everybody is woken
        let woken = redial.wake_idle(now, |_| true);

        // then: the waiting id is left alone and keeps its deadline
        assert!(woken.is_empty(), "woken: {woken:?}");
        assert_eq!(redial.next_due(), Some(now + REDIAL_FIRST));
    }

    #[test]
    fn long_wait_is_not_shortened() {
        // given: an id that failed nine times in a row and waits the long pause
        let now = Instant::now();
        let (mut redial, failed_at) = in_the_long_pause(now);
        let deadline = failed_at + REDIAL_LONG;
        assert_eq!(
            redial.next_due(),
            Some(deadline),
            "the setup must reach the long pause"
        );

        // when: everybody is woken
        let woken = redial.wake_idle(failed_at, |_| true);

        // then: the long pause stands
        assert!(woken.is_empty(), "woken: {woken:?}");
        assert_eq!(redial.next_due(), Some(deadline));
    }

    #[test]
    fn dialing_peer_is_untouched() {
        // given: an id whose dial is in progress
        let now = Instant::now();
        let mut redial = Redial::default();
        redial.found(addr(1, 4001));
        redial.started(id(1));

        // when: everybody is woken
        let woken = redial.wake_idle(now, |_| true);

        // then: nothing is woken and nothing becomes due
        assert!(woken.is_empty(), "woken: {woken:?}");
        assert!(redial.take_due(now).is_empty(), "a dial is already going");
    }

    #[test]
    fn nothing_to_wake_returns_empty() {
        // given: an empty Redial and a predicate that records its calls
        let mut redial = Redial::default();
        let asked = Cell::new(false);

        // when: it is woken
        let woken = redial.wake_idle(Instant::now(), |_| {
            asked.set(true);
            true
        });

        // then: nothing is returned and the predicate was never asked
        assert!(woken.is_empty(), "woken: {woken:?}");
        assert!(!asked.get(), "the predicate must not be called without ids");
    }

    #[test]
    fn predicate_is_not_asked_about_waiting_or_dialing_ids() {
        // given: one waiting id and one dialing id
        let now = Instant::now();
        let mut redial = Redial::default();
        redial.down(id(1), now, 0);
        redial.found(addr(2, 4001));
        redial.started(id(2));
        let asked = Cell::new(false);

        // when: it is woken
        redial.wake_idle(now, |_| {
            asked.set(true);
            true
        });

        // then: the predicate was not consulted
        assert!(!asked.get(), "only idle ids may be asked");
    }
}

mod refused_after_wake {
    use super::*;

    #[test]
    fn refused_after_wake_backs_off() {
        // given: an idle id that was woken and taken for a dial
        let now = Instant::now();
        let (mut redial, a) = idle_with_address();
        redial.wake_idle(now, |_| true);
        assert_eq!(redial.take_due(now), vec![(a.clone(), 1)]);

        // when: the other side still refuses it (rnd 0)
        let failed = redial.failed(a.id, now, 0, false);

        // then: it is attempt 1 with the ordinary first pause
        assert_eq!(failed, (1, REDIAL_FIRST));
        assert_eq!(redial.next_due(), Some(now + REDIAL_FIRST));
    }
}

// --- setup and helpers ---

/// Test member id, the same trick as in `redial_tests.rs`.
fn id(byte: u8) -> PeerId {
    NodeSecret::from_bytes(&[byte; 32]).peer_id()
}

/// A test member address with a port, so it differs from the bare key.
fn addr(byte: u8, port: u16) -> PeerAddr {
    let socket: SocketAddr = format!("203.0.113.{byte}:{port}").parse().unwrap();
    PeerAddr {
        id: id(byte),
        hints: vec![AddrHint::Ip(socket)],
    }
}

/// A `Redial` with one id that has a remembered address, was dialed and came up: idle, no failures.
fn idle_with_address() -> (Redial, PeerAddr) {
    let mut redial = Redial::default();
    let a = addr(1, 4001);
    redial.found(a.clone());
    redial.started(a.id);
    redial.up(a.id);
    (redial, a)
}

/// A `Redial` where the id failed nine times in a row (rnd 0), every earlier pause was waited out, and the
/// ninth failure at the returned instant started the long pause.
fn in_the_long_pause(now: Instant) -> (Redial, Instant) {
    let mut redial = Redial::default();
    let a = addr(1, 4001);
    redial.found(a.clone());
    redial.started(a.id);
    let mut at = now;
    for attempt in 1..=REDIAL_LONG_AFTER + 1 {
        let (n, pause) = redial.failed(a.id, at, 0, false);
        assert_eq!(n, attempt);
        if attempt == REDIAL_LONG_AFTER + 1 {
            assert_eq!(
                pause, REDIAL_LONG,
                "the ninth failure starts the long pause"
            );
            break;
        }
        at += pause;
        assert_eq!(
            redial.take_due(at).len(),
            1,
            "the pause of attempt {attempt} is over"
        );
    }
    (redial, at)
}
