//! The clap definitions of the app: `Cli`, `CliCommand`, the helpers `is_flag_typed`/`toggle_from_flag_pair` that
//! tell a typed flag from a default, and the default subcommand substitution (`with_default_subcommand`):
//! starting with no arguments behaves like `tray` (a build with the tray) or `run` (without it).
//! It does not execute subcommands; that is `commands`.

use std::ffi::OsString;

use super::help::{
    after_help_text, has_tray, rename_commands, sub_help_flag, with_program_name, HELP, HELP_HINT,
    HELP_ORDER,
};
use crate::cli::program_name::program_name;
use clap::parser::ValueSource;
use clap::{ArgAction, ArgMatches, CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "meshtown",
    version,
    about = "Reach the services on your devices from anywhere — a private peer-to-peer network with no port forwarding, no central server and no accounts.",
    help_template = HELP,
    override_usage = "meshtown [COMMAND]",
    subcommand_help_heading = "Commands",
    next_help_heading = "Options",
    disable_help_flag = true,
    disable_version_flag = true,
    disable_help_subcommand = true
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: CliCommand,
    // `global = true` on `-h` is required: without it `meshtown run --help` fails with
    // "unexpected argument '--help' found", because the subcommand's own help template cancels
    // its own `-h`. `-V` can't be global: clap 4.6.7 panics at build
    // ("Command run: `ArgAction::Version` used without providing Command::version"), so
    // the version stays on the root only.
    /// Show help (for one command: meshtown run --help)
    #[arg(short, long, global = true, action = ArgAction::Help)]
    help: Option<bool>,
    /// Show version
    #[arg(short = 'V', long, action = ArgAction::Version)]
    version: Option<bool>,
}

impl Cli {
    /// Describes the command line for parsing and help. A wrapper over the derived `command()`:
    /// every subcommand has its own `-h, --help` line ("Show help") last in the
    /// "Options:" section; at the top `-h` and `-V` are also last. This method owns the name: it shadows
    /// the method of the same name on `CommandFactory`, so `Cli::command()` gives this layout everywhere.
    pub(crate) fn command() -> clap::Command {
        let cmd = <Self as CommandFactory>::command()
            .mut_arg("help", |a| {
                a.display_order(HELP_ORDER)
                    .help(with_program_name(HELP_HINT, program_name()))
            })
            .mut_arg("version", |a| a.display_order(HELP_ORDER + 1))
            .after_help(after_help_text(has_tray()));
        let cmd = rename_commands(cmd, program_name());
        let names: Vec<String> = cmd
            .get_subcommands()
            .map(|c| c.get_name().to_string())
            .collect();
        names.iter().fold(cmd, |cmd, name| {
            cmd.mut_subcommand(name, |sub| sub.arg(sub_help_flag()))
        })
    }
}

// `Run` has twenty-odd flags; the value is created once per process, so its size doesn't matter.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
pub(crate) enum CliCommand {
    /// Show the Mesh Town icon next to the clock and start Mesh Town
    #[cfg(feature = "gui")]
    #[command(help_template = HELP, next_help_heading = "Options", override_usage = "meshtown tray [OPTIONS] [-- RUN_OPTIONS…]")]
    Tray(crate::gui::tray::app::tray_role::TrayArgs),
    /// Run Mesh Town in this window, without an icon
    #[command(help_template = HELP, next_help_heading = "Options", override_usage = "meshtown run [OPTIONS]")]
    Run(crate::cli::commands::RunArgs),
    // group: access
    // group: config
    // group: member
    // group: network
    /// Print a new network key with this computer as its admin (advanced; most people create a network on the page or join with an invite)
    #[command(help_template = HELP, next_help_heading = "Options", override_usage = "meshtown keygen [--raw] [--config <PATH>] [LABEL]…")]
    Keygen {
        /// A network label, words separated by spaces: meshtown keygen our game. It doesn't affect the key.
        /// The random part is two words from the EFF list (CC BY 3.0 US) and a group of 21 characters.
        #[arg(value_name = "LABEL", help_heading = "Options")]
        label: Vec<String>,
        /// Print the machine key mesh2-… instead of the key with words.
        #[arg(long, conflicts_with = "label")]
        raw: bool,
        /// Settings file (JSON) whose node identity becomes the network admin; can be set with the MESHTOWN_CONFIG variable.
        #[arg(long, value_name = "PATH", env = "MESHTOWN_CONFIG", hide_env = true)]
        config: Option<std::path::PathBuf>,
    },
    /// Print the path to the settings file
    #[command(help_template = HELP, next_help_heading = "Options", override_usage = "meshtown config-path [--config <PATH>]")]
    ConfigPath {
        /// Settings file (JSON); can be set with the MESHTOWN_CONFIG variable.
        #[arg(long, value_name = "PATH", env = "MESHTOWN_CONFIG", hide_env = true)]
        config: Option<std::path::PathBuf>,
    },
    /// Reset the Mesh Town page password and print a one-time sign-in link
    #[command(help_template = HELP, next_help_heading = "Options", override_usage = "meshtown password reset [--config <PATH>] [--web-port <PORT>]")]
    Password {
        /// What to do with the password: for now only reset.
        #[arg(
            value_name = "ACTION",
            value_enum,
            hide_possible_values = true,
            help_heading = "Options"
        )]
        action: PasswordAction,
        /// Settings file (JSON); can be set with the MESHTOWN_CONFIG variable.
        #[arg(long, value_name = "PATH", env = "MESHTOWN_CONFIG", hide_env = true)]
        config: Option<std::path::PathBuf>,
        /// Port of the running Mesh Town page, if it isn't from the settings (default: 7777).
        #[arg(long, value_name = "PORT")]
        web_port: Option<u16>,
    },
    /// Open the Mesh Town page in the browser, or print a sign-in link
    #[command(help_template = HELP, next_help_heading = "Options", override_usage = "meshtown open [--print] [--config <PATH>] [--web-port <PORT>]")]
    Open {
        /// Only print the link, don't start a browser.
        #[arg(long)]
        print: bool,
        /// Settings file (JSON); can be set with the MESHTOWN_CONFIG variable.
        #[arg(long, value_name = "PATH", env = "MESHTOWN_CONFIG", hide_env = true)]
        config: Option<std::path::PathBuf>,
        /// Port of the running Mesh Town page, if it isn't from the settings (default: 7777).
        #[arg(long, value_name = "PORT")]
        web_port: Option<u16>,
    },
    /// Who can open the Mesh Town page: localhost (this computer only) or localnet (the local network too)
    #[command(help_template = HELP, next_help_heading = "Options", override_usage = "meshtown access <localhost|localnet> [--config <PATH>] [--web-port <PORT>]")]
    Access {
        /// Mode: localhost or localnet.
        #[arg(value_name = "MODE", value_parser = parse_access, help_heading = "Options")]
        mode: crate::config::config_file::ListenAccess,
        /// Settings file (JSON); can be set with the MESHTOWN_CONFIG variable.
        #[arg(long, value_name = "PATH", env = "MESHTOWN_CONFIG", hide_env = true)]
        config: Option<std::path::PathBuf>,
        /// Port of the running Mesh Town page, if it isn't from the settings (default: 7777).
        #[arg(long, value_name = "PORT")]
        web_port: Option<u16>,
    },
    /// Which networks Mesh Town accepts connections from: interface names, auto (automatic) or '*' (no restriction)
    #[command(help_template = HELP, next_help_heading = "Options", override_usage = "meshtown ifaces <NAME>... [--if-unset] [--config <PATH>] [--web-port <PORT>]")]
    Ifaces {
        /// Interface names (br-lan), the single word auto (automatic) or '*' (no restriction).
        #[arg(value_name = "NAME", required = true, num_args = 1.., help_heading = "Options")]
        names: Vec<String>,
        /// Write only if the file has neither socks.ifaces nor web.ifaces yet (for autostart).
        #[arg(long)]
        if_unset: bool,
        /// Settings file (JSON); can be set with the MESHTOWN_CONFIG variable.
        #[arg(long, value_name = "PATH", env = "MESHTOWN_CONFIG", hide_env = true)]
        config: Option<std::path::PathBuf>,
        /// Port of the running Mesh Town page, if it isn't from the settings (default: 7777).
        #[arg(long, value_name = "PORT")]
        web_port: Option<u16>,
    },
}

/// The only action on the page password.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum PasswordAction {
    Reset,
}

/// Parses the `access` mode: the error names the allowed values.
pub(crate) fn parse_access(s: &str) -> Result<crate::config::config_file::ListenAccess, String> {
    crate::config::config_file::ListenAccess::parse(s)
        .ok_or_else(|| "allowed: localhost, localnet".to_string())
}

/// Whether a flag was typed by a person (rather than taken from a default). The identifier is the field name, not `--flag`.
pub(crate) fn is_flag_typed(m: &ArgMatches, id: &str) -> bool {
    matches!(m.value_source(id), Some(ValueSource::CommandLine))
}

/// The pair "--x / --no-x": if the first was typed — Some(true), the second — Some(false), neither — None.
/// `overrides_with` leaves `true` only on the one typed last.
pub(crate) fn toggle_from_flag_pair(on: bool, off: bool) -> Option<bool> {
    if off {
        Some(false)
    } else if on {
        Some(true)
    } else {
        None
    }
}

/// If `args` is exactly the program name (one element, no flags or subcommand), appends
/// `tray` when `has_tray`, otherwise `run`; otherwise returns it as is (an empty list too). The substitution
/// happens before `clap` parses, so the value sources (`ValueSource`) don't change: `run` with no
/// arguments looks to `is_flag_typed`/`toggle_from_flag_pair` the same as a `run` subcommand typed by a person.
pub(crate) fn with_default_subcommand(
    args: impl IntoIterator<Item = OsString>,
    has_tray: bool,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = args.into_iter().collect();
    if args.len() == 1 {
        args.push(OsString::from(if has_tray { "tray" } else { "run" }));
    }
    args
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
