//! Reading the file's fields into `Settings` by the table (`build_settings`), a key's value
//! (`file_value_at`) and recording a field error "file has …, expected …; using default …" (`push_field_error`).

use serde_json::Value;

use super::fields;
use super::report::FieldError;
use super::settings::Settings;

/// The value of the field `config.<section>.<key>`; no section, a non-object section or no key gives `None`.
pub(super) fn file_value_at<'v>(cfg: &'v Value, section: &str, key: &str) -> Option<&'v Value> {
    cfg.get(section)?.get(key)
}

/// What was in the file, for the message. The values of `network.key` and `identity.secret` are not shown,
/// neither whole nor truncated — only the type.
fn file_value_for_message(field_path: &str, v: &Value) -> String {
    match field_path {
        _ if field_path == "identity.secret"
            || fields::field_by_path(field_path).is_some_and(|f| f.redacted) =>
        {
            match v {
                Value::String(_) => "empty string".to_string(),
                _ => "not a string".to_string(),
            }
        }
        "network" | "identity" => "not an object".to_string(),
        // Slicing by characters: `&s[..40]` would `panic` in the middle of a multi-byte character such as CJK.
        _ => v.to_string().chars().take(40).collect::<String>(),
    }
}

/// Records a `FieldError` of the form "file has …, expected …; using default …".
pub(super) fn push_field_error(
    errs: &mut Vec<FieldError>,
    field_path: String,
    file_value: &Value,
    expected: &str,
    default: impl std::fmt::Display,
) {
    let message = format!(
        "file has {}, expected {expected}; using default {default}",
        file_value_for_message(&field_path, file_value)
    );
    errs.push(FieldError {
        field_path,
        message,
    });
}

/// All `Settings` fields from the file: defaults, then by the table rows. The error order is the table
/// order (the file output order).
pub(super) fn build_settings(cfg: &Value, errs: &mut Vec<FieldError>) -> Settings {
    let mut s = Settings::default();
    for f in fields::FIELDS {
        f.value.read_from_file(
            f.field_path,
            file_value_at(cfg, f.section(), f.key()),
            &mut s,
            errs,
        );
    }
    s
}
