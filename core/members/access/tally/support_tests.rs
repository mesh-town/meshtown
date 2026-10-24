//! Shared helpers of the forget tests of the tally: record builders, runners and the step-by-step scene.

use super::*;
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;

pub(super) const A: usize = 0;
pub(super) const B: usize = 1;
pub(super) const C: usize = 2;
pub(super) const D: usize = 3;
pub(super) const E: usize = 4;
pub(super) const F: usize = 5;

pub(super) fn forget(author: PeerId, subject: PeerId, made_unix: u64) -> ForgetIn {
    ForgetIn {
        author,
        subject,
        made_unix,
    }
}

/// A network of `n` active members: `peers[0]` is the own node `me`, and `me` admits all the
/// others (`Via::Invite`, `made_unix` 0). Returns `me`, all `n` members and the admit records.
pub(super) fn net(n: usize) -> (PeerId, Vec<PeerId>, Vec<AdmitIn>) {
    let peers: Vec<PeerId> = (1..=n as u8).map(pid).collect();
    let me = peers[0];
    let admits = peers[1..]
        .iter()
        .map(|p| admit_at(me, *p, Via::Invite, 0))
        .collect();
    (me, peers, admits)
}

pub(super) fn admit_at(author: PeerId, subject: PeerId, via: Via, made_unix: u64) -> AdmitIn {
    AdmitIn {
        author,
        subject,
        via,
        made_unix,
    }
}

/// Rewrites the record of the pair (`author`, `subject`), as the replica does: one record per pair.
pub(super) fn rewrite(
    admits: Vec<AdmitIn>,
    author: PeerId,
    subject: PeerId,
    via: Via,
    made_unix: u64,
) -> Vec<AdmitIn> {
    let mut rest: Vec<AdmitIn> = admits
        .into_iter()
        .filter(|a| !(a.author == author && a.subject == subject))
        .collect();
    rest.push(admit_at(author, subject, via, made_unix));
    rest
}

pub(super) fn vote_in(voter: PeerId, subject: PeerId, secs: u64) -> VoteIn {
    VoteIn {
        voter,
        subject,
        ts_micros: secs * 1_000_000,
    }
}

pub(super) fn round(voters: &[PeerId], at_unix: Option<u64>) -> ForgetRound {
    ForgetRound {
        voters: voters.to_vec(),
        at_unix,
    }
}

pub(super) fn run(me: PeerId, admits: Vec<AdmitIn>, forgets: Vec<ForgetIn>) -> Ledger {
    run_votes(me, admits, forgets, Vec::new())
}

pub(super) fn run_votes(
    me: PeerId,
    admits: Vec<AdmitIn>,
    forgets: Vec<ForgetIn>,
    votes: Vec<VoteIn>,
) -> Ledger {
    tally(&TallyInput {
        me,
        admits,
        votes,
        forgets,
    })
}

pub(super) fn rev<T: Clone>(items: &[T]) -> Vec<T> {
    items.iter().rev().cloned().collect()
}

pub(super) fn rot<T: Clone>(items: &[T], by: usize) -> Vec<T> {
    let mut out = items.to_vec();
    out.rotate_left(by % items.len().max(1));
    out
}

// --- scene ---

/// The records of one scene, built step by step; the nodes come from `net(n)`.
pub(super) struct World {
    me: PeerId,
    peers: Vec<PeerId>,
    admits: Vec<AdmitIn>,
    forgets: Vec<ForgetIn>,
    votes: Vec<VoteIn>,
}

/// `n` active members admitted by the own node, no votes yet.
pub(super) fn setup(n: usize) -> World {
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
    pub(super) fn id(&self, who: usize) -> PeerId {
        self.peers[who]
    }

    /// A forget vote of `author` on `subject`, written at `made_unix`.
    pub(super) fn forget(&mut self, author: usize, subject: usize, made_unix: u64) -> &mut World {
        let vote = forget(self.id(author), self.id(subject), made_unix);
        self.forgets.push(vote);
        self
    }

    /// The vote is withdrawn: its record is gone from the live input.
    pub(super) fn without_forget(&mut self, author: usize, subject: usize) -> &mut World {
        let pair = (self.id(author), self.id(subject));
        self.forgets.retain(|f| (f.author, f.subject) != pair);
        self
    }

    /// An exclusion vote of `voter` on `subject`, cast `secs` seconds after the epoch.
    pub(super) fn exclude(&mut self, voter: usize, subject: usize, secs: u64) -> &mut World {
        let vote = VoteIn {
            voter: self.id(voter),
            subject: self.id(subject),
            ts_micros: secs * 1_000_000,
        };
        self.votes.push(vote);
        self
    }

    /// An admit record of member `author` about any node.
    pub(super) fn admit(&mut self, author: usize, subject: PeerId, via: Via, made_unix: u64) -> &mut World {
        let author = self.id(author);
        self.admits.push(AdmitIn {
            author,
            subject,
            via,
            made_unix,
        });
        self
    }

    pub(super) fn ledger(&self) -> Ledger {
        tally(&TallyInput {
            me: self.me,
            admits: self.admits.clone(),
            votes: self.votes.clone(),
            forgets: self.forgets.clone(),
        })
    }

    /// The ledger holds a round on `who` with exactly these voters; `at_unix` is set once forgotten.
    pub(super) fn assert_round(&self, ledger: &Ledger, who: usize, voters: &[usize], at_unix: Option<u64>) {
        let voters = voters.iter().map(|v| self.id(*v)).collect();
        let want = ForgetRound { voters, at_unix };
        assert_eq!(ledger.forgets.get(&self.id(who)), Some(&want));
    }
}
