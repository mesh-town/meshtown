//! The machine form of a network key.

use crate::members::network_key::{managed_key_with_label, parse_user_input};

/// The `mesh2-…` form of a key given in words or already in the machine form: the same network, label kept.
pub(super) fn raw_form(key: &str) -> anyhow::Result<String> {
    let parsed = parse_user_input(key)?;
    Ok(managed_key_with_label(
        parsed.label.as_deref(),
        &parsed.base,
        &parsed.root,
    ))
}

#[cfg(test)]
#[path = "key_form_tests.rs"]
mod tests;
