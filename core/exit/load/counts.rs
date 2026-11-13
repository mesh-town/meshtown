//! What one bucket holds, per process and per member.

use super::buckets::Tally;
use super::verdict::Reason;

/// A decision about one new stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Admitted,
    Refused(Reason),
}

/// Decisions of the whole process in one bucket.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub(super) struct ProcCounts {
    pub(super) admitted: u64,
    pub(super) limit: u64,
    pub(super) share: u64,
    pub(super) memory: u64,
}

impl ProcCounts {
    pub(super) fn refused(&self) -> u64 {
        self.limit + self.share + self.memory
    }

    pub(super) fn note(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Admitted => self.admitted += 1,
            Outcome::Refused(Reason::Limit) => self.limit += 1,
            Outcome::Refused(Reason::Share) => self.share += 1,
            Outcome::Refused(Reason::Memory) => self.memory += 1,
        }
    }

    pub(super) fn of(&self, reason: Reason) -> u64 {
        match reason {
            Reason::Limit => self.limit,
            Reason::Share => self.share,
            Reason::Memory => self.memory,
        }
    }
}

impl Tally for ProcCounts {
    fn add(&mut self, o: &Self) {
        self.admitted += o.admitted;
        self.limit += o.limit;
        self.share += o.share;
        self.memory += o.memory;
    }
}

/// Decisions about one member in one bucket.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub(super) struct MemberCounts {
    pub(super) admitted: u64,
    pub(super) refused: u64,
}

impl MemberCounts {
    pub(super) fn note(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Admitted => self.admitted += 1,
            Outcome::Refused(_) => self.refused += 1,
        }
    }

    pub(super) fn attempts(&self) -> u64 {
        self.admitted + self.refused
    }
}

impl Tally for MemberCounts {
    fn add(&mut self, o: &Self) {
        self.admitted += o.admitted;
        self.refused += o.refused;
    }
}
