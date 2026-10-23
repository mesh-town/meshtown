//! Per-network slot storage: the identifier of a slot, the slot folders under `nets/` beside the main
//! settings file, setting a slot aside, the file access seam (`SettingsFile`) and building the start
//! settings of an added slot.

mod aside;
mod fresh;
mod ids;
mod layout;
mod list;
mod prepare;
mod prepare_log;
mod settings_file;
mod slot_id;

pub use aside::{set_aside_main, set_aside_slot};
pub use fresh::{copy_shared_net, fresh_net_slot, new_net_slot};
pub use ids::new_slot_id;
pub use layout::net_slot_path;
pub use list::{list_net_slots, NetSlotOnDisk};
pub use prepare::prepare_net_slot;
pub use settings_file::{LoadedSettings, MainFile, NetFile, SettingsFile};
pub use slot_id::NetSlotId;

#[cfg(test)]
#[path = "list_tests.rs"]
mod list_tests;

#[cfg(test)]
#[path = "prepare_tests.rs"]
mod prepare_tests;
