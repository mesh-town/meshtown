//! Text list value kind: a list of short phrases, trimmed, lowercased and without repeats.

use std::collections::HashSet;

use serde_json::Value;

use super::super::slot_store::NetSlotId;
use super::value::Kind;

/// A list of phrases: at most `max_items`, each at most `max_chars` characters (not bytes).
/// Phrases are trimmed and lowercased; empty ones and repeats are dropped, order of first occurrence is kept.
pub(crate) struct TextList {
    pub max_items: usize,
    pub max_chars: usize,
}

impl Kind<Vec<String>> for TextList {
    fn to_json(&self, v: &Vec<String>) -> Option<Value> {
        Some(v.iter().map(|s| Value::from(s.as_str())).collect())
    }

    fn parse(&self, v: &Value) -> Option<Vec<String>> {
        let items = v.as_array()?;
        let mut seen = HashSet::new();
        let mut list = Vec::new();
        for item in items {
            let phrase = item.as_str()?.trim().to_lowercase();
            if phrase.chars().count() > self.max_chars {
                return None;
            }
            if !phrase.is_empty() && seen.insert(phrase.clone()) {
                list.push(phrase);
            }
        }
        (list.len() <= self.max_items).then_some(list)
    }

    fn expected(&self) -> String {
        format!(
            "a list of up to {} phrases, up to {} characters each",
            self.max_items, self.max_chars
        )
    }

    fn default_text(&self, _: &Vec<String>) -> String {
        "[]".to_string()
    }
}

/// A network slot reference (`proxy.net`): `main` or a six-character network id.
pub(crate) struct SlotRef;

impl Kind<NetSlotId> for SlotRef {
    fn to_json(&self, v: &NetSlotId) -> Option<Value> {
        (*v != NetSlotId::main()).then(|| Value::from(v.as_str()))
    }

    fn parse(&self, v: &Value) -> Option<NetSlotId> {
        NetSlotId::parse(v.as_str()?)
    }

    fn expected(&self) -> String {
        "main or a network id: 6 characters a-z, 2-7".to_string()
    }

    fn default_text(&self, d: &NetSlotId) -> String {
        d.as_str().to_string()
    }
}

#[cfg(test)]
#[path = "kinds_text_tests.rs"]
mod tests;
