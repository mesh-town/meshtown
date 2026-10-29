//! Options of the relay node of this member: the permanent key, who may pair through it and how it is announced.

use crate::transport::net::id::PeerId;
use crate::transport::net_hyper::keys::peer_of;
use std::sync::Arc;
use townjump_dht::Firewall;
use townjump_dht_peeroxide::RelayOpts;
use townjump_keys::KeyPair;

/// Whether a peer may pair through the relay.
pub type RelayAdmit = Arc<dyn Fn(PeerId) -> bool + Send + Sync>;

/// Whether the relay key is announced on the DHT (the serving side finds the relay by it).
pub(crate) const RELAY_ANNOUNCE: bool = true;

/// Options of a relay with the given key pair that pairs only the peers `admit` accepts.
/// A key that is not a node identity is refused.
pub(crate) fn relay_opts(identity: KeyPair, admit: RelayAdmit) -> RelayOpts {
    let firewall: Firewall = Arc::new(move |key: &[u8; 32]| match peer_of(key) {
        Ok(peer) => !admit(peer),
        Err(_) => true,
    });
    RelayOpts {
        identity: Some(identity),
        firewall: Some(firewall),
        announce: RELAY_ANNOUNCE,
        port: 0,
        assume_open: false,
    }
}

#[cfg(test)]
#[path = "opts_tests.rs"]
mod tests;
