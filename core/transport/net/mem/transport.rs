//! `MemNet::transport`: the full `mem` transport — the hub link plus the `MemCarrier` carrier.

use crate::transport::net::id::NodeSecret;
use crate::transport::net::mem::carrier::MemCarrier;
use crate::transport::net::mem::hub::MemNet;
use crate::transport::net::split::SplitTransport;
use std::sync::Arc;

impl MemNet {
    /// Transport of the node with key `secret`: a new hub slot and a lossless carrier.
    pub fn transport(&self, secret: &NodeSecret) -> Arc<SplitTransport> {
        let connector = self.connector(secret);
        let carrier = Arc::new(MemCarrier::new());
        Arc::new(SplitTransport::new(secret.peer_id(), connector, carrier))
    }
}
