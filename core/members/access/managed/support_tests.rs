//! Shared helpers of the tests of the managed tally: a scene of records written one after another.

// assumed: this file is declared in `managed/mod.rs` as
// `#[cfg(test)] #[path = "support_tests.rs"] mod support;`.

use crate::members::access::authority::{Power, PowerIn};
use crate::members::access::record::Via;
use crate::members::access::tally::{tally, Ledger, TallyInput, VoteIn};
use crate::transport::net::PeerId;

/// Microseconds in a second: the scene counts time in seconds.
const SECOND: u64 = 1_000_000;

/// The records of one network of one network, built step by step.
pub(super) struct Scene {
    pub(super) root: PeerId,
    me: PeerId,
    powers: Vec<PowerIn>,
    votes: Vec<VoteIn>,
}

impl Scene {
    /// An empty network created by `root`, seen by `root`.
    pub(super) fn new(root: PeerId) -> Scene {
        Scene {
            root,
            me: root,
            powers: vec![],
            votes: vec![],
        }
    }

    /// The tally is taken at this node.
    pub(super) fn seen_by(&mut self, me: PeerId) -> &mut Scene {
        self.me = me;
        self
    }

    /// `author` admits `subject` by an invitation, at second `at`.
    pub(super) fn admit(&mut self, at: u64, author: PeerId, subject: PeerId) -> &mut Scene {
        self.admit_via(at, author, subject, Via::Invite)
    }

    /// `author` vouches for `subject` as a witness, at second `at`.
    pub(super) fn witness(&mut self, at: u64, author: PeerId, subject: PeerId) -> &mut Scene {
        self.admit_via(at, author, subject, Via::Witness)
    }

    pub(super) fn admit_via(
        &mut self,
        at: u64,
        author: PeerId,
        subject: PeerId,
        via: Via,
    ) -> &mut Scene {
        self.power(at * SECOND, author, Power::Admit(via), subject)
    }

    pub(super) fn grant(&mut self, at: u64, author: PeerId, subject: PeerId) -> &mut Scene {
        self.power(at * SECOND, author, Power::Grant, subject)
    }

    pub(super) fn revoke(&mut self, at: u64, author: PeerId, subject: PeerId) -> &mut Scene {
        self.power(at * SECOND, author, Power::Revoke, subject)
    }

    /// The close record names its author as the subject.
    pub(super) fn close(&mut self, at: u64, author: PeerId) -> &mut Scene {
        self.close_micros(at * SECOND, author)
    }

    pub(super) fn close_micros(&mut self, ts_micros: u64, author: PeerId) -> &mut Scene {
        self.power(ts_micros, author, Power::Close, author)
    }

    /// `voter` votes to exclude `subject`.
    pub(super) fn exclude(&mut self, at: u64, voter: PeerId, subject: PeerId) -> &mut Scene {
        self.votes.push(VoteIn {
            voter,
            subject,
            ts_micros: at * SECOND,
        });
        self
    }

    /// `who` leaves: a vote against itself.
    pub(super) fn leave(&mut self, at: u64, who: PeerId) -> &mut Scene {
        self.exclude(at, who, who)
    }

    fn power(
        &mut self,
        ts_micros: u64,
        author: PeerId,
        power: Power,
        subject: PeerId,
    ) -> &mut Scene {
        self.powers.push(PowerIn {
            author,
            power,
            subject,
            ts_micros,
        });
        self
    }

    /// The input of the scene.
    pub(super) fn input(&self) -> TallyInput {
        TallyInput {
            me: self.me,
            root: self.root,
            votes: self.votes.clone(),
            powers: self.powers.clone(),
        }
    }

    pub(super) fn ledger(&self) -> Ledger {
        tally(&self.input())
    }
}

/// Every order of the items (Heap's algorithm); `n!` vectors.
pub(super) fn permutations<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    let mut work = items.to_vec();
    let mut out = vec![work.clone()];
    let mut state = vec![0usize; work.len()];
    let mut i = 0;
    while i < work.len() {
        if state[i] < i {
            let j = if i % 2 == 0 { 0 } else { state[i] };
            work.swap(j, i);
            out.push(work.clone());
            state[i] += 1;
            i = 0;
        } else {
            state[i] = 0;
            i += 1;
        }
    }
    out
}
