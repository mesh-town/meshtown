//! Parsing the file text into `ParsedConfigFile` (docs/CONFIG-FILE.md section 8: a damaged field gets the default, not a rejection) and reading
//! the file from disk (`load_config_file`). Checking individual fields is in `parse_fields.rs`.

use crate::transport::net::NodeSecret;
use std::path::Path;

use serde_json::Value;

use super::fields;
use super::parse_fields::{build_settings, push_field_error};
use super::report::FieldError;
use super::settings::Settings;
use super::CONFIG_VERSION;

/// What is up with the node secret in the file envelope.
#[derive(Clone, Debug)]
pub enum SecretRead {
    /// There is no secret in the file.
    Missing,
    /// The secret parsed: base64, exactly 32 bytes.
    Valid(NodeSecret),
    /// The secret is damaged; holds the reason as text (without the value itself).
    Bad(String),
}

/// The parsed file.
#[derive(Clone, Debug)]
pub struct ParsedConfigFile {
    pub rev: u64,
    pub secret: SecretRead,
    pub settings: Settings,
    pub field_errors: Vec<FieldError>,
    /// Paths of unknown fields, for one log line.
    pub unknown_paths: Vec<String>,
    /// Paths of retired fields (`RETIRED`): accepted, ignored and dropped on the next write.
    pub retired_paths: Vec<String>,
}

/// Fields that no longer exist: found in a file, they are ignored and reported apart from the unknown ones.
/// Paths are `section.key`, without the `config.` prefix.
const RETIRED: &[&str] = &["transport.mode"];

/// What is on disk. One value per launch and short-lived, so the large `ParsedConfigFile` is not boxed.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum LoadedConfigFile {
    Absent,
    /// Not readable; the reason as text.
    Unreadable(String),
    /// Did not parse as a document; the reason as text.
    Invalid(String),
    Parsed(ParsedConfigFile),
}

/// The node secret from the envelope: `identity.secret`.
fn read_secret(root: &serde_json::Map<String, Value>, errs: &mut Vec<FieldError>) -> SecretRead {
    let Some(identity) = root.get("identity") else {
        return SecretRead::Missing;
    };
    let why = match identity.as_object() {
        None => "the identity section is not an object".to_string(),
        Some(obj) => match obj.get("secret") {
            None => return SecretRead::Missing,
            Some(Value::String(s)) => match data_encoding::BASE64.decode(s.as_bytes()) {
                Err(_) => "not base64".to_string(),
                Ok(bytes) => match <[u8; 32]>::try_from(bytes.as_slice()) {
                    Ok(arr) => return SecretRead::Valid(NodeSecret::from_bytes(&arr)),
                    Err(_) => format!("{} bytes instead of 32", bytes.len()),
                },
            },
            Some(_) => "not a string".to_string(),
        },
    };
    errs.push(FieldError {
        field_path: "identity.secret".to_string(),
        message: format!(
            "identity secret is corrupted ({why}); this computer has a new identity, other members will see it as a new computer"
        ),
    });
    SecretRead::Bad(why)
}

/// The `config` section: an object, otherwise `Err`.
pub(super) fn config_section(root_obj: &serde_json::Map<String, Value>) -> Result<&Value, String> {
    root_obj
        .get("config")
        .filter(|c| c.is_object())
        .ok_or_else(|| "no config section".to_string())
}

/// `config.version`: present, not newer than this program, not zero.
fn check_version(cfg: &Value) -> Result<u64, String> {
    let version = cfg
        .get("version")
        .and_then(Value::as_u64)
        .ok_or_else(|| "no config.version".to_string())?;
    if version > CONFIG_VERSION {
        return Err(format!(
            "config.version {version} is newer than this program ({CONFIG_VERSION})"
        ));
    }
    if version == 0 {
        return Err("config.version 0 is not allowed".to_string());
    }
    Ok(version)
}

/// `rev`: absent — 0 with no error (like any missing field, docs/CONFIG-FILE.md section 8); wrong type — 0 and an error.
fn read_rev(root_obj: &serde_json::Map<String, Value>, errs: &mut Vec<FieldError>) -> u64 {
    match root_obj.get("rev") {
        None => 0,
        Some(v) => match v.as_u64() {
            Some(r) => r,
            None => {
                push_field_error(errs, "rev".to_string(), v, "non-negative integer", 0);
                0
            }
        },
    }
}

/// The `config.<section>` sections, if present, must be objects.
fn check_sections_are_objects(cfg: &Value, errs: &mut Vec<FieldError>) {
    for sec in fields::section_names() {
        if let Some(v) = cfg.get(sec) {
            if !v.is_object() {
                push_field_error(errs, sec.to_string(), v, "object", "section defaults");
            }
        }
    }
}

/// Unknown and retired fields: the root, `config`, the sections. Computed keys are part of `known_keys`.
/// Returns `(unknown, retired)`.
fn collect_unknown(
    root_obj: &serde_json::Map<String, Value>,
    cfg: &Value,
) -> (Vec<String>, Vec<String>) {
    let mut unknown = unknown_root_keys(root_obj);
    let (unknown_in_config, retired) = unknown_config_paths(cfg);
    unknown.extend(unknown_in_config);
    (unknown, retired)
}

/// Keys of the root object that are not part of the file envelope.
fn unknown_root_keys(root_obj: &serde_json::Map<String, Value>) -> Vec<String> {
    root_obj
        .keys()
        .filter(|k| !["rev", "saved_at", "identity", "config"].contains(&k.as_str()))
        .cloned()
        .collect()
}

/// Paths (`config.<key>` and `config.<section>.<key>`) of the unknown and of the retired fields inside `config`.
fn unknown_config_paths(cfg: &Value) -> (Vec<String>, Vec<String>) {
    let mut unknown: Vec<String> = Vec::new();
    let mut retired: Vec<String> = Vec::new();
    let Some(cfg_obj) = cfg.as_object() else {
        return (unknown, retired);
    };
    for (k, v) in cfg_obj {
        if k == "version" {
            continue;
        }
        if !fields::section_names().contains(&k.as_str()) {
            split_retired_section(k, v, &mut unknown, &mut retired);
            continue;
        }
        if let Some(sec_obj) = v.as_object() {
            for sk in sec_obj.keys() {
                if !fields::known_keys(k).contains(&sk.as_str()) {
                    unknown.push(format!("config.{k}.{sk}"));
                }
            }
        }
    }
    (unknown, retired)
}

/// A section the table does not know: when it holds a retired key, its keys are listed one by one
/// (retired apart from unknown); otherwise the whole section is one unknown path.
fn split_retired_section(
    section: &str,
    value: &Value,
    unknown: &mut Vec<String>,
    retired: &mut Vec<String>,
) {
    let is_retired = |key: &String| RETIRED.contains(&format!("{section}.{key}").as_str());
    let keys: Vec<&String> = value.as_object().map_or(vec![], |o| o.keys().collect());
    if !keys.iter().any(|k| is_retired(k)) {
        unknown.push(format!("config.{section}"));
        return;
    }
    for key in keys {
        let path = format!("config.{section}.{key}");
        if is_retired(key) {
            retired.push(path);
        } else {
            unknown.push(path);
        }
    }
}

/// Parses the file text. `Err` is the `invalid` state (the reason text is in English).
/// A damaged field does not void the file: it gets the default and a line in `field_errors`.
pub fn parse_text(text: &str) -> Result<ParsedConfigFile, String> {
    // Windows Notepad and some editors save UTF-8 with a BOM; `serde_json` does not skip it.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let root: Value = serde_json::from_str(text).map_err(|e| {
        format!(
            "Couldn't parse JSON: error at line {}, column {}",
            e.line(),
            e.column()
        )
    })?;
    let root_obj = root
        .as_object()
        .ok_or_else(|| "expected a JSON object".to_string())?;
    let cfg = config_section(root_obj)?;
    check_version(cfg)?;

    let mut errs: Vec<FieldError> = Vec::new();
    let rev = read_rev(root_obj, &mut errs);
    let secret = read_secret(root_obj, &mut errs);
    check_sections_are_objects(cfg, &mut errs);
    let settings = build_settings(cfg, &mut errs);
    let (unknown_paths, retired_paths) = collect_unknown(root_obj, cfg);

    Ok(ParsedConfigFile {
        rev,
        secret,
        settings,
        field_errors: errs,
        unknown_paths,
        retired_paths,
    })
}

/// Reads and parses the file: no file — `Absent`, unreadable — `Unreadable`, not a document —
/// `Invalid`, otherwise `Parsed`.
pub fn load_config_file(path: &Path) -> LoadedConfigFile {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return LoadedConfigFile::Absent,
        Err(e) => return LoadedConfigFile::Unreadable(format!("not readable: {e}")),
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return LoadedConfigFile::Invalid("not UTF-8".to_string());
    };
    match parse_text(&text) {
        Ok(p) => LoadedConfigFile::Parsed(p),
        Err(m) => LoadedConfigFile::Invalid(m),
    }
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "parse_json_error_tests.rs"]
mod json_error_tests;

#[cfg(test)]
#[path = "parse_retired_tests.rs"]
mod retired_tests;

#[cfg(test)]
#[path = "parse_golden_tests.rs"]
mod golden_tests;

#[cfg(test)]
#[path = "parse_golden_messages_tests.rs"]
mod golden_messages_tests;

#[cfg(test)]
#[path = "parse_named_tests.rs"]
mod named_tests;

#[cfg(test)]
#[path = "parse_net_tests.rs"]
mod net_tests;
