//! The replica's in-memory state: the map "(author, key) → entry", merging, the author's clock, summaries for reconciliation.

use super::entry::{check, to_entry, wins, WireEntry};
use crate::net::id::PeerId;
use crate::net::replica::{ReplicaEntry, ReplicaId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A per-author summary for reconciliation: where it differs between two nodes, those author entries are what is missing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WriterSum {
    pub author: [u8; 32],
    pub max_ts_micros: u64,
    pub count: u32,
    pub digest: [u8; 32],
}

pub(crate) struct State {
    id: ReplicaId,
    me: PeerId,
    entry_by_author_key: BTreeMap<([u8; 32], Vec<u8>), WireEntry>,
    /// The greatest `ts_micros` of its own entries (including those restored by reconciliation).
    my_max_ts_micros: u64,
    /// Grows with every merge that changed the map: it shows that a session managed to merge something.
    generation: u64,
}

impl State {
    pub(crate) fn new(id: ReplicaId, me: PeerId) -> State {
        State {
            id,
            me,
            entry_by_author_key: BTreeMap::new(),
            my_max_ts_micros: 0,
            generation: 0,
        }
    }

    pub(crate) fn id(&self) -> ReplicaId {
        self.id
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// Builds the state from the file's entries; the second number is how many were dropped by the check.
    pub(crate) fn from_entries(id: ReplicaId, me: PeerId, all: Vec<WireEntry>) -> (State, u32) {
        let mut state = State::new(id, me);
        let mut rejected = 0u32;
        for w in all {
            let ok = check(&state.id, &w).is_ok();
            rejected += u32::from(!ok);
            if ok {
                state.merge(w);
            }
        }
        (state, rejected)
    }

    /// Merges an already verified entry; `true` if the map changed.
    pub(crate) fn merge(&mut self, w: WireEntry) -> bool {
        let slot = (w.author, w.key.clone());
        if self.entry_by_author_key.get(&slot).is_some_and(|old| !wins(&w, old)) {
            return false;
        }
        if w.author == *self.me.as_bytes() {
            self.my_max_ts_micros = self.my_max_ts_micros.max(w.ts_micros);
        }
        self.entry_by_author_key.insert(slot, w);
        self.generation += 1;
        true
    }

    /// The `ts_micros` of a new own entry: strictly greater than all its previous ones, even if the clock went back.
    pub(crate) fn next_ts_micros(&self, now_micros: u64) -> u64 {
        now_micros.max(self.my_max_ts_micros + 1)
    }

    /// A live entry of an author by exact key; a tombstone is `None`, as in `MemSync`.
    pub(crate) fn get(&self, author: PeerId, key: &[u8]) -> Option<ReplicaEntry> {
        let w = self.entry_by_author_key.get(&(*author.as_bytes(), key.to_vec()))?;
        let entry = to_entry(author, w);
        (!entry.is_tombstone()).then_some(entry)
    }

    /// All entries with a key prefix (tombstones too), in "author, key" order.
    pub(crate) fn scan(&self, prefix: &[u8]) -> Vec<ReplicaEntry> {
        let with_prefix = self.entry_by_author_key.values().filter(|w| w.key.starts_with(prefix));
        with_prefix.filter_map(|w| self.entry_of(w)).collect()
    }

    /// For each key the entry with the greatest `(ts_micros, author)`; tombstones included.
    pub(crate) fn latest_per_key(&self, prefix: &[u8]) -> Vec<ReplicaEntry> {
        let mut best: BTreeMap<Vec<u8>, ReplicaEntry> = BTreeMap::new();
        for e in self.scan(prefix) {
            let rank = |x: &ReplicaEntry| (x.ts_micros, *x.author.as_bytes());
            let beats = best.get(&e.key).is_none_or(|old| rank(&e) > rank(old));
            if beats {
                best.insert(e.key.clone(), e);
            }
        }
        best.into_values().collect()
    }

    fn entry_of(&self, w: &WireEntry) -> Option<ReplicaEntry> {
        let author = PeerId::from_bytes(&w.author).ok()?;
        Some(to_entry(author, w))
    }

    /// A summary per author, ordered by author.
    pub(crate) fn summary(&self) -> Vec<WriterSum> {
        let mut sums: Vec<(WriterSum, blake3::Hasher)> = Vec::new();
        for w in self.entry_by_author_key.values() {
            let fresh = sums.last().is_none_or(|(s, _)| s.author != w.author);
            if fresh {
                sums.push((blank_sum(w.author), blake3::Hasher::new()));
            }
            let (sum, hasher) = sums.last_mut().expect("just added");
            sum.max_ts_micros = sum.max_ts_micros.max(w.ts_micros);
            sum.count += 1;
            feed(hasher, w);
        }
        sums.into_iter()
            .map(|(mut sum, hasher)| {
                sum.digest = *hasher.finalize().as_bytes();
                sum
            })
            .collect()
    }

    /// All entries of authors whose summary at the counterpart differs or is absent.
    pub(crate) fn missing_for(&self, theirs: &[WriterSum]) -> Vec<WireEntry> {
        let ours = self.summary();
        let differs = |author: &[u8; 32]| {
            let mine = ours.iter().find(|s| s.author == *author);
            let other = theirs.iter().find(|s| s.author == *author);
            mine != other
        };
        self.entry_by_author_key
            .values()
            .filter(|w| differs(&w.author))
            .cloned()
            .collect()
    }

    pub(crate) fn entries(&self) -> Vec<WireEntry> {
        self.entry_by_author_key.values().cloned().collect()
    }
}

fn blank_sum(author: [u8; 32]) -> WriterSum {
    WriterSum {
        author,
        max_ts_micros: 0,
        count: 0,
        digest: [0; 32],
    }
}

/// The author's hash takes `(len(key), key, ts_micros, blake3(value))`: so a value mismatch at an equal `ts_micros` is visible too.
fn feed(hasher: &mut blake3::Hasher, w: &WireEntry) {
    hasher.update(&(w.key.len() as u32).to_be_bytes());
    hasher.update(&w.key);
    hasher.update(&w.ts_micros.to_be_bytes());
    hasher.update(blake3::hash(&w.value).as_bytes());
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
