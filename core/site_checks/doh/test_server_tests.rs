//! Tests of the resolver against a loopback DoH server: how a "name does not exist" answer travels up.

use super::test_server::{TestDoh, ANSWER_V4};
use simple_dns::TYPE;

#[tokio::test]
async fn nonexistent_name_is_reported_as_nx() {
    let server = TestDoh::start().await;

    let answer = server.resolver().resolve_cached_nx("gone.nx.test").await;

    assert_eq!(answer.unwrap(), (vec![], true));
}

#[tokio::test]
async fn existing_name_is_not_nx_and_is_cached() {
    let server = TestDoh::start().await;
    let resolver = server.resolver();

    let first = resolver.resolve_cached_nx("host.test").await.unwrap();
    let second = resolver.resolve_cached_nx("host.test").await.unwrap();

    assert_eq!(first, (vec![ANSWER_V4], false));
    assert_eq!(second, first);
    assert_eq!(server.queries().len(), 1, "the second call is a cache hit");
}

#[tokio::test]
async fn nonexistent_name_is_asked_again_each_time() {
    let server = TestDoh::start().await;
    let resolver = server.resolver();

    for _ in 0..2 {
        let answer = resolver.resolve_cached_nx("gone.nx.test").await.unwrap();
        assert_eq!(answer, (vec![], true));
    }

    assert_eq!(server.queries().len(), 2, "NXDOMAIN is not cached");
}

#[tokio::test]
async fn empty_answer_is_not_nx() {
    let server = TestDoh::start().await;

    let answer = server.resolver().resolve_cached_nx("a.empty.test").await;

    assert_eq!(answer.unwrap(), (vec![], false));
    assert_eq!(server.queries()[0].1, TYPE::A);
}
