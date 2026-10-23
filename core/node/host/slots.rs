//! The host's map of network slots: for each slot its page-facing handle, its controller and the task
//! that runs its node cycle. The page reaches the slots through `HostControl`; the stop signal reaches
//! them through `stop_all_on_signal`.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::saved_slots::{multi_net_for, MAX_NETS};
use crate::config::config_file::{NetSlotId, RunConfig};
use crate::node::guard_sources::GuardSources;
use crate::node::node_ctl::{NodeCtl, Relaunch};
use crate::node::output;
use crate::web_gui::{AddedSlot, ConfigState, HostControl, HostError, SlotHandle};

/// One slot of the host.
struct Slot {
    handle: SlotHandle,
    /// The same controller as `handle.node`, kept concretely to stop it without the page-request line.
    ctl: Arc<NodeCtl>,
    /// The task running the slot's node cycle; `None` until it is spawned and after it is taken.
    task: Option<JoinHandle<anyhow::Result<()>>>,
    /// Turns `true` when the slot's node cycle has ended (a closed sender counts as the end too);
    /// `None` for the main slot, whose task the host waits for itself.
    #[allow(dead_code)] // read by the slot retirement
    ended: Option<watch::Receiver<bool>>,
}

/// What adding a network at run time needs from the start of the run.
pub(crate) struct SlotContext {
    /// The main slot's run configuration as it was at the start.
    pub(crate) main_run: RunConfig,
    pub(crate) relaunch: Relaunch,
    pub(crate) sources: GuardSources,
}

/// All slots of the process host.
pub(crate) struct HostSlots {
    slots: RwLock<Vec<Slot>>,
    /// Set once the stop began: no slot is added after it.
    stopping: AtomicBool,
    /// Set once at the start of a run that keeps a settings file; without it no network is added.
    context: OnceLock<SlotContext>,
    /// Background tasks of the slots being left, awaited together with the slot tasks.
    #[allow(dead_code)] // filled by the slot retirement
    retiring: Mutex<Vec<JoinHandle<()>>>,
    /// Ids of the saved slots that could not be read or have no usable key, in disk order.
    broken: RwLock<Vec<NetSlotId>>,
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
            ended: None,
        };
        Arc::new(HostSlots {
            slots: RwLock::new(vec![slot]),
            stopping: AtomicBool::new(false),
            context: OnceLock::new(),
            retiring: Mutex::new(Vec::new()),
            broken: RwLock::new(Vec::new()),
        })
    }

    /// Adds a slot and spawns its node cycle, labelled with the slot id. Refused while the host is
    /// stopping and when the map is full; the count and the flag are checked under the map's write lock.
    pub(crate) fn push_slot(
        &self,
        handle: SlotHandle,
        ctl: Arc<NodeCtl>,
        cycle: impl Future<Output = anyhow::Result<()>> + Send + 'static,
    ) -> Result<(), HostError> {
        {
            let mut slots = self.write();
            if self.stopping.load(Ordering::SeqCst) {
                return Err(HostError::Busy("the node is stopping".into()));
            }
            if slots.len() >= MAX_NETS {
                return Err(HostError::TooManyNets);
            }
            slots.push(spawned_slot(handle, ctl, cycle));
        }
        self.sync_multi_net();
        Ok(())
    }

    /// Keeps what adding a network needs; the first call wins.
    pub(crate) fn set_context(&self, context: SlotContext) {
        self.context.set(context).ok();
    }

    /// What adding a network needs; `None` when the run keeps one network.
    pub(crate) fn context(&self) -> Option<&SlotContext> {
        self.context.get()
    }

    /// Whether the stop has begun.
    pub(super) fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::SeqCst)
    }

    /// Takes the background tasks of the slots being left.
    pub(crate) fn take_retiring(&self) -> Vec<JoinHandle<()>> {
        std::mem::take(&mut *self.retiring.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// Takes a slot out of the map; the main slot stays.
    #[allow(dead_code)] // used by the slot retirement
    fn remove_slot(&self, id: &NetSlotId) -> Option<Slot> {
        let removed = {
            let mut slots = self.write();
            let at = slots.iter().position(|s| s.handle.id == *id);
            at.filter(|_| *id != NetSlotId::main())
                .map(|at| slots.remove(at))
        };
        self.sync_multi_net();
        removed
    }

    /// Turns the log's network label on or off by the number of slots; the map's lock is not held.
    fn sync_multi_net(&self) {
        output::set_multi_net(multi_net_for(self.len()));
    }

    /// Remembers a saved slot that could not be started because its folder is unusable.
    pub(super) fn mark_broken(&self, id: NetSlotId) {
        let mut broken = self.broken.write().unwrap_or_else(|e| e.into_inner());
        if !broken.contains(&id) {
            broken.push(id);
        }
    }

    /// Forgets a broken slot; `true` when it was one.
    #[allow(dead_code)] // used by the slot retirement
    pub(super) fn take_broken(&self, id: &NetSlotId) -> bool {
        let mut broken = self.broken.write().unwrap_or_else(|e| e.into_inner());
        let before = broken.len();
        broken.retain(|b| b != id);
        broken.len() != before
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
        let slots = self.write();
        self.stopping.store(true, Ordering::SeqCst);
        slots.iter().for_each(|s| s.ctl.stop_on_os_signal());
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, Vec<Slot>> {
        self.slots.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Vec<Slot>> {
        self.slots.write().unwrap_or_else(|e| e.into_inner())
    }
}

/// A slot whose node cycle is spawned now; `ended` turns `true` after the cycle returns.
fn spawned_slot(
    handle: SlotHandle,
    ctl: Arc<NodeCtl>,
    cycle: impl Future<Output = anyhow::Result<()>> + Send + 'static,
) -> Slot {
    let (done, ended) = watch::channel(false);
    let label = Some(Arc::from(handle.id.to_string()));
    let run = async move {
        let outcome = cycle.await;
        done.send_replace(true);
        outcome
    };
    let task = output::spawn(output::with_net(label, run));
    Slot {
        handle,
        ctl,
        task: Some(task),
        ended: Some(ended),
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

    fn slots(&self) -> Vec<SlotHandle> {
        self.read().iter().map(|s| s.handle.clone()).collect()
    }

    fn stop_all(&self) {
        let slots = self.write();
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

    fn retire_slot(&self, _id: &NetSlotId, _announced: bool) -> Result<(), HostError> {
        Err(HostError::Busy("not yet".into()))
    }
}

#[cfg(test)]
#[path = "slots_tests.rs"]
mod tests;
