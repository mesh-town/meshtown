//! Texts of the command line errors: the report of a subcommand error and the lines for an unknown command,
//! a wrong option and a wrong value.

use crate::proxy::BindError;
use clap::error::{ContextKind, ContextValue, ErrorKind};

/// The word error texts start with: a text that already begins with it gets no prefix.
const PREFIX: &str = "Error";

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

/// Two lines about an unknown command: what is wrong and which ones exist.
pub(crate) fn unknown_command_text(name: &str, commands: &[String]) -> String {
    format!(
        "{PREFIX}: unknown command \"{name}\".\nCommands: {}. Help: {} --help",
        commands.join(", "),
        crate::cli::program_name::program_name()
    )
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

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
