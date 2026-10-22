//! Parsing the file text into `ParsedConfigFile` (CONFIG-FILE.md section 8: a damaged field gets the default, not a rejection) and reading
//! the file from disk (`load_config_file`). Checking individual fields is in `parse_fields.rs`.

use crate::net::NodeSecret;
use std::path::Path;

use serde_json::Value;

use super::parse_fields::{build_settings, push_field_error};
use super::fields;
use super::report::FieldError;
use super::retired;
use super::settings::Settings;
use super::CONFIG_VERSION;
use crate::output::NET_NAME;

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
    /// Retired keys, one log line each.
    pub retired_paths: Vec<String>,
}

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
            "node secret is corrupted ({why}); the node has a new identity, for {NET_NAME} this is a different node"
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

/// `rev`: absent — 0 with no error (like any missing field, CONFIG-FILE.md section 8); wrong type — 0 and an error.
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

/// Unknown fields: the root, `config`, the sections. Computed keys are part of `known_keys`;
/// retired keys go to `retired_paths`.
fn collect_unknown(root_obj: &serde_json::Map<String, Value>, cfg: &Value) -> Vec<String> {
    let mut unknown: Vec<String> = Vec::new();
    for k in root_obj.keys() {
        if !["rev", "saved_at", "identity", "config"].contains(&k.as_str()) {
            unknown.push(k.clone());
        }
    }
    if let Some(cfg_obj) = cfg.as_object() {
        for (k, v) in cfg_obj {
            if k == "version" {
                continue;
            }
            if !fields::section_names().contains(&k.as_str()) {
                unknown.push(format!("config.{k}"));
                continue;
            }
            if let Some(sec_obj) = v.as_object() {
                for sk in sec_obj.keys() {
                    if retired::is_retired(k, sk) {
                        continue;
                    }
                    if !fields::known_keys(k).contains(&sk.as_str()) {
                        unknown.push(format!("config.{k}.{sk}"));
                    }
                }
            }
        }
    }
    unknown
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
    let unknown_paths = collect_unknown(root_obj, cfg);
    let retired_paths = retired::collect_retired_paths(cfg);

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
