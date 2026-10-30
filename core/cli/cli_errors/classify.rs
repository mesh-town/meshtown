//! What parsing refused (`classify`) and the exit of the entry point on a refusal (`parse_or_exit`).

use std::ffi::OsString;

use super::text::{bad_flag_text, bad_value_text, unknown_command_text};
use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{ArgMatches, Command};

use crate::cli::args::Cli;

/// What command line parsing said when it refused.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ParseFail {
    /// The word `help` was typed (or `help command`): print help, exit code 0.
    HelpWord,
    /// No such command: the name that was typed.
    UnknownCommand(String),
    /// An unknown option or an extra word: `ErrorKind::UnknownArgument`.
    BadFlag,
    /// A wrong option value or an option without a value: `ValueValidation`, `InvalidValue`.
    BadValue,
    /// Everything else (`--help`, `--version`): as `clap` decides.
    LeaveToClap,
}

/// The reason parsing refused: the word `help`, an unknown command or something else.
pub(crate) fn classify(err: &clap::Error) -> ParseFail {
    match err.kind() {
        ErrorKind::UnknownArgument => return ParseFail::BadFlag,
        ErrorKind::ValueValidation | ErrorKind::InvalidValue => return ParseFail::BadValue,
        _ => {}
    }
    let word = match err.get(ContextKind::InvalidSubcommand) {
        Some(ContextValue::String(name)) if err.kind() == ErrorKind::InvalidSubcommand => name,
        _ => return ParseFail::LeaveToClap,
    };
    match word.as_str() {
        "help" => ParseFail::HelpWord,
        name => ParseFail::UnknownCommand(name.to_string()),
    }
}

/// The names of the visible subcommands in order.
pub(crate) fn command_names(cmd: &Command) -> Vec<String> {
    cmd.get_subcommands()
        .filter(|c| !c.is_hide_set())
        .map(|c| c.get_name().to_string())
        .collect()
}

/// The subcommand that was typed: the first word after the program name equal to a visible command name.
pub(crate) fn typed_command(args: &[OsString], commands: &[String]) -> Option<String> {
    args.iter()
        .skip(1)
        .filter_map(|a| a.to_str())
        .find(|a| commands.iter().any(|c| c == a))
        .map(str::to_string)
}

/// Prints one line to stderr and exits with code 2.
fn exit_with_error_text(text: &str) -> ! {
    eprintln!("{text}");
    std::process::exit(2)
}

/// The arguments for re-parsing for the word `help`: `[program, command, --help]`, and without a command —
/// `[program, --help]`. A name that isn't among the commands is `Err(name)`.
fn help_args(args: &[OsString], commands: &[String]) -> Result<Vec<OsString>, String> {
    let program = args.first().cloned().unwrap_or_default();
    let mut rest = args.iter().skip(1).skip_while(|a| *a != "help").skip(1);
    let name = rest.next().and_then(|a| a.to_str()).unwrap_or("-");
    if name.starts_with('-') {
        return Ok(vec![program, OsString::from("--help")]);
    }
    if commands.iter().any(|c| c == name) {
        return Ok(vec![
            program,
            OsString::from(name),
            OsString::from("--help"),
        ]);
    }
    Err(name.to_string())
}

/// The word `help`: help by the same path as `--help` (stdout, code 0); an unknown command is a refusal.
fn print_help_word(args: &[OsString], commands: &[String]) -> ! {
    let again = match help_args(args, commands) {
        Ok(again) => again,
        Err(name) => exit_with_error_text(&unknown_command_text(&name, commands)),
    };
    match Cli::command().try_get_matches_from(again) {
        Err(err) => err.exit(),
        Ok(_) => std::process::exit(0),
    }
}

/// Parses the arguments; on refusal answers itself and exits: help — stdout/0, an unknown command,
/// a wrong option and value — stderr/2, anything else — as `clap`.
pub(crate) fn parse_or_exit(args: Vec<OsString>) -> ArgMatches {
    let err = match Cli::command().try_get_matches_from(&args) {
        Ok(matches) => return matches,
        Err(err) => err,
    };
    let commands = command_names(&Cli::command());
    let typed = typed_command(&args, &commands);
    match classify(&err) {
        ParseFail::HelpWord => print_help_word(&args, &commands),
        ParseFail::UnknownCommand(name) => {
            exit_with_error_text(&unknown_command_text(&name, &commands))
        }
        ParseFail::BadFlag => exit_with_error_text(&bad_flag_text(&err, typed.as_deref())),
        ParseFail::BadValue => exit_with_error_text(&bad_value_text(&err, typed.as_deref())),
        ParseFail::LeaveToClap => err.exit(),
    }
}

#[cfg(test)]
#[path = "classify_tests.rs"]
mod tests;
