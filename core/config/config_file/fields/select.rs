//! Selections from the table: paths in table order.

use super::row::{Block, Field, Mode};
use super::table::FIELDS;

fn paths(keep: impl Fn(&Field) -> bool) -> Vec<&'static str> {
    FIELDS
        .iter()
        .filter(|f| keep(f))
        .map(|f| f.field_path)
        .collect()
}

/// Returns the paths the page can change.
pub(crate) fn editable_paths() -> Vec<&'static str> {
    paths(|f| f.mode.is_editable())
}

/// Returns the paths the page only shows (`read_only` in responses).
pub(crate) fn read_only_paths() -> Vec<&'static str> {
    paths(|f| f.mode == Mode::ReadOnly)
}

/// Returns the paths `restart_fields` reports as needing a restart.
pub(crate) fn restart_paths() -> Vec<&'static str> {
    paths(|f| f.mode == Mode::RestartShown)
}

/// Returns the paths applied to a running node without a restart.
pub(crate) fn live_paths() -> Vec<&'static str> {
    paths(|f| f.mode == Mode::Live)
}

/// Returns the known keys of a section, including computed ones.
pub(crate) fn known_keys(section: &str) -> Vec<&'static str> {
    FIELDS
        .iter()
        .filter(|f| f.section() == section)
        .map(Field::key)
        .collect()
}

/// Returns the sections by first appearance in the table.
pub(crate) fn section_names() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for f in FIELDS {
        if !out.contains(&f.section()) {
            out.push(f.section());
        }
    }
    out
}

/// Returns the rows of one settings block in table order.
pub(crate) fn rows_of(block: Block) -> impl Iterator<Item = &'static Field> {
    FIELDS.iter().filter(move |f| f.block() == block)
}

/// Returns the sections that have a row of the block, by first appearance in the table.
pub(crate) fn section_names_of(block: Block) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for f in rows_of(block) {
        if !out.contains(&f.section()) {
            out.push(f.section());
        }
    }
    out
}

/// Returns the table row for a path.
pub(crate) fn field_by_path(field_path: &str) -> Option<&'static Field> {
    FIELDS.iter().find(|f| f.field_path == field_path)
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
