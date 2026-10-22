//! Table-driven output: the `config` section of the file (`ConfigDoc`) and the settings `Debug` (`debug_settings`).

use std::fmt;

use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};
use serde_json::Value;

use super::super::settings::Settings;
use super::super::CONFIG_VERSION;
use super::{select::section_names, table::FIELDS};

/// Sections that vanish entirely when they have no keys (the others are written as `{}`).
const DROPPED_WHEN_EMPTY: [&str; 1] = ["net"];

/// Key order in objects inside lists (`serde_json` without `preserve_order` sorts them
/// alphabetically): `ports.published` — port, name, for; `ports.opened` — node, port, local.
const KEY_ORDER: [&str; 5] = ["node", "port", "name", "for", "local"];

/// The `config` section of the document: `version`, then the sections in table order.
pub(crate) struct ConfigDoc<'a>(pub &'a Settings);

impl Serialize for ConfigDoc<'_> {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let mut map = ser.serialize_map(None)?;
        map.serialize_entry("version", &CONFIG_VERSION)?;
        for name in section_names() {
            let section = Section { s: self.0, name };
            if !section.entries().is_empty() || !DROPPED_WHEN_EMPTY.contains(&name) {
                map.serialize_entry(name, &section)?;
            }
        }
        map.end()
    }
}

struct Section<'a> {
    s: &'a Settings,
    name: &'static str,
}

impl Section<'_> {
    /// Returns the section's keys that are written, in table order.
    fn entries(&self) -> Vec<(&'static str, Value)> {
        let rows = FIELDS.iter().filter(|f| f.section() == self.name);
        rows.filter_map(|f| f.value.render_for_file(self.s).map(|v| (f.key(), v)))
            .collect()
    }
}

impl Serialize for Section<'_> {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let entries = self.entries();
        let mut map = ser.serialize_map(Some(entries.len()))?;
        for (key, v) in &entries {
            map.serialize_entry(key, &Ordered(v))?;
        }
        map.end()
    }
}

/// A value with object keys in `KEY_ORDER`.
struct Ordered<'a>(&'a Value);

impl Serialize for Ordered<'_> {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        if let Value::Array(items) = self.0 {
            let mut seq = ser.serialize_seq(Some(items.len()))?;
            items
                .iter()
                .try_for_each(|i| seq.serialize_element(&Ordered(i)))?;
            return seq.end();
        }
        if let Value::Object(obj) = self.0 {
            let mut pairs: Vec<_> = obj.iter().collect();
            pairs.sort_by_key(|(k, _)| KEY_ORDER.iter().position(|o| o == k));
            let mut map = ser.serialize_map(Some(pairs.len()))?;
            pairs
                .iter()
                .try_for_each(|(k, v)| map.serialize_entry(k, &Ordered(v)))?;
            return map.end();
        }
        self.0.serialize(ser)
    }
}

/// Ready-made value text for `debug_struct`.
struct Text(String);

impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Settings `Debug`: a line per stored path, for a secret — `Some("<hidden>")` or `None`.
pub(crate) fn debug_settings(s: &Settings, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut out = f.debug_struct("Settings");
    for row in FIELDS.iter().filter(|r| r.value.is_stored_in_settings()) {
        let hidden = row
            .redacted
            .then(|| row.value.render_for_file(s).map(|_| "<hidden>"));
        let text = hidden.map_or_else(|| row.value.debug_text(s), |h| format!("{h:?}"));
        out.field(row.field_path, &Text(text));
    }
    out.finish()
}
