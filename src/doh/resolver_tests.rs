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
