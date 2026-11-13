//! Tests of the overload state: entering, holding, leaving and the reason.

use super::*;

/// Member rings and the process ring filled from `(bucket, admitted, refused, reason)` events.
struct Board {
    procs: Ring<ProcCounts>,
    members: Vec<Ring<MemberCounts>>,
}

impl Board {
    fn new(members: usize) -> Self {
        Self {
            procs: Ring::new(),
            members: (0..members).map(|_| Ring::new()).collect(),
        }
    }

    fn admit(&mut self, member: usize, bucket: u64, n: u64) {
        self.procs.at(bucket).admitted += n;
        self.members[member].at(bucket).admitted += n;
    }

    fn refuse(&mut self, member: usize, bucket: u64, n: u64, reason: Reason) {
        let proc = self.procs.at(bucket);
        match reason {
            Reason::Limit => proc.limit += n,
            Reason::Share => proc.share += n,
            Reason::Memory => proc.memory += n,
        }
        self.members[member].at(bucket).refused += n;
    }

    fn decide(&self, was: bool, now: u64) -> Verdict {
        decide(was, &self.procs, self.members.iter(), now)
    }
}

#[test]
fn burst_in_one_bucket_does_not_enter() {
    // given: 30 refusals to one member in one bucket
    let mut board = Board::new(1);
    board.refuse(0, 3, 30, Reason::Share);

    // when: the state is read in that bucket
    let verdict = board.decide(false, 3);

    // then: not overloaded
    assert!(!verdict.overloaded, "one burst is not overload");
}

#[test]
fn refusals_in_two_buckets_enter() {
    // given: 10 refusals out of 12 attempts of one member in two buckets
    let mut board = Board::new(1);
    board.refuse(0, 2, 5, Reason::Share);
    board.refuse(0, 3, 5, Reason::Share);
    board.admit(0, 3, 2);

    // when / then: overloaded
    assert!(board.decide(false, 3).overloaded);
}

#[test]
fn low_share_of_one_member_does_not_enter() {
    // given: 10 refusals out of 100 attempts of the same member
    let mut board = Board::new(1);
    board.refuse(0, 2, 5, Reason::Share);
    board.refuse(0, 3, 5, Reason::Share);
    board.admit(0, 3, 90);

    // when / then: not overloaded
    assert!(!board.decide(false, 3).overloaded);
}

#[test]
fn busy_neighbour_does_not_hide_a_refused_member() {
    // given: member A has 10 refusals of 12, member B has 500 admitted
    let mut board = Board::new(2);
    board.refuse(0, 2, 5, Reason::Share);
    board.refuse(0, 3, 5, Reason::Share);
    board.admit(0, 3, 2);
    board.admit(1, 3, 500);

    // when / then: overloaded
    assert!(board.decide(false, 3).overloaded);
}

#[test]
fn refusals_spread_over_members_do_not_enter() {
    // given: four refusals to each of three members
    let mut board = Board::new(3);
    for member in 0..3 {
        board.refuse(member, 2, 2, Reason::Limit);
        board.refuse(member, 3, 2, Reason::Limit);
    }

    // when / then: not overloaded
    assert!(!board.decide(false, 3).overloaded);
}

#[test]
fn stays_overloaded_until_refusals_leave_five_minutes() {
    // given: it was overloaded and the last refusals sit in bucket 0
    let mut board = Board::new(1);
    board.refuse(0, 0, 5, Reason::Share);

    // when: read 299 s and 301 s after them (buckets 19 and 20)
    let held = board.decide(true, 19);
    let gone = board.decide(true, 20);

    // then: still overloaded, then not
    assert!(held.overloaded, "299 s after the last refusal");
    assert!(!gone.overloaded, "301 s after the last refusal");
}

#[test]
fn two_stray_refusals_do_not_keep_it() {
    // given: it was overloaded, two refusals in the leave window
    let mut two = Board::new(1);
    two.refuse(0, 0, 2, Reason::Share);
    let mut three = Board::new(1);
    three.refuse(0, 0, 3, Reason::Share);

    // when / then: two release it, three hold it
    assert!(!two.decide(true, 10).overloaded);
    assert!(three.decide(true, 10).overloaded);
}

#[test]
fn reason_is_the_most_frequent() {
    // given: 8 share and 3 limit refusals; and another process with memory and limit equal
    let mut mixed = Board::new(1);
    mixed.refuse(0, 2, 4, Reason::Share);
    mixed.refuse(0, 3, 4, Reason::Share);
    mixed.refuse(0, 3, 3, Reason::Limit);
    let mut tie = Board::new(1);
    tie.refuse(0, 2, 4, Reason::Memory);
    tie.refuse(0, 2, 4, Reason::Limit);
    tie.refuse(0, 3, 2, Reason::Share);

    // when: both are read in bucket 3
    let first = mixed.decide(false, 3);
    let second = tie.decide(false, 3);

    // then: share wins the first; memory wins the tie with the cap
    assert_eq!(first.reason, Some(Reason::Share));
    assert_eq!(second.reason, Some(Reason::Memory));
}
