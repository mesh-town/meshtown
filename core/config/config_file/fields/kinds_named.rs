//! Name-like value kinds: a choice from a closed list (with and without `null`), a node name, a network key.

use serde_json::Value;

use super::value::Kind;

/// One value from a closed list of strings: `parse` parses, `name` is the name in the file, `expected` is
/// the expectation text.
pub(crate) struct Choice<T> {
    pub parse: fn(&str) -> Option<T>,
    pub name: fn(T) -> &'static str,
    pub expected: &'static str,
}

impl<T: Copy + Sync> Kind<T> for Choice<T> {
    fn to_json(&self, v: &T) -> Option<Value> {
        Some(Value::from((self.name)(*v)))
    }

    fn parse(&self, v: &Value) -> Option<T> {
        (self.parse)(v.as_str()?)
    }
    fn expected(&self) -> String {
        self.expected.to_string()
    }
    fn default_text(&self, d: &T) -> String {
        (self.name)(*d).to_string()
    }
}

/// Node name: a string, not empty after trimming whitespace, no longer than 64 characters.
pub(crate) struct NodeName;

impl Kind<Option<String>> for NodeName {
    fn to_json(&self, v: &Option<String>) -> Option<Value> {
        v.clone().map(Value::from)
    }

    fn parse(&self, v: &Value) -> Option<Option<String>> {
        let s = v
            .as_str()
            .filter(|s| !s.trim().is_empty() && s.chars().count() <= 64)?;
        Some(Some(s.to_string()))
    }
    fn expected(&self) -> String {
        "a string up to 64 characters, not empty after trimming whitespace".to_string()
    }
    fn default_text(&self, _: &Option<String>) -> String {
        "no name".to_string()
    }
}

/// Network key: a string, not empty after trimming; the key itself is parsed later, at start.
pub(crate) struct NetKey;

impl Kind<Option<String>> for NetKey {
    fn to_json(&self, v: &Option<String>) -> Option<Value> {
        v.clone().map(Value::from)
    }

    fn parse(&self, v: &Value) -> Option<Option<String>> {
        let s = v.as_str().map(str::trim).filter(|s| !s.is_empty())?;
        Some(Some(s.to_string()))
    }
    fn expected(&self) -> String {
        "a non-empty string".to_string()
    }
    fn default_text(&self, _: &Option<String>) -> String {
        "no key".to_string()
    }
}
