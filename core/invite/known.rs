//! Fills bare member entries of an invite with an address the node itself has proven.

use crate::transport::net::{PeerAddr, PeerId};

/// Replaces every entry without hints by `known(id)` when that gives one; other entries,
/// their order and their count stay as they are.
pub(crate) fn with_known(
    list: Vec<PeerAddr>,
    known: impl Fn(PeerId) -> Option<PeerAddr>,
) -> Vec<PeerAddr> {
    list.into_iter().map(|entry| fill(entry, &known)).collect()
}

fn fill(entry: PeerAddr, known: &impl Fn(PeerId) -> Option<PeerAddr>) -> PeerAddr {
    match entry.hints.is_empty() {
        true => known(entry.id).unwrap_or(entry),
        false => entry,
    }
}

#[cfg(test)]
#[path = "known_tests.rs"]
mod tests;
