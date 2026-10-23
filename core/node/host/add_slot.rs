//! Adding a network at run time: the page's "+" hands back a slot that has no key yet, or makes a new one
//! whose file appears with its first key.

use std::sync::Arc;

use super::saved_slots::MAX_NETS;
use super::slots::{HostSlots, SlotContext};
use crate::config::config_file::{fresh_net_slot, new_slot_id, NetSettings, NetSlotId, RunConfig};
use crate::node::node_ctl::{NodeCtl, Relaunch};
use crate::node::output::say;
use crate::node::{run_node_awaiting_key, IdentityState};
use crate::web_gui::{
    now_unix, AddedSlot, ConfigState, HostControl, HostError, NodeStatus, SlotHandle,
};

/// How many random ids are tried before the add gives up.
const ID_ATTEMPTS: usize = 8;

const ONE_NET_RUN: &str = "this run keeps one network: start with a settings file";

fn io_error(e: anyhow::Error) -> HostError {
    HostError::Io(format!("{e:#}"))
}

/// A slot that has no key and is not joining one.
fn waits_for_key(handle: &SlotHandle) -> bool {
    let keyless = handle
        .config
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .current_network_key
        .is_none();
    keyless && handle.node.snapshot().node.status == NodeStatus::Offline
}

/// Runs a new slot's node cycle; its failure is a log line, the process goes on.
async fn run_fresh(
    id: NetSlotId,
    run: RunConfig,
    ctl: Arc<NodeCtl>,
    relaunch: Relaunch,
) -> anyhow::Result<()> {
    let identity = IdentityState::from_report(&run.file_report);
    if let Err(e) = run_node_awaiting_key(run.effective_settings, identity, ctl, relaunch).await {
        say(
            "node",
            format!("net slot {id} stopped with an error: {e:#}"),
        );
    }
    Ok(())
}

impl HostSlots {
    /// The id of an existing slot that has no key (added ones first, then the main one), if any.
    fn slot_without_key(&self) -> Option<NetSlotId> {
        let mut all = self.slots();
        all.rotate_left(1);
        all.iter().find(|h| waits_for_key(h)).map(|h| h.id.clone())
    }

    /// An id free on disk and among the running and broken slots.
    fn free_slot_id(&self, ctx: &SlotContext) -> Result<NetSlotId, HostError> {
        let main_path = ctx.main_run.file.as_ref().map(|f| f.path().to_path_buf());
        let main_path = main_path.ok_or_else(|| HostError::Busy(ONE_NET_RUN.into()))?;
        for _ in 0..ID_ATTEMPTS {
            let id = new_slot_id(&main_path).map_err(io_error)?;
            if self.slot(&id).is_none() && !self.broken().contains(&id) {
                return Ok(id);
            }
        }
        Err(HostError::Io("couldn't find a free network id".into()))
    }

    /// Every domain the running networks know, once, in slot order.
    fn known_domains(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for handle in self.slots() {
            for domain in handle.node.snapshot().domains {
                if !names.contains(&domain.name) {
                    names.push(domain.name);
                }
            }
        }
        names
    }

    /// What the main slot's file holds of the settings shared by all networks.
    fn shared_net(&self) -> NetSettings {
        let main = self.main_slot();
        let state = main.config.lock().unwrap_or_else(|e| e.into_inner());
        state.file_settings.net.clone()
    }

    /// The launch settings of a new slot: the lists of the run, seeded with the domains known now.
    pub(super) fn relaunch_for_new(&self, ctx: &SlotContext, run: &RunConfig) -> Relaunch {
        let mut relaunch = ctx.relaunch.for_net_slot(run);
        relaunch.initial_domains = self.known_domains();
        relaunch
    }

    /// Hands back a slot waiting for a key, or makes a new one; the new slot's file is not written yet.
    pub(super) fn add_new_slot(&self) -> Result<AddedSlot, HostError> {
        let ctx = self
            .context()
            .ok_or_else(|| HostError::Busy(ONE_NET_RUN.into()))?;
        if self.is_stopping() {
            return Err(HostError::Busy("the node is stopping".into()));
        }
        if let Some(id) = self.slot_without_key() {
            return Ok(AddedSlot { id, fresh: false });
        }
        if self.len() >= MAX_NETS {
            return Err(HostError::TooManyNets);
        }
        let id = self.free_slot_id(ctx)?;
        let run =
            fresh_net_slot(&ctx.main_run, &self.shared_net(), &id, now_unix()).map_err(io_error)?;
        self.start_fresh(ctx, id.clone(), run)?;
        say("host", format!("network slot {id} added"));
        Ok(AddedSlot { id, fresh: true })
    }

    fn start_fresh(
        &self,
        ctx: &SlotContext,
        id: NetSlotId,
        run: RunConfig,
    ) -> Result<(), HostError> {
        let ctl = NodeCtl::new_without_network(&run.effective_settings);
        ctl.set_guard_sources(id.clone(), ctx.sources.clone());
        let relaunch = self.relaunch_for_new(ctx, &run);
        let handle = SlotHandle {
            id: id.clone(),
            node: ctl.clone(),
            config: Arc::new(std::sync::Mutex::new(ConfigState::from_run(&run))),
        };
        let cycle = run_fresh(id, run, ctl.clone(), relaunch);
        self.push_slot(handle, ctl, cycle)
    }
}

#[cfg(test)]
#[path = "add_slot_tests.rs"]
mod tests;
