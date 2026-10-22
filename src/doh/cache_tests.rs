//! Tests for the DoH address cache (`doh/cache.rs`): TTL bounds, key, lifetime, eviction.

use super::*;

const A: Ipv4Addr = Ipv4Addr::new(1, 1, 1, 1);
const B: Ipv4Addr = Ipv4Addr::new(2, 2, 2, 2);

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[test]
fn short_ttl_is_raised_to_floor() {
    assert_eq!(clamp_ttl(0), MIN_TTL);
    assert_eq!(clamp_ttl(5), MIN_TTL);
}

#[test]
fn long_ttl_is_cut_to_ceiling() {
    assert_eq!(clamp_ttl(86_400), MAX_TTL);
}

#[test]
fn ttl_inside_bounds_is_kept() {
    assert_eq!(clamp_ttl(45), secs(45));
}

#[test]
fn key_ignores_case_and_trailing_dot() {
    assert_eq!(cache_key("Example.COM."), cache_key("example.com"));
    assert_eq!(cache_key("Example.COM."), "example.com");
}

#[test]
fn fresh_entry_is_returned() {
    let now = Instant::now();
    let mut cache = DnsCache::new(4);
    cache.insert("a.com".into(), vec![A], secs(60), now);
    assert_eq!(cache.get("a.com", now + secs(59)), Some(vec![A]));
}

#[test]
fn expired_entry_is_a_miss_and_dropped() {
    let now = Instant::now();
    let mut cache = DnsCache::new(4);
    cache.insert("a.com".into(), vec![A], secs(60), now);
    assert_eq!(cache.get("a.com", now + secs(60)), None);
    assert!(cache.entries.is_empty());
}

#[test]
fn empty_answer_is_not_stored() {
    let now = Instant::now();
    let mut cache = DnsCache::new(4);
    cache.insert("a.com".into(), Vec::new(), secs(60), now);
    assert_eq!(cache.get("a.com", now), None);
}

#[test]
fn same_key_is_replaced() {
    let now = Instant::now();
    let mut cache = DnsCache::new(4);
    cache.insert("a.com".into(), vec![A], secs(60), now);
    cache.insert("a.com".into(), vec![B], secs(60), now);
    assert_eq!(cache.get("a.com", now), Some(vec![B]));
    assert_eq!(cache.entries.len(), 1);
}

#[test]
fn full_cache_drops_expired_first() {
    let now = Instant::now();
    let mut cache = DnsCache::new(2);
    cache.insert("a".into(), vec![A], secs(10), now);
    cache.insert("b".into(), vec![A], secs(100), now);
    cache.insert("c".into(), vec![A], secs(100), now + secs(20));
    let later = now + secs(20);
    assert_eq!(cache.get("a", later), None);
    assert!(cache.get("b", later).is_some());
    assert!(cache.get("c", later).is_some());
}

#[test]
fn full_cache_drops_soonest_to_expire() {
    let now = Instant::now();
    let mut cache = DnsCache::new(2);
    cache.insert("a".into(), vec![A], secs(100), now);
    cache.insert("b".into(), vec![A], secs(50), now);
    cache.insert("c".into(), vec![A], secs(100), now);
    assert_eq!(cache.get("b", now), None);
    assert!(cache.get("a", now).is_some());
    assert!(cache.get("c", now).is_some());
}

#[test]
fn replacing_key_in_full_cache_evicts_nothing() {
    let now = Instant::now();
    let mut cache = DnsCache::new(2);
    cache.insert("a".into(), vec![A], secs(60), now);
    cache.insert("b".into(), vec![A], secs(60), now);
    cache.insert("a".into(), vec![B], secs(60), now);
    assert_eq!(cache.get("a", now), Some(vec![B]));
    assert!(cache.get("b", now).is_some());
}

#[test]
fn remove_forgets_entry() {
    let now = Instant::now();
    let mut cache = DnsCache::new(4);
    cache.insert("a.com".into(), vec![A], secs(60), now);
    cache.remove("a.com");
    assert_eq!(cache.get("a.com", now), None);
}
