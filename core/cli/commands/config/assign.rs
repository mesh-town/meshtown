//! `path=value` text of `--set` and `config set`: the split, the value as a JSON literal or a string, and the way back.

use serde_json::Value;

use crate::config::config_file::{FieldSet, SetRefusal};

/// Splits `path=value` at the first `=` and checks it against the field table.
pub(crate) fn parse_assignment(text: &str) -> Result<FieldSet, SetRefusal> {
    match text.split_once('=') {
        Some((path, raw)) if !path.is_empty() => FieldSet::check(path, value_of(raw)),
        _ => Err(SetRefusal::Unknown(text.to_string())),
    }
}

/// A JSON literal (`20`, `true`, `null`, `[1]`, `"text"`) as it is; anything else is a string.
pub(crate) fn value_of(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
}

/// The inverse of `value_of`: a string that does not read as JSON is printed as it is, everything else as JSON.
// used by `config get`
#[allow(dead_code)]
pub(crate) fn shown(v: &Value) -> String {
    match v {
        Value::String(s) if serde_json::from_str::<Value>(s).is_err() => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
#[path = "assign_tests.rs"]
mod tests;
