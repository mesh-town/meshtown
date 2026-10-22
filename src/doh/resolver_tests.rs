//! Tests of the DNS-over-HTTPS resolver (`resolver.rs`): the cache path and plain queries without a network.

use super::*;

/// A resolver pointed at a closed port: any trip to DoH fails at once, without a network.
fn dead_resolver() -> Resolver {
    Resolver::new(
        SocketAddr::from(([127, 0, 0, 1], 1)),
        HttpLimits::default(),
        "meshtown-test".to_string(),
    )
    .expect("resolver with a placeholder address")
}

const CACHED: Ipv4Addr = Ipv4Addr::new(93, 184, 216, 34);

/// Puts `example.com` → `CACHED` into the cache for 60 s.
fn resolver_with_cached_example() -> Resolver {
    let resolver = dead_resolver();
    let ttl = Duration::from_secs(60);
    resolver
        .cache()
        .insert(cache_key("example.com"), vec![CACHED], ttl, Instant::now());
    resolver
}

#[tokio::test]
async fn cached_answer_is_served_without_network() {
    let resolver = resolver_with_cached_example();
    let addrs = resolver.resolve_cached("Example.COM.").await.unwrap();
    assert_eq!(addrs, vec![CACHED]);
}

#[tokio::test]
async fn forget_sends_next_call_to_server() {
    let resolver = resolver_with_cached_example();
    resolver.forget("Example.COM.");
    assert!(resolver.resolve_cached("example.com").await.is_err());
}

#[tokio::test]
async fn plain_resolve_bypasses_cache() {
    let resolver = resolver_with_cached_example();
    assert!(resolver.resolve("example.com").await.is_err());
}

#[tokio::test]
async fn failed_lookup_leaves_cache_empty() {
    let resolver = dead_resolver();
    assert!(resolver.resolve_cached("example.com").await.is_err());
    assert_eq!(resolver.cache().get("example.com", Instant::now()), None);
}

const CACHED_V6: Ipv6Addr = Ipv6Addr::new(0x2606, 0x4700, 0, 0, 0, 0, 0, 0x1111);

/// Puts `v6only.test` → no IPv4, `CACHED_V6` into the cache for 60 s.
fn resolver_with_cached_v6_only() -> Resolver {
    let resolver = dead_resolver();
    let ttl = Duration::from_secs(60);
    let mut cache = resolver.cache();
    cache.insert_v6(
        cache_key("v6only.test"),
        vec![CACHED_V6],
        ttl,
        Instant::now(),
    );
    cache.note_empty_v4(&cache_key("v6only.test"), Instant::now());
    drop(cache);
    resolver
}

#[tokio::test]
async fn cached_v6_answer_is_served_without_network() {
    let resolver = resolver_with_cached_v6_only();
    assert_eq!(
        resolver.resolve_cached_v6("v6only.test").await.unwrap(),
        vec![CACHED_V6]
    );
    assert_eq!(
        resolver.resolve_cached("v6only.test").await.unwrap(),
        Vec::<Ipv4Addr>::new()
    );
}

#[tokio::test]
async fn v6_miss_failure_is_an_error_and_not_cached() {
    let resolver = dead_resolver();
    assert!(resolver.resolve_cached_v6("example.com").await.is_err());
    assert_eq!(resolver.cache().get_v6("example.com", Instant::now()), None);
}

#[tokio::test]
async fn forget_drops_both_families() {
    let resolver = resolver_with_cached_v6_only();
    resolver.forget("v6only.test");
    assert!(resolver.resolve_cached("v6only.test").await.is_err());
    assert!(resolver.resolve_cached_v6("v6only.test").await.is_err());
}

#[cfg(feature = "net-hyper")]
#[tokio::test]
async fn dual_seed_makes_a_v6_only_name() {
    let resolver = dead_resolver();
    resolver.seed_cache_dual_for_tests("v6only.test", Vec::new(), vec![CACHED_V6]);
    assert_eq!(
        resolver.resolve_cached("v6only.test").await.unwrap(),
        Vec::<Ipv4Addr>::new()
    );
    assert_eq!(
        resolver.resolve_cached_v6("v6only.test").await.unwrap(),
        vec![CACHED_V6]
    );
}
