//! Tests for `proxy/path.rs`: a domain's address from the resolver cache without the network, and dropping the entry after
//! a failed direct connection. The resolver is "dead" (loopback port 1 is not listening): a live
//! DoH request would have failed, so success proves the answer was taken from the cache.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use super::*;
use crate::doh::Resolver;
use crate::exit::PeerConnPool;
use crate::http_min::HttpLimits;

const DOMAIN: &str = "site.test";

/// A proxy config with a dead resolver and a `host_overrides` table; the node is real, on loopback.
async fn config(hosts: HashMap<String, SocketAddr>) -> ProxyConfig {
    let key = crate::network_key::NetworkKey::generate().unwrap();
    let node = crate::node::start_node(crate::node::NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    let resolver = Resolver::new(
        SocketAddr::from(([127, 0, 0, 1], 1)),
        HttpLimits::default(),
        "meshtown-test".to_string(),
    )
    .expect("resolver with a stub address");
    ProxyConfig {
        route: RouteCtx::new(node.members.clone(), node.domains.clone(), node.id, vec![]),
        stack: node.stack.clone(),
        pool: PeerConnPool::default(),
        resolver: Arc::new(resolver),
        host_overrides: Arc::new(hosts),
        udp_slots: crate::proxy::udp_slots(),
    }
}

fn domain() -> Target {
    Target::Domain(DOMAIN.to_string())
}

mod address_of {
    use super::*;

    /// given a seeded cache and a dead resolver, when `address_of` a domain, then the address from the cache.
    #[tokio::test]
    async fn seeded_cache_answers_without_network() {
        let cfg = config(HashMap::new()).await;
        cfg.resolver
            .seed_cache_for_tests(DOMAIN, vec![Ipv4Addr::new(93, 184, 216, 34)]);

        let addr = address_of(&cfg, &domain(), 443)
            .await
            .expect("address from the cache without contacting DoH");

        assert_eq!(addr, SocketAddr::from(([93, 184, 216, 34], 443)));
    }

    /// given a cache with one closed address, when `address_of`, then the refusal "no usable address".
    #[tokio::test]
    async fn unusable_cached_address_is_filtered() {
        let cfg = config(HashMap::new()).await;
        cfg.resolver
            .seed_cache_for_tests(DOMAIN, vec![Ipv4Addr::new(127, 0, 0, 1)]);

        let e = address_of(&cfg, &domain(), 443).await.unwrap_err();

        assert!(e.to_string().contains("no usable address"), "{e:#}");
    }
}

mod direct {
    use super::*;

    /// given a cache entry and an address from `host_overrides` on a closed port, when the connection did not open,
    /// then the domain's cache entry is dropped.
    #[tokio::test]
    async fn failed_connect_forgets_cached_domain() {
        let dead = SocketAddr::from(([127, 0, 0, 1], 1));
        let cfg = config(HashMap::from([(DOMAIN.to_string(), dead)])).await;
        cfg.resolver
            .seed_cache_for_tests(DOMAIN, vec![Ipv4Addr::new(93, 184, 216, 34)]);
        assert!(cfg.resolver.cached_for_tests(DOMAIN).is_some());

        let refused = open_direct_tcp(&cfg, &domain(), 1, "test").await;

        assert!(refused.is_err(), "loopback port 1 is not listening");
        assert!(cfg.resolver.cached_for_tests(DOMAIN).is_none());
    }
}

#[test]
fn no_answer_reason_names_busy_exit() {
    assert_eq!(no_answer_reason(true), "exit is busy (stream limit)");
    assert_eq!(no_answer_reason(false), "the selected exits did not answer");
}
