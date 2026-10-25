//! DHT factory of the node: bootstrap list and the saved table.

use crate::transport::net_hyper::dht_table::{load, TTL};
use crate::transport::net_mainline::unix_now;
use std::path::Path;
use townjump_dht::DhtFactory;
use townjump_dht_peeroxide::PeeroxideFactory;

/// The DHT factory: the given bootstrap list, or the environment's defaults when there is none,
/// plus the nodes saved in `table_file`.
pub(super) fn factory_for(
    bootstrap: Option<Vec<String>>,
    table_file: Option<&Path>,
) -> PeeroxideFactory {
    let mut factory = base_factory(bootstrap);
    if let Some(path) = table_file {
        factory.set_nodes(load(path, unix_now(), TTL));
    }
    factory
}

fn base_factory(bootstrap: Option<Vec<String>>) -> PeeroxideFactory {
    match bootstrap {
        Some(list) => PeeroxideFactory {
            bootstrap: Some(list),
            ..Default::default()
        },
        None => PeeroxideFactory::from_env(),
    }
}
