//! The "overloaded" state as a pure function of the previous state, the buckets and the time.

use super::buckets::Ring;
use super::counts::{MemberCounts, ProcCounts};

/// Buckets of the entry window (two minutes).
pub(super) const ENTER_BUCKETS_WINDOW: u64 = 8;
/// Refusals of one member in the entry window, at least.
pub(super) const ENTER_REFUSED: u64 = 10;
/// Share of refusals among the same member's attempts, percent, at least.
pub(super) const ENTER_SHARE: u64 = 20;
/// The member's refusals must lie in at least this many buckets.
pub(super) const ENTER_BUCKETS: usize = 2;
/// Buckets of the leave window (five minutes).
pub(super) const LEAVE_WINDOW: u64 = 20;
/// Overload is lifted when the process refused no more than this in the leave window.
pub(super) const CALM_REFUSED: u64 = 2;

/// Why streams are refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reason {
    /// The cap on all streams.
    Limit,
    /// One member reached its share.
    Share,
    /// The process memory guard.
    Memory,
}

impl Reason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Reason::Limit => "limit",
            Reason::Share => "share",
            Reason::Memory => "memory",
        }
    }
}

/// The state and, while overloaded, the reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Verdict {
    pub(super) overloaded: bool,
    pub(super) reason: Option<Reason>,
}

/// One member has enough refusals, a high enough share of its own attempts, in two buckets.
fn member_enters(ring: &Ring<MemberCounts>, now: u64) -> bool {
    let sum = ring.total(now, ENTER_BUCKETS_WINDOW);
    let spread = ring
        .buckets(now, ENTER_BUCKETS_WINDOW)
        .iter()
        .filter(|b| b.refused > 0)
        .count();
    sum.refused >= ENTER_REFUSED
        && sum.refused * 100 >= ENTER_SHARE * sum.attempts()
        && spread >= ENTER_BUCKETS
}

/// The most frequent reason in the entry window; the leave window if the entry one is empty.
/// A tie goes to memory, then to the cap, then to the share.
fn reason_of(procs: &Ring<ProcCounts>, now: u64) -> Reason {
    let mut sum = procs.total(now, ENTER_BUCKETS_WINDOW);
    if sum.refused() == 0 {
        sum = procs.total(now, LEAVE_WINDOW);
    }
    [Reason::Share, Reason::Limit, Reason::Memory]
        .into_iter()
        .max_by_key(|r| sum.of(*r))
        .expect("the list is not empty")
}

/// Overloaded: some member enters, or it was overloaded and refusals have not calmed down.
pub(super) fn decide<'a>(
    was: bool,
    procs: &Ring<ProcCounts>,
    members: impl Iterator<Item = &'a Ring<MemberCounts>>,
    now: u64,
) -> Verdict {
    let held = was && procs.total(now, LEAVE_WINDOW).refused() > CALM_REFUSED;
    let overloaded = held || members.into_iter().any(|m| member_enters(m, now));
    Verdict {
        overloaded,
        reason: overloaded.then(|| reason_of(procs, now)),
    }
}

#[cfg(test)]
#[path = "verdict_tests.rs"]
mod tests;
