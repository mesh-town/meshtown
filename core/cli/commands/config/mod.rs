//! The `config` command group. For now it holds the settings file path and the `path=value` text of `--set`.

pub(super) mod assign;
mod path;

pub(crate) use path::config_path;
