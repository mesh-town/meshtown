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
use crate::web_gui::http::{json_err, ConnInfo};
use crate::web_gui::NodeControl;

use super::put_merge::{merge_with_config, parse_merged_settings, reject_unavailable_fields};
use super::put_write::{write_new_settings, ProxyPick};
use super::state::ConfigState;
use super::support::field_entry;

impl ConfigState {
    /// A write without a connection (self-lockout is not checked): needed by tests.
    #[cfg(test)]
    pub(crate) fn put(&mut self, body: &[u8], node: &dyn NodeControl) -> Response {
        self.put_keeping(body, node, None).0
    }

    /// `put` with self-lockout protection: `conn` is the connection the request came through.
    pub(crate) fn put_keeping(
        &mut self,
        body: &[u8],
        node: &dyn NodeControl,
        conn: Option<ConnInfo>,
    ) -> (Response, Option<ProxyPick>) {
        match self.put_checked(body, node, conn) {
            Ok(done) => done,
            Err(resp) => (resp, None),
        }
    }

    /// Steps 1–10; `Err` is a refusal before anything was written.
    fn put_checked(
        &mut self,
        body: &[u8],
        node: &dyn NodeControl,
        conn: Option<ConnInfo>,
    ) -> Result<(Response, Option<ProxyPick>), Response> {
        if let Some(resp) = self.refuse_slot_without_network() {
            return Err(resp);
        }
        let (rev_in, mut config_in) = parse_put_request(body)?;
        self.drop_process_fields(&mut config_in);
        let file = writable_file(self)?;
        let loaded = load_file_or_reject_stale_rev(self, file.as_ref(), rev_in)?;
        let merged = merge_with_config(self, &config_in);
        reject_unavailable_fields(&merged)?;
        let new = parse_merged_settings(self, &merged)?;
        reject_web_lockout(self, &new, conn)?;
        Ok(write_new_settings(self, new, loaded, file.as_ref(), rev_in, node))
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

/// Step 5: the file revision on disk matches `rev_in` from the request; otherwise 409 and syncing
/// the state with what is actually in the file. Returns the loaded file — it is also needed at step 9 to
/// choose the write method.
fn load_file_or_reject_stale_rev(
    state: &mut ConfigState,
    file: &dyn SettingsFile,
    rev_in: u64,
) -> Result<LoadedSettings, Response> {
    let loaded = file.load(&state.file_settings);
    if let LoadedSettings::Parsed { rev, settings } = &loaded {
        if *rev != rev_in {
            let new_rev = *rev;
            state.file_settings = settings.clone();
            state.file_report.rev = new_rev;
            return Err(state.stale_rev_response(new_rev));
        }
    }
    Ok(loaded)
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

#[cfg(test)]
#[path = "put_tests.rs"]
mod tests;
