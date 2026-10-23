//! The node's proxy pieces built at start: the configuration a proxy front serves with, and the published face.

use tokio::sync::watch;

use crate::exit::PeerConnPool;
use crate::proxy::route::RouteCtx;
use crate::proxy::{self, ProxyConfig, ProxyFace};

use super::network::Net;

/// What a proxy listener needs to serve this node's traffic (no listener of its own).
pub(super) fn proxy_config_of(route: &RouteCtx, net: &Net) -> ProxyConfig {
    ProxyConfig {
        route: route.clone(),
        stack: net.stack.clone(),
        pool: PeerConnPool::default().with_traffic(net.exit_traffic.clone()),
        resolver: net.resolver.clone(),
        host_overrides: net.exit_hosts.clone(),
        udp_slots: proxy::udp_slots(),
    }
}

/// The proxy face the node reads: the supplied one, or a constant `OFF` when there is no listener.
pub(super) fn face_of(given: Option<watch::Receiver<ProxyFace>>) -> watch::Receiver<ProxyFace> {
    given.unwrap_or_else(|| watch::channel(ProxyFace::OFF).1)
}
