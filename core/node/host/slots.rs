//! The host's map of network slots: for each slot its page-facing handle, its controller and the task
//! that runs its node cycle. The page reaches the slots through `HostControl`; the stop signal reaches
//! them through `stop_all_on_signal`.

use std::sync::{Arc, Mutex, RwLock};

use tokio::task::JoinHandle;

use crate::config::config_file::NetSlotId;
use crate::node::node_ctl::NodeCtl;
use crate::web_gui::{ConfigState, HostControl, SlotHandle};

/// One slot of the host.
struct Slot {
    handle: SlotHandle,
    /// The same controller as `handle.node`, kept concretely to stop it without the page-request line.
    ctl: Arc<NodeCtl>,
    /// The task running the slot's node cycle; `None` until it is spawned and after it is taken.
    task: Option<JoinHandle<anyhow::Result<()>>>,
}

/// All slots of the process host.
pub(crate) struct HostSlots {
    slots: RwLock<Vec<Slot>>,
}

impl HostSlots {
    /// A map holding the `main` slot made of `ctl` and its config state.
    pub(crate) fn with_main(ctl: Arc<NodeCtl>, config: ConfigState) -> Arc<HostSlots> {
        let handle = SlotHandle {
            id: NetSlotId::main(),
            node: ctl.clone(),
            config: Arc::new(Mutex::new(config)),
        };
        let slot = Slot {
            handle,
            ctl,
            task: None,
        };
        Arc::new(HostSlots {
            slots: RwLock::new(vec![slot]),
        })
    }

    /// Adds a started slot with the task that runs its node cycle.
    pub(crate) fn add_slot(
        &self,
        handle: SlotHandle,
        ctl: Arc<NodeCtl>,
        task: JoinHandle<anyhow::Result<()>>,
    ) {
        let slot = Slot {
            handle,
            ctl,
            task: Some(task),
        };
        self.write().push(slot);
    }

    /// How many slots the map holds.
    pub(crate) fn len(&self) -> usize {
        self.read().len()
    }

    /// Records the task that runs the slot's node cycle.
    pub(crate) fn set_task(&self, id: &NetSlotId, task: JoinHandle<anyhow::Result<()>>) {
        let mut slots = self.write();
        if let Some(slot) = slots.iter_mut().find(|s| s.handle.id == *id) {
            slot.task = Some(task);
        }
    }

    /// Takes the slots' tasks out, in slot order.
    pub(crate) fn take_tasks(&self) -> Vec<(NetSlotId, JoinHandle<anyhow::Result<()>>)> {
        let mut slots = self.write();
        slots
            .iter_mut()
            .filter_map(|s| s.task.take().map(|t| (s.handle.id.clone(), t)))
            .collect()
    }

    /// Stops every slot on the OS signal: the same transition as the page's stop, without its log line.
    pub(crate) fn stop_all_on_signal(&self) {
        self.read().iter().for_each(|s| s.ctl.stop_on_os_signal());
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, Vec<Slot>> {
        self.slots.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Vec<Slot>> {
        self.slots.write().unwrap_or_else(|e| e.into_inner())
    }
}

impl HostControl for HostSlots {
    fn slot(&self, id: &NetSlotId) -> Option<SlotHandle> {
        let slots = self.read();
        slots
            .iter()
            .find(|s| s.handle.id == *id)
            .map(|s| s.handle.clone())
    }

    fn main_slot(&self) -> SlotHandle {
        self.slot(&NetSlotId::main())
            .expect("the main slot is created with the map and never removed")
    }

    fn stop_all(&self) {
        self.read().iter().for_each(|s| s.handle.node.stop());
    }
}

#[cfg(test)]
#[path = "slots_tests.rs"]
mod tests;
