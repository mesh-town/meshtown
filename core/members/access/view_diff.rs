//! Differences between two published views: who was newly denied and who was newly allowed.
//!
//! What it does: compares a view with the previous one over the nodes either of them knows.
//! What it doesn't do: no state, no I/O.

use std::collections::BTreeSet;

use super::view::{Deny, Verdicts};
use crate::transport::net::PeerId;

impl Verdicts {
    /// The nodes denied by this view that the previous one (if any) let in.
    pub(super) fn newly_denied(&self, old: Option<&Verdicts>) -> Vec<(PeerId, Deny)> {
        let before = old.into_iter().flat_map(|o| o.known());
        let candidates: BTreeSet<PeerId> = self.known().into_iter().chain(before).collect();
        candidates
            .into_iter()
            .filter(|p| old.is_none_or(|o| o.check(*p).is_ok()))
            .filter_map(|p| self.check(p).err().map(|deny| (p, deny)))
            .collect()
    }

    /// The nodes allowed by this view that the previous one denied; nothing for the first view.
    pub(super) fn newly_allowed(&self, old: Option<&Verdicts>) -> BTreeSet<PeerId> {
        let Some(o) = old else {
            return BTreeSet::new();
        };
        let candidates: BTreeSet<PeerId> = self.known().into_iter().chain(o.known()).collect();
        let denied_before = |p: &PeerId| o.check(*p).is_err();
        candidates
            .into_iter()
            .filter(denied_before)
            .filter(|p| self.check(*p).is_ok())
            .collect()
    }
}
