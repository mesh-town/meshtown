//! Start at login: the entry that launches the tray app when the user signs in.
//!
//! The platform crate provides an [`AutostartBackend`] and registers it with [`install`];
//! the node page reads and changes the entry through it. The texts of the entries for all
//! systems are pure functions here, so they are checked on any host.

mod backend;
mod desktop_file;
mod file_entry;
mod launch_agent;
mod run_key;

#[cfg(test)]
pub(crate) mod fake;

pub use backend::{install, installed, plan_change, AutostartBackend, Change, EntryState};
pub use desktop_file::{desktop_entry, desktop_entry_path, desktop_exec_arg, desktop_switched_off};
pub use file_entry::FileEntry;
pub use launch_agent::{
    launch_agent_path, launch_agent_plist, launch_agent_switched_off, mac_launch, MacLaunch,
    FALLBACK_LABEL,
};
pub use run_key::{
    approved_disabled, run_entry_state, run_value, APPROVED_KEY, RUN_KEY, VALUE_NAME,
};
