//! Adding a network at run time: the page's "+" hands back a slot that has no key yet, or makes a new one
//! whose file appears with its first key.

use super::launch::SlotLaunch;
use super::saved_slots::MAX_NETS;
use super::slots::{HostSlots, SlotContext};
use crate::config::config_file::{new_net_slot, NetSettings, NetSlotId, RunConfig};
use crate::node::node_ctl::Relaunch;
use crate::node::output::say;
use crate::web_gui::{
    now_unix, AddedSlot, HostControl, HostError, InviteJoinState, NodeStatus, SlotHandle,
};

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
    let waiting = matches!(
        handle.node.invite_join(),
        Some(v) if v.state == InviteJoinState::Waiting
    );
    keyless && !waiting && handle.node.snapshot().node.status == NodeStatus::Offline
}

impl HostSlots {
    /// The id of an existing slot that has no key (added ones first, then the main one), if any.
    fn slot_without_key(&self) -> Option<NetSlotId> {
        let mut all = self.slots();
        all.rotate_left(1);
        all.iter().find(|h| waits_for_key(h)).map(|h| h.id.clone())
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
        let slot = self.fresh_launch(ctx)?;
        let id = slot.id.clone();
        self.launch(slot, &ctx.sources)?;
        say("host", format!("network slot {id} added"));
        Ok(AddedSlot { id, fresh: true })
    }

    /// The launch of a new slot: a free id and the unwritten settings beside the main file.
    fn fresh_launch(&self, ctx: &SlotContext) -> Result<SlotLaunch, HostError> {
        let main_path = ctx.main_run.file.as_ref().map(|f| f.path().to_path_buf());
        let main_path = main_path.ok_or_else(|| HostError::Busy(ONE_NET_RUN.into()))?;
        let taken = |id: &NetSlotId| self.slot(id).is_some() || self.broken().contains(id);
        let (id, run) = new_net_slot(
            &ctx.main_run,
            &main_path,
            &self.shared_net(),
            &taken,
            now_unix(),
        )
        .map_err(io_error)?;
        let relaunch = self.relaunch_for_new(ctx, &run);
        Ok(SlotLaunch {
            id,
            run,
            relaunch,
            start: None,
        })
    }
}

#[cfg(test)]
#[path = "add_slot_tests.rs"]
mod tests;
