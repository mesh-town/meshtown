use super::*;

fn stamp(local: u64, remote: u64, at: Instant) -> Stamp {
    Stamp { local, remote, at }
}

fn secs(base: Instant, s: u64) -> Instant {
    base + Duration::from_secs(s)
}

#[test]
fn no_cache_reads() {
    assert!(should_read(None, &stamp(0, 0, Instant::now())));
}

#[test]
fn local_change_reads_at_once() {
    let b = Instant::now();
    assert!(should_read(Some(&stamp(0, 0, b)), &stamp(1, 0, secs(b, 1))));
}

#[test]
fn remote_change_waits_ten_seconds() {
    let b = Instant::now();
    let prev = stamp(0, 0, b);
    assert!(!should_read(Some(&prev), &stamp(0, 1, secs(b, 9))));
    assert!(should_read(Some(&prev), &stamp(0, 1, secs(b, 10))));
}

#[test]
fn no_change_reads_after_sixty_seconds() {
    let b = Instant::now();
    let prev = stamp(3, 4, b);
    assert!(!should_read(Some(&prev), &stamp(3, 4, secs(b, 59))));
    assert!(should_read(Some(&prev), &stamp(3, 4, secs(b, 60))));
}

#[test]
fn no_change_within_a_minute_comes_from_cache() {
    let b = Instant::now();
    assert!(!should_read(
        Some(&stamp(2, 2, b)),
        &stamp(2, 2, secs(b, 1))
    ));
}

#[test]
fn change_during_read_triggers_reread_next_tick() {
    // The counters were captured before the read (local = 5); during the read it became 6 — on the next tick
    // comparing with the captured value triggers a re-read.
    let b = Instant::now();
    let taken_before_read = stamp(5, 0, b);
    assert!(should_read(
        Some(&taken_before_read),
        &stamp(6, 0, secs(b, 1))
    ));
}

#[test]
fn partial_read_is_not_cached() {
    let mut cache = DocCache::default();
    let at = stamp(0, 0, Instant::now());
    cache.store(at, &DocParts::default(), true);
    assert!(
        cache.held.is_some(),
        "a complete read is stored in the cache"
    );
    cache.store(at, &DocParts::default(), false);
    assert!(cache.held.is_none(), "a partial read clears the cache");
}

#[tokio::test]
async fn fetch_caches_full_read_of_live_node() {
    let key = crate::members::network_key::NetworkKey::generate().unwrap();
    let node = crate::node::start_node(crate::node::NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    let mut cache = DocCache::default();
    let (_, partial) = cache.read_doc_parts_cached(&node, 0).await;
    assert!(!partial && cache.held.is_some());
    // At startup the node itself writes to the document (version, name): wait for the writes to settle.
    let mut first = cache.held.as_ref().map(|(s, _)| s.at);
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        cache.read_doc_parts_cached(&node, 0).await;
        let now = cache.held.as_ref().map(|(s, _)| s.at);
        if now == first {
            break;
        }
        first = now;
    }
    let (_, partial) = cache.read_doc_parts_cached(&node, 0).await;
    assert!(!partial);
    assert_eq!(
        cache.held.as_ref().map(|(s, _)| s.at),
        first,
        "a tick without changes took it from the cache"
    );
    node.doc_changes.bump_local();
    cache.read_doc_parts_cached(&node, 0).await;
    assert_ne!(
        cache.held.as_ref().map(|(s, _)| s.at),
        first,
        "own change — re-read"
    );
}

#[tokio::test]
async fn own_insert_bumps_local_counter() {
    let key = crate::members::network_key::NetworkKey::generate().unwrap();
    let node = crate::node::start_node(crate::node::NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    let before = node.doc_changes.local_and_remote_counts().0;
    node.domains.add("example.com").await.unwrap();
    for _ in 0..40 {
        if node.doc_changes.local_and_remote_counts().0 > before {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("InsertLocal did not reach the local counter");
}
