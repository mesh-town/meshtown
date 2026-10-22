//! Command line error texts in English: the report of a subcommand error and the handling of a wrong command.
//! What it does: prints `Error: …` with advice (the proxy port is busy — use another port), turns the word
//! `help` (and `help command`) into help, an unknown command into a line with the list of commands, a wrong
//! option and a wrong value into one line of text.
//! What it doesn't do: it doesn't parse flags (that is `clap` and `cli.rs`) and doesn't execute commands.

use std::ffi::OsString;

use crate::proxy::BindError;
use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{ArgMatches, Command};

use crate::cli::args::Cli;

/// The word error texts start with: a text that already begins with it gets no prefix.
const PREFIX: &str = "Error";

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

/// The report of a subcommand error: `Error: reason: deeper`; if the proxy port is busy — a second line with advice.
pub(crate) fn error_report(e: &anyhow::Error) -> String {
    let text = format!("{e:#}");
    let head = if text.starts_with(&format!("{PREFIX}: ")) {
        text
    } else {
        format!("{PREFIX}: {text}")
    };
    match e.chain().find_map(|c| c.downcast_ref::<BindError>()) {
        Some(bind) => format!("{head}\n{}", bind.advice()),
        None => head,
    }
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

/// Two lines about an unknown command: what is wrong and which ones exist.
pub(crate) fn unknown_command_text(name: &str, commands: &[String]) -> String {
    format!(
        "{PREFIX}: unknown command \"{name}\".\nCommands: {}. Help: {} --help",
        commands.join(", "),
        crate::cli::program_name::program_name()
    )
}

/// The subcommand that was typed: the first word after the program name equal to a visible command name.
pub(crate) fn typed_command(args: &[OsString], commands: &[String]) -> Option<String> {
    args.iter()
        .skip(1)
        .filter_map(|a| a.to_str())
        .find(|a| commands.iter().any(|c| c == a))
        .map(str::to_string)
}

/// The tail of the error text: where to look for the list of options.
fn tail_text(command: Option<&str>) -> String {
    match command {
        Some(name) => format!(
            "See all options: {} {name} --help",
            crate::cli::program_name::program_name()
        ),
        None => format!(
            "See all options: {} --help",
            crate::cli::program_name::program_name()
        ),
    }
}

/// A string from the `clap` error context (empty if `clap` didn't put one).
fn context_text(err: &clap::Error, kind: ContextKind) -> String {
    match err.get(kind) {
        Some(ContextValue::String(text)) => text.clone(),
        _ => String::new(),
    }
}

/// The flag name without the tail after a space: `--socks-port <PORT>` — `--socks-port`.
fn flag_name(err: &clap::Error) -> String {
    let arg = context_text(err, ContextKind::InvalidArg);
    arg.split(' ').next().unwrap_or("").to_string()
}

/// What is wrong with the value, by the text of the error source: a range, not a number; otherwise nothing.
fn reason_text(err: &clap::Error) -> Option<String> {
    let source = std::error::Error::source(err)?.to_string();
    if source.starts_with("allowed:") {
        return Some(source);
    }
    if source.contains("invalid digit") || source.contains("cannot parse integer") {
        return Some("use a whole number".to_string());
    }
    let (_, range) = source.split_once(" is not in ")?;
    let (from, to) = range.split_once("..=")?;
    Some(format!("use a number from {from} to {to}"))
}

/// A hint about a similar option, if `clap` found one: "Did you mean --socks-port? " or empty.
fn suggestion_text(err: &clap::Error) -> String {
    let hint = context_text(err, ContextKind::SuggestedArg);
    let name = hint.split(' ').next().unwrap_or("");
    if name.is_empty() {
        return String::new();
    }
    format!("Did you mean {name}? ")
}

/// The text about an unknown option or an extra word without a dash.
pub(crate) fn bad_flag_text(err: &clap::Error, command: Option<&str>) -> String {
    let arg = flag_name(err);
    let hint = suggestion_text(err);
    let tail = tail_text(command);
    if arg.starts_with('-') {
        return format!("{PREFIX}: unknown option \"{arg}\". {hint}{tail}");
    }
    format!("{PREFIX}: unexpected argument \"{arg}\". {hint}{tail}")
}

/// The text about a wrong option value or an option without a value.
pub(crate) fn bad_value_text(err: &clap::Error, command: Option<&str>) -> String {
    let (flag, tail) = (flag_name(err), tail_text(command));
    let value = context_text(err, ContextKind::InvalidValue);
    if err.kind() == ErrorKind::InvalidValue && value.is_empty() {
        return format!("{PREFIX}: {flag} needs a value. {tail}");
    }
    let why = reason_text(err)
        .map(|r| format!(": {r}"))
        .unwrap_or_default();
    format!("{PREFIX}: invalid value \"{value}\" for {flag}{why}. {tail}")
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
#[path = "cli_errors_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "cli_errors_text_tests.rs"]
mod text_tests;
