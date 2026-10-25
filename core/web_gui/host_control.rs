//! What the page sees of the process host: the network slots it can address. The page reaches a node and its
//! config state only through a `SlotHandle`, so one page can serve any number of slots.

use std::sync::{Arc, Mutex};

use super::config_api::ConfigState;
use super::control::NodeControl;
use crate::config::config_file::NetSlotId;
pub use crate::node::host::Notice;
use crate::proxy::ProxyFront;

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

/// A slot made by `HostControl::add_slot`: `fresh` is false when an existing empty slot was handed back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddedSlot {
    pub id: NetSlotId,
    pub fresh: bool,
}

/// A `local` port already opened by one network of this node, and the name of that network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenedLocal {
    pub local: u16,
    pub net: String,
}

/// The process host as the page sees it.
pub trait HostControl: Send + Sync {
    /// The slot with this id, if the host has it.
    fn slot(&self, id: &NetSlotId) -> Option<SlotHandle>;
    /// The primary slot; it always exists.
    fn main_slot(&self) -> SlotHandle;
    /// Every live slot, in the host's own order (the primary first).
    fn slots(&self) -> Vec<SlotHandle>;
    /// The `local` ports opened by every slot except `except`, each with its network's name.
    fn opened_locals(&self, except: &NetSlotId) -> Vec<OpenedLocal> {
        opened_locals_of(self.slots(), except)
    }
    /// Stops every slot's node (as `NodeControl::stop` does for one).
    fn stop_all(&self);
    /// Adds an empty slot waiting for a network key, or hands back an existing empty one.
    fn add_slot(&self) -> Result<AddedSlot, HostError>;
    /// Takes a slot out; `announced` says whether the network was told about the leaving. The files
    /// are moved aside in the background.
    fn retire_slot(&self, id: &NetSlotId, announced: bool) -> Result<(), HostError>;
    /// Slots whose files could not be used; they have no node and no handle.
    fn broken(&self) -> Vec<NetSlotId> {
        Vec::new()
    }
    /// Makes this slot the one that owns the proxy front's exit.
    fn claim_proxy(&self, _id: &NetSlotId) -> Result<(), HostError> {
        Ok(())
    }
    /// The slot that owns the proxy front's exit.
    fn proxy_net(&self) -> NetSlotId {
        NetSlotId::main()
    }
    /// The process-wide proxy front, if the host has one.
    fn proxy_front(&self) -> Option<ProxyFront> {
        None
    }
    /// What the page must show until it is acknowledged, oldest first.
    fn notices(&self) -> Vec<Notice> {
        Vec::new()
    }
    /// Removes the notice; `false` when there is none with this id.
    fn ack_notice(&self, _id: u64) -> bool {
        false
    }
}

/// Every `local` of `ports.opened` in the slots other than `except`. Config locks are taken one at a time;
/// a slot without a key opens nothing.
pub(crate) fn opened_locals_of(slots: Vec<SlotHandle>, except: &NetSlotId) -> Vec<OpenedLocal> {
    let mut out = Vec::new();
    for slot in slots.iter().filter(|s| s.id != *except) {
        let cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
        if cfg.current_network_key.is_none() {
            continue;
        }
        let net = net_name(&cfg.network_block(), &slot.id);
        let opened = cfg.file_settings.net.ports_opened.iter();
        out.extend(opened.map(|o| OpenedLocal {
            local: o.local,
            net: net.clone(),
        }));
    }
    out
}

/// The name a network goes by in messages: its label, else `Network` and the first four characters of its id;
/// the slot id when the key does not parse.
fn net_name(block: &serde_json::Value, slot: &NetSlotId) -> String {
    super::net_name::net_display_name(block["label"].as_str(), block["id_hex"].as_str())
        .unwrap_or_else(|| slot.to_string())
}

/// A host with a single `main` slot.
struct SingleSlotHost {
    main: SlotHandle,
    front: Option<ProxyFront>,
}

impl HostControl for SingleSlotHost {
    fn slot(&self, id: &NetSlotId) -> Option<SlotHandle> {
        (*id == self.main.id).then(|| self.main.clone())
    }

    fn main_slot(&self) -> SlotHandle {
        self.main.clone()
    }

    fn slots(&self) -> Vec<SlotHandle> {
        vec![self.main.clone()]
    }

    fn stop_all(&self) {
        self.main.node.stop();
    }

    fn add_slot(&self) -> Result<AddedSlot, HostError> {
        Err(HostError::TooManyNets)
    }

    fn retire_slot(&self, id: &NetSlotId, _announced: bool) -> Result<(), HostError> {
        crate::node::output::say(
            "web",
            format!("this run keeps one network: slot {id} stays"),
        );
        Ok(())
    }

    fn proxy_front(&self) -> Option<ProxyFront> {
        self.front.clone()
    }
}

/// A host holding exactly one slot, `main`, built from a node and its config state.
pub fn single_slot_host(node: Arc<dyn NodeControl>, config: ConfigState) -> Arc<dyn HostControl> {
    single_slot_host_with_front(node, config, None)
}

/// Like `single_slot_host`, with the process proxy front the page may reconfigure.
pub fn single_slot_host_with_front(
    node: Arc<dyn NodeControl>,
    config: ConfigState,
    front: Option<ProxyFront>,
) -> Arc<dyn HostControl> {
    let main = SlotHandle {
        id: NetSlotId::main(),
        node,
        config: Arc::new(Mutex::new(config)),
    };
    Arc::new(SingleSlotHost { main, front })
}

#[cfg(test)]
#[path = "opened_locals_tests.rs"]
mod opened_locals_tests;
