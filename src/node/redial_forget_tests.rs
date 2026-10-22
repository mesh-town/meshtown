//! Tests of forgetting members cut by the prune (`Redial::forget`) and the long pause after a series of failures
//! (`redial_pause` from attempt 9). Synchronous, time is `Instant::now()` plus manual shifts.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Duration;

use crate::net::{AddrHint, NodeSecret, PeerAddr, PeerId};
use tokio::time::Instant;

use super::*;

fn id(byte: u8) -> PeerId {
    NodeSecret::from_bytes(&[byte; 32]).peer_id()
}

fn addr(byte: u8) -> PeerAddr {
    let socket: SocketAddr = format!("203.0.113.{byte}:4001").parse().unwrap();
    PeerAddr { id: id(byte), hints: vec![AddrHint::Ip(socket)] }
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
            assert_eq!(redial_pause(attempt, 0), Duration::from_secs(600), "attempt {attempt}, rnd 0");
            assert_eq!(redial_pause(attempt, u32::MAX), Duration::from_secs(750), "attempt {attempt}, rnd max");
        }
    }

    #[test]
    fn down_restarts_series_with_short_pause() {
        let mut redial = Redial::default();
        let (_, long) = (0..9).map(|_| redial.failed(id(1), Instant::now(), 0, false)).last().unwrap();
        assert_eq!(long, Duration::from_secs(600));
        assert_eq!(redial.down(id(1), Instant::now(), 0), Duration::from_secs(5));
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
        assert_eq!(
            due.iter().map(|d| d.0.id).collect::<Vec<_>>(),
            vec![id(2)]
        );
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
        let last = (0..failures).map(|_| redial.failed(id(1), Instant::now(), 0, unlisted)).last();
        last.unwrap().1
    }

    #[test]
    fn unlisted_refusal_keeps_pause_at_cap() {
        for failures in [9, 20] {
            assert_eq!(pause_after(failures, true), REDIAL_CAP, "{failures} refusals as not admitted");
        }
    }

    #[test]
    fn other_failures_still_get_long_pause() {
        assert_eq!(pause_after(9, false), REDIAL_LONG);
    }
}
