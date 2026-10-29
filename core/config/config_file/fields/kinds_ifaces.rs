//! The "interface list" value kind: `[]` — automatic, `["*"]` — no limits, names — only those.

use serde_json::Value;

use super::super::iface_filter::{name_ok, IfaceFilter};
use super::value::Kind;

/// The `socks.ifaces` / `web.ifaces` field: an array of names without repeats, `null` is the same as `[]`.
pub(crate) struct IfaceNames;

fn names_of(v: &Value) -> Option<Vec<String>> {
    v.as_array()?
        .iter()
        .map(|x| x.as_str().map(str::to_string))
        .collect()
}

impl Kind<IfaceFilter> for IfaceNames {
    fn parse(&self, v: &Value) -> Option<IfaceFilter> {
        if v.is_null() {
            return Some(IfaceFilter::Auto);
        }
        IfaceFilter::from_names(names_of(v)?)
    }
    fn expected(&self) -> String {
        "a list of interface names without repeats ([] — automatic, [\"*\"] — no limits)"
            .to_string()
    }
    fn default_text(&self, _: &IfaceFilter) -> String {
        "[]".to_string()
    }
    fn to_json(&self, v: &IfaceFilter) -> Option<Value> {
        Some(Value::from(v.parts().1))
    }
}

/// The `exit.interface` field: the exit's outgoing interface name; `null` — any network (`None`).
pub(crate) struct ExitIface;

impl Kind<Option<String>> for ExitIface {
    fn parse(&self, v: &Value) -> Option<Option<String>> {
        if v.is_null() {
            return Some(None);
        }
        let name = v.as_str()?;
        let clean = name.trim() == name && !name.is_empty();
        (clean && name_ok(name)).then(|| Some(name.to_string()))
    }
    fn expected(&self) -> String {
        "an interface name (1\u{2013}64 bytes, no control characters) or null \u{2014} any network"
            .to_string()
    }
    fn default_text(&self, _: &Option<String>) -> String {
        "any network".to_string()
    }
    fn to_json(&self, v: &Option<String>) -> Option<Value> {
        v.clone().map(Value::from)
    }
}

#[cfg(test)]
#[path = "kinds_ifaces_tests.rs"]
mod tests;
