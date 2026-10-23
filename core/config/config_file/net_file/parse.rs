//! Reading a network slot file: the main file's parser, then the network block, `created_unix`
//! and the process keys that do not belong in a slot file.

use std::path::Path;

use serde_json::Value;

use super::super::fields::{self, Block};
use super::super::net_settings::NetSettings;
use super::super::parse::{parse_text, SecretRead};
use super::super::parse_fields::push_field_error;
use super::super::report::FieldError;

/// A parsed slot file.
#[derive(Clone, Debug)]
pub struct ParsedNetFile {
    pub rev: u64,
    /// When the slot was created; 0 if the file has no such key or it is damaged.
    pub created_unix: u64,
    pub secret: SecretRead,
    pub net: NetSettings,
    pub field_errors: Vec<FieldError>,
    /// Paths of unknown fields, for one log line.
    pub unknown_paths: Vec<String>,
    /// Process keys found in the file; they are ignored and disappear on the next write.
    pub process_paths: Vec<String>,
}

/// What is on disk. One value per slot and short-lived, so the large `ParsedNetFile` is not boxed.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum LoadedNetFile {
    Absent,
    /// Not readable; the reason as text.
    Unreadable(String),
    /// Did not parse as a document; the reason as text.
    Invalid(String),
    Parsed(ParsedNetFile),
}

/// Whether an error path points at a process row or at a section that has no network row.
fn is_process_error(field_path: &str) -> bool {
    match fields::field_by_path(field_path) {
        Some(f) => f.block() == Block::Process,
        None => {
            fields::section_names().contains(&field_path)
                && !fields::section_names_of(Block::Net).contains(&field_path)
        }
    }
}

/// The process keys present in `config`, as `config.<section>.<key>`, in table order.
fn collect_process_paths(cfg: &Value) -> Vec<String> {
    fields::rows_of(Block::Process)
        .filter(|f| cfg.get(f.section()).and_then(|s| s.get(f.key())).is_some())
        .map(|f| format!("config.{}", f.field_path))
        .collect()
}

/// `created_unix`: absent — 0 with no error, wrong type — 0 and an error.
fn read_created(root: &Value, errs: &mut Vec<FieldError>) -> u64 {
    match root.get("created_unix") {
        None => 0,
        Some(v) => v.as_u64().unwrap_or_else(|| {
            push_field_error(
                errs,
                "created_unix".to_string(),
                v,
                "non-negative integer",
                0,
            );
            0
        }),
    }
}

/// Parses the text of a slot file. `Err` is not a document (same rules as the main file).
pub fn parse_net_text(text: &str) -> Result<ParsedNetFile, String> {
    let p = parse_text(text)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let root: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let cfg = root.get("config").cloned().unwrap_or(Value::Null);

    let mut field_errors: Vec<FieldError> = p
        .field_errors
        .into_iter()
        .filter(|e| !is_process_error(&e.field_path))
        .collect();
    let created_unix = read_created(&root, &mut field_errors);
    let unknown_paths = p
        .unknown_paths
        .into_iter()
        .filter(|path| path != "created_unix")
        .collect();
    Ok(ParsedNetFile {
        rev: p.rev,
        created_unix,
        secret: p.secret,
        net: p.settings.net,
        field_errors,
        unknown_paths,
        process_paths: collect_process_paths(&cfg),
    })
}

/// Reads and parses a slot file: no file — `Absent`, unreadable — `Unreadable`, not a document — `Invalid`.
pub fn load_net_file(path: &Path) -> LoadedNetFile {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return LoadedNetFile::Absent,
        Err(e) => return LoadedNetFile::Unreadable(format!("not readable: {e}")),
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return LoadedNetFile::Invalid("not UTF-8".to_string());
    };
    match parse_net_text(&text) {
        Ok(p) => LoadedNetFile::Parsed(p),
        Err(m) => LoadedNetFile::Invalid(m),
    }
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
