//! `mem` dialed by a single key, like `hyper`: a wrapper over the `MemNet` transport that, for an address without
//! its own hint, appends `MemNet::addr_of`. The invite and the `hyperdht` source carry no `mem` hints,
//! so without the wrapper joining in `mem` tests would give `NoRoute`. Only for `NodeConfig.test_net`.

use std::sync::Arc;

use crate::net::error::NetResult;
use crate::net::mem::MemNet;
use crate::net::split::SplitTransport;
use crate::net::{
    AddrHint, BoxFuture, NetInfo, NodeSecret, PeerAddr, PeerConn, PeerId, RelayRef, ServiceId, Transport,
};

/// The node's `mem` transport that dials a peer by key.
pub(crate) struct KeyedMem {
    net: Arc<MemNet>,
    slot: Arc<SplitTransport>,
}

impl KeyedMem {
    /// A new node slot with the key `secret` in the network `net`.
    pub(crate) fn new(net: &Arc<MemNet>, secret: &NodeSecret) -> Arc<Self> {
        Arc::new(KeyedMem { net: net.clone(), slot: net.transport(secret) })
    }

    /// An address without a `mem` hint gets the hint of the target's live slot; no slot — the address as it was.
    fn addr_with_mem_hint(&self, to: &PeerAddr) -> PeerAddr {
        let has_mem_hint = to.hints.iter().any(|h| matches!(h, AddrHint::Opaque { kind, .. } if *kind == crate::net::mem::MEM_HINT_KIND));
        match (has_mem_hint, self.net.addr_of(to.id)) {
            (false, Some(live)) => PeerAddr { id: to.id, hints: [to.hints.clone(), live.hints].concat() },
            _ => to.clone(),
        }
    }
}

impl Transport for KeyedMem {
    fn kind(&self) -> &'static str {
        self.slot.kind()
    }

    fn me(&self) -> PeerId {
        self.slot.me()
    }

    fn local_addr(&self) -> PeerAddr {
        self.slot.local_addr()
    }

    fn can_reach(&self, to: &PeerAddr) -> bool {
        self.slot.can_reach(&self.addr_with_mem_hint(to))
    }

    fn add_hints(&self, addr: &PeerAddr) {
        self.slot.add_hints(addr)
    }

    fn set_relays(&self, relays: &[RelayRef]) {
        self.slot.set_relays(relays)
    }

    fn serve(&self, svcs: &[ServiceId]) {
        self.slot.serve(svcs)
    }

    fn connect(&self, to: &PeerAddr, svc: ServiceId) -> BoxFuture<'_, NetResult<Arc<dyn PeerConn>>> {
        let filled = self.addr_with_mem_hint(to);
        Box::pin(async move { self.slot.connect(&filled, svc).await })
    }

    fn accept(&self) -> BoxFuture<'_, Option<(ServiceId, Arc<dyn PeerConn>)>> {
        self.slot.accept()
    }

    fn net_info(&self) -> NetInfo {
        self.slot.net_info()
    }

    fn wait_net_info(&self) -> BoxFuture<'_, ()> {
        self.slot.wait_net_info()
    }

    fn close(&self) -> BoxFuture<'_, ()> {
        self.slot.close()
    }
}

#[cfg(test)]
#[path = "transports_mem_tests.rs"]
mod tests;
