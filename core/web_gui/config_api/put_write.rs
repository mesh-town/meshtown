//! Steps 9–10 of the `put()` dispatcher (`put.rs`): writing the final settings to disk and answering by the
//! result; also `stale_outcome` — the outcome of re-reading the file after `SaveError::StaleRev`.

use std::path::Path;

use axum::http::StatusCode;
use axum::response::Response;

use crate::config::config_file::{
    self, fields, FileState, LoadedConfigFile, ParsedConfigFile, SaveError, Settings,
};
use crate::web_gui::{now_unix, NodeControl};

use super::state::ConfigState;
use super::support::{get_path, json_response};

/// Steps 9–10: fills in the unwritten (read-only) fields from the current file, computes the list of
/// changed editable fields, writes the file — the write method depends on what was
/// loaded at step 5 (`loaded`) — and answers by the result.
pub(super) fn write_new_settings(
    state: &mut ConfigState,
    mut new: Settings,
    loaded: LoadedConfigFile,
    path: &Path,
    rev_in: u64,
    node: &dyn NodeControl,
) -> Response {
    new.net.network_key = state.file_settings.net.network_key.clone();
    new.process.web_enabled = state.file_settings.process.web_enabled;
    new.process.web_port = state.file_settings.process.web_port;

    let old_doc = state.config_doc_of(&state.file_settings);
    let new_doc = state.config_doc_of(&new);
    let changed: Vec<&'static str> = fields::editable_paths()
        .into_iter()
        .filter(|&f| get_path(&new_doc, f) != get_path(&old_doc, f))
        .collect();

    match perform_write(state, path, loaded, rev_in, &new) {
        Ok(new_rev) => finish_successful_write(state, node, new, new_rev, changed),
        Err(SaveError::StaleRev(_)) => respond_to_stale_write(state, path),
        Err(SaveError::Io(t)) => state.config_write_failed(&t),
    }
}

/// The write itself; the method depends on what was on disk (see `load_file_or_reject_stale_rev`).
fn perform_write(
    state: &ConfigState,
    path: &Path,
    loaded: LoadedConfigFile,
    rev_in: u64,
    new: &Settings,
) -> Result<u64, SaveError> {
    match loaded {
        LoadedConfigFile::Parsed(_) => {
            config_file::save_config_file(path, Some(rev_in), now_unix(), &state.secret, new)
        }
        LoadedConfigFile::Absent => config_file::save_with_floor(
            path,
            None,
            state.file_report.rev,
            now_unix(),
            &state.secret,
            new,
        ),
        LoadedConfigFile::Unreadable(_) | LoadedConfigFile::Invalid(_) => {
            match config_file::set_aside_bad(path, now_unix()) {
                Err(t) => Err(SaveError::Io(t)),
                Ok(dest) => {
                    crate::node::output::say(
                        "web",
                        format!(
                            "page: broken settings file moved aside to {}",
                            dest.display()
                        ),
                    );
                    config_file::save_with_floor(
                        path,
                        None,
                        state.file_report.rev,
                        now_unix(),
                        &state.secret,
                        new,
                    )
                }
            }
        }
    }
}

fn finish_successful_write(
    state: &mut ConfigState,
    node: &dyn NodeControl,
    new: Settings,
    new_rev: u64,
    changed: Vec<&'static str>,
) -> Response {
    state.file_settings = new;
    state.file_report.state = FileState::Ok;
    state.file_report.message = None;
    state.file_report.field_errors.clear();
    state.file_report.rev = new_rev;
    state.file_report.persistent_identity = true;
    state.file_report.overwrite_refused = false;

    let restart_paths = fields::restart_paths();
    let apply_now: Vec<&'static str> = changed
        .iter()
        .copied()
        .filter(|&f| !restart_paths.contains(&f) && !state.cli_override_paths.contains(&f))
        .collect();
    if !apply_now.is_empty() {
        node.apply_config(&state.file_settings, &apply_now);
    }

    json_response(
        StatusCode::OK,
        serde_json::json!({
            "rev": new_rev,
            "config": state.page_config_document(),
            "read_only": fields::read_only_paths(),
            "restart_fields": fields::restart_paths(),
            "restart_pending": state.pending_restart_paths(),
            "changed": changed,
            "cli_overrides": state.cli_override_paths,
            "file": state.file_block(),
        }),
    )
}

fn respond_to_stale_write(state: &mut ConfigState, path: &Path) -> Response {
    match stale_outcome(config_file::load_config_file(path)) {
        StaleOutcome::Conflict(p) => {
            let new_rev = p.rev;
            state.file_settings = p.settings.clone();
            state.file_report.rev = new_rev;
            state.stale_rev_response(new_rev)
        }
        StaleOutcome::Failed => state
            .config_write_failed("the settings file changed during the write and can't be read"),
    }
}

/// The outcome of `stale_outcome(config_file::load_config_file(path))` after `SaveError::StaleRev` — a separate pure
/// function of `LoadedConfigFile`, so a unit test can check it without writing to disk.
pub(super) enum StaleOutcome {
    Conflict(ParsedConfigFile),
    Failed,
}

pub(super) fn stale_outcome(loaded: LoadedConfigFile) -> StaleOutcome {
    match loaded {
        LoadedConfigFile::Parsed(p) => StaleOutcome::Conflict(p),
        _ => StaleOutcome::Failed,
    }
}
