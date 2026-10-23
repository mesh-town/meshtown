//! The process block of an added network slot. The block belongs to the process, not to the network, so a
//! slot shows the primary slot's values, cannot change them, and its file keeps only the network block.

use axum::http::StatusCode;
use axum::response::Response;
use serde_json::Value;

use crate::config::config_file::fields::{self, Block};
use crate::config::config_file::ProcessSettings;
use crate::web_gui::http::json_err;

use super::state::ConfigState;

impl ConfigState {
    /// Shows `p` as this slot's process block: the file view and the boot baseline take it (so it never
    /// counts as a pending restart), the process fields become read-only and launch flags do not apply.
    pub(crate) fn overlay_process(&mut self, p: &ProcessSettings) {
        self.file_settings.process = p.clone();
        self.boot_file_settings.process = p.clone();
        self.cli_override_paths.clear();
        self.process_overlaid = true;
    }

    /// The paths a response lists as read-only: the table's, plus the process block for an added slot.
    pub(super) fn read_only_list(&self) -> Vec<&'static str> {
        let mut paths = fields::read_only_paths();
        if self.process_overlaid {
            let extra = fields::rows_of(Block::Process).map(|f| f.field_path);
            paths.extend(extra.filter(|p| !paths.contains(p)).collect::<Vec<_>>());
        }
        paths
    }

    /// Removes the process fields from a request's `config`, so the merge keeps the overlaid values.
    pub(super) fn drop_process_fields(&self, config_in: &mut Value) {
        if !self.process_overlaid {
            return;
        }
        for field in fields::rows_of(Block::Process) {
            remove_path(config_in, field.field_path);
        }
    }

    /// A slot that waits for a network has no file yet; the page must join or create one first.
    pub(super) fn refuse_slot_without_network(&self) -> Option<Response> {
        if !self.process_overlaid || self.current_network_key.is_some() {
            return None;
        }
        Some(json_err(
            StatusCode::CONFLICT,
            "busy",
            "join or create a network in this tab first",
        ))
    }
}

fn remove_path(doc: &mut Value, path: &str) {
    let mut cur = doc;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            if let Some(obj) = cur.as_object_mut() {
                obj.remove(part);
            }
            return;
        }
        match cur.get_mut(part) {
            Some(next) => cur = next,
            None => return,
        }
    }
}

#[cfg(test)]
#[path = "slot_overlay_tests.rs"]
mod tests;
