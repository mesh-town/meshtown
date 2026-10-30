//! Command line parsing for the app (`clap`).
//!
//! - `cli` - the clap definitions (`Cli`, `CliCommand`) and the default subcommand.
//! - `help` - help texts and layout.

mod cli;
mod help;

pub(crate) use cli::{
    is_flag_typed, parse_access, toggle_from_flag_pair, with_default_subcommand, Cli, CliCommand,
    PasswordAction,
};
#[cfg(test)]
pub(crate) use help::after_help_text;
pub(crate) use help::set_has_tray;
