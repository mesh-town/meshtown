//! The node's settings file on disk (`docs/CONFIG-FILE.md`).
//!
//! What it does: finds `config.json`, reads it field by field (a damaged field gets the default), merges it with
//! command-line flags, writes the file atomically, keeps the node secret in the file envelope.
//! What it doesn't do: doesn't bring up the network or parse arguments — that is `node.rs` and `main.rs`;
//! doesn't invent the node name — that is the node identity's job; doesn't hand the config to the page — that is a separate web module.
//! Related: `main.rs` calls `resolve_config_source` and `prepare`; the result goes to `node::NodeConfig`.
//!
//! Parsing by file (this entry point only declares them and re-exports the public path
//! `crate::config::config_file::*`, so callers don't notice the move into a folder):
//! `settings` — the `Settings` document (two blocks) and the `ListenAccess` enum;
//! `process_settings`/`net_settings` — the `ProcessSettings` (process host) and `NetSettings` blocks
//! (what the node reads); `path` — finding the file on disk; `time` —
//! time in the docs/CONFIG-FILE.md format; `report` — what the settings module hands out; `parse_fields`/`parse` — reading
//! JSON into `Settings`; `render_config_file` — the inverse, `Settings` to JSON; `save_config_file` — writing the file to disk and its
//! relocation; `overrides` — command-line flags over the file; `run` — `prepare()`, assembling
//! everything together for node start.

mod field_set;
pub(crate) mod fields;
mod iface_filter;
mod net_file;
mod net_settings;
mod overrides;
mod parse;
mod parse_fields;
mod parse_ports;
mod path;
mod presence;
mod process_settings;
mod relay_mode;
mod render;
mod report;
mod retired;
mod run;
mod save;
mod settings;
mod slot_store;
mod time;

#[cfg(test)]
pub(crate) mod test_support;

pub use crate::node::machine_memory::NODE_MEMORY_WARN_MIB_DEFAULT;
pub use field_set::{FieldSet, SetRefusal};
pub use iface_filter::IfaceFilter;
pub use net_file::{
    load_net_file, parse_net_text, render_net_file, save_net_file, LoadedNetFile, ParsedNetFile,
};
pub use net_settings::NetSettings;
pub use overrides::CliOverrides;
pub use parse::{load_config_file, parse_text, LoadedConfigFile, ParsedConfigFile, SecretRead};
pub use path::{
    current_env, is_root_linux, resolve_config_source, standard_path, ConfigSource, CurrentEnv, Os,
    PathEnv, PathRule,
};
pub use presence::has_field;
pub use process_settings::ProcessSettings;
pub use relay_mode::RelayMode;
pub use render::render_config_file;
pub use report::{FieldError, FileReport, FileState};
pub use retired::log_retired;
pub use run::{create_fresh, new_secret, prepare, Fresh, RunConfig};
pub(crate) use save::write_tmp_and_rename;
pub use save::{save_config_file, save_with_floor, set_aside_bad, SaveError};
pub use settings::{ListenAccess, Settings};
pub use slot_store::{
    copy_shared_net, forget_invited_peers, list_net_slots, load_invited_peers, net_slot_path,
    new_net_slot, new_slot_id, prepare_net_slot, save_invited_peers, set_aside_main,
    set_aside_slot, LoadedSettings, MainFile, NetFile, NetSlotId, NetSlotOnDisk, SettingsFile,
};
pub use time::{iso_utc, stamp_utc, utc_parts};

pub const FILE_NAME: &str = "config.json";
pub const DIR_NAME: &str = "meshtown";
pub const CONFIG_VERSION: u64 = 1;
