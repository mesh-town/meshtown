//! The wire "country from the network document → `collect()` → `peers` row" checked with a real node on
//! loopback (modelled on `collect_tests.rs`): the assembly rule itself (`countries[id]` → `country`)
//! is already checked by `build_country_tests.rs`; here the point is that `collect` really reads
//! `geoip::read_countries` and not a stub.

use super::*;

use crate::members::network_key::NetworkKey;
use crate::node::output;
use crate::node::NodeConfig;
use crate::site_checks::geoip::{self, Country};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collect_reads_own_country_from_document() {
    let key = NetworkKey::generate().unwrap();
    let node = crate::node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();

    geoip::announce_country(
        &*node.domains.shared_replica(),
        Country::parse("RU").unwrap(),
    )
    .await
    .expect("the country should be written to the network document");

    let collected = collect(&node, None, &mut DocCache::default()).await;
    let own = collected
        .snap
        .peers
        .iter()
        .find(|p| p.id == output::short_peer_id(&node.id))
        .expect("the own row should be in peers");
    assert_eq!(
        own.country,
        Some("RU".to_string()),
        "collect should read the country from the network document, not return a stub"
    );

    node.shutdown().await;
}
