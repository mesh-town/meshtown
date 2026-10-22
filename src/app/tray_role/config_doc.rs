//! The `GET`/`PUT /api/config` response as the poller reads it: `rev` and the selected exits.
//!
//! What it does: a single parse of the response into `ConfigDoc` (`rev`, `config.route.exits`); a missing key and a foreign
//! type both give the field default (`rev` is `None`, `exits` is empty), as with the snapshot selection.
//! What it does not do: it does not read the settings file (`config_file/*`) and does not write the `PUT` body.
//! Links: `poller_exit.rs` takes `rev` and the list of exits from here; the field readers are in `snapshot::lenient`.

use serde_json::Value;

use crate::app::snapshot::lenient;

/// The config response: `rev` and the settings document under the key `config`.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
pub(super) struct ConfigDoc {
    #[serde(deserialize_with = "lenient::opt_u64")]
    pub(super) rev: Option<u64>,
    #[serde(deserialize_with = "lenient::object_or_default")]
    config: ConfigBody,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
struct ConfigBody {
    #[serde(deserialize_with = "lenient::object_or_default")]
    route: ConfigRoute,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
struct ConfigRoute {
    #[serde(deserialize_with = "lenient::strings")]
    exits: Vec<String>,
}

impl ConfigDoc {
    /// The document from the node's reply; a non-object gives an empty one.
    pub(super) fn from_value(value: &Value) -> Self {
        lenient::from_object(value)
    }

    /// `config.route.exits` is a list of strings; a missing field, a non-list or non-strings give an empty list.
    pub(super) fn exits(self) -> Vec<String> {
        self.config.route.exits
    }
}

#[cfg(test)]
#[path = "config_doc_tests.rs"]
mod tests;
