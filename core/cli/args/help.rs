//! Help texts of the command line: the footer, the help template, the program name inside help texts and
//! the layout of the `-h, --help` lines.

use crate::cli::program_name::DEFAULT_PROGRAM_NAME;
use clap::{Arg, ArgAction};

/// Whether this process has a tray backend; set once at startup, before the arguments are parsed.
static HAS_TRAY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Records whether a tray backend was handed to the process (the first call wins).
pub(crate) fn set_has_tray(has_tray: bool) {
    let _ = HAS_TRAY.set(has_tray);
}

/// Whether the help text should describe the tray start; before `set_has_tray` (tests) it follows the
/// `gui` feature.
pub(super) fn has_tray() -> bool {
    HAS_TRAY.get().copied().unwrap_or(cfg!(feature = "gui"))
}

/// The footer of the top-level help: what a start with no command does, and how to join a network.
pub(crate) fn after_help_text(has_tray: bool) -> &'static str {
    match has_tray {
        true => "With no command, the Mesh Town icon starts next to the clock.\nJoin a network with a member's invite:\n  meshtown run --invite \"<invite>\"",
        false => "With no command, Mesh Town starts.\nJoin a network with a member's invite:\n  meshtown run --invite \"<invite>\"",
    }
}

/// Replaces the word `meshtown` that starts a command in a help text with `name`.
pub(super) fn with_program_name(text: &str, name: &str) -> String {
    text.replace("meshtown ", &format!("{name} "))
}

/// The help template of the top level and subcommands: the word "Usage:" as text (`{usage-heading}` gives `Usage:`).
pub(super) const HELP: &str = "{about}\n\nUsage: {usage}\n\n{all-args}{after-help}";

/// The order of the last lines of the "Options:" section: after all the others, `-h` before `-V`.
pub(super) const HELP_ORDER: usize = 10_000;

/// The description of the top level `-h`, with the command name replaced at build time.
pub(super) const HELP_HINT: &str = "Show help (for one command: meshtown run --help)";

/// Puts `name` into the usage line and the examples of the top level and of every subcommand.
pub(super) fn rename_commands(cmd: clap::Command, name: &str) -> clap::Command {
    if name == DEFAULT_PROGRAM_NAME {
        return cmd;
    }
    let names: Vec<String> = cmd
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    let cmd = rename_texts(cmd, name);
    names.iter().fold(cmd, |cmd, sub| {
        cmd.mut_subcommand(sub, |c| rename_texts(c, name))
    })
}

/// Replaces the command name in one command's `override_usage` and `after_help`.
fn rename_texts(cmd: clap::Command, name: &str) -> clap::Command {
    let usage = cmd
        .get_overridden_usage()
        .map(|u| with_program_name(&u.to_string(), name));
    let after = cmd
        .get_after_help()
        .map(|u| with_program_name(&u.to_string(), name));
    let cmd = usage.into_iter().fold(cmd, |c, u| c.override_usage(u));
    after.into_iter().fold(cmd, |c, a| c.after_help(a))
}

/// The `-h, --help` line of a subcommand: its own, so it doesn't inherit the top level description.
pub(super) fn sub_help_flag() -> Arg {
    Arg::new("help")
        .short('h')
        .long("help")
        .help("Show help")
        .action(ArgAction::Help)
        .display_order(HELP_ORDER)
}

#[cfg(test)]
#[path = "help_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "help_layout_tests.rs"]
mod layout_tests;
