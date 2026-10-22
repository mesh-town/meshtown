//! Steps 6–8 of the `put()` dispatcher (`put.rs`): merging the patch with the current document, checking fields
//! unavailable in this build, and parsing the final settings.

use axum::http::StatusCode;
use axum::response::Response;
use serde_json::Value;

use crate::config_file::{self, fields, ParsedConfigFile, Settings};

use super::state::ConfigState;
use super::support::{field_entry, get_path, json_response, AUTOSTART_REASON, FULL_TUNNEL_REASON};

/// Step 6: the request patch is laid over the current document; read-only sections (`network`, `web`,
/// the service fields `autostart`/`full_tunnel`) are returned to the values from the file — whatever the client
/// sent there; `node.name` is normalized separately (`put_name::normalize_node_name`):
/// `null` is removed, a string empty after `trim` becomes a random name.
pub(super) fn merge_with_config(state: &ConfigState, config_in: &Value) -> Value {
    let mut merged = state.page_config_document();
    merge_json(&mut merged, config_in);
    let Some(mobj) = merged.as_object_mut() else {
        return merged;
    };
    let network_value = match &state.file_settings.net.network_key {
        Some(k) => serde_json::json!({ "key": k }),
        None => serde_json::json!({}),
    };
    mobj.insert("network".to_string(), network_value);
    let access = merged_access(mobj, state);
    let ifaces = merged_web_ifaces(mobj, state);
    mobj.insert(
        "web".to_string(),
        serde_json::json!({
            "enabled": state.file_settings.process.web_enabled,
            "port": state.file_settings.process.web_port,
            "access": access,
            "ifaces": ifaces,
        }),
    );
    for section in ["autostart", "full_tunnel"] {
        if let Some(sec) = mobj.get_mut(section).and_then(Value::as_object_mut) {
            sec.remove("available");
            sec.remove("reason");
        }
    }
    super::put_name::normalize_node_name(&mut merged);
    merged
}

/// `web.access` from the merged document (patch over file); no value — from the file.
fn merged_access(mobj: &serde_json::Map<String, Value>, state: &ConfigState) -> Value {
    match mobj.get("web").and_then(|w| w.get("access")) {
        Some(v) => v.clone(),
        None => Value::from(state.file_settings.process.web_access.as_str()),
    }
}

/// `web.ifaces` from the merged document (patch over file); no value — from the file.
fn merged_web_ifaces(mobj: &serde_json::Map<String, Value>, state: &ConfigState) -> Value {
    match mobj.get("web").and_then(|w| w.get("ifaces")) {
        Some(v) => v.clone(),
        None => Value::from(state.file_settings.process.web_ifaces.parts().1),
    }
}

/// Step 7: `autostart.enabled`/`full_tunnel.enabled` are unavailable in this build.
pub(super) fn reject_unavailable_fields(merged: &Value) -> Result<(), Response> {
    let mut blocked: Vec<(&'static str, &'static str)> = Vec::new();
    if get_path(merged, "autostart.enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        blocked.push(("autostart.enabled", AUTOSTART_REASON));
    }
    if get_path(merged, "full_tunnel.enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        blocked.push(("full_tunnel.enabled", FULL_TUNNEL_REASON));
    }
    if blocked.is_empty() {
        return Ok(());
    }
    let message = blocked[0].1;
    let fields = blocked
        .iter()
        .map(|(p, _)| field_entry(p, "unavailable in this build"))
        .collect();
    Err(not_available_response(message, fields))
}

/// Step 8: the merged document is a valid `config_file` (version 1, no errors except `rev`/`identity`,
/// those are checked separately at step 5 and at node startup).
pub(super) fn parse_merged_settings(
    state: &ConfigState,
    merged: &Value,
) -> Result<Settings, Response> {
    let text = serde_json::json!({ "config": merged }).to_string();
    let p: ParsedConfigFile = match config_file::parse_text(&text) {
        Err(_) => {
            return Err(state.invalid_response(vec![field_entry(
                "version",
                "only version 1 is supported",
            )]));
        }
        Ok(p) => p,
    };
    let bad: Vec<&config_file::FieldError> = p
        .field_errors
        .iter()
        .filter(|e| e.field_path != "rev" && !e.field_path.starts_with("identity"))
        .collect();
    if !bad.is_empty() {
        let fields = bad
            .iter()
            .map(|e| field_entry(&e.field_path, &invalid_text(&e.field_path, &e.message)))
            .collect();
        return Err(state.invalid_response(fields));
    }
    Ok(p.settings)
}

/// Recursive merge: a key common to the base and the patch — if both are objects → merge inside, otherwise
/// replace the value with the patch as a whole.
fn merge_json(base: &mut Value, patch: &Value) {
    let (Value::Object(base_map), Value::Object(patch_map)) = (base, patch) else {
        return;
    };
    for (k, v) in patch_map {
        match base_map.get_mut(k) {
            Some(existing) if existing.is_object() && v.is_object() => merge_json(existing, v),
            _ => {
                base_map.insert(k.clone(), v.clone());
            }
        }
    }
}

/// The 422 text for a path: `version` and "not an object" have their own; an editable row gets the `expected()` of its kind
/// (the same text as in file parsing); the rest (file-only, read-only) gets the raw parse message.
fn invalid_text(path: &str, raw: &str) -> String {
    if path == "version" {
        return "only version 1 is supported".to_string();
    }
    if !path.contains('.') {
        return "expected an object".to_string();
    }
    match fields::field_by_path(path).filter(|f| f.mode.is_editable()) {
        Some(f) => f.value.expected(),
        None => raw.to_string(),
    }
}

fn not_available_response(message: &str, fields: Vec<Value>) -> Response {
    json_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        serde_json::json!({
            "error": "not_available",
            "message": message,
            "fields": fields,
        }),
    )
}
