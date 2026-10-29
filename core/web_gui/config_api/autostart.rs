//! Start at login on the page: the view read from the system and the change applied on `PUT`.

use serde_json::{Map, Value};

use super::put_merge::not_available_response;
use super::state::ConfigState;
use super::support::{field_entry, get_path, AUTOSTART_REASON};
use crate::autostart::{plan_change, AutostartBackend, Change};
use axum::response::Response;

/// What the page shows for `autostart`.
pub(super) struct AutostartView {
    pub enabled: bool,
    pub available: bool,
    pub reason: String,
}

/// The state of the login entry as the page sees it: the system is the source of truth.
pub(super) fn autostart_view(backend: Option<&dyn AutostartBackend>) -> AutostartView {
    let Some(backend) = backend else {
        return view(false, false, AUTOSTART_REASON.to_string());
    };
    match backend.state() {
        Ok(state) => view(state.is_on(), true, String::new()),
        Err(e) => view(false, false, format!("could not read the login entry: {e}")),
    }
}

fn view(enabled: bool, available: bool, reason: String) -> AutostartView {
    AutostartView {
        enabled,
        available,
        reason,
    }
}

/// Writes the view into the `autostart` section of the document.
pub(super) fn fill_autostart(section: &mut Map<String, Value>, view: &AutostartView) {
    section.insert("enabled".to_string(), Value::Bool(view.enabled));
    section.insert("available".to_string(), Value::Bool(view.available));
    section.insert("reason".to_string(), Value::String(view.reason.clone()));
}

/// Step 11: brings the login entry to the requested `autostart.enabled`.
/// An added slot never touches it; an unreadable entry is left alone when switching off.
pub(super) fn apply_autostart(state: &ConfigState, merged: &Value) -> Result<(), Response> {
    let Some(backend) = state.autostart.filter(|_| !state.process_overlaid) else {
        return Ok(());
    };
    let desired = get_path(merged, "autostart.enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let current = match backend.state() {
        Ok(current) => current,
        Err(e) if desired => return Err(refusal(&format!("could not read the login entry: {e}"))),
        Err(_) => return Ok(()),
    };
    let result = match plan_change(desired, current) {
        Some(Change::Enable) => backend
            .enable()
            .map_err(|e| format!("could not turn on start at login: {e}")),
        Some(Change::Disable) => backend
            .disable()
            .map_err(|e| format!("could not turn off start at login: {e}")),
        None => Ok(()),
    };
    result.map_err(|message| refusal(&message))
}

fn refusal(message: &str) -> Response {
    not_available_response(message, vec![field_entry("autostart.enabled", message)])
}
