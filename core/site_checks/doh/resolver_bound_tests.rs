//! The exit's copy of the resolver (`bound_copy`): its own cache, DoH connections bound to the exit's
//! interface, and the original left as it was.

use super::*;
use crate::iface_bind::test_support::{loopback_name, MISSING_IFACE};
use crate::iface_bind::{check, BindError, OutIface};
use crate::site_checks::doh::test_server::{TestDoh, ANSWER_V4};

const SEEDED: Ipv4Addr = Ipv4Addr::new(93, 184, 216, 34);

mod cache {
    use super::*;

    #[tokio::test]
    async fn bound_copy_has_own_cache() {
        // given: a parent with `seeded.test` cached
        let parent = dead_resolver();
        seed(&parent, "seeded.test");

        // when: the copy is made and then gets an entry of its own
        let copy = parent.bound_copy(OutIface::default());
        seed(&copy, "copy.test");

        // then: neither sees the other's entry
        assert!(copy.resolve_cached("seeded.test").await.is_err(), "the parent's entry is not shared");
        assert!(parent.resolve_cached("copy.test").await.is_err(), "the copy's entry is not shared");
        assert_eq!(parent.resolve_cached("seeded.test").await.unwrap(), vec![SEEDED]);
    }

    #[tokio::test]
    async fn clear_cache_empties() {
        // given: a copy with a cached entry that is served without network
        let copy = dead_resolver().bound_copy(OutIface::default());
        seed(&copy, "seeded.test");
        assert_eq!(copy.resolve_cached("seeded.test").await.unwrap(), vec![SEEDED]);

        // when: the cache is cleared
        copy.clear_cache();

        // then: the name goes to the server again (here a dead one) and fails
        assert!(copy.resolve_cached("seeded.test").await.is_err());
    }
}

mod bound_to_interface {
    use super::*;

    #[tokio::test]
    async fn bound_to_missing_never_reaches_server() {
        // given: a DoH server and a copy bound to an interface that doesn't exist
        let doh = TestDoh::start().await;
        let copy = doh.resolver().bound_copy(OutIface::new(Some(MISSING_IFACE)));

        // when: a name is resolved
        let result = copy.resolve_cached("host.test").await;

        // then: it fails and the server got no query
        assert!(result.is_err(), "no answer through a missing interface");
        assert!(doh.queries().is_empty(), "the server must see nothing: {:?}", doh.queries());
    }

    #[tokio::test]
    async fn bound_to_loopback_resolves() {
        // given: a DoH server on 127.0.0.1 and a copy bound to the loopback interface
        let Some(name) = bindable_loopback() else {
            return;
        };
        let doh = TestDoh::start().await;
        let copy = doh.resolver().bound_copy(OutIface::new(Some(&name)));

        // when: a name is resolved
        let addrs = copy.resolve_cached("host.test").await;

        // then: the answer comes from the server
        assert_eq!(addrs.unwrap(), vec![ANSWER_V4]);
    }

    #[tokio::test]
    async fn switching_the_interface_applies_to_next_query() {
        // given: a copy that resolved through loopback
        let Some(name) = bindable_loopback() else {
            return;
        };
        let doh = TestDoh::start().await;
        let iface = OutIface::new(Some(&name));
        let copy = doh.resolver().bound_copy(iface.clone());
        copy.resolve_cached("host.test").await.expect("resolved through loopback");

        // when: the interface is switched to a missing one and the cache is cleared
        iface.set(Some(MISSING_IFACE));
        copy.clear_cache();

        // then: the next query fails
        assert!(copy.resolve_cached("host.test").await.is_err());
    }
}

mod parent {
    use super::*;

    #[tokio::test]
    async fn parent_resolver_stays_unbound() {
        // given: a resolver for checks and a copy of it bound to a missing interface
        let doh = TestDoh::start().await;
        let parent = doh.resolver();

        // when: the copy is made
        let _copy = parent.bound_copy(OutIface::new(Some(MISSING_IFACE)));

        // then: the parent has no binding and still reaches the server
        assert!(parent.iface.is_none(), "the original is not bound");
        assert_eq!(parent.resolve_cached("host.test").await.unwrap(), vec![ANSWER_V4]);
    }
}

/// A resolver pointed at a closed port: any trip to DoH fails at once, without a network.
fn dead_resolver() -> Resolver {
    Resolver::new(
        SocketAddr::from(([127, 0, 0, 1], 1)),
        HttpLimits::default(),
        "meshtown-test".to_string(),
    )
    .expect("resolver with a placeholder address")
}

/// Puts `name` → `SEEDED` into the resolver's cache for 60 s.
fn seed(resolver: &Resolver, name: &str) {
    let ttl = Duration::from_secs(60);
    resolver
        .cache()
        .insert(cache_key(name), vec![SEEDED], ttl, Instant::now());
}

/// The loopback interface name, or `None` (the test is skipped) where the kernel refuses binding to a device.
fn bindable_loopback() -> Option<String> {
    let name = loopback_name();
    match check(&name) {
        Ok(()) => Some(name),
        Err(BindError::NoPermission(_)) => {
            eprintln!("skipped: binding to a device needs CAP_NET_RAW on this kernel");
            None
        }
        Err(e) => panic!("the loopback interface {name} must be usable: {e}"),
    }
}
