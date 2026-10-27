//! Steps 9–10 of the `put()` dispatcher (`put.rs`): writing the final settings to disk and answering by the
//! result; also `stale_outcome` — the outcome of re-reading the file after `SaveError::StaleRev`.

use axum::http::StatusCode;
use axum::response::Response;

use crate::config::config_file::{
    fields, FileState, LoadedSettings, NetSlotId, SaveError, Settings, SettingsFile,
};
use crate::web_gui::NodeControl;

use super::settings_access::write_loaded;
use super::state::ConfigState;
use super::support::{get_path, json_response};

/// Which slot should own the proxy's exit after a successful write.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ProxyPick {
    /// The slot that was written: it got a non-empty `route.exits`.
    ThisSlot,
    /// The slot named by the changed `proxy.net`.
    Named(NetSlotId),
}

/// The switch a successful write asks for: new exits win over a changed `proxy.net`.
fn proxy_pick(new: &Settings, changed: &[&'static str]) -> Option<ProxyPick> {
    if changed.contains(&"route.exits") && !new.net.route_exits.is_empty() {
        return Some(ProxyPick::ThisSlot);
    }
    changed
        .contains(&"proxy.net")
        .then(|| ProxyPick::Named(new.process.proxy_net.clone()))
}

/// Steps 9–10: fills in the unwritten (read-only) fields from the current file, computes the list of
/// changed editable fields, writes the file — the write method depends on what was
/// loaded at step 5 (`loaded`) — and answers by the result.
pub(super) fn write_new_settings(
    state: &mut ConfigState,
    mut new: Settings,
    loaded: LoadedSettings,
    file: &dyn SettingsFile,
    (rev_in, reapply_opened): (u64, bool),
    node: &dyn NodeControl,
) -> (Response, Option<ProxyPick>) {
    new.net.network_key = state.file_settings.net.network_key.clone();
    new.process.web_enabled = state.file_settings.process.web_enabled;
    new.process.web_port = state.file_settings.process.web_port;

    let old_doc = state.config_doc_of(&state.file_settings);
    let new_doc = state.config_doc_of(&new);
    let changed: Vec<&'static str> = fields::editable_paths()
        .into_iter()
        .filter(|&f| get_path(&new_doc, f) != get_path(&old_doc, f))
        .collect();

    match perform_write(state, file, &loaded, rev_in, &new) {
        Ok(new_rev) => {
            finish_successful_write(state, node, new, new_rev, (changed, reapply_opened))
        }
        Err(SaveError::StaleRev(_)) => (respond_to_stale_write(state, file), None),
        Err(SaveError::Io(t)) => (state.config_write_failed(&t), None),
    }
}

/// The write itself; the method depends on what was on disk (see `load_settings_file`).
fn perform_write(
    state: &ConfigState,
    file: &dyn SettingsFile,
    loaded: &LoadedSettings,
    rev_in: u64,
    new: &Settings,
) -> Result<u64, SaveError> {
    let rev_floor = state.file_report.rev;
    write_loaded(file, loaded, Some(rev_in), rev_floor, &state.secret, new)
}

fn finish_successful_write(
    state: &mut ConfigState,
    node: &dyn NodeControl,
    new: Settings,
    new_rev: u64,
    (changed, reapply_opened): (Vec<&'static str>, bool),
) -> (Response, Option<ProxyPick>) {
    let pick = proxy_pick(&new, &changed);
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
        .chain((reapply_opened && !changed.contains(&"ports.opened")).then_some("ports.opened"))
        .filter(|&f| !restart_paths.contains(&f) && !state.cli_override_paths.contains(&f))
        .collect();
    if !apply_now.is_empty() {
        node.apply_config(&state.file_settings, &apply_now);
    }

    let resp = json_response(
        StatusCode::OK,
        serde_json::json!({
            "rev": new_rev,
            "config": state.page_config_document(),
            "read_only": state.read_only_list(),
            "restart_fields": fields::restart_paths(),
            "restart_pending": state.pending_restart_paths(),
            "changed": changed,
            "cli_overrides": state.cli_override_paths,
            "file": state.file_block(),
        }),
    );
    (resp, pick)
}

fn respond_to_stale_write(state: &mut ConfigState, file: &dyn SettingsFile) -> Response {
    match stale_outcome(file.load(&state.file_settings)) {
        StaleOutcome::Conflict { rev, settings } => {
            state.file_settings = settings;
            state.file_report.rev = rev;
            state.stale_rev_response(rev)
        }
        StaleOutcome::Failed => state
            .config_write_failed("the settings file changed during the write and can't be read"),
    }
}

/// The outcome of `stale_outcome(file.load(..))` after `SaveError::StaleRev` — a separate pure
/// function of `LoadedSettings`, so a unit test can check it without writing to disk.
pub(super) enum StaleOutcome {
    Conflict { rev: u64, settings: Settings },
    Failed,
}

pub(super) fn stale_outcome(loaded: LoadedSettings) -> StaleOutcome {
    match loaded {
        LoadedSettings::Parsed { rev, settings } => StaleOutcome::Conflict { rev, settings },
        _ => StaleOutcome::Failed,
    }
}
