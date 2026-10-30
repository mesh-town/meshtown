//! What parsing refused (`classify`) and the exit of the entry point on a refusal (`parse_or_exit`).

use std::ffi::OsString;

use super::text::{bad_flag_text, bad_value_text, other_text, renamed_text, unknown_command_text};
use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{ArgMatches, Command};

use crate::cli::args::{renamed_hint, Cli, Renamed};

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
    /// A command or option that was replaced: the row of the table with what to type now.
    Renamed(&'static Renamed),
    /// Any other refusal: a missing argument, a conflict, no command.
    Other,
    /// `--help`, `--version` and the help of a command that lacks an argument: as `clap` decides.
    LeaveToClap,
}

/// The reason parsing refused: the word `help`, an unknown command, a wrong option or value, or something else.
pub(crate) fn classify(err: &clap::Error) -> ParseFail {
    match err.kind() {
        ErrorKind::UnknownArgument => return ParseFail::BadFlag,
        ErrorKind::ValueValidation | ErrorKind::InvalidValue => return ParseFail::BadValue,
        ErrorKind::DisplayHelp
        | ErrorKind::DisplayVersion
        | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => return ParseFail::LeaveToClap,
        ErrorKind::InvalidSubcommand => {}
        _ => return ParseFail::Other,
    }
    match err.get(ContextKind::InvalidSubcommand) {
        Some(ContextValue::String(word)) if word == "help" => ParseFail::HelpWord,
        Some(ContextValue::String(name)) => ParseFail::UnknownCommand(name.to_string()),
        _ => ParseFail::Other,
    }
}

/// The strings `clap` names as the cause of a refusal: the unknown command, option or value.
fn offending_words(err: &clap::Error) -> Vec<String> {
    [
        ContextKind::InvalidSubcommand,
        ContextKind::InvalidArg,
        ContextKind::InvalidValue,
    ]
    .into_iter()
    .filter_map(|kind| match err.get(kind) {
        Some(ContextValue::String(text)) => text.split(' ').next().map(str::to_string),
        _ => None,
    })
    .collect()
}

/// Replaces an unknown command, option or value with the row of `RENAMED` when the refused word is the
/// last word of that row: a name that is still alive never gets there, and an unrelated mistake in the
/// same command line keeps its own text.
fn refine(fail: ParseFail, err: &clap::Error, args: &[OsString]) -> ParseFail {
    let replaced = matches!(
        fail,
        ParseFail::UnknownCommand(_) | ParseFail::BadFlag | ParseFail::BadValue
    );
    let found = renamed_hint(args).filter(|row| {
        replaced
            && offending_words(err)
                .iter()
                .any(|w| Some(&w.as_str()) == row.old.last())
    });
    match found {
        Some(row) => ParseFail::Renamed(row),
        None => fail,
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

/// Parses the arguments; on refusal answers itself and exits: help — stdout/0, anything else the app
/// words itself — stderr/2 in the form `Error: …`.
pub(crate) fn parse_or_exit(args: Vec<OsString>) -> ArgMatches {
    let err = match Cli::command().try_get_matches_from(&args) {
        Ok(matches) => return matches,
        Err(err) => err,
    };
    let commands = command_names(&Cli::command());
    let typed = typed_command(&args, &commands);
    match refine(classify(&err), &err, &args) {
        ParseFail::HelpWord => print_help_word(&args, &commands),
        ParseFail::UnknownCommand(name) => {
            exit_with_error_text(&unknown_command_text(&name, &commands))
        }
        ParseFail::BadFlag => exit_with_error_text(&bad_flag_text(&err, typed.as_deref())),
        ParseFail::BadValue => exit_with_error_text(&bad_value_text(&err, typed.as_deref())),
        ParseFail::Renamed(row) => exit_with_error_text(&renamed_text(row)),
        ParseFail::Other => exit_with_error_text(&other_text(&err, typed.as_deref())),
        ParseFail::LeaveToClap => leave_to_clap(&err),
    }
}

/// Help and version go to stdout with code 0; the help of a command that lacks an argument is the same.
fn leave_to_clap(err: &clap::Error) -> ! {
    match err.kind() {
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            print!("{}", err.render());
            std::process::exit(0)
        }
        _ => err.exit(),
    }
}

#[cfg(test)]
#[path = "classify_tests.rs"]
mod tests;
