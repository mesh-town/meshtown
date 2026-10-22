//! Lenient reading of snapshot fields: a missing key and a foreign type both give the same field default.
//!
//! What it does: `from_object` reads a struct from an object (anything that is not an object gives the default), the other
//! functions are `deserialize_with` for the fields of `types.rs`; it is the only place where the tray looks
//! into `serde_json::Value`.
//! What it does not do: it knows no snapshot field by name and writes nothing to the log.
//! Links: `types.rs` plugs them into `#[serde(deserialize_with = …)]`.

use meshtown::web::RouteMode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use super::types::TrayNodeStatus;

/// A struct from an object; anything that is not an object (an array would be read positionally) gives the default.
pub(crate) fn from_object<T: DeserializeOwned + Default>(value: &Value) -> T {
    match value.is_object() {
        true => T::deserialize(value).unwrap_or_default(),
        false => T::default(),
    }
}

pub(crate) fn object_or_default<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(
    d: D,
) -> Result<T, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(from_object(&value))
}

pub(crate) fn opt_object<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(
    d: D,
) -> Result<Option<T>, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(value.is_object().then(|| from_object(&value)))
}

/// A list of structs: not an array — empty, a non-object element — the element's default.
pub(crate) fn list_or_empty<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(
    d: D,
) -> Result<Vec<T>, D::Error> {
    Ok(opt_list(d)?.unwrap_or_default())
}

/// Like `list_or_empty`, but "not an array" (`None`) is distinguishable from an empty array (`Some([])`).
pub(crate) fn opt_list<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(
    d: D,
) -> Result<Option<Vec<T>>, D::Error> {
    let value = Value::deserialize(d)?;
    let items = value.as_array();
    Ok(items.map(|items| items.iter().map(from_object).collect()))
}

/// A list of strings: non-strings are dropped, not an array — empty.
pub(crate) fn strings<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let value = Value::deserialize(d)?;
    let items = value.as_array().map(Vec::as_slice).unwrap_or_default();
    Ok(items.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
}

pub(crate) fn bool_or_false<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(value.as_bool().unwrap_or(false))
}

pub(crate) fn opt_string<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(value.as_str().map(str::to_string))
}

pub(crate) fn string_or_empty<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(opt_string(d)?.unwrap_or_default())
}

pub(crate) fn opt_u64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(value.as_u64())
}

/// A non-negative integer; a missing key or a foreign type gives zero.
pub(crate) fn u64_or_zero<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(value.as_u64().unwrap_or(0))
}

/// `true` only for an explicit `null`; a missing key gives the field default (`false`), which `default` sets.
pub(crate) fn is_null<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(value.is_null())
}

/// Route: an unknown string, or a non-string, gives `Direct`.
pub(crate) fn route_mode<'de, D: Deserializer<'de>>(d: D) -> Result<RouteMode, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(RouteMode::deserialize(value).unwrap_or_default())
}

/// Node status: a non-string gives `None`, an unknown string gives `Unknown`.
pub(crate) fn node_word<'de, D: Deserializer<'de>>(d: D) -> Result<Option<TrayNodeStatus>, D::Error> {
    let value = Value::deserialize(d)?;
    match value.is_string() {
        true => Ok(Some(TrayNodeStatus::deserialize(value).unwrap_or(TrayNodeStatus::Unknown))),
        false => Ok(None),
    }
}
