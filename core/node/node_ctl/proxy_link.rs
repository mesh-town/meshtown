//! The controller's link to the process proxy: the slot id it publishes under and the seat it
//! publishes to.

use std::sync::Arc;

use tokio::sync::watch;

use super::state::NodeCtl;
use crate::config::config_file::{NetSlotId, ProcessSettings};
use crate::node::RunningNode;
use crate::proxy::{ProxyFace, ProxyFront, ProxySeat};

impl NodeCtl {
    /// The process host gives the controller the proxy seat and the slot id to publish under;
    /// the first call wins.
    pub fn set_proxy_seat(&self, id: NetSlotId, seat: Arc<ProxySeat>) {
        let _ = self.proxy.set((id, seat));
    }

    /// The face of the process listener; `None` when there is no seat.
    pub(crate) fn proxy_face(&self) -> Option<watch::Receiver<ProxyFace>> {
        self.proxy.get().map(|(_, seat)| seat.face())
    }

    /// The process listener, for the console; `None` when there is no seat or no listener.
    pub(crate) fn proxy_front(&self) -> Option<ProxyFront> {
        self.proxy.get().and_then(|(_, seat)| seat.front())
    }

    /// The node is up: its proxy configuration goes to the seat.
    pub(crate) fn node_up(&self, node: &RunningNode) {
        let link = self.proxy.get();
        link.iter()
            .for_each(|(id, seat)| seat.node_up(id, node.proxy_config()));
    }

    /// The node is going down: the seat drops its proxy configuration.
    pub(crate) fn node_down(&self) {
        let link = self.proxy.get();
        link.iter().for_each(|(id, seat)| seat.node_down(id));
    }

    /// Brings the process listener to the settings; only the slot that carries the process owns it.
    pub(crate) async fn apply_process_proxy(
        &self,
        settings: &ProcessSettings,
    ) -> anyhow::Result<()> {
        let front = self
            .proxy
            .get()
            .filter(|(id, _)| id.carries_process())
            .and_then(|(_, seat)| seat.front());
        match front {
            Some(front) => front.apply(settings).await,
            None => Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "proxy_link_tests.rs"]
mod tests;
