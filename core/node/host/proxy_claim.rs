//! Choosing the network whose exit the proxy uses: there is one such network at a time. The choice is
//! written to the main settings file, and the exits of the network that held it before are removed.

use std::sync::MutexGuard;

use super::slots::HostSlots;
use crate::config::config_file::NetSlotId;
use crate::node::output::say;
use crate::web_gui::{HostControl, HostError, SlotHandle};

impl HostSlots {
    /// Makes `id` the network the proxy serves. With `clear_prev`, the route exits of the network that
    /// held the choice are removed once the choice is written. A failed write of the main file leaves
    /// the choice as it was.
    pub(crate) fn claim(&self, id: &NetSlotId, clear_prev: bool) -> Result<(), HostError> {
        let guard = self.claim_guard();
        self.claim_locked(id, clear_prev, &guard)
    }

    /// Writes the choice and moves the seat to `id`; the caller holds the claim lock.
    pub(super) fn claim_locked(
        &self,
        id: &NetSlotId,
        clear_prev: bool,
        _claim: &MutexGuard<'_, ()>,
    ) -> Result<(), HostError> {
        let Some(seat) = self.proxy.get() else {
            return Ok(());
        };
        let prev = seat.chosen();
        if prev == *id {
            return Ok(());
        }
        let main = self.main_slot();
        self.slot(id).ok_or(HostError::UnknownNet)?;
        write_choice(&main, id).map_err(HostError::Io)?;
        seat.choose(id.clone());
        if clear_prev {
            self.clear_exits_of(&prev, id);
        }
        Ok(())
    }

    /// The claim lock, taken before any slot's config state.
    pub(super) fn claim_guard(&self) -> MutexGuard<'_, ()> {
        self.claim.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A slot that held the choice has left: the proxy goes back to the main network. The slot is gone,
    /// so there are no exits to remove.
    pub(super) fn leave_proxy_seat(&self, gone: &NetSlotId, claim: &MutexGuard<'_, ()>) {
        let Some(seat) = self.proxy.get().filter(|s| s.chosen() == *gone) else {
            return;
        };
        if let Err(e) = self.claim_locked(&NetSlotId::main(), false, claim) {
            let why = format!("couldn't return the proxy to the main network: {e:?}");
            say("proxy", why);
            seat.choose(NetSlotId::main());
        }
    }

    /// Removes the route exits of `prev`, which no longer holds the choice; a failure is only logged.
    fn clear_exits_of(&self, prev: &NetSlotId, chosen: &NetSlotId) {
        let Some(slot) = self.slot(prev) else {
            return;
        };
        let mut removed = 0;
        let edit = |s: &mut crate::config::config_file::Settings| {
            removed = s.net.route_exits.len();
            s.net.route_exits.clear();
        };
        let mut state = slot.config.lock().unwrap_or_else(|e| e.into_inner());
        let done = state.rewrite_settings(slot.node.as_ref(), edit);
        drop(state);
        match done {
            Ok(true) => say(
                "web",
                format!(
                    "page: exit chosen in network {chosen}, route exits removed in {prev}: {removed}"
                ),
            ),
            Ok(false) => {}
            Err(e) => say(
                "web",
                format!(
                    "page: exit chosen in network {chosen}, route exits not removed in {prev}: {e}"
                ),
            ),
        }
    }

    /// Start-up lines about a choice that cannot work: a network that is not loaded, and exits left
    /// in networks the proxy does not serve.
    pub(crate) fn report_proxy_start(&self) {
        let Some(seat) = self.proxy.get() else {
            return;
        };
        let chosen = seat.chosen();
        let slots = self.slots();
        report_unloaded(&slots, &chosen);
        slots.iter().for_each(|s| report_unused_exits(s, &chosen));
    }
}

/// Sets `proxy.net` of the main file to `id`.
fn write_choice(main: &SlotHandle, id: &NetSlotId) -> Result<(), String> {
    let mut state = main.config.lock().unwrap_or_else(|e| e.into_inner());
    state
        .rewrite_settings(main.node.as_ref(), |s| s.process.proxy_net = id.clone())
        .map(|_| ())
}

fn report_unloaded(slots: &[SlotHandle], chosen: &NetSlotId) {
    if slots.iter().any(|s| s.id == *chosen) {
        return;
    }
    say(
        "proxy",
        format!("network {chosen} is not loaded; the proxy refuses connections"),
    );
}

fn report_unused_exits(slot: &SlotHandle, chosen: &NetSlotId) {
    let state = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    if slot.id == *chosen || state.file_settings.net.route_exits.is_empty() {
        return;
    }
    say(
        "proxy",
        format!(
            "network {} has route exits, but the proxy serves {chosen}; they are not used",
            slot.id
        ),
    );
}

#[cfg(test)]
#[path = "proxy_claim_race_tests.rs"]
mod race_tests;
#[cfg(test)]
#[path = "proxy_claim_tests.rs"]
mod tests;
