//! The page's view of the slot map: `HostControl` over `HostSlots`.

use std::sync::atomic::Ordering;

use super::slots::HostSlots;
use crate::config::config_file::NetSlotId;
use crate::proxy::ProxyFront;
use crate::web_gui::{AddedSlot, HostControl, HostError, SlotHandle};

impl HostControl for HostSlots {
    fn slot(&self, id: &NetSlotId) -> Option<SlotHandle> {
        let slots = self.read_slots();
        slots
            .iter()
            .find(|s| s.handle.id == *id)
            .map(|s| s.handle.clone())
    }

    fn main_slot(&self) -> SlotHandle {
        self.slot(&NetSlotId::main())
            .expect("the main slot is created with the map and never removed")
    }

    fn slots(&self) -> Vec<SlotHandle> {
        self.read_slots().iter().map(|s| s.handle.clone()).collect()
    }

    fn stop_all(&self) {
        let slots = self.write_slots();
        self.stopping.store(true, Ordering::SeqCst);
        slots.iter().for_each(|s| s.handle.node.stop());
    }

    fn broken(&self) -> Vec<NetSlotId> {
        self.broken
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn add_slot(&self) -> Result<AddedSlot, HostError> {
        self.add_new_slot()
    }

    fn retire_slot(&self, id: &NetSlotId, announced: bool) -> Result<(), HostError> {
        self.retire(id, announced)
    }

    fn claim_proxy(&self, id: &NetSlotId) -> Result<(), HostError> {
        self.claim(id, true)
    }

    fn proxy_net(&self) -> NetSlotId {
        let seat = self.proxy.get();
        seat.map_or_else(NetSlotId::main, |s| s.chosen())
    }

    fn proxy_front(&self) -> Option<ProxyFront> {
        self.proxy.get().and_then(|s| s.front())
    }
}
