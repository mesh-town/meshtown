//! Vote tally of the membership list: a pure deterministic function from admit records and
//! exclusion votes to the ledger (who is a member, who is excluded, what is pending).
//!
//! What it does: one forward pass over the votes in time order. Who is admitted comes from
//! `admission::admit_closure`: one confirmation per newcomer, a fixed point independent of the order.
//! Forget votes are settled by `forget::settle_forgets` on every recount; admissions a node made after it was
//! forgotten leave the closure, and a forgotten node's exclusion votes count only up to its forgetting.
//! An exclusion narrows the reach and lowers the exclusion thresholds, so a pending removal may complete during the
//! pass, but the excluded only grow, so there is no oscillation and a cascade is the end of a pass.
//! What it doesn't do: no replica, no clock, no I/O; blocks, the boot marker and ignored entries are not its business.

mod pass;
mod types;
mod votes;

pub use super::authority::Rules;
pub use super::forget::{ForgetIn, ForgetRound};
pub use pass::tally;
pub use types::{
    Admission, AdmitIn, Closed, Exclusion, How, Ledger, PendingVote, TallyInput, VoteIn,
};
pub(super) use votes::Excluded;

// Names the test modules take through `use super::*`.
#[cfg(test)]
use {super::record::Via, pass::Pass};

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "cascade_tests.rs"]
mod cascade_tests;

#[cfg(test)]
#[path = "support_tests.rs"]
mod support;

#[cfg(test)]
#[path = "forget_tests.rs"]
mod forget_tests;

#[cfg(test)]
#[path = "forget_order_tests.rs"]
mod forget_order_tests;

#[cfg(test)]
#[path = "forget_vote_tests.rs"]
mod forget_vote_tests;

#[cfg(test)]
#[path = "forget_marks_tests.rs"]
mod forget_marks_tests;

#[cfg(test)]
#[path = "forget_need_tests.rs"]
mod forget_need_tests;

#[cfg(test)]
#[path = "characterization_tests.rs"]
mod characterization_tests;

#[cfg(test)]
#[path = "characterization_cascade_tests.rs"]
mod characterization_cascade_tests;

#[cfg(test)]
#[path = "characterization_forget_tests.rs"]
mod characterization_forget_tests;

#[cfg(test)]
#[path = "characterization_forget_return_tests.rs"]
mod characterization_forget_return_tests;
