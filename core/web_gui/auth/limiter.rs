//! Limiter of failed sign-ins by peer IP. Time is the `now` parameter.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

pub(crate) const FAILS_TO_BLOCK: u32 = 5;
pub(crate) const FAIL_WINDOW: Duration = Duration::from_secs(15 * 60);
pub(crate) const FIRST_BLOCK: Duration = Duration::from_secs(60);
pub(crate) const MAX_BLOCK: Duration = Duration::from_secs(15 * 60);
pub(crate) const MAX_TRACKED_IPS: usize = 1024;

struct Entry {
    fails: u32,
    window_start: Instant,
    last_fail: Instant,
    /// Length of the last block; zero — there has been no block yet.
    block_len: Duration,
    blocked_until: Instant,
}

impl Entry {
    fn fresh(now: Instant) -> Self {
        Entry {
            fails: 0,
            window_start: now,
            last_fail: now,
            block_len: Duration::ZERO,
            blocked_until: now,
        }
    }

    /// Stale: 15 min without failures and without a block.
    fn stale(&self, now: Instant) -> bool {
        let quiet = now.saturating_duration_since(self.last_fail) >= FAIL_WINDOW;
        quiet && now >= self.blocked_until
    }

    fn register(&mut self, now: Instant) {
        self.last_fail = now;
        if self.block_len.is_zero() {
            self.count_in_window(now);
            return;
        }
        self.block_for((self.block_len * 2).min(MAX_BLOCK), now);
    }

    fn count_in_window(&mut self, now: Instant) {
        if now.saturating_duration_since(self.window_start) >= FAIL_WINDOW {
            self.fails = 0;
            self.window_start = now;
        }
        self.fails += 1;
        if self.fails >= FAILS_TO_BLOCK {
            self.block_for(FIRST_BLOCK, now);
        }
    }

    fn block_for(&mut self, len: Duration, now: Instant) {
        self.block_len = len;
        self.blocked_until = now + len;
    }
}

pub(crate) struct Limiter {
    map: HashMap<IpAddr, Entry>,
}

impl Limiter {
    pub(crate) fn new() -> Self {
        Limiter {
            map: HashMap::new(),
        }
    }

    /// `Err` — how long to wait; the caller rounds it to seconds for `Retry-After`.
    pub(crate) fn check_not_blocked(&self, ip: IpAddr, now: Instant) -> Result<(), Duration> {
        let Some(e) = self.map.get(&ip) else {
            return Ok(());
        };
        match e.blocked_until.checked_duration_since(now) {
            Some(left) if !left.is_zero() => Err(left),
            _ => Ok(()),
        }
    }

    pub(crate) fn record_fail(&mut self, ip: IpAddr, now: Instant) {
        self.make_room(ip, now);
        let e = self.map.entry(ip).or_insert_with(|| Entry::fresh(now));
        if e.stale(now) {
            *e = Entry::fresh(now);
        }
        e.register(now);
    }

    pub(crate) fn clear_fails(&mut self, ip: IpAddr) {
        self.map.remove(&ip);
    }

    fn make_room(&mut self, ip: IpAddr, now: Instant) {
        if self.map.contains_key(&ip) || self.map.len() < MAX_TRACKED_IPS {
            return;
        }
        self.map.retain(|_, e| !e.stale(now));
        if self.map.len() < MAX_TRACKED_IPS {
            return;
        }
        let oldest = self
            .map
            .iter()
            .min_by_key(|(_, e)| e.last_fail)
            .map(|(k, _)| *k);
        if let Some(k) = oldest {
            self.map.remove(&k);
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.map.len()
    }
}

#[cfg(test)]
#[path = "limiter_tests.rs"]
mod tests;
