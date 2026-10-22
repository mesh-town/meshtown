//! What the page sees of the process host: the network slots it can address. The page reaches a node and its
//! config state only through a `SlotHandle`, so one page can serve any number of slots.

use std::sync::{Arc, Mutex};

use super::config_api::ConfigState;
use super::control::NodeControl;
use crate::config::config_file::NetSlotId;

/// One slot: its id, the node control surface and the config state the page edits for it.
#[derive(Clone)]
pub struct SlotHandle {
    pub id: NetSlotId,
    pub node: Arc<dyn NodeControl>,
    pub config: Arc<Mutex<ConfigState>>,
}

/// Why a host operation on slots failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostError {
    TooManyNets,
    UnknownNet,
    AlreadyAdded,
    Busy(String),
    Io(String),
}

/// The process host as the page sees it.
pub trait HostControl: Send + Sync {
    /// The slot with this id, if the host has it.
    fn slot(&self, id: &NetSlotId) -> Option<SlotHandle>;
    /// The primary slot; it always exists.
    fn main_slot(&self) -> SlotHandle;
    /// Stops every slot's node (as `NodeControl::stop` does for one).
    fn stop_all(&self);
}

/// A host with a single `main` slot.
struct SingleSlotHost {
    main: SlotHandle,
}

impl HostControl for SingleSlotHost {
    fn slot(&self, id: &NetSlotId) -> Option<SlotHandle> {
        (*id == self.main.id).then(|| self.main.clone())
    }

    fn main_slot(&self) -> SlotHandle {
        self.main.clone()
    }

    fn stop_all(&self) {
        self.main.node.stop();
    }
}

/// A host holding exactly one slot, `main`, built from a node and its config state.
pub fn single_slot_host(node: Arc<dyn NodeControl>, config: ConfigState) -> Arc<dyn HostControl> {
    let main = SlotHandle {
        id: NetSlotId::main(),
        node,
        config: Arc::new(Mutex::new(config)),
    };
    Arc::new(SingleSlotHost { main })
}
