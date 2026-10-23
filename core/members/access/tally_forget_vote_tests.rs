//! Tests of the forget pass inside the tally (`tally.rs`, `forget.rs`), part two: the vote threshold,
//! the cascade after a forgetting, growth of the network, mutual and withdrawn votes, the marks taken
//! from the raw records, and whose votes count. Who is forgotten and who returns lives in
//! `tally_forget_tests.rs`. The nodes of `net(n)` are named by index: `A` is the own node.

use super::forget_tests::{forget, net};
use super::*;
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;

const A: usize = 0;
const B: usize = 1;
const C: usize = 2;
const D: usize = 3;
const E: usize = 4;
const F: usize = 5;

// --- threshold ---

mod threshold {
    use super::*;

    #[test]
    fn one_vote_forgets_when_two_active() {
        // given: two members, A and B
        let mut w = setup(2);
        // when: A votes to forget B
        let ledger = w.forget(A, B, 100).ledger();
        // then: B is forgotten at the time of the vote, A is the only voter, one vote was enough
        assert!(ledger.is_forgotten(&w.id(B)) && !ledger.is_member(&w.id(B)));
        w.assert_round(&ledger, B, &[A], Some(100));
        assert_eq!(ledger.need, 1);
    }

    #[test]
    fn one_vote_waits_when_three_active() {
        // given: three members
        let mut w = setup(3);
        // when: A votes to forget C
        let ledger = w.forget(A, C, 100).ledger();
        // then: C is still a member, the round waits with A as the proposer, two votes are needed
        assert!(ledger.is_member(&w.id(C)) && !ledger.is_forgotten(&w.id(C)));
        w.assert_round(&ledger, C, &[A], None);
        assert_eq!(ledger.need, 2);
    }

    #[test]
    fn second_vote_forgets_when_three_active() {
        // given: three members; A voted to forget C at 100
        let mut w = setup(3);
        w.forget(A, C, 100);
        // when: B votes to forget C as well, at 200
        let ledger = w.forget(B, C, 200).ledger();
        // then: C is forgotten at the time of the second vote, voters in time order
        assert_eq!(ledger.forgotten_at(&w.id(C)), Some(200));
        w.assert_round(&ledger, C, &[A, B], Some(200));
        assert!(ledger.is_member(&w.id(A)) && ledger.is_member(&w.id(B)));
    }
}

// --- cascade ---

mod cascade {
    use super::*;

    #[test]
    fn forgetting_drops_need_and_settles_waiting_round() {
        // given: three members; A waits on C (10) and on B (20)
        let mut w = setup(3);
        w.forget(A, C, 10).forget(A, B, 20);
        // when: C votes to forget B (30), the second vote that forgets B
        let ledger = w.forget(C, B, 30).ledger();
        // then: two are left, one vote is enough, so C is forgotten at the same moment by A's vote
        w.assert_round(&ledger, B, &[A, C], Some(30));
        w.assert_round(&ledger, C, &[A], Some(30));
        assert!(ledger.is_member(&w.id(A)));
    }

    #[test]
    fn forgetting_to_three_active_keeps_other_round_waiting() {
        // given: four members; A waits on C (20) and A voted to forget D (10)
        let mut w = setup(4);
        w.forget(A, D, 10).forget(A, C, 20);
        // when: B votes to forget D (30)
        let ledger = w.forget(B, D, 30).ledger();
        // then: D is forgotten, three are left and two votes are still needed: C waits on A alone
        assert_eq!(ledger.forgotten_at(&w.id(D)), Some(30));
        w.assert_round(&ledger, C, &[A], None);
        assert!(ledger.is_member(&w.id(C)));
        assert_eq!(ledger.need, 2);
    }

    #[test]
    fn exclusion_lowering_need_completes_pending_removal() {
        // given: three members; A waits on C
        let mut w = setup(3);
        w.forget(A, C, 100);
        // when: A and C vote to exclude B (20 s, 30 s), so B is excluded
        let ledger = w.exclude(A, B, 20).exclude(C, B, 30).ledger();
        // then: two are left, A's single vote forgets C, and C's earlier exclusion vote still stands
        assert!(ledger.excluded.contains_key(&w.id(B)));
        assert_eq!(ledger.forgotten_at(&w.id(C)), Some(100));
    }
}

// --- growth of the network ---

mod growth {
    use super::*;

    #[test]
    fn newcomer_reopens_single_vote_removal() {
        // given: two members; A voted to forget B at 100, so B was forgotten by one vote
        let mut w = setup(2);
        w.forget(A, B, 100);
        assert!(w.ledger().is_forgotten(&w.id(B)));
        // when: A admits a newcomer at 150
        let ledger = w.admit(A, pid(90), Via::Invite, 150).ledger();
        // then: three are active, B is a member again, the round is [A] and a second vote is needed
        assert!(ledger.is_member(&w.id(B)) && ledger.is_member(&pid(90)));
        w.assert_round(&ledger, B, &[A], None);
        assert_eq!(ledger.need, 2);
    }
}

// --- mutual votes ---

mod mutual_votes {
    use super::*;

    #[test]
    fn mutual_votes_forget_only_the_earlier_target() {
        // given: two members; A votes to forget B at 100, B votes to forget A at 200
        let mut w = setup(2);
        w.forget(A, B, 100).forget(B, A, 200);
        // when: tallied
        let ledger = w.ledger();
        // then: B is forgotten, its vote is skipped, A stays a member and has no round
        assert!(ledger.is_forgotten(&w.id(B)));
        assert!(ledger.is_member(&w.id(A)) && !ledger.forgets.contains_key(&w.id(A)));
    }

    #[test]
    fn forgotten_author_waiting_votes_dropped() {
        // given: three members; B waits on A (50), then A (100) and C (120) vote to forget B
        let mut w = setup(3);
        w.forget(B, A, 50).forget(A, B, 100).forget(C, B, 120);
        // when: tallied
        let ledger = w.ledger();
        // then: B is forgotten, its waiting vote is gone with the round on A, A is a member
        w.assert_round(&ledger, B, &[A, C], Some(120));
        assert!(!ledger.forgets.contains_key(&w.id(A)));
        assert!(ledger.is_member(&w.id(A)));
    }
}

// --- withdrawn votes ---

mod withdrawn_votes {
    use super::*;

    #[test]
    fn withdrawn_second_vote_returns_member() {
        // given: three members; A and B forgot C
        let mut w = setup(3);
        w.forget(A, C, 100).forget(B, C, 110);
        assert!(w.ledger().is_forgotten(&w.id(C)), "two votes forget C");
        // when: B withdraws the vote (the tombstone leaves the input)
        let ledger = w.without_forget(B, C).ledger();
        // then: C is a member again and the round is [A]
        assert!(ledger.is_member(&w.id(C)));
        w.assert_round(&ledger, C, &[A], None);
    }
}

// --- marks taken from the raw records ---

mod marks {
    use super::*;

    #[test]
    fn forget_mark_counts_uncounted_votes() {
        // given: three members; A and C exclude B (20 s, 30 s)
        let mut w = setup(3);
        w.exclude(A, B, 20).exclude(C, B, 30);
        // when: B's vote to forget C (300) is skipped as the vote of an excluded author
        let ledger = w.forget(B, C, 300).ledger();
        // then: it forgets nobody, yet its time is the mark
        assert!(ledger.excluded.contains_key(&w.id(B)));
        assert!(ledger.is_member(&w.id(C)) && !ledger.forgets.contains_key(&w.id(C)));
        assert_eq!(ledger.forget_mark(&w.id(C)), Some(300));
    }

    #[test]
    fn forget_mark_is_the_latest_vote_whatever_the_order() {
        // given: four members; B voted to forget D at 300 and A at 200, listed in that order
        let mut w = setup(4);
        w.forget(B, D, 300).forget(A, D, 200);
        // when: tallied
        let ledger = w.ledger();
        // then: the mark is the later time; a node without votes has no mark
        assert_eq!(ledger.forget_mark(&w.id(D)), Some(300));
        assert_eq!(ledger.forget_mark(&w.id(C)), None);
    }

    #[test]
    fn last_admit_unix_counts_dead_admits() {
        // given: three members; A and C forgot B (100, 110)
        let newcomer = pid(90);
        let mut w = setup(3);
        w.forget(A, B, 100).forget(C, B, 110);
        // when: B admits a newcomer at 150, after it was forgotten
        let ledger = w.admit(B, newcomer, Via::Invite, 150).ledger();
        // then: the admission is dead, the newcomer is no member, but the mark keeps its time
        assert!(!ledger.is_member(&newcomer));
        assert_eq!(ledger.last_admit_unix(&newcomer), Some(150));
    }
}

// --- whose votes count ---

mod voters {
    use super::*;

    #[test]
    fn vote_of_forgotten_author_after_forget_ignored() {
        // given: four members; A and C forgot B (100, 110)
        let mut w = setup(4);
        w.forget(A, B, 100).forget(C, B, 110);
        // when: B votes to forget D (200) and A does (300)
        let ledger = w.forget(B, D, 200).forget(A, D, 300).ledger();
        // then: B's vote is skipped: D waits on A alone
        assert!(ledger.is_forgotten(&w.id(B)));
        w.assert_round(&ledger, D, &[A], None);
        assert!(ledger.is_member(&w.id(D)));
    }

    #[test]
    fn vote_before_author_forgotten_counts() {
        // given: four members; B and A forgot D (50, 60)
        let mut w = setup(4);
        w.forget(B, D, 50).forget(A, D, 60);
        // when: A and C forget B later (100, 110)
        let ledger = w.forget(A, B, 100).forget(C, B, 110).ledger();
        // then: both are forgotten; B's earlier vote stays in the round of D
        w.assert_round(&ledger, D, &[B, A], Some(60));
        assert_eq!(ledger.forgotten_at(&w.id(B)), Some(110));
    }

    #[test]
    fn excluded_subject_has_no_round() {
        // given: three members; A and C vote to exclude B (20 s, 30 s)
        let mut w = setup(3);
        w.exclude(A, B, 20).exclude(C, B, 30);
        // when: A votes to forget B (100)
        let ledger = w.forget(A, B, 100).ledger();
        // then: B is excluded and has no forget round
        assert!(ledger.excluded.contains_key(&w.id(B)));
        assert!(!ledger.forgets.contains_key(&w.id(B)));
    }

    #[test]
    fn exclusion_votes_need_counts_without_forgotten() {
        // given: four members; A and B forgot D, three are active
        let mut w = setup(4);
        w.forget(A, D, 100).forget(B, D, 110);
        // when: A votes to exclude B
        let ledger = w.exclude(A, B, 200).ledger();
        // then: the vote waits for a second one
        let want = PendingVote {
            voters: vec![w.id(A)],
            need: 2,
        };
        assert!(ledger.is_forgotten(&w.id(D)));
        assert_eq!(ledger.pending.get(&w.id(B)), Some(&want));
        assert!(ledger.excluded.is_empty());
    }

    #[test]
    fn exclusion_needs_one_vote_when_forgotten_leave_two_active() {
        // given: three members; A and B forgot C, two are active
        let mut w = setup(3);
        w.forget(A, C, 100).forget(B, C, 110);
        // when: A votes to exclude B
        let ledger = w.exclude(A, B, 200).ledger();
        // then: one vote excludes
        assert!(ledger.excluded.contains_key(&w.id(B)));
    }
}

// --- a batch of votes ---

mod batch {
    use super::*;

    #[test]
    fn batch_of_votes_waits_each_for_second() {
        // given: six members; A votes to forget D, E and F (100..102)
        let mut w = setup(6);
        w.forget(A, D, 100).forget(A, E, 101).forget(A, F, 102);
        // when: tallied
        let ledger = w.ledger();
        // then: all three are members, each round holds A alone
        for who in [D, E, F] {
            assert!(ledger.is_member(&w.id(who)), "node {who} is a member");
            w.assert_round(&ledger, who, &[A], None);
        }
        // when: B votes for the same three (200..202)
        w.forget(B, D, 200).forget(B, E, 201).forget(B, F, 202);
        let ledger = w.ledger();
        // then: all three are forgotten, each at the time of B's vote
        for (who, at) in [(D, 200), (E, 201), (F, 202)] {
            w.assert_round(&ledger, who, &[A, B], Some(at));
        }
    }
}

// --- setup and helpers ---

/// The records of one scene, built step by step; the nodes come from `net(n)`.
struct World {
    me: PeerId,
    peers: Vec<PeerId>,
    admits: Vec<AdmitIn>,
    forgets: Vec<ForgetIn>,
    votes: Vec<VoteIn>,
}

/// `n` active members admitted by the own node, no votes yet.
fn setup(n: usize) -> World {
    let (me, peers, admits) = net(n);
    World {
        me,
        peers,
        admits,
        forgets: Vec::new(),
        votes: Vec::new(),
    }
}

impl World {
    fn id(&self, who: usize) -> PeerId {
        self.peers[who]
    }

    /// A forget vote of `author` on `subject`, written at `made_unix`.
    fn forget(&mut self, author: usize, subject: usize, made_unix: u64) -> &mut World {
        let vote = forget(self.id(author), self.id(subject), made_unix);
        self.forgets.push(vote);
        self
    }

    /// The vote is withdrawn: its record is gone from the live input.
    fn without_forget(&mut self, author: usize, subject: usize) -> &mut World {
        let pair = (self.id(author), self.id(subject));
        self.forgets.retain(|f| (f.author, f.subject) != pair);
        self
    }

    /// An exclusion vote of `voter` on `subject`, cast `secs` seconds after the epoch.
    fn exclude(&mut self, voter: usize, subject: usize, secs: u64) -> &mut World {
        let vote = VoteIn {
            voter: self.id(voter),
            subject: self.id(subject),
            ts_micros: secs * 1_000_000,
        };
        self.votes.push(vote);
        self
    }

    /// An admit record of member `author` about any node.
    fn admit(&mut self, author: usize, subject: PeerId, via: Via, made_unix: u64) -> &mut World {
        let author = self.id(author);
        self.admits.push(AdmitIn {
            author,
            subject,
            via,
            made_unix,
        });
        self
    }

    fn ledger(&self) -> Ledger {
        tally(&TallyInput {
            me: self.me,
            admits: self.admits.clone(),
            votes: self.votes.clone(),
            forgets: self.forgets.clone(),
        })
    }

    /// The ledger holds a round on `who` with exactly these voters; `at_unix` is set once forgotten.
    fn assert_round(&self, ledger: &Ledger, who: usize, voters: &[usize], at_unix: Option<u64>) {
        let voters = voters.iter().map(|v| self.id(*v)).collect();
        let want = ForgetRound { voters, at_unix };
        assert_eq!(ledger.forgets.get(&self.id(who)), Some(&want));
    }
}
