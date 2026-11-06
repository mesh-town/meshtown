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

/// Cap on the receive window of every stream of the node, in bytes: how much unread data a peer may
/// have in flight towards us on one stream.
const RECV_WINDOW_MAX: u32 = 256 * 1024;

/// The factory before the saved table is added. LAN discovery stays off until it is a product
/// decision; its path ignores the dial options (relay, hole punch).
fn base_factory(bootstrap: Option<Vec<String>>) -> PeeroxideFactory {
    let mut factory = match bootstrap {
        Some(list) => PeeroxideFactory {
            bootstrap: Some(list),
            ..Default::default()
        },
        None => PeeroxideFactory::from_env(),
    };
    factory.lan_discovery = false;
    factory.stream_limits = townjump_dht::StreamLimits {
        recv_window_max: Some(RECV_WINDOW_MAX),
        ..Default::default()
    };
    factory
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
