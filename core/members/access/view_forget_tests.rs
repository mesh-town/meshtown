//! Tests of the verdict view for removed ("forgotten") nodes: the decision for a node I voted to remove and
//! for a node removed by others, its place in the order of denial reasons, the codes on the wire and the
//! one-time notice for a removed node. Ledgers come from `tally`, except where a combination of reasons
//! cannot be reached by votes alone and is set up by hand.
//! In `setup(n)` the own node `m` admits `a`, `b`, `c`, `d` (as many as `n - 1` of them) at second 0.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::members::access::record::Via;
use crate::members::access::tally::{
    tally, AdmitIn, Exclusion, ForgetIn, ForgetRound, How, Ledger, Rules, TallyInput,
};
use crate::members::access::view::{AccessView, Deny, Verdicts, ACCESS_CLOSE_CODE};
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;

mod my_vote {
    use super::*;

    #[test]
    fn my_waiting_vote_gets_removed_by_me() {
        // given: four members, I vote to remove c and a second vote is still needed
        let s = setup(4);
        let v = s.verdicts(&[s.vote(s.m, s.c, 100)]);
        // then: c is refused at once, by my own vote
        assert_eq!(v.check(s.c), Err(Deny::RemovedByMe));
        assert_eq!(v.my_forgets(), set(&[s.c]));
        assert_eq!(Deny::RemovedByMe.code(), "removed_by_me");
    }

    #[test]
    fn others_pending_vote_lets_peer_in() {
        // given: four members, a votes to remove c, I did not vote
        let s = setup(4);
        let v = s.verdicts(&[s.vote(s.a, s.c, 100)]);
        // then: c is let in and I hide nobody
        assert_eq!(v.check(s.c), Ok(()));
        assert!(v.my_forgets().is_empty(), "a vote of another is not mine");
    }

    #[test]
    fn spent_my_vote_does_not_deny() {
        // given: I voted to remove c at 100, then a admitted c again at 150
        let mut s = setup(4);
        s.admits.push(admit_at(s.a, s.c, 150));
        let v = s.verdicts(&[s.vote(s.m, s.c, 100)]);
        // then: my old vote no longer hides c
        assert_eq!(v.check(s.c), Ok(()));
        assert!(v.my_forgets().is_empty(), "a spent vote must not count");
    }

    #[test]
    fn my_forgets_lists_only_subjects_i_voted_for() {
        // given: five members; I vote to remove b and c, a votes to remove d
        let s = setup(5);
        let votes = [
            s.vote(s.m, s.b, 100),
            s.vote(s.m, s.c, 110),
            s.vote(s.a, s.d, 120),
        ];
        // then: only b and c are mine
        assert_eq!(s.verdicts(&votes).my_forgets(), set(&[s.b, s.c]));
    }

    #[test]
    fn my_forgets_never_lists_me() {
        // given: a ledger with a removed circle on my own node, voted by me
        let s = setup(3);
        let mut ledger = s.ledger(&[]);
        ledger.forgets.insert(s.m, round(&[s.m], Some(5)));
        let v = verdicts(s.m, ledger);
        // then: my node is not on the list, and it is not denied
        assert!(v.my_forgets().is_empty());
        assert_eq!(v.check(s.m), Ok(()));
    }
}

mod removed_by_others {
    use super::*;

    #[test]
    fn forgotten_peer_gets_forgotten() {
        // given: four members, a and b vote to remove c
        let s = setup(4);
        let v = s.verdicts(&[s.vote(s.a, s.c, 100), s.vote(s.b, s.c, 110)]);
        // then: c is refused as removed and is not on my list; a is let in
        assert_eq!(v.check(s.c), Err(Deny::Forgotten));
        assert_eq!(Deny::Forgotten.code(), "removed");
        assert!(v.my_forgets().is_empty());
        assert_eq!(v.check(s.a), Ok(()));
    }

    #[test]
    fn forgotten_beats_removed_by_me() {
        // given: four members, I and a vote to remove c
        let s = setup(4);
        let v = s.verdicts(&[s.vote(s.m, s.c, 100), s.vote(s.a, s.c, 110)]);
        // then: c is removed for good, and my vote is still on the list
        assert_eq!(v.check(s.c), Err(Deny::Forgotten));
        assert_eq!(v.my_forgets(), set(&[s.c]));
    }

    #[test]
    fn returning_admit_lets_forgotten_peer_in() {
        // given: c was removed by a and b at 110, then admitted again at 200
        let mut s = setup(4);
        s.admits.push(admit_at(s.a, s.c, 200));
        let v = s.verdicts(&[s.vote(s.a, s.c, 100), s.vote(s.b, s.c, 110)]);
        // then: c is let in
        assert_eq!(v.check(s.c), Ok(()));
    }
}

mod order_of_reasons {
    use super::*;

    #[test]
    fn left_beats_forgotten() {
        // given: c is removed and I have left
        let (s, mut v) = removed_setup();
        Arc::make_mut(&mut v.ledger)
            .excluded
            .insert(s.m, left_by_myself());
        // then: everybody gets `Left`, c too
        assert_eq!(v.check(s.c), Err(Deny::Left));
    }

    #[test]
    fn excluded_and_forgotten_gets_excluded() {
        // given: c is both removed and excluded
        let (s, mut v) = removed_setup();
        Arc::make_mut(&mut v.ledger)
            .excluded
            .insert(s.c, by_votes());
        // then: the exclusion is named
        assert_eq!(v.check(s.c), Err(Deny::Excluded));
    }

    #[test]
    fn voted_by_me_beats_forgotten() {
        // given: c is removed and I voted to exclude it
        let (s, mut v) = removed_setup();
        v.my_votes.insert(s.c);
        // then: my exclusion vote is named
        assert_eq!(v.check(s.c), Err(Deny::VotedByMe));
    }

    #[test]
    fn blocked_and_forgotten_gets_blocked() {
        // given: c is removed and blocked by me
        let (s, mut v) = removed_setup();
        v.blocks.insert(s.c);
        // then: the block is named
        assert_eq!(v.check(s.c), Err(Deny::Blocked));
    }

    #[test]
    fn removed_by_me_beats_unlisted() {
        // given: a circle of mine on a node that is not in the admitted set
        let s = setup(3);
        let x = pid(9);
        let mut ledger = s.ledger(&[]);
        ledger.forgets.insert(x, round(&[s.m], None));
        // then: the removal vote is named, not the missing admission
        assert_eq!(verdicts(s.m, ledger).check(x), Err(Deny::RemovedByMe));
    }
}

mod on_the_wire {
    use super::*;

    #[test]
    fn removed_reason_round_trips() {
        // given: the close text for a removed node
        let text = on_the_wire(Deny::Forgotten);
        // then: it says `removed` and is read back as the same reason
        assert_eq!(text, "code 403: removed");
        assert_eq!(Deny::from_wire(&text), Some(Deny::Forgotten));
    }

    #[test]
    fn removed_by_me_goes_out_as_refused() {
        // given: the close text for a node I voted to remove
        let text = on_the_wire(Deny::RemovedByMe);
        // then: the other side only learns that it was refused
        assert_eq!(text, "code 403: refused");
        assert_eq!(Deny::from_wire(&text), Some(Deny::Refused));
    }
}

mod publication {
    use super::*;

    #[test]
    fn newly_forgotten_peer_is_reported_as_denied() {
        // given: a view where everybody is let in
        let (s, removed) = removed_setup();
        let view = AccessView::open();
        view.publish(s.verdicts(&[]));
        // when: verdicts where c is removed are published
        let fresh = view.publish(removed);
        // then: c is reported with the reason `removed`
        assert_eq!(fresh, vec![(s.c, Deny::Forgotten)]);
        assert_eq!(view.check(s.c), Err(Deny::Forgotten));
    }
}

mod lifted_forget {
    use super::*;

    #[test]
    fn lifted_forget_wakes() {
        // given: four members; in one view I voted to remove c, in the other a and b removed it
        let s = setup(4);
        let by_me = s.verdicts(&[s.vote(s.m, s.c, 100)]);
        let by_others = s.verdicts(&[s.vote(s.a, s.c, 100), s.vote(s.b, s.c, 110)]);
        assert_eq!(by_me.check(s.c), Err(Deny::RemovedByMe));
        assert_eq!(by_others.check(s.c), Err(Deny::Forgotten));
        // when: a view without any vote is published over each
        let wakes =
            [by_me, by_others].map(|before| wakes_when_replaced_by(before, s.verdicts(&[])));
        // then: the subscriber is told in both cases
        assert_eq!(wakes, [true, true], "woken for RemovedByMe, Forgotten");
    }
}

mod removed_notice {
    use super::*;

    #[test]
    fn removed_line_once_per_run() {
        // given: a view that has not given the notice yet
        let view = AccessView::open();
        // when / then: the first call says so; later ones, also through a clone, do not
        assert!(view.first_removed_notice(), "the first call must be true");
        assert!(
            !view.first_removed_notice(),
            "the second call must be false"
        );
        assert!(!view.clone().first_removed_notice(), "a clone shares it");
    }
}

// --- setup and helpers ---

/// The own node `m` and its neighbours; `m` admitted the first `n - 1` of `a`..`d`.
#[rustfmt::skip]
struct Net { m: PeerId, a: PeerId, b: PeerId, c: PeerId, d: PeerId, admits: Vec<AdmitIn> }

#[rustfmt::skip]
fn setup(n: usize) -> Net {
    let p: Vec<PeerId> = (1..=5u8).map(pid).collect();
    let admits = p[1..n].iter().map(|s| admit_at(p[0], *s, 0)).collect();
    let (m, a, b, c, d) = (p[0], p[1], p[2], p[3], p[4]);
    Net { m, a, b, c, d, admits }
}

impl Net {
    #[rustfmt::skip]
    fn vote(&self, author: PeerId, subject: PeerId, made_unix: u64) -> ForgetIn {
        ForgetIn { author, subject, made_unix }
    }

    fn ledger(&self, forgets: &[ForgetIn]) -> Ledger {
        tally(&TallyInput {
            me: self.m,
            admits: self.admits.clone(),
            votes: Vec::new(),
            forgets: forgets.to_vec(),
            rules: Rules::Peer,
            powers: vec![],
        })
    }

    /// Verdicts of `m` after the forget votes, with no exclusion votes and no blocks.
    fn verdicts(&self, forgets: &[ForgetIn]) -> Verdicts {
        verdicts(self.m, self.ledger(forgets))
    }
}

/// Four members; a and b voted to remove c. Returns the net and my verdicts.
fn removed_setup() -> (Net, Verdicts) {
    let s = setup(4);
    let v = s.verdicts(&[s.vote(s.a, s.c, 100), s.vote(s.b, s.c, 110)]);
    assert!(v.ledger.is_forgotten(&s.c), "the setup must forget c");
    (s, v)
}

fn verdicts(me: PeerId, ledger: Ledger) -> Verdicts {
    Verdicts {
        me,
        ledger: Arc::new(ledger),
        my_votes: BTreeSet::new(),
        blocks: BTreeSet::new(),
        reasons: Default::default(),
    }
}

#[rustfmt::skip]
fn admit_at(author: PeerId, subject: PeerId, made_unix: u64) -> AdmitIn {
    AdmitIn { author, subject, via: Via::Invite, made_unix }
}

#[rustfmt::skip]
fn round(voters: &[PeerId], at_unix: Option<u64>) -> ForgetRound {
    ForgetRound { voters: voters.to_vec(), at_unix }
}

#[rustfmt::skip]
fn by_votes() -> Exclusion {
    Exclusion { how: How::Votes, at_micros: 10, voters: Vec::new() }
}

#[rustfmt::skip]
fn left_by_myself() -> Exclusion {
    Exclusion { how: How::Left, at_micros: 10, voters: Vec::new() }
}

fn set(ids: &[PeerId]) -> BTreeSet<PeerId> {
    ids.iter().copied().collect()
}

/// The close text a neighbour sees for `deny`, as `PeerConn::close_reason` reports it.
fn on_the_wire(deny: Deny) -> String {
    let reason = String::from_utf8(deny.wire_reason().to_vec()).expect("wire reason is text");
    format!("code {ACCESS_CLOSE_CODE}: {reason}")
}

/// Whether publishing `after` over `before` moves the "allowed again" counter.
fn wakes_when_replaced_by(before: Verdicts, after: Verdicts) -> bool {
    let view = AccessView::open();
    view.publish(before);
    let allowed = view.allowed_changes();
    view.publish(after);
    allowed.has_changed().expect("the view keeps the sender")
}
