//! `GET`/`PUT /api/config`: the page's config state on top of `crate::config::config_file`.
//!
//! Files: `support.rs` — shared constants and small helpers (`get_path`, `field_entry`,
//! `json_response`); `state.rs` — `ConfigState` itself: reading (`page_config_document`, `config_get_body`, directory write
//! probe) and responses to write errors; `put.rs` — the `ConfigState::put` dispatcher by steps;
//! `put_merge.rs` — steps 6–8 (patch merge, check of unavailable fields, parsing of settings);
//! `put_write.rs` — steps 9–10 (writing to disk and the response), also `stale_outcome`; `put_name.rs` —
//! normalization of `node.name` inside the merge (step 6); `save_key.rs` — `ConfigState::save_network_key`;
//! `rewrite.rs` — writing the settings of a slot on the page's own initiative; `slot_overlay.rs` — the process block of an added slot;
//! `settings_access.rs` — the write through the slot's `SettingsFile`.

mod put;
mod put_merge;
mod put_name;
mod put_write;
mod rewrite;
mod save_key;
mod slot_overlay;
mod settings_access;
mod state;
mod support;

pub(crate) use put_write::ProxyPick;
pub use state::ConfigState;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "config_split_tests.rs"]
mod split_tests;
