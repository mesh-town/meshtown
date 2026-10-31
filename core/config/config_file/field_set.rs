//! "Path = value" for one settings field: the core of `--set` and of writing the settings file from the console.
//! A value is accepted by the same reader that reads the file, so the bounds match the file's.

use serde_json::Value;

use super::fields::{field_by_path, Field, Mode};
use super::settings::Settings;

/// A checked assignment to one stored field. The path is the table's string, not the typed one.
#[derive(Clone, Debug)]
pub struct FieldSet {
    field_path: &'static str,
    value: Value,
}

/// Why a path and value were not accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetRefusal {
    /// No such field (the typed text).
    Unknown(String),
    /// The field is shown, not changed (page port, computed values).
    ReadOnly(&'static str),
    /// The network key is not set this way.
    NetworkKey,
    /// The value does not fit the field; `expected` is what the field accepts.
    BadValue {
        path: &'static str,
        expected: String,
    },
}

impl FieldSet {
    /// Checks the path against the field table, then its mode, then the value.
    pub fn check(path: &str, value: Value) -> Result<FieldSet, SetRefusal> {
        let field = field_by_path(path).ok_or_else(|| SetRefusal::Unknown(path.to_string()))?;
        refuse_by_path(field)?;
        accepts(field, &value)?;
        Ok(FieldSet {
            field_path: field.field_path,
            value,
        })
    }

    /// Writes the value into `s`. The value was checked; a path missing from the table changes nothing.
    pub fn apply_to(&self, s: &mut Settings) {
        let found = field_by_path(self.field_path);
        debug_assert!(found.is_some(), "unknown path {}", self.field_path);
        if let Some(field) = found {
            read_into(field, &self.value, s);
        }
    }

    /// The table's path of the field.
    pub fn path(&self) -> &'static str {
        self.field_path
    }

    /// The checked value, as typed.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Whether the field lives only in the file (a running node cannot take it).
    pub fn is_file_only(&self) -> bool {
        field_by_path(self.field_path).is_some_and(|f| f.mode == Mode::FileOnly)
    }
}

fn refuse_by_path(field: &Field) -> Result<(), SetRefusal> {
    match (field.field_path, field.mode) {
        ("network.key", _) => Err(SetRefusal::NetworkKey),
        (path, Mode::ReadOnly) => Err(SetRefusal::ReadOnly(path)),
        _ => Ok(()),
    }
}

/// Reads `value` into `s` the way the file reader does; returns whether the reader found no fault.
fn read_into(field: &Field, value: &Value, s: &mut Settings) -> bool {
    let mut errs = Vec::new();
    field
        .value
        .read_from_file(field.field_path, Some(value), s, &mut errs);
    errs.is_empty()
}

fn accepts(field: &Field, value: &Value) -> Result<(), SetRefusal> {
    match read_into(field, value, &mut Settings::default()) {
        true => Ok(()),
        false => Err(SetRefusal::BadValue {
            path: field.field_path,
            expected: field.value.expected(),
        }),
    }
}

#[cfg(test)]
#[path = "field_set_tests.rs"]
mod tests;
