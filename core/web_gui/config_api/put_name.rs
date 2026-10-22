//! Normalization of `node.name` at step 6 of `merge_with_config` (`put_merge.rs`): `null` — as before (the field
//! is removed, then the value from the file is substituted as read-only); a string empty after `trim`
//! gets a random pool name. A failure of the system generator leaves the string as is — then
//! `trim_and_validate_node_name` in `config_file::parse_text` rejects it (422).

use serde_json::Value;

use crate::members::words::random_node_name;

/// `node.name` in the merged document: `null` is removed (as it was before); a string empty after `trim`
/// is replaced with [`random_node_name`]; a non-empty string and a generator failure are left alone.
pub(super) fn normalize_node_name(merged: &mut Value) {
    let Some(node_sec) = merged.get_mut("node").and_then(Value::as_object_mut) else {
        return;
    };
    if node_sec.get("name").map(Value::is_null).unwrap_or(false) {
        node_sec.remove("name");
        return;
    }
    let is_blank = node_sec
        .get("name")
        .and_then(Value::as_str)
        .map(|s| s.trim().is_empty())
        .unwrap_or(false);
    if !is_blank {
        return;
    }
    if let Ok(name) = random_node_name() {
        node_sec.insert("name".to_string(), Value::String(name));
    }
}
