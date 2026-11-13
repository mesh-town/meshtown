//! A ring of time buckets. Time is always an argument; nothing here reads the clock.

use std::time::Instant;

/// Length of one bucket in seconds.
pub(super) const BUCKET_SECS: u64 = 15;
/// Number of buckets kept: ten minutes.
pub(super) const SLOTS: u64 = 40;

/// What a bucket holds: it can be summed.
pub(super) trait Tally: Default + Clone {
    fn add(&mut self, other: &Self);
}

/// The absolute bucket number of `now`, counted from `origin`.
pub(super) fn index_of(origin: Instant, now: Instant) -> u64 {
    now.saturating_duration_since(origin).as_secs() / BUCKET_SECS
}

/// The last `SLOTS` buckets, each tagged with its absolute number.
pub(super) struct Ring<T> {
    slots: Vec<Option<(u64, T)>>,
}

impl<T: Tally> Ring<T> {
    pub(super) fn new() -> Self {
        Self {
            slots: (0..SLOTS).map(|_| None).collect(),
        }
    }

    /// The bucket `index`; a stale one in its place is replaced by an empty one.
    pub(super) fn at(&mut self, index: u64) -> &mut T {
        let slot = &mut self.slots[(index % SLOTS) as usize];
        if slot.as_ref().is_none_or(|(i, _)| *i != index) {
            *slot = Some((index, T::default()));
        }
        &mut slot.as_mut().expect("slot was just filled").1
    }

    /// The buckets `now - n + 1 ..= now` that hold anything.
    fn recent(&self, now: u64, n: u64) -> impl Iterator<Item = &T> {
        let first = (now + 1).saturating_sub(n.min(SLOTS));
        self.slots
            .iter()
            .flatten()
            .filter(move |(i, _)| *i >= first && *i <= now)
            .map(|(_, t)| t)
    }

    /// The sum of the last `n` buckets ending at `now`.
    pub(super) fn total(&self, now: u64, n: u64) -> T {
        let mut sum = T::default();
        self.recent(now, n).for_each(|t| sum.add(t));
        sum
    }

    /// The buckets of the last `n` as a list, oldest first.
    pub(super) fn buckets(&self, now: u64, n: u64) -> Vec<&T> {
        let first = (now + 1).saturating_sub(n.min(SLOTS));
        (first..=now)
            .filter_map(|i| self.slots[(i % SLOTS) as usize].as_ref())
            .filter(|(s, _)| *s >= first && *s <= now)
            .map(|(_, t)| t)
            .collect()
    }

    /// Nothing happened within the kept ten minutes.
    pub(super) fn is_stale(&self, now: u64) -> bool {
        self.recent(now, SLOTS).next().is_none()
    }
}

#[cfg(test)]
#[path = "buckets_tests.rs"]
mod tests;
