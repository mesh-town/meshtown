//! Help texts of the command line: the top-level help grouped by purpose, the footer, the help template,
//! the program name inside help texts, the layout of the `-h, --help` lines and the default subcommand.

use std::ffi::OsString;

use crate::cli::program_name::{program_name, DEFAULT_PROGRAM_NAME};
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
        true => "With no command, the Mesh Town icon starts next to the clock.\nJoin a network with a member's invite:\n  meshtown join \"<invite>\"",
        false => "Run `meshtown run` to start the node; join a network with a member's invite:\n  meshtown join \"<invite>\"",
    }
}

/// The groups of the top-level help, in order; a command is listed under the first group that names it.
/// Commands the table doesn't name go to `OTHER`.
const GROUPS: &[(&str, &[&str])] = &[
    ("Start", &["run", "join", "status", "stop", "tray"]),
    ("Network", &["invite", "leave", "network", "member"]),
    ("Node page", &["open", "password", "access"]),
    ("Settings", &["config"]),
];

/// The heading of the commands that are in no group.
const OTHER: &str = "Other";

/// The argument ids of the global options shown at the end of the top-level help.
const GLOBAL_IDS: [&str; 5] = ["config", "web_port", "net", "json", "yes"];

/// A line of a help table: the left column and its description.
type Row = (String, String);

/// The first line of a command's description.
fn about_line(cmd: &clap::Command) -> String {
    let about = cmd.get_about().map(|a| a.to_string()).unwrap_or_default();
    about.lines().next().unwrap_or("").to_string()
}

/// The visible subcommands as (name, description).
fn command_rows(cmd: &clap::Command) -> Vec<Row> {
    cmd.get_subcommands()
        .filter(|c| !c.is_hide_set())
        .map(|c| (c.get_name().to_string(), about_line(c)))
        .collect()
}

/// The commands split into the groups of `GROUPS`; a name missing from `rows` is skipped and an empty group dropped.
fn grouped(rows: &[Row]) -> Vec<(&'static str, Vec<&Row>)> {
    let pick = |names: &[&str]| -> Vec<&Row> {
        names
            .iter()
            .filter_map(|n| rows.iter().find(|r| r.0 == *n))
            .collect()
    };
    let known: Vec<&str> = GROUPS.iter().flat_map(|g| g.1.iter().copied()).collect();
    let rest: Vec<&Row> = rows
        .iter()
        .filter(|r| !known.contains(&r.0.as_str()))
        .collect();
    let mut groups: Vec<(&'static str, Vec<&Row>)> =
        GROUPS.iter().map(|(h, names)| (*h, pick(names))).collect();
    groups.push((OTHER, rest));
    groups.into_iter().filter(|g| !g.1.is_empty()).collect()
}

/// Rows as `  {left:<W}  {right}` lines, `W` being the longest left column.
fn table_lines<'a>(rows: impl IntoIterator<Item = &'a Row>) -> Vec<String> {
    let rows: Vec<&Row> = rows.into_iter().collect();
    let width = rows.iter().map(|r| r.0.chars().count()).max().unwrap_or(0);
    rows.iter()
        .map(|(left, right)| format!("  {left:<width$}  {right}"))
        .collect()
}

/// `--name <VALUE>` of an argument, as typed.
fn flag_text(arg: &Arg) -> String {
    let long = arg.get_long().unwrap_or_default();
    let value = arg.get_value_names().and_then(|v| v.first());
    match value.filter(|_| arg.get_action().takes_values()) {
        Some(value) => format!("--{long} <{value}>"),
        None => format!("--{long}"),
    }
}

/// The global options, then `-h` and `-V`.
fn global_rows(cmd: &clap::Command) -> Vec<Row> {
    let flags = GLOBAL_IDS
        .iter()
        .filter_map(|id| cmd.get_arguments().find(|a| a.get_id() == *id))
        .map(|a| {
            (
                flag_text(a),
                a.get_help().map(|h| h.to_string()).unwrap_or_default(),
            )
        });
    let help = with_program_name(HELP_HINT, program_name());
    let tail = [
        ("-h, --help".to_string(), help),
        ("-V, --version".to_string(), "Show version".to_string()),
    ];
    flags.chain(tail).collect()
}

/// The top-level help: the description, the usage, the commands by group, the global options and the footer.
/// The same text answers `--help`, `-h`, `help` and a start with no arguments in a build without a tray.
pub(super) fn root_help(cmd: &clap::Command, has_tray: bool) -> String {
    let about = cmd.get_about().map(|a| a.to_string()).unwrap_or_default();
    let mut out = format!("{about}\n\nUsage: {} [COMMAND]\n", program_name());
    let rows = command_rows(cmd);
    for (heading, group) in grouped(&rows) {
        out.push_str(&format!(
            "\n{heading}:\n{}\n",
            table_lines(group).join("\n")
        ));
    }
    let globals = global_rows(cmd);
    out.push_str(&format!(
        "\nGlobal options:\n{}\n",
        table_lines(&globals).join("\n")
    ));
    out.push_str(&format!("\n{}", after_help_text(has_tray)));
    with_program_name(&out, program_name())
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

/// If `args` is exactly the program name (one element, no flags or subcommand), appends `tray` when
/// `has_tray`, otherwise `--help`; otherwise returns it as is (an empty list too). The substitution happens
/// before `clap` parses, so the value sources (`ValueSource`) don't change: `tray` with no arguments looks to
/// `is_flag_typed`/`toggle_from_flag_pair` the same as a `tray` subcommand typed by a person.
pub(crate) fn with_default_subcommand(
    args: impl IntoIterator<Item = OsString>,
    has_tray: bool,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = args.into_iter().collect();
    if args.len() == 1 {
        args.push(OsString::from(if has_tray { "tray" } else { "--help" }));
    }
    args
}

#[cfg(test)]
#[path = "help_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "help_layout_tests.rs"]
mod layout_tests;
