//! Command line parsing for the app (`clap`).
//!
//! - `cli` - the clap definitions (`Cli`, `CliCommand`).
//! - `globals` - the options every command accepts and the checks on the ones it doesn't use.
//! - `help` - help texts, layout and the default subcommand.
//! - `renamed` - the commands and options that were replaced, with their replacements.

mod cli;
mod globals;
mod help;
mod renamed;

pub(crate) use cli::{is_flag_typed, toggle_from_flag_pair, Cli, CliCommand, PasswordAction};
pub(crate) use globals::{Global, Globals};
#[cfg(test)]
pub(crate) use help::after_help_text;
pub(crate) use help::{set_has_tray, with_default_subcommand, HELP};
pub(crate) use renamed::{renamed_hint, Renamed};
