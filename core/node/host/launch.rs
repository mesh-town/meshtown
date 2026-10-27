//! Starting one network slot: the memory guard and the proxy seat, the page's handle, and the node task.

use std::sync::{Arc, Mutex};

use super::slots::HostSlots;
use crate::config::config_file::{NetSlotId, RunConfig};
use crate::node::guard_sources::GuardSources;
use crate::node::node_ctl::{NodeCtl, Relaunch};
use crate::node::output::say;
use crate::node::{run_network, NodeConfig};
use crate::web_gui::{ConfigState, HostError, SlotHandle};

/// What one slot starts with. `start` is the node config of a slot that has its key; without it the node
/// waits for the first key from the page.
pub(crate) struct SlotLaunch {
    pub id: NetSlotId,
    pub run: RunConfig,
    pub relaunch: Relaunch,
    pub start: Option<NodeConfig>,
}

/// Runs a slot's node cycle; its failure is a log line, the process goes on.
async fn logged(
    id: NetSlotId,
    cycle: impl std::future::Future<Output = anyhow::Result<()>>,
) -> anyhow::Result<()> {
    if let Err(e) = cycle.await {
        say(
            "node",
            format!("net slot {id} stopped with an error: {e:#}"),
        );
    }
    Ok(())
}

impl HostSlots {
    /// Puts the slot on the map and starts its node task; the error comes from adding it to the map.
    pub(super) fn launch(
        &self,
        launch: SlotLaunch,
        sources: &GuardSources,
    ) -> Result<(), HostError> {
        let SlotLaunch {
            id,
            run,
            relaunch,
            start,
        } = launch;
        let ctl = match start {
            Some(_) => NodeCtl::new(&run.effective_settings),
            None => NodeCtl::new_without_network(&run.effective_settings),
        };
        self.seat(&id, &ctl, sources.clone());
        let handle = SlotHandle {
            id: id.clone(),
            node: ctl.clone(),
            config: Arc::new(Mutex::new(ConfigState::from_run(&run))),
        };
        // Set before the task runs: a join started right after the slot is added reads these.
        ctl.set_test_net(relaunch.test_net.clone());
        ctl.set_owner_secret(relaunch.secret.clone());
        let cycle = logged(id, run_network(start, run, ctl.clone(), relaunch));
        self.push_slot(handle, ctl, cycle)
    }
}
