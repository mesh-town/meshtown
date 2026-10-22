//! Reading the state files before the stack is built: peer names and the address cache with the discovery sources.
//! What it doesn't do: build the stack (that is `network`).

use std::sync::Arc;

use crate::membership::Members;
use crate::net::Discovery;
use crate::net_mainline;
use crate::node::config::NodeConfig;
use crate::node::discovery_sources::node_sources;
use crate::peer_names::PeerNames;
use crate::peers_cache::PeersCache;

use super::boot::Boot;
use super::transports_hyper::hyper_records;

/// Reads the last known names and inserts them into `Members` before `build_stack`: a stack with a handshake
/// handler starts accepting handshakes right after it is built, and an incoming handshake in that
/// window would update the name earlier — had we applied the file later, the fresh name would be overwritten by the old one from
/// the file.
pub(super) fn preload_peer_names_into_members(cfg: &NodeConfig, boot: &Boot, members: &Members) -> PeerNames {
    let peer_names = PeerNames::load(cfg.peer_names_path.as_deref(), &boot.subkeys.network_id_hex());
    peer_names.apply_to(members);
    peer_names
}

/// Reads the address cache and builds the discovery sources from it and the config.
pub(super) fn load_sources(cfg: &NodeConfig, boot: &Boot) -> (PeersCache, Vec<Arc<dyn Discovery>>) {
    let cache = PeersCache::load(
        cfg.peers_cache_path.as_deref(),
        &boot.subkeys.network_id_hex(),
        net_mainline::unix_now(),
    );
    let sources = node_sources(cfg, &cache, boot.id, &boot.subkeys, &boot.telemetry, hyper_records(boot));
    (cache, sources)
}

#[cfg(test)]
#[path = "preload_tests.rs"]
mod tests;
