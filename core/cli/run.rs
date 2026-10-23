//! The process entry of the Mesh Town console app (called by the `meshtown` binary).
//!
//! What it does: parses the command line arguments (`cli.rs`) and passes them to the subcommand
//! executor (`commands.rs`): `keygen`, `run` or `config-path`.
//! What it doesn't do: it holds no node business logic — that is in the `meshtown` library;
//! it prints the network key nowhere except `keygen`, which creates it.
//! Starting with no subcommand substitutes one itself (`crate::cli::args::with_default_subcommand`):
//! `run` when the process has no tray backend, `tray` when the platform binary handed one in.
//! Links: `commands` calls `crate::members::network_key`, `crate::members::words`,
//! `crate::invite` and `crate::node`; the node console commands are in `crate::proxy::domains`.

use crate::cli::args::{Cli, CliCommand};
use clap::FromArgMatches;

/// The tray backend handle a platform binary hands to the entry point.
#[cfg(feature = "gui")]
type TrayRef = &'static dyn crate::gui::tray::TrayBackend;

/// Without the `gui` feature there is no backend type: no value of this enum exists.
#[cfg(not(feature = "gui"))]
enum NoTray {}
#[cfg(not(feature = "gui"))]
type TrayRef = NoTray;

/// Printed when the `tray` command runs in a process without a tray backend.
#[cfg(feature = "gui")]
const NO_TRAY_TEXT: &str =
    "this meshtown build has no tray icon; use the desktop build or `meshtown run`";

/// The backend, or an error for a process that has none.
#[cfg(feature = "gui")]
fn require_tray(tray: Option<TrayRef>) -> anyhow::Result<TrayRef> {
    tray.ok_or_else(|| anyhow::anyhow!(NO_TRAY_TEXT))
}

/// Sets up log output (`tracing`) to stderr; the default filter is warnings and above.
fn init_tracing() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();
}

/// The process entry without a tray backend: no command means `run`.
pub fn run() -> std::process::ExitCode {
    start(None)
}

/// The process entry of a platform binary that owns a tray backend: no command means `tray`.
#[cfg(feature = "gui")]
pub fn run_with_tray(tray: &'static dyn crate::gui::tray::TrayBackend) -> std::process::ExitCode {
    start(Some(tray))
}

#[tokio::main]
async fn start(tray: Option<TrayRef>) -> std::process::ExitCode {
    let has_tray = tray.is_some();
    crate::cli::args::set_has_tray(has_tray);
    crate::cli::os_console::init(has_tray);
    init_tracing();
    let fd_note = crate::node::fd_limit::raise();
    match dispatch(fd_note, tray).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", crate::cli::cli_errors::error_report(&e));
            std::process::ExitCode::from(1)
        }
    }
}

/// Parses the arguments and executes the subcommand; `main` prints the error (`crate::cli::cli_errors::error_report`).
async fn dispatch(fd_note: Option<String>, tray: Option<TrayRef>) -> anyhow::Result<()> {
    #[cfg(not(feature = "gui"))]
    let _ = tray;
    if let Some(argv0) = std::env::args_os().next() {
        crate::cli::program_name::set_program_name(&argv0);
    }
    let matches = crate::cli::cli_errors::parse_or_exit(crate::cli::args::with_default_subcommand(
        std::env::args_os(),
        tray.is_some(),
    ));
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    match cli.command {
        CliCommand::Keygen { label, raw } => crate::cli::commands::keygen(label, raw),
        CliCommand::ConfigPath { config } => crate::cli::commands::config_path(config),
        CliCommand::Run(args) => {
            crate::node::fd_limit::report(fd_note);
            let m = matches.subcommand_matches("run").expect("run subcommand");
            crate::cli::commands::run_node_command(args, m).await
        }
        CliCommand::Password {
            config,
            web_port,
            action: crate::cli::args::PasswordAction::Reset,
        } => crate::cli::commands::password::run_password_command(config, web_port),
        CliCommand::Access {
            config,
            web_port,
            mode,
        } => crate::cli::commands::access::run_access_command(config, web_port, mode),
        CliCommand::Ifaces {
            names,
            if_unset,
            config,
            web_port,
        } => crate::cli::commands::ifaces::run_ifaces_command(config, web_port, names, if_unset),
        #[cfg(feature = "gui")]
        CliCommand::Tray(args) => {
            crate::node::fd_limit::report(fd_note);
            crate::gui::tray::app::tray_role::run_tray(args, require_tray(tray)?)
        }
    }
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
