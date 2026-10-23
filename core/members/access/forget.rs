//! Types of the forget votes (`acl/forget/` records) and the ledger accessors over them.

use crate::transport::net::PeerId;

use super::tally::Ledger;

/// A live forget vote: `author` votes to forget `subject`, written at `made_unix`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForgetIn {
    pub author: PeerId,
    pub subject: PeerId,
    pub made_unix: u64,
}

/// A vote circle on one subject; `voters[0]` proposed. `at_unix` is set once the subject is forgotten.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForgetRound {
    pub voters: Vec<PeerId>,
    pub at_unix: Option<u64>,
}

impl Ledger {
    pub fn is_forgotten(&self, p: &PeerId) -> bool {
        self.forgotten_at(p).is_some()
    }

    /// When the subject was forgotten, if it was.
    pub fn forgotten_at(&self, p: &PeerId) -> Option<u64> {
        self.forgets.get(p)?.at_unix
    }

    /// Latest `made_unix` among the live forget records about `p`.
    pub fn forget_mark(&self, p: &PeerId) -> Option<u64> {
        self.forget_marks.get(p).copied()
    }

    /// Latest `made_unix` among the live admissions of `p` that count elsewhere.
    pub fn last_admit_unix(&self, p: &PeerId) -> Option<u64> {
        self.admit_unix.get(p).copied()
    }
}
