//! Tests for `peers_cache.rs`: remembering (`remember`) and refreshing (`touch`).

use super::*;

// ---------- remembering (`remember`) ----------

#[test]
fn remember_new_peer_is_added() {
    let mut cache = PeersCache::load(None, "net", 1000);
    let entry = entry_for(
        None,
        endpoint_id(1),
        Some(&AddrHint::Ip(sock_addr(1))),
        1000,
    );
    assert!(cache.remember(entry, 1000));
    assert_eq!(cache.len(), 1);
}

#[test]
fn remember_no_change_within_hour_does_not_rewrite() {
    let mut cache = PeersCache::load(None, "net", 1000);
    let addr = sock_addr(1);
    let first = entry_for(None, endpoint_id(1), Some(&AddrHint::Ip(addr)), 1000);
    assert!(cache.remember(first, 1000));

    let same_later = entry_for(None, endpoint_id(1), Some(&AddrHint::Ip(addr)), 1000 + 100);
    let changed = cache.remember(same_later, 1000 + 100);
    assert!(
        !changed,
        "with no changes, an entry must not be rewritten before REFRESH_SECS"
    );
    assert_eq!(
        entry_in(&cache, 1).seen_unix,
        1000,
        "seen_unix in the cache must stay as it was"
    );
}

#[test]
fn remember_no_change_after_refresh_secs_updates_seen_unix() {
    let mut cache = PeersCache::load(None, "net", 1000);
    let addr = sock_addr(1);
    let first = entry_for(None, endpoint_id(1), Some(&AddrHint::Ip(addr)), 1000);
    assert!(cache.remember(first, 1000));

    let later = 1000 + REFRESH_SECS + 1;
    let same_but_old = entry_for(None, endpoint_id(1), Some(&AddrHint::Ip(addr)), later);
    let changed = cache.remember(same_but_old, later);
    assert!(
        changed,
        "with no changes but older than REFRESH_SECS, the entry must be updated"
    );
    assert_eq!(entry_in(&cache, 1).seen_unix, later);
}

#[test]
fn remember_changed_address_is_true_even_within_hour() {
    let mut cache = PeersCache::load(None, "net", 1000);
    let first = entry_for(
        None,
        endpoint_id(1),
        Some(&AddrHint::Ip(sock_addr(1))),
        1000,
    );
    assert!(cache.remember(first, 1000));

    let moved = entry_for(
        None,
        endpoint_id(1),
        Some(&AddrHint::Ip(sock_addr(2))),
        1100,
    );
    let changed = cache.remember(moved, 1100);
    assert!(
        changed,
        "an address change must require a write regardless of time"
    );
}

#[test]
fn remember_without_addrs_is_ignored() {
    let mut cache = PeersCache::load(None, "net", 1000);
    let empty_entry = entry_for(None, endpoint_id(1), None, 1000);
    let changed = cache.remember(empty_entry, 1000);
    assert!(
        !changed,
        "an entry with no addresses must not require a write"
    );
    assert!(
        cache.is_empty(),
        "an entry with no addresses must not get into the cache"
    );
}

#[test]
fn remember_evicts_oldest_when_over_capacity() {
    let mut cache = PeersCache::load(None, "net", 0);
    for n in 1u8..=32 {
        let entry = entry_for(
            None,
            endpoint_id(n),
            Some(&AddrHint::Ip(sock_addr(n))),
            n as u64,
        );
        assert!(cache.remember(entry, n as u64));
    }
    assert_eq!(cache.len(), MAX_PEERS);

    // member 1 is the oldest (seen_unix = 1); the 33rd is newer than all the others
    let newcomer = entry_for(
        None,
        endpoint_id(33),
        Some(&AddrHint::Ip(sock_addr(33))),
        100,
    );
    assert!(cache.remember(newcomer, 100));

    assert_eq!(cache.len(), MAX_PEERS, "the limit must not be exceeded");
    let ids = ids_in(&cache);
    assert!(
        !ids.contains(&id_bytes(1)),
        "the oldest member must be evicted"
    );
    assert!(
        ids.contains(&id_bytes(33)),
        "the new member must get into the cache"
    );
}

// ---------- refreshing (`touch`) ----------

#[test]
fn touch_refreshes_entry_older_than_refresh_secs() {
    let mut cache = PeersCache::load(None, "net", 1000);
    let addr = sock_addr(1);
    let entry = entry_for(None, endpoint_id(1), Some(&AddrHint::Ip(addr)), 1000);
    cache.remember(entry, 1000);

    let now = 1000 + REFRESH_SECS + 1;
    let changed = cache.touch(endpoint_id(1), now);
    assert!(changed);
    let refreshed = entry_in(&cache, 1);
    assert_eq!(refreshed.seen_unix, now);
    assert_eq!(
        refreshed.addrs,
        vec![addr],
        "touch must not change the addresses or their order"
    );
}

#[test]
fn touch_ignores_entry_fresher_than_hour() {
    let mut cache = PeersCache::load(None, "net", 1000);
    let entry = entry_for(
        None,
        endpoint_id(1),
        Some(&AddrHint::Ip(sock_addr(1))),
        1000,
    );
    cache.remember(entry, 1000);

    let changed = cache.touch(endpoint_id(1), 1000 + 100);
    assert!(
        !changed,
        "an entry fresher than an hour must not be refreshed"
    );
    assert_eq!(
        entry_in(&cache, 1).seen_unix,
        1000,
        "the entry must not change"
    );
}

#[test]
fn touch_missing_peer_is_noop() {
    let mut cache = PeersCache::load(None, "net", 1000);
    let changed = cache.touch(endpoint_id(9), 1000);
    assert!(!changed);
    assert!(cache.is_empty());
}
