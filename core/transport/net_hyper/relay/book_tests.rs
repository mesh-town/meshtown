//! `RelayBook`: which relay candidate a dial to a peer uses.

use super::*;
use crate::transport::net::contract::pid;
use crate::transport::net::NodeSecret;

mod no_candidates {
    use super::*;

    #[test]
    fn gives_nothing() {
        // given: an empty candidate list
        let book = RelayBook::default();
        // when: picking for a peer
        let pick = book.pick(pid(1), &[], Instant::now());
        // then: there is nothing to dial through
        assert_eq!(pick, None);
    }
}

mod one_candidate {
    use super::*;

    #[test]
    fn is_picked_with_the_direct_phase_first() {
        // given: a single candidate and a fresh book
        let book = RelayBook::default();
        let only = cand(10);
        // when: picking
        let pick = book.pick(pid(1), &[only], Instant::now());
        // then: it is the one, and the direct attempt comes first
        assert_eq!(
            pick,
            Some(RelayPick {
                cand: only,
                direct_first: true
            })
        );
    }
}

mod after_a_failure {
    use super::*;

    #[test]
    fn failure_moves_the_cursor() {
        // given: two candidates, the first was picked and failed
        let (book, list) = (RelayBook::default(), [cand(10), cand(11)]);
        assert_eq!(picked(&book, 1, &list), list[0], "test precondition");
        book.failed(pid(1), list[0]);
        // when: picking again for the same peer
        let next = picked(&book, 1, &list);
        // then: the second candidate is next
        assert_eq!(next, list[1]);
    }

    #[test]
    fn cursor_wraps_around_the_list() {
        // given: two candidates that both failed once
        let (book, list) = (RelayBook::default(), [cand(10), cand(11)]);
        book.failed(pid(1), list[0]);
        book.failed(pid(1), list[1]);
        // when: picking
        let next = picked(&book, 1, &list);
        // then: the turn comes back to the first
        assert_eq!(next, list[0]);
    }

    #[test]
    fn other_peers_keep_their_turn() {
        // given: a failure while dialing peer 1
        let (book, list) = (RelayBook::default(), [cand(10), cand(11)]);
        book.failed(pid(1), list[0]);
        // when: picking for peer 2
        let next = picked(&book, 2, &list);
        // then: peer 2 starts from the first candidate
        assert_eq!(next, list[0]);
    }

    #[test]
    fn forgets_the_success_of_the_failed_candidate() {
        // given: a candidate that worked a moment ago and has now failed
        let (book, list) = (RelayBook::default(), [cand(10), cand(11)]);
        let t0 = Instant::now();
        book.succeeded(pid(1), list[1], t0);
        book.failed(pid(1), list[1]);
        // when: picking right after
        let pick = book
            .pick(pid(1), &list, t0 + Duration::from_secs(1))
            .unwrap();
        // then: the direct phase is back
        assert!(
            pick.direct_first,
            "a failed relay stayed preferred: {pick:?}"
        );
    }
}

mod after_a_success {
    use super::*;

    #[test]
    fn success_sticks() {
        // given: the second candidate reached the peer
        let (book, list) = (RelayBook::default(), [cand(10), cand(11)]);
        book.succeeded(pid(1), list[1], Instant::now());
        // when: picking with the same list
        let next = picked(&book, 1, &list);
        // then: the second again, not the first in turn
        assert_eq!(next, list[1]);
    }

    #[test]
    fn recent_success_skips_direct() {
        // given: the relay reached the peer a minute ago
        let (book, list) = (RelayBook::default(), [cand(10), cand(11)]);
        let t0 = Instant::now();
        book.succeeded(pid(1), list[1], t0);
        // when: picking now
        let pick = book
            .pick(pid(1), &list, t0 + Duration::from_secs(60))
            .unwrap();
        // then: the same relay, without the direct phase
        assert_eq!(
            pick,
            RelayPick {
                cand: list[1],
                direct_first: false
            }
        );
    }

    #[test]
    fn old_success_tries_direct_again() {
        // given: the relay reached the peer longer ago than the first-choice window
        let (book, list) = (RelayBook::default(), [cand(10), cand(11)]);
        let t0 = Instant::now();
        book.succeeded(pid(1), list[1], t0);
        // when: picking after the window
        let pick = book
            .pick(
                pid(1),
                &list,
                t0 + RELAY_FIRST_TTL + Duration::from_secs(60),
            )
            .unwrap();
        // then: the same relay, but the direct phase comes first again
        assert_eq!(
            pick,
            RelayPick {
                cand: list[1],
                direct_first: true
            }
        );
    }

    #[test]
    fn vanished_candidate_falls_back_to_the_turn() {
        // given: a relay that worked, then left the list
        let (book, list) = (RelayBook::default(), [cand(10), cand(11)]);
        let t0 = Instant::now();
        book.succeeded(pid(1), list[1], t0);
        // when: picking from a list without it
        let pick = book
            .pick(pid(1), &list[..1], t0 + Duration::from_secs(1))
            .unwrap();
        // then: the candidate in turn, with the direct phase
        assert_eq!(
            pick,
            RelayPick {
                cand: list[0],
                direct_first: true
            }
        );
    }
}

mod many_peers {
    use super::*;

    #[test]
    fn the_book_stays_within_its_limit() {
        // given: 257 peers, each with a success
        let book = RelayBook::default();
        let t0 = Instant::now();
        let list = [cand(10)];
        for n in 0..=256u16 {
            book.succeeded(peer(n), list[0], t0);
        }
        // when: counting what the book remembers
        let remembered = book.peers.lock().unwrap().len();
        // then: not more than the limit, and the newest peer is among them
        assert!(
            remembered <= MAX_BOOK_PEERS,
            "the book grew to {remembered}"
        );
        let pick = book
            .pick(peer(256), &list, t0 + Duration::from_secs(1))
            .unwrap();
        assert!(!pick.direct_first, "the newest success was dropped");
    }
}

fn cand(seed: u8) -> RelayCandidate {
    RelayCandidate {
        member: pid(seed),
        relay: [seed; 32],
        address: None,
    }
}

/// The candidate the book picks for peer `n` now.
fn picked(book: &RelayBook, n: u8, list: &[RelayCandidate]) -> RelayCandidate {
    book.pick(pid(n), list, Instant::now())
        .expect("a candidate")
        .cand
}

/// The id of the `n`-th distinct peer.
fn peer(n: u16) -> PeerId {
    let mut seed = [7u8; 32];
    seed[..2].copy_from_slice(&n.to_be_bytes());
    NodeSecret::from_bytes(&seed).peer_id()
}
