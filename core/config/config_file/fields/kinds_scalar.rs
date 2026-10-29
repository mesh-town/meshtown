//! Scalar value kinds: a flag, numbers, ports, addresses, a name from a list, a node name, a network key,
//! "off only" and the memory guard exit threshold.

use std::net::SocketAddr;

use serde_json::Value;

use super::super::parse_fields::push_field_error;
use super::super::report::FieldError;
use super::super::settings::Settings;
use super::row::Block;
use super::value::{FieldValue, Kind};

/// `true` or `false`.
pub(crate) struct BoolKind;

impl Kind<bool> for BoolKind {
    fn to_json(&self, v: &bool) -> Option<Value> {
        Some(Value::from(*v))
    }

    fn parse(&self, v: &Value) -> Option<bool> {
        v.as_bool()
    }
    fn expected(&self) -> String {
        "true or false".to_string()
    }
    fn default_text(&self, d: &bool) -> String {
        d.to_string()
    }
}

/// An integer `min…max`.
pub(crate) struct Int {
    pub min: u64,
    pub max: u64,
}

impl Kind<u64> for Int {
    fn to_json(&self, v: &u64) -> Option<Value> {
        Some(Value::from(*v))
    }

    fn parse(&self, v: &Value) -> Option<u64> {
        v.as_u64().filter(|n| (self.min..=self.max).contains(n))
    }
    fn expected(&self) -> String {
        format!("an integer {}…{}", self.min, self.max)
    }
    fn default_text(&self, d: &u64) -> String {
        d.to_string()
    }
}

/// A port `min…max`.
pub(crate) struct Port {
    pub min: u16,
    pub max: u16,
}

impl Kind<u16> for Port {
    fn to_json(&self, v: &u16) -> Option<Value> {
        Some(Value::from(*v))
    }

    fn parse(&self, v: &Value) -> Option<u16> {
        number_in(v, &self.min, &self.max)
    }
    fn expected(&self) -> String {
        format!("a port {}…{}", self.min, self.max)
    }
    fn default_text(&self, d: &u16) -> String {
        d.to_string()
    }
}

/// An integer from the value that fits in `T` and lies in `min…max`.
fn number_in<T: TryFrom<u64> + PartialOrd>(v: &Value, min: &T, max: &T) -> Option<T> {
    v.as_u64()
        .and_then(|n| T::try_from(n).ok())
        .filter(|n| n >= min && n <= max)
}

/// `full_tunnel.enabled`: cannot be turned on (`true`) for now.
pub(crate) struct OffOnly;

impl Kind<bool> for OffOnly {
    fn to_json(&self, v: &bool) -> Option<Value> {
        Some(Value::from(*v))
    }

    fn parse(&self, v: &Value) -> Option<bool> {
        (v.as_bool() == Some(false)).then_some(false)
    }
    fn expected(&self) -> String {
        "false (enabling is unavailable in this build)".to_string()
    }
    fn default_text(&self, d: &bool) -> String {
        d.to_string()
    }
}

/// An `address:port` string.
pub(crate) struct Addr;

impl Kind<SocketAddr> for Addr {
    fn to_json(&self, v: &SocketAddr) -> Option<Value> {
        Some(Value::from(v.to_string()))
    }

    fn parse(&self, v: &Value) -> Option<SocketAddr> {
        v.as_str()?.parse().ok()
    }
    fn expected(&self) -> String {
        "a string like address:port".to_string()
    }
    fn default_text(&self, d: &SocketAddr) -> String {
        d.to_string()
    }
}

/// `null` or an `address:port` string.
pub(crate) struct OptAddr;

impl Kind<Option<SocketAddr>> for OptAddr {
    fn to_json(&self, v: &Option<SocketAddr>) -> Option<Value> {
        Some(v.map_or(Value::Null, |a| Value::from(a.to_string())))
    }

    fn parse(&self, v: &Value) -> Option<Option<SocketAddr>> {
        if v.is_null() {
            return Some(None);
        }
        Addr.parse(v).map(Some)
    }
    fn expected(&self) -> String {
        "a string like address:port or null".to_string()
    }
    fn default_text(&self, _: &Option<SocketAddr>) -> String {
        "null".to_string()
    }
}

pub(crate) use super::kinds_named::{Choice, NetKey, NodeName};

/// `node.memory_exit_mib`: like an integer, but not below the already parsed `node.memory_warn_mib` if
/// both are non-zero (otherwise the memory guard would exit before printing its summary). Works without the key too.
pub(crate) struct MemoryExit;

const EXIT_RANGE: Int = Int {
    min: 0,
    max: 1_048_576,
};

impl FieldValue for MemoryExit {
    fn read_from_file(
        &self,
        field_path: &'static str,
        file_value: Option<&Value>,
        s: &mut Settings,
        errs: &mut Vec<FieldError>,
    ) {
        let default = Settings::default().process.node_memory_exit_mib;
        let exit = match file_value {
            None => default,
            Some(v) => EXIT_RANGE.parse(v).unwrap_or_else(|| {
                push_field_error(
                    errs,
                    field_path.to_string(),
                    v,
                    &EXIT_RANGE.expected(),
                    default,
                );
                default
            }),
        };
        s.process.node_memory_exit_mib = exit;
        check_exit_against_warn(field_path, file_value.is_some(), s, errs);
    }

    fn expected(&self) -> String {
        EXIT_RANGE.expected()
    }

    fn copy_value_between(&self, from: &Settings, to: &mut Settings) {
        to.process.node_memory_exit_mib = from.process.node_memory_exit_mib;
    }

    fn render_for_file(&self, s: &Settings) -> Option<Value> {
        Some(Value::from(s.process.node_memory_exit_mib))
    }
    fn debug_text(&self, s: &Settings) -> String {
        s.process.node_memory_exit_mib.to_string()
    }
    fn is_stored_in_settings(&self) -> bool {
        true
    }
    fn block(&self) -> Block {
        Block::Process
    }
}

/// The exit threshold must exceed the warning one (or be 0 / have no warning): otherwise it is raised to the
/// warning value, and a value that came from the file is reported.
fn check_exit_against_warn(
    field_path: &'static str,
    from_file: bool,
    s: &mut Settings,
    errs: &mut Vec<FieldError>,
) {
    let warn = s.process.node_memory_warn_mib;
    let exit = s.process.node_memory_exit_mib;
    if exit == 0 || warn == 0 || exit >= warn {
        return;
    }
    let default = Settings::default().process.node_memory_exit_mib;
    s.process.node_memory_exit_mib = default.max(warn);
    if from_file {
        let expected = format!("at least node.memory_warn_mib ({warn}) or 0");
        push_field_error(
            errs,
            field_path.to_string(),
            &Value::from(exit),
            &expected,
            default.max(warn),
        );
    }
}

#[cfg(test)]
#[path = "kinds_scalar_tests.rs"]
mod tests;
