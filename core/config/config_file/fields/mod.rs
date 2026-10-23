//! The settings field table: one row per path, from which lists, key sets and parsing are taken.

mod doc;
mod kinds_ifaces;
mod kinds_list;
mod kinds_named;
mod kinds_scalar;
mod kinds_text;
mod row;
mod select;
mod table;
mod value;

pub(crate) use doc::{debug_settings, ConfigDoc, NetDoc};
pub(crate) use row::Block;
pub(crate) use select::{
    editable_paths, field_by_path, known_keys, live_paths, read_only_paths, restart_paths, rows_of,
    section_names, section_names_of,
};
pub(crate) use table::FIELDS;
