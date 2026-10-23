//! What the log says about a slot file that is being started. Key values are never printed, only paths.

use std::path::{Path, PathBuf};

use super::slot_id::NetSlotId;
use crate::config::config_file::ParsedNetFile;
use crate::node::output::say;

/// Says what was found in the file: damaged fields, process keys, unknown and retired keys.
pub(super) fn log_file_findings(id: &NetSlotId, main: Option<&Path>, p: &ParsedNetFile) {
    for fe in &p.field_errors {
        say(
            "config",
            format!("net slot {id}: field {}: {}", fe.field_path, fe.message),
        );
    }
    if !p.process_paths.is_empty() {
        let home = main.map_or("the main settings file".to_string(), |m| {
            m.display().to_string()
        });
        say(
            "config",
            format!(
                "net slot {id}: process keys are ignored in a net slot file, they live in {home}: {}",
                p.process_paths.join(", ")
            ),
        );
    }
    if !p.unknown_paths.is_empty() {
        say(
            "config",
            format!(
                "net slot {id}: unknown fields skipped and will disappear on the next write: {}",
                p.unknown_paths.join(", ")
            ),
        );
    }
    for w in crate::config::config_file::retired_key_warnings(&p.retired_paths) {
        say("config", format!("net slot {id}: {w}"));
    }
}

/// Says what happened to the slot's identity: a copy kept, a failed write, a temporary identity.
pub(super) fn log_identity(id: &NetSlotId, copy: Option<&PathBuf>, problem: Option<&str>) {
    if let Some(name) = copy.and_then(|c| c.file_name()) {
        say(
            "config",
            format!(
                "net slot {id}: original file saved as {}",
                name.to_string_lossy()
            ),
        );
    }
    if let Some(why) = problem {
        say(
            "config",
            format!("net slot {id}: couldn't write the file: {why}"),
        );
    }
}
