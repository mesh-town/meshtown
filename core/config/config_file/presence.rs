//! Whether the settings file text has a key of the form `"section.key"` (the value doesn't matter: `[]` and `null` also count as present).
//! Needed by the `ifaces --if-unset` subcommand: seeding must not touch what the owner has already chosen.

use serde_json::Value;

use super::parse::config_section;

/// `true` if the settings section of the file has `<section>.<key>`. Unparseable JSON, no section
/// or no subsection gives `false`.
pub fn has_field(text: &str, path: &str) -> bool {
    let Some((section, key)) = path.split_once('.') else {
        return false;
    };
    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    let Ok(cfg) = config_section(&root) else {
        return false;
    };
    cfg.get(section)
        .and_then(Value::as_object)
        .is_some_and(|s| s.contains_key(key))
}

#[cfg(test)]
#[path = "presence_tests.rs"]
mod tests;
