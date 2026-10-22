//! Source of node information (discovery plugin): publishes its own address and returns foreign ones.
//! Merging of sources is in `discovery_mux`, only the trait is here.

use crate::net::addr::PeerAddr;
use crate::net::error::NetResult;
use crate::net::stream::BoxFuture;
use std::sync::Arc;
use tokio::sync::mpsc;

pub trait Discovery: Send + Sync {
    fn name(&self) -> &'static str;
    /// Returns whether the source publishes its own address.
    fn publishes(&self) -> bool {
        false
    }
    /// Readers' sources return `Ok(())`.
    fn publish(&self, own_addr: &PeerAddr) -> BoxFuture<'_, NetResult<()>>;
    /// Works while `out` is open.
    fn run_source(self: Arc<Self>, out: mpsc::Sender<PeerAddr>) -> BoxFuture<'static, ()>;
}

/// The node's own address, for the sources that publish it.
pub trait LocalAddr: Send + Sync {
    /// Own address right now.
    fn current(&self) -> PeerAddr;
    /// Waits for "online"; there is no deadline — the caller sets it.
    fn ready(&self) -> BoxFuture<'_, ()>;
}
