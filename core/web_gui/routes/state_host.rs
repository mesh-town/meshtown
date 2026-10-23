//! Keys of `/api/state` that name the slot and list the networks of the process.

use serde_json::{json, Value};

use crate::config::config_file::NetSlotId;
use crate::web_gui::net_brief::NetBrief;

/// The api version of the multi-network replies.
pub(super) const API: u32 = 2;

/// Marks the state with the api version and the slot it describes.
pub(super) fn apply_slot(value: &mut Value, slot: &NetSlotId) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    obj.insert("api".into(), json!(API));
    obj.insert("slot".into(), json!(slot.to_string()));
}

/// Adds the list of networks of the process to the state of `slot`.
pub(super) fn apply_host(value: &mut Value, slot: &NetSlotId, nets: Vec<NetBrief>) {
    apply_slot(value, slot);
    if let Some(obj) = value.as_object_mut() {
        obj.insert("nets".into(), json!(nets));
    }
}
