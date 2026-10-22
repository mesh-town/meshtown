//! The node's settings file on disk (`CONFIG-FILE.md`).
//!
//! What it does: finds `config.json`, reads it field by field (a damaged field gets the default), merges it with
//! command-line flags, writes the file atomically, keeps the node secret in the file envelope.
//! What it doesn't do: doesn't bring up the network or parse arguments — that is `node.rs` and `main.rs`;
//! doesn't invent the node name — that is the node identity's job; doesn't hand the config to the page — that is a separate web module.
//! Related: `main.rs` calls `resolve_config_source` and `prepare`; the result goes to `node::NodeConfig`.
//!
//! Parsing by file (this entry point only declares them and re-exports the public path
//! `crate::config_file::*`, so callers don't notice the move into a folder):
//! `settings` — the `Settings` document (two blocks) and the `TransportMode`/`ListenAccess` enums;
//! `process_settings`/`net_settings` — the `ProcessSettings` (process host) and `NetSettings` blocks
//! (what the node reads); `path` — finding the file on disk; `time` —
//! time in the CONFIG-FILE.md format; `report` — what the settings module hands out; `parse_fields`/`parse` — reading
//! JSON into `Settings`; `render_config_file` — the inverse, `Settings` to JSON; `save_config_file` — writing the file to disk and its
//! relocation; `overrides` — command-line flags over the file; `run` — `prepare()`, assembling
//! everything together for node start.

pub(crate) mod fields;
mod iface_filter;
mod overrides;
mod parse;
mod parse_fields;
mod parse_ports;
mod presence;
mod net_settings;
mod path;
mod process_settings;
mod render;
mod report;
mod retired;
mod run;
mod save;
mod settings;
mod time;

#[cfg(test)]
pub(crate) mod test_support;

pub use iface_filter::IfaceFilter;
pub use presence::has_field;
pub use overrides::CliOverrides;
pub use parse::{load_config_file, parse_text, LoadedConfigFile, ParsedConfigFile, SecretRead};
pub use path::{
    current_env, is_root_linux, resolve_config_source, standard_path, ConfigSource, CurrentEnv, Os,
    PathEnv, PathRule,
};
pub use render::render_config_file;
pub use retired::port_flag_warning;
pub use report::{FieldError, FileReport, FileState};
pub use run::{create_fresh, new_secret, prepare, Fresh, RunConfig};
pub use save::{save_config_file, save_with_floor, set_aside_bad, SaveError};
pub(crate) use save::write_tmp_and_rename;
pub use net_settings::NetSettings;
pub use process_settings::ProcessSettings;
pub use settings::{Settings, TransportMode, ListenAccess};
pub use time::{iso_utc, stamp_utc, utc_parts};

pub const FILE_NAME: &str = "config.json";
pub const DIR_NAME: &str = "meshtown";
pub const CONFIG_VERSION: u64 = 1;
