//! The process entry of the Mesh Town console app (called by the `meshtown` binary).
//!
//! What it does: parses the command line arguments (`cli.rs`) and passes them to the subcommand
//! executor (`commands.rs`): `run`, `join`, `status`, `stop` and the other commands.
//! What it doesn't do: it holds no node business logic — that is in the `meshtown` library;
//! it prints the network key nowhere except `keygen`, which creates it.
//! Starting with no arguments substitutes one itself (`crate::cli::args::with_default_subcommand`):
//! the help when the process has no tray backend, `tray` when the platform binary handed one in.
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

/// Sets up log output (`tracing`) to stderr; the filter comes from `RUST_LOG`
/// (`target=level` list), warnings and above when it is unset or invalid.
fn init_tracing() {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};
    let filter = log_filter(std::env::var("RUST_LOG").ok().as_deref());
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();
}

/// Parses a `RUST_LOG` value; `None` or a parse error gives `warn` for every target.
fn log_filter(raw: Option<&str>) -> tracing_subscriber::filter::Targets {
    use tracing_subscriber::filter::{LevelFilter, Targets};
    let fallback = || Targets::new().with_default(LevelFilter::WARN);
    raw.and_then(|v| v.parse::<Targets>().ok())
        .unwrap_or_else(fallback)
}

/// The process entry without a tray backend: no command prints the help.
pub fn run() -> std::process::ExitCode {
    start(None)
}

/// The process entry of a platform binary that owns a tray backend: no command means `tray`.
#[cfg(feature = "gui")]
pub fn run_with_tray(tray: &'static dyn crate::gui::tray::TrayBackend) -> std::process::ExitCode {
    start(Some(tray))
}

/// Runs the process on its own runtime, which does not wait long for stuck blocking threads at exit.
fn start(tray: Option<TrayRef>) -> std::process::ExitCode {
    crate::cli::runtime::run_to_exit(start_async(tray))
}

async fn start_async(tray: Option<TrayRef>) -> std::process::ExitCode {
    let has_tray = tray.is_some();
    crate::cli::args::set_has_tray(has_tray);
    crate::cli::os_console::init(has_tray);
    init_tracing();
    let fd_note = crate::node::fd_limit::raise();
    match dispatch(fd_note, tray).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", crate::cli::exit::error_report(&e));
            crate::cli::exit::exit_code(&e)
        }
    }
}

/// Parses the arguments and executes the subcommand; `start_async` prints the error and picks the exit code.
async fn dispatch(fd_note: Option<String>, tray: Option<TrayRef>) -> anyhow::Result<()> {
    if let Some(argv0) = std::env::args_os().next() {
        crate::cli::program_name::set_program_name(&argv0);
    }
    let matches = crate::cli::cli_errors::parse_or_exit(crate::cli::args::with_default_subcommand(
        std::env::args_os(),
        tray.is_some(),
    ));
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    execute(cli, &matches, fd_note, tray).await
}

/// Runs the command the line named. Each command refuses the global options it doesn't use.
async fn execute(
    cli: Cli,
    matches: &clap::ArgMatches,
    fd_note: Option<String>,
    tray: Option<TrayRef>,
) -> anyhow::Result<()> {
    #[cfg(not(feature = "gui"))]
    let _ = tray;
    let g = &cli.globals;
    match cli.command {
        CliCommand::Run(args) => {
            g.allow("run", &[])?;
            report_run_start(fd_note);
            let m = matches.subcommand_matches("run").expect("run subcommand");
            crate::cli::commands::run_node_command(args.with_globals(g), m).await
        }
        CliCommand::Join(args) => crate::cli::commands::join::run(args, g).await,
        CliCommand::Status(args) => crate::cli::commands::status::run(args, g).await,
        CliCommand::Stop(args) => crate::cli::commands::stop::run(args, g).await,
        CliCommand::Invite(args) => crate::cli::commands::invite::run(args, g).await,
        CliCommand::Leave(args) => crate::cli::commands::leave::run(args, g).await,
        // group: access
        CliCommand::Access(args) => crate::cli::commands::access::run(args, g).await,
        // group: config
        // group: member
        // group: network
        CliCommand::Password {
            action: crate::cli::args::PasswordAction::Reset,
        } => crate::cli::commands::password::run_password_command(g).await,
        CliCommand::Open { print } => crate::cli::commands::open::run_open_command(print, g).await,
        CliCommand::ConfigPath => {
            g.allow("config-path", &[])?;
            crate::cli::commands::config_path(g.config.clone())
        }
        CliCommand::Keygen { label, raw } => {
            g.allow("keygen", &[])?;
            crate::cli::commands::keygen(label, raw, g.config.clone())
        }
        #[cfg(feature = "gui")]
        CliCommand::Tray(args) => {
            g.allow("tray", &[])?;
            crate::node::fd_limit::report(fd_note);
            crate::gui::tray::app::tray_role::run_tray(args, g, require_tray(tray)?)
        }
    }
}

/// First log lines of `run`: the program version, then the raised open-files limit (if it changed).
fn report_run_start(fd_note: Option<String>) {
    crate::node::output::say(
        "node",
        format!(
            "meshtown {}",
            crate::node::program_version::current_program_version()
        ),
    );
    crate::node::fd_limit::report(fd_note);
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
