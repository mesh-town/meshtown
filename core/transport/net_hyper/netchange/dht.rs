//! The real executor of the network change reaction: a townjump DHT node and the shared dial state.

use super::react::{react_loop, NetReact};
use super::source::NetwatchSource;
use crate::net::stream::BoxFuture;
use crate::net_hyper::accept::AcceptCtx;
use crate::output::say;
use townjump_dht::Dht;
use townjump_dht_peeroxide::PeeroxideDht;
use std::sync::Arc;
use tokio::sync::watch;

/// Executor of the reaction over a DHT node: sessions are closed by `AcceptCtx`, `suspend`/`resume` by the node.
pub(crate) struct DhtReact {
    node: Arc<PeeroxideDht>,
    ctx: Arc<AcceptCtx>,
}

impl DhtReact {
    pub(crate) fn new(node: Arc<PeeroxideDht>, ctx: Arc<AcceptCtx>) -> Self {
        Self { node, ctx }
    }

    async fn resume_open(&self) -> Result<(), String> {
        let res = self.node.resume().await.map_err(|e| e.to_string());
        self.ctx.hold_dials(false);
        res
    }
}

impl NetReact for DhtReact {
    fn close_sessions(&self, code: u32, reason: &[u8]) -> usize {
        self.ctx.hold_dials(true);
        self.ctx.close_sessions(code, reason)
    }

    fn suspend(&self) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async move { self.node.suspend().await.map_err(|e| e.to_string()) })
    }

    fn resume(&self) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(self.resume_open())
    }
}

/// Starts the network watcher; if it did not come up — a log line, the transport works without it.
pub(crate) async fn spawn(react: Arc<dyn NetReact>, stop: watch::Receiver<bool>) {
    match NetwatchSource::start().await {
        Ok(source) => {
            say("hyper", "network watcher started");
            tokio::spawn(react_loop(Box::new(source), react, stop));
        }
        Err(e) => say("hyper", format!("network watcher did not come up: {e}")),
    }
}

#[cfg(test)]
#[path = "dht_tests.rs"]
mod tests;
