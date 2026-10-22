//! `MemNet::transport`: the full `mem` transport — the hub link plus the `MemCarrier` carrier.

use crate::net::id::NodeSecret;
use crate::net::mem::carrier::MemCarrier;
use crate::net::mem::hub::MemNet;
use crate::net::split::SplitTransport;
use std::sync::Arc;

impl MemNet {
    /// Transport of the node with key `secret`: a new hub slot and a lossless carrier.
    pub fn transport(&self, secret: &NodeSecret) -> Arc<SplitTransport> {
        let connector = self.connector(secret);
        let carrier = Arc::new(MemCarrier::new());
        Arc::new(SplitTransport::new(secret.peer_id(), connector, carrier))
    }
}
