//! The DoH address cache for the exit: name → IPv4 addresses for the response TTL clamped to 30–120 s.
//!
//! What it does: stores "name → addresses" pairs with an expiry time, returns live ones, drops
//! expired ones; when full, evicts all dead ones first, then the one closest to expiry.
//! What it does not do: it does not touch the network, does not read the clock (time comes as a parameter), does not store
//! empty answers or failures, does not take a lock — the lock is held by `Resolver`.
//! Links: `doh::Resolver::resolve_cached` and `forget`.

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

/// Lower bound of an entry's lifetime: the remaining TTL at Cloudflare can be 0–5 s.
pub(super) const MIN_TTL: Duration = Duration::from_secs(30);
/// Upper bound: holding a dead address for more than two minutes is already a loss of connectivity.
pub(super) const MAX_TTL: Duration = Duration::from_secs(120);
/// Limit on the number of names in the cache.
pub(super) const CACHE_ENTRIES: usize = 1024;

/// A "name → addresses" cache with a limit on the number of entries.
pub(super) struct DnsCache {
    entries: HashMap<String, Entry>,
    capacity: usize,
}

/// The addresses of a name and the moment from which the entry counts as expired.
struct Entry {
    addrs: Vec<Ipv4Addr>,
    expires: Instant,
}

impl DnsCache {
    pub(super) fn new(capacity: usize) -> DnsCache {
        DnsCache {
            entries: HashMap::new(),
            capacity,
        }
    }

    /// A fresh entry is a copy of the addresses; an expired one (`expires <= now`) is removed and the answer is `None`.
    pub(super) fn get(&mut self, key: &str, now: Instant) -> Option<Vec<Ipv4Addr>> {
        let entry = self.entries.get(key)?;
        if entry.expires > now {
            return Some(entry.addrs.clone());
        }
        self.entries.remove(key);
        None
    }

    /// An empty `addrs` is not remembered; otherwise: make room, then insert for `ttl`.
    pub(super) fn insert(
        &mut self,
        key: String,
        addrs: Vec<Ipv4Addr>,
        ttl: Duration,
        now: Instant,
    ) {
        if addrs.is_empty() {
            return;
        }
        self.make_room(&key, now);
        let expires = now + ttl;
        self.entries.insert(key, Entry { addrs, expires });
    }

    /// Forgets a name; if there is no entry, does nothing.
    pub(super) fn remove(&mut self, key: &str) {
        self.entries.remove(key);
    }

    /// Makes room for a new key: first all expired entries, then the one closest to expiry.
    fn make_room(&mut self, key: &str, now: Instant) {
        if self.entries.contains_key(key) || self.entries.len() < self.capacity {
            return;
        }
        self.entries.retain(|_, entry| entry.expires > now);
        self.evict_soonest_if_full();
    }

    /// When full, evicts the entry with the earliest `expires`.
    fn evict_soonest_if_full(&mut self) {
        if self.entries.len() < self.capacity {
            return;
        }
        let soonest = self.entries.iter().min_by_key(|(_, entry)| entry.expires);
        let Some(key) = soonest.map(|(key, _)| key.clone()) else {
            return;
        };
        self.entries.remove(&key);
    }
}

/// Seconds from the response → an entry lifetime clamped to `MIN_TTL`–`MAX_TTL`.
pub(super) fn clamp_ttl(secs: u32) -> Duration {
    Duration::from_secs(u64::from(secs)).clamp(MIN_TTL, MAX_TTL)
}

/// The cache key: the name in ASCII lowercase without one trailing dot.
pub(super) fn cache_key(domain: &str) -> String {
    domain
        .strip_suffix('.')
        .unwrap_or(domain)
        .to_ascii_lowercase()
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
