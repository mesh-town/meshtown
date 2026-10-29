//! `ConfigState::put` — the dispatcher of `PUT /api/config`: calls, step by step, functions of this file (request
//! parsing, path lookup, revision check) and functions of neighboring files — merge and checks (steps
//! 6–8, `put_merge.rs`) and writing to disk (steps 9–10, `put_write.rs`, also `stale_outcome`).

use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::Response;
use serde_json::Value;

use crate::config::config_file::{ListenAccess, LoadedSettings, Settings, SettingsFile};
use crate::web_gui::auth::iface_gate::judge;
use crate::web_gui::auth::iface_os::current_view;
use crate::web_gui::host_control::OpenedLocal;
use crate::web_gui::http::{json_err, ConnInfo};
use crate::web_gui::NodeControl;

use super::autostart::apply_autostart;
use super::put_merge::{merge_with_config, parse_merged_settings, reject_unavailable_fields};
use super::put_write::{write_new_settings, ProxyPick};
use super::state::ConfigState;
use super::support::{field_entry, get_path};

impl ConfigState {
    /// A write without a connection (self-lockout is not checked): needed by tests.
    #[cfg(test)]
    pub(crate) fn put(&mut self, body: &[u8], node: &dyn NodeControl) -> Response {
        self.put_keeping(body, node, None, &[]).0
    }

    /// `put` with self-lockout protection: `conn` is the connection the request came through.
    pub(crate) fn put_keeping(
        &mut self,
        body: &[u8],
        node: &dyn NodeControl,
        conn: Option<ConnInfo>,
        taken: &[OpenedLocal],
    ) -> (Response, Option<ProxyPick>) {
        match self.put_checked(body, node, conn, taken) {
            Ok(done) => done,
            Err(resp) => (resp, None),
        }
    }

    /// Steps 1–11; `Err` is a refusal before anything was written.
    fn put_checked(
        &mut self,
        body: &[u8],
        node: &dyn NodeControl,
        conn: Option<ConnInfo>,
        taken: &[OpenedLocal],
    ) -> Result<(Response, Option<ProxyPick>), Response> {
        if let Some(resp) = self.refuse_slot_without_network() {
            return Err(resp);
        }
        let (rev_in, mut config_in) = parse_put_request(body)?;
        self.drop_process_fields(&mut config_in);
        let file = writable_file(self)?;
        let loaded = load_settings_file(self, file.as_ref());
        reject_stale_rev(self, rev_in, &loaded)?;
        let merged = merge_with_config(self, &config_in);
        reject_unavailable_fields(self, &merged)?;
        let new = parse_merged_settings(self, &merged)?;
        reject_web_lockout(self, &new, conn)?;
        reject_taken_locals(self, &new, taken)?;
        apply_autostart(self, &merged)?;
        // An opened list in the body is re-applied even if unchanged, so failed binds are retried.
        let reapply_opened = get_path(&config_in, "ports.opened").is_some();
        Ok(write_new_settings(
            self,
            new,
            loaded,
            file.as_ref(),
            (rev_in, reapply_opened),
            node,
        ))
    }
}

/// Steps 1–2: the body is JSON, an object with fields `rev` (an integer) and `config` (an object).
fn parse_put_request(body: &[u8]) -> Result<(u64, Value), Response> {
    let raw: Value = serde_json::from_slice(body).map_err(|_| {
        json_err(
            StatusCode::BAD_REQUEST,
            "bad_json",
            "the request body couldn't be parsed as JSON",
        )
    })?;
    let bad_request = || {
        json_err(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "fields rev (an integer) and config (an object) are required",
        )
    };
    let obj = raw.as_object().ok_or_else(bad_request)?;
    let rev_in = obj
        .get("rev")
        .and_then(Value::as_u64)
        .ok_or_else(bad_request)?;
    let config_in = obj
        .get("config")
        .filter(|c| c.is_object())
        .ok_or_else(bad_request)?
        .clone();
    Ok((rev_in, config_in))
}

/// Steps 3–4: the file is in use and can be overwritten without loss.
fn writable_file(state: &ConfigState) -> Result<Arc<dyn SettingsFile>, Response> {
    let Some(file) = state.file.clone() else {
        return Err(state.config_write_failed("the settings file is not used (--no-config)"));
    };
    if state.file_report.overwrite_refused {
        let msg = state.file_report.message.clone().unwrap_or_else(|| {
            "the settings file was left untouched: overwriting would lose data".to_string()
        });
        return Err(state.config_write_failed(&msg));
    }
    Ok(file)
}

/// Step 5a: loads the settings file; the result is also needed at step 9 to choose the write method.
fn load_settings_file(state: &ConfigState, file: &dyn SettingsFile) -> LoadedSettings {
    file.load(&state.file_settings)
}

/// Step 5b: the file revision on disk matches `rev_in` from the request; otherwise 409 and syncing
/// the state with what is actually in the file.
fn reject_stale_rev(
    state: &mut ConfigState,
    rev_in: u64,
    loaded: &LoadedSettings,
) -> Result<(), Response> {
    let LoadedSettings::Parsed { rev, settings } = loaded else {
        return Ok(());
    };
    if *rev == rev_in {
        return Ok(());
    }
    let new_rev = *rev;
    state.file_settings = settings.clone();
    state.file_report.rev = new_rev;
    Err(state.stale_rev_response(new_rev))
}

/// Step 8a: the new `web.ifaces` must not close the page for the current connection (loopback, own
/// address and `conn = None` always pass): otherwise 422 with a `web.ifaces` field error, the file is not written.
fn reject_web_lockout(
    state: &ConfigState,
    new: &Settings,
    conn: Option<ConnInfo>,
) -> Result<(), Response> {
    let Some(conn) = conn else {
        return Ok(());
    };
    let filter = &new.process.web_ifaces;
    let view = current_view();
    let local = Some(conn.local.ip());
    let verdict = judge(
        ListenAccess::Localnet,
        filter,
        &view,
        conn.client_addr.ip(),
        local,
    );
    if verdict.allows() {
        return Ok(());
    }
    let name = verdict.iface.as_deref().unwrap_or("?");
    let text =
        format!("this list would close the page for you: the connection came through \"{name}\"");
    Err(state.invalid_response(vec![field_entry("web.ifaces", &text)]))
}

/// Step 8b: a `local` that is new in this list must not be opened in another network of this node (the old
/// list of this slot may keep what it already holds): otherwise 422 with a `ports.opened` field error.
fn reject_taken_locals(
    state: &ConfigState,
    new: &Settings,
    taken: &[OpenedLocal],
) -> Result<(), Response> {
    let old = &state.file_settings.net.ports_opened;
    let fresh = new
        .net
        .ports_opened
        .iter()
        .filter(|o| !old.iter().any(|p| (p.proto, p.local) == (o.proto, o.local)));
    for opened in fresh {
        if let Some(other) = taken
            .iter()
            .find(|t| (t.proto, t.local) == (opened.proto, opened.local))
        {
            let text = format!(
                "local {}port {} is already opened in network \"{}\"",
                opened.proto.pick("", "UDP "),
                other.local,
                other.net
            );
            return Err(state.invalid_response(vec![field_entry("ports.opened", &text)]));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "put_tests.rs"]
mod tests;
