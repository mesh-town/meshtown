//! Tests of `Redial::refused_by_us`: our own membership list refused the peer we were dialing. The ladder of
//! pauses is skipped: the id waits idle until `wake_idle` finds access open, or is due at once if access
//! already lets the peer in. Synchronous, time is `Instant::now()` plus manual shifts.

use std::net::SocketAddr;
use std::time::Duration;

use crate::transport::net::{AddrHint, NodeSecret, PeerAddr, PeerId};
use tokio::time::Instant;

use super::*;

mod refused_by_us {
    use super::*;

    mod still_denied {
        use super::*;

        #[test]
        fn waits_idle_until_allowed() {
            // given: an id being dialed, with a known address
            let now = Instant::now();
            let (mut redial, a) = dialing_with_address();

            // when: our list refuses it and access still denies it
            redial.refused_by_us(a.id, now, false);

            // then: no deadline is set, so nothing matures on its own
            assert_eq!(redial.next_due(), None, "an idle id has no deadline");
            assert!(
                redial.take_due(now + Duration::from_secs(3600)).is_empty(),
                "nothing may be due while access denies the peer"
            );

            // then: once access opens, the id is woken and dialed at the same address as attempt 1
            assert_eq!(redial.wake_idle(now, |_| true), vec![a.id]);
            assert_eq!(redial.take_due(now), vec![(a, 1)]);
        }

        #[test]
        fn resets_a_long_ladder() {
            // given: an id that failed past the long pause threshold and is being dialed again
            let now = Instant::now();
            let (mut redial, a) = dialing_with_address();
            for _ in 0..=REDIAL_LONG_AFTER {
                redial.failed(a.id, now, 0, false);
            }
            redial.started(a.id);

            // when: our list refuses it, and later access opens
            redial.refused_by_us(a.id, now, false);
            redial.wake_idle(now, |_| true);

            // then: the failure count starts over, the attempt is the first one
            assert_eq!(redial.take_due(now), vec![(a, 1)]);
        }

        #[test]
        fn found_again_asks_to_dial() {
            // given: an id that waits idle after our refusal
            let now = Instant::now();
            let (mut redial, a) = dialing_with_address();
            redial.refused_by_us(a.id, now, false);

            // when: its address is found again
            let asked = redial.found(a.clone());

            // then: a dial is requested as attempt 1 (whether it starts is up to `Members::try_begin_dial`)
            assert_eq!(asked, Some((a, 1)));
        }
    }

    mod already_allowed {
        use super::*;

        #[test]
        fn is_due_now() {
            // given: an id being dialed, with a known address
            let now = Instant::now();
            let (mut redial, a) = dialing_with_address();

            // when: our list refuses it, but access already lets it in
            redial.refused_by_us(a.id, now, true);

            // then: the next attempt is due at once, without a pause
            assert_eq!(redial.next_due(), Some(now));
            assert_eq!(redial.take_due(now), vec![(a, 1)]);
        }
    }
}

// --- setup and helpers ---

/// Test member id.
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

/// A `Redial` with one id that has a remembered address and whose dial has started.
fn dialing_with_address() -> (Redial, PeerAddr) {
    let mut redial = Redial::default();
    let a = addr(1, 4001);
    redial.found(a.clone());
    redial.started(a.id);
    (redial, a)
}
