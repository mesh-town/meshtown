//! Tests of the redial state of a known but not yet connected member
//! (`node/redial.rs`): memory of the last address and the failure counter per id (`Redial`), the pause with
//! growth and a cap (`redial_pause`), which attempts are loud (`is_loud`). All synchronous, time is
//! `tokio::time::Instant::now()` plus manual shifts, no sleeping — the whole file takes < 1 s.

use std::net::SocketAddr;
use std::time::Duration;

use crate::transport::net::{AddrHint, NodeSecret, PeerAddr, PeerId};
use tokio::time::Instant;

use super::*;

mod first_address {
    use super::*;

    #[test]
    fn new_id_dials_immediately() {
        // given: an empty Redial
        let mut redial = Redial::default();
        let a = addr(1, 4001);

        // when: found(A)
        let result = redial.found(a.clone());

        // then: dial right away, attempt 1
        assert_eq!(
            result,
            Some((a, 1)),
            "a new id must be dialed at once, attempt 1"
        );
    }
}

mod after_failure {
    use super::*;

    #[test]
    fn retries_after_pause() {
        // given: found, started, failed(rnd 0) — wait 5 s
        let now = Instant::now();
        let (mut redial, a) = waiting_after_first_failure(now);

        // when: take_due before and after the pause
        assert!(
            redial.take_due(now).is_empty(),
            "before the 5-second pause ends the retry must not be ready"
        );
        let due = redial.take_due(now + Duration::from_secs(5));

        // then: attempt 2 at the same address
        assert_eq!(due, vec![(a, 2)], "after the pause attempt 2 must be ready");
    }

    #[test]
    fn new_address_dials_immediately() {
        // given: waiting on A after the first failure
        let now = Instant::now();
        let (mut redial, _a) = waiting_after_first_failure(now);
        let b = addr(1, 4002); // the same id, a different address

        // when: found(B)
        let result = redial.found(b.clone());

        // then: dial right away, attempt 2 (the next in order)
        assert_eq!(
            result,
            Some((b, 2)),
            "a new address while waiting must be dialed immediately with the next attempt's number"
        );
    }

    #[test]
    fn same_address_does_not_disturb_schedule() {
        // given: waiting on A
        let now = Instant::now();
        let (mut redial, a) = waiting_after_first_failure(now);
        let due_before = redial.next_due();

        // when: found(A) — the same address
        let result = redial.found(a.clone());

        // then: the dial isn't started, the schedule doesn't shift
        assert_eq!(result, None, "the same address must not start a dial early");
        assert_eq!(
            redial.next_due(),
            due_before,
            "a repeated found with the same address must not change the retry schedule"
        );
    }

    #[test]
    fn address_found_during_dial_is_used_by_next_retry() {
        // given: started on A — a dial is in progress
        let now = Instant::now();
        let mut redial = Redial::default();
        let a = addr(1, 4001);
        redial.found(a.clone());
        redial.started(a.id);

        // when: found(B) during the dial — taken only by the next retry
        let b = addr(1, 4002);
        let during_dial = redial.found(b.clone());
        assert_eq!(
            during_dial, None,
            "an address found during a dial in progress must not start a second dial"
        );

        // when: the dial fails, the retry deadline comes
        let (_, pause) = redial.failed(a.id, now, 0, false);
        let due = redial.take_due(now + pause);

        // then: the retry is at address B, attempt 2
        assert_eq!(
            due,
            vec![(b, 2)],
            "the retry must use the address found during the dial in progress"
        );
    }
}

mod member {
    use super::*;

    #[test]
    fn up_cancels_pending_redial() {
        // given: waiting for a retry on A
        let now = Instant::now();
        let (mut redial, a) = waiting_after_first_failure(now);

        // when: up — the member is confirmed
        redial.up(a.id);

        // then: the retry schedule is cleared
        assert_eq!(
            redial.next_due(),
            None,
            "after up there must be no retry wait"
        );
        assert!(
            redial.take_due(now + Duration::from_secs(120)).is_empty(),
            "after up the retry must not start, however much time passes"
        );
    }

    #[test]
    fn down_schedules_first_retry() {
        // given: the member was confirmed (up), the address is known
        let now = Instant::now();
        let mut redial = Redial::default();
        let a = addr(1, 4001);
        redial.found(a.clone());
        redial.up(a.id);

        // when: down(rnd 0)
        let pause = redial.down(a.id, now, 0);

        // then: the first retry pause is 5 s, the retry is attempt 1 at the last known address
        assert_eq!(
            pause,
            Duration::from_secs(5),
            "the first retry after losing the member is in 5 s"
        );
        let due = redial.take_due(now + Duration::from_secs(5));
        assert_eq!(
            due,
            vec![(a, 1)],
            "the retry after losing the member must be attempt number 1"
        );
    }

    #[test]
    fn down_without_known_address_uses_bare_id() {
        // given: an empty Redial — the member's address never came (only an incoming connection)
        let now = Instant::now();
        let mut redial = Redial::default();
        let bare_id = id(9);

        // when: down(id)
        redial.down(bare_id, now, 0);

        // then: the retry is scheduled for a bare id, with no addresses
        let due = redial.take_due(now + Duration::from_secs(5));
        assert_eq!(due.len(), 1, "exactly one waiting attempt must appear");
        assert_eq!(
            due[0].0,
            PeerAddr::new(bare_id),
            "without a known address the retry must go to a bare id, with no addresses"
        );
    }
}

mod redial_pause_fn {
    use super::*;

    #[test]
    fn grows_from_5s_doubling_up_to_cap() {
        // given: rnd 0 — no addition / when: attempts 1, 2, 5, 8 / then: 5, 10, 60, 60 s
        let expected = [(1u32, 5u64), (2, 10), (5, 60), (8, 60)];
        for (attempt, secs) in expected {
            assert_eq!(
                redial_pause(attempt, 0),
                Duration::from_secs(secs),
                "attempt {attempt} with rnd 0 must give {secs} s"
            );
        }
    }

    #[test]
    fn extra_from_rnd_does_not_exceed_a_quarter() {
        // given: rnd u32::MAX — the maximum addition / when: attempts 1 and 6
        // then: no more than base + a quarter (6.25 s and 75 s), no less than the base itself (5 s and 60 s)
        let cases = [
            (1u32, Duration::from_secs(5), Duration::from_millis(6250)),
            (6u32, Duration::from_secs(60), Duration::from_secs(75)),
        ];
        for (attempt, min, max) in cases {
            let pause = redial_pause(attempt, u32::MAX);
            assert!(
                pause >= min && pause <= max,
                "attempt {attempt} with the maximum rnd must be in {min:?}..={max:?}, got {pause:?}"
            );
        }
    }

    #[test]
    fn does_not_overflow_on_large_attempt() {
        // given / when: attempt u32::MAX / then: doesn't panic, hits the long pause of 600 s
        let pause = redial_pause(u32::MAX, 0);
        assert_eq!(
            pause, REDIAL_LONG,
            "a huge attempt must hit the cap without overflow"
        );
    }
}

mod is_loud_fn {
    use super::*;

    #[test]
    fn is_loud_on_first_three_attempts_and_powers_of_two() {
        // given: attempts 1–9, 16, 17 / then: loud — 1, 2, 3, 4, 8, 16; quiet — the rest
        let loud = [1u32, 2, 3, 4, 8, 16];
        let quiet = [5u32, 6, 7, 9, 17];
        for attempt in loud {
            assert!(is_loud(attempt), "attempt {attempt} must be loud");
        }
        for attempt in quiet {
            assert!(!is_loud(attempt), "attempt {attempt} must not be loud");
        }
    }
}

mod down_now_fn {
    use super::*;

    #[test]
    fn down_now_schedules_attempt_at_once() {
        // given: member A is idle / when: down_now / then: pause 0, deadline exactly now
        let now = Instant::now();
        let mut redial = Redial::default();
        let pause = redial.down_now(id(1), now);
        assert_eq!(
            pause,
            Duration::ZERO,
            "the pause after a network change is zero"
        );
        assert_eq!(
            redial.next_due(),
            Some(now),
            "the attempt is scheduled for now"
        );
    }

    #[test]
    fn failure_after_down_now_uses_first_step() {
        // given: down_now and the attempt taken / when: it failed (rnd 0) / then: pause REDIAL_FIRST
        let now = Instant::now();
        let mut redial = Redial::default();
        redial.down_now(id(1), now);
        assert_eq!(redial.take_due(now).len(), 1, "the attempt matured at once");
        let (attempt, pause) = redial.failed(id(1), now, 0, false);
        assert_eq!(
            (attempt, pause),
            (1, REDIAL_FIRST),
            "a failure after down_now - the ordinary first step"
        );
    }
}

/// Test member id — the same trick as in `node_ctl/state_tests.rs`.
fn id(byte: u8) -> PeerId {
    NodeSecret::from_bytes(&[byte; 32]).peer_id()
}

/// A test member address: the id from `byte`, an IP address with port `port` — so two addresses of one id
/// differ from each other in the "a new address was found" tests.
fn addr(byte: u8, port: u16) -> PeerAddr {
    let socket: SocketAddr = format!("203.0.113.{byte}:{port}").parse().unwrap();
    PeerAddr {
        id: id(byte),
        hints: vec![AddrHint::Ip(socket)],
    }
}

/// A `Redial` after the first failure: address A found, dial started and failed at once (rnd 0) — we wait
/// for the first 5 s pause from the current moment `now`.
fn waiting_after_first_failure(now: Instant) -> (Redial, PeerAddr) {
    let mut redial = Redial::default();
    let a = addr(1, 4001);
    redial.found(a.clone());
    redial.started(a.id);
    redial.failed(a.id, now, 0, false);
    (redial, a)
}
