//! Integration tests of the node's country (`geoip::board`).
//!
//! What it does: checks real live iroh-docs synchronization between two nodes on
//! loopback — a peer sees the announced country, the own cache is read with its write time, a tombstone
//! and an invalid value are indistinguishable from a missing record, and the old key is not confused with
//! `list/`/`chk/`/`node/exit/` (modeled on `core/tests/exit_flag.rs`).
//! What it doesn't do: it doesn't check the poll schedule (`geoip::task`) and the node snapshot — those are
//! `geoip::task_tests` and `node_ctl::snapshot::collect_country_tests`.

use std::collections::HashMap;
use std::time::Duration;

use bytes::Bytes;
use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::node_state;
use meshtown_core::node::{self, NodeConfig, RunningNode};
use meshtown_core::site_checks::geoip::{self, Country};
use meshtown_core::transport::net::{PeerAddr, PeerId};
use tokio::time::{sleep, Instant};

/// A copy of the helper from `core/tests/exit_flag.rs`: introduces the address of `b` into `a` over loopback.
async fn introduce(a: &RunningNode, b: &RunningNode) {
    a.introduce(PeerAddr::new(b.id)).await;
}

/// Waits until `node` sees `other`'s exit flag (`run_exit_announce` arrives asynchronously), no
/// longer than `limit` — a stable "before" baseline for the old-readers test.
async fn wait_exits_ready(
    node: &RunningNode,
    other: PeerId,
    limit: Duration,
) -> HashMap<PeerId, bool> {
    let deadline = Instant::now() + limit;
    loop {
        let exits = node_state::read_exit_flags(&*node.domains.shared_replica())
            .await
            .unwrap();
        if exits.get(&other) == Some(&true) || Instant::now() >= deadline {
            return exits;
        }
        sleep(Duration::from_millis(50)).await;
    }
}

/// Polls every 50 ms up to `limit`: reads the countries at `node` and returns the last map, as
/// soon as `want` returns true or the time runs out (the same technique as `wait_exits`).
async fn wait_countries(
    node: &RunningNode,
    limit: Duration,
    want: impl Fn(&HashMap<PeerId, Country>) -> bool,
) -> HashMap<PeerId, Country> {
    let deadline = Instant::now() + limit;
    loop {
        let countries = geoip::read_countries(&*node.domains.shared_replica())
            .await
            .unwrap();
        if want(&countries) || Instant::now() >= deadline {
            return countries;
        }
        sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn neighbor_sees_announced_country() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    introduce(&a, &b).await;

    geoip::announce_country(&*a.domains.shared_replica(), Country::parse("RU").unwrap())
        .await
        .unwrap();

    let countries = wait_countries(&b, Duration::from_secs(20), |c| c.contains_key(&a.id)).await;
    assert_eq!(
        countries.get(&a.id).map(Country::as_str),
        Some("RU"),
        "B must see the country announced by A"
    );

    a.shutdown().await;
    b.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn own_cache_reads_country_and_write_time() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();

    let before_us = now_micros();
    geoip::announce_country(&*a.domains.shared_replica(), Country::parse("RU").unwrap())
        .await
        .unwrap();
    let after_us = now_micros();

    let (country, at) = geoip::own_country_record(&*a.domains.shared_replica())
        .await
        .unwrap()
        .expect("the own record must be readable right after the announcement");
    assert_eq!(country.as_str(), "RU");
    assert!(
        at >= before_us && at <= after_us,
        "the write time {at} must be between {before_us} and {after_us}"
    );

    a.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tombstone_clears_country_for_everyone() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    introduce(&a, &b).await;

    geoip::announce_country(&*a.domains.shared_replica(), Country::parse("RU").unwrap())
        .await
        .unwrap();
    wait_countries(&b, Duration::from_secs(20), |c| c.contains_key(&a.id)).await;

    a.domains
        .shared_replica()
        .del_own(geoip::COUNTRY_KEY)
        .await
        .unwrap();

    let countries = wait_countries(&b, Duration::from_secs(20), |c| !c.contains_key(&a.id)).await;
    assert!(
        !countries.contains_key(&a.id),
        "a tombstone must remove A's country at the peer"
    );
    assert!(
        geoip::own_country_record(&*a.domains.shared_replica())
            .await
            .unwrap()
            .is_none(),
        "a tombstone must remove the country in the own read too"
    );

    a.shutdown().await;
    b.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn garbage_value_is_dropped_by_reader() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    introduce(&a, &b).await;

    a.domains
        .shared_replica()
        .set_own(geoip::COUNTRY_KEY, Bytes::from_static(b"r!"))
        .await
        .unwrap();

    sleep(Duration::from_secs(2)).await;
    let countries = geoip::read_countries(&*b.domains.shared_replica())
        .await
        .unwrap();
    assert!(
        !countries.contains_key(&a.id),
        "an invalid country code must not appear at the peer"
    );

    a.shutdown().await;
    b.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn old_readers_unaffected_by_country_key() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    introduce(&a, &b).await;

    // A's exit flag arrives at B asynchronously (`run_exit_announce`) — we wait until B sees
    // it before taking the "before", otherwise the comparison catches that race, not our key.
    let exits_before = wait_exits_ready(&b, a.id, Duration::from_secs(20)).await;
    let list_before = b.domains.list().await.unwrap();
    let views_before = b.checks.check_views().await.unwrap();

    geoip::announce_country(&*a.domains.shared_replica(), Country::parse("RU").unwrap())
        .await
        .unwrap();
    wait_countries(&b, Duration::from_secs(20), |c| c.contains_key(&a.id)).await;

    assert_eq!(
        b.domains.list().await.unwrap(),
        list_before,
        "announcing a country must not change the domain list"
    );
    assert_eq!(
        b.checks.check_views().await.unwrap().len(),
        views_before.len(),
        "announcing a country must not change the number of check results"
    );
    assert_eq!(
        node_state::read_exit_flags(&*b.domains.shared_replica())
            .await
            .unwrap(),
        exits_before,
        "announcing a country must not change the exit flags"
    );

    a.shutdown().await;
    b.shutdown().await;
}

fn now_micros() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}
