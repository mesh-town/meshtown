//! The topic's duplicate cache: the message id (blake3 of the body), the entry lifetime, the entry count limit.
//! There is no network or I/O here.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// The message id is the body's hash; identical content counts as a duplicate.
pub fn msg_id(body: &[u8]) -> [u8; 32] {
    *blake3::hash(body).as_bytes()
}

pub struct Seen {
    cap: usize,
    life: Duration,
    at: HashMap<[u8; 32], Instant>,
    order: VecDeque<[u8; 32]>,
}

impl Seen {
    pub fn new(cap: usize, life: Duration) -> Seen {
        Seen {
            cap,
            life,
            at: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Expired entries and the oldest ones beyond `cap` are evicted.
    pub fn remember_if_new(&mut self, id: [u8; 32], now: Instant) -> bool {
        self.expire(now);
        if self.at.contains_key(&id) {
            return false;
        }
        self.at.insert(id, now);
        self.order.push_back(id);
        self.evict_over_cap();
        true
    }

    fn expire(&mut self, now: Instant) {
        while let Some(front) = self.order.front() {
            if self.at.get(front).is_some_and(|t| now < *t + self.life) {
                return;
            }
            let old = self.order.pop_front();
            old.iter().for_each(|id| {
                self.at.remove(id);
            });
        }
    }

    fn evict_over_cap(&mut self) {
        while self.order.len() > self.cap {
            let old = self.order.pop_front();
            old.iter().for_each(|id| {
                self.at.remove(id);
            });
        }
    }
}

#[cfg(test)]
#[path = "seen_tests.rs"]
mod tests;
