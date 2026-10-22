//! The DoH address cache for the exit: name → IPv4 and IPv6 addresses for the response TTL clamped to 30–120 s.
//!
//! What it does: stores one entry per name with an IPv4 and an IPv6 list (either may be "not asked yet")
//! and an expiry time, returns live ones, drops expired ones; when full, evicts all dead ones first, then the one closest to expiry.
//! What it does not do: it does not touch the network, does not read the clock (time comes as a parameter), does not store
//! failures or an empty answer without a non-empty one of the other family beside it, does not take a lock — the lock is held by `Resolver`.
//! Links: `doh::Resolver::resolve_cached`, `resolve_cached_v6` and `forget`.

use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
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

/// The addresses of a name per family (`None` — not asked yet) and the moment from which the entry counts as expired.
struct Entry {
    v4: Option<Vec<Ipv4Addr>>,
    v6: Option<Vec<Ipv6Addr>>,
    expires: Instant,
}

impl DnsCache {
    pub(super) fn new(capacity: usize) -> DnsCache {
        DnsCache {
            entries: HashMap::new(),
            capacity,
        }
    }

    /// The IPv4 addresses of a live entry whose family is known; an expired entry (`expires <= now`) is removed.
    pub(super) fn get(&mut self, key: &str, now: Instant) -> Option<Vec<Ipv4Addr>> {
        self.live(key, now)?.v4.clone()
    }

    /// The IPv6 addresses of a live entry whose family is known; an expired entry is removed.
    pub(super) fn get_v6(&mut self, key: &str, now: Instant) -> Option<Vec<Ipv6Addr>> {
        self.live(key, now)?.v6.clone()
    }

    /// An empty `addrs` is not remembered; otherwise: make room, then insert for `ttl`. The entry is replaced
    /// as a whole: the IPv6 family becomes "not asked".
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
        let entry = Entry {
            v4: Some(addrs),
            v6: None,
            expires: now + ttl,
        };
        self.entries.insert(key, entry);
    }

    /// Adds the IPv6 family to a live entry; its expiry is the smaller of the old one and `now + ttl` (an empty
    /// answer keeps the old one). Without a live entry only a non-empty answer is stored, with IPv4 unknown.
    pub(super) fn insert_v6(
        &mut self,
        key: String,
        addrs: Vec<Ipv6Addr>,
        ttl: Duration,
        now: Instant,
    ) {
        if let Some(entry) = self.live(&key, now) {
            let fresh = if addrs.is_empty() {
                entry.expires
            } else {
                now + ttl
            };
            entry.expires = entry.expires.min(fresh);
            entry.v6 = Some(addrs);
            return;
        }
        if addrs.is_empty() {
            return;
        }
        self.make_room(&key, now);
        let entry = Entry {
            v4: None,
            v6: Some(addrs),
            expires: now + ttl,
        };
        self.entries.insert(key, entry);
    }

    /// Notes that IPv4 is empty, only for a live entry that has IPv6 addresses and an unknown IPv4: an empty
    /// answer lives no longer than the non-empty one next to it. The expiry does not change.
    pub(super) fn note_empty_v4(&mut self, key: &str, now: Instant) {
        let Some(entry) = self.live(key, now) else {
            return;
        };
        if entry.v4.is_none() && entry.v6.as_ref().is_some_and(|v6| !v6.is_empty()) {
            entry.v4 = Some(Vec::new());
        }
    }

    /// Forgets a name; if there is no entry, does nothing.
    pub(super) fn remove(&mut self, key: &str) {
        self.entries.remove(key);
    }

    /// A live entry; an expired one is removed.
    fn live(&mut self, key: &str, now: Instant) -> Option<&mut Entry> {
        if self.entries.get(key)?.expires <= now {
            self.entries.remove(key);
            return None;
        }
        self.entries.get_mut(key)
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
