//! The `meshtown` binary: a thin wrapper that hands the process over to the library entry point.
//!
//! What it does: calls `meshtown_core::cli::run::run`, which parses the command line and runs the subcommand.
//! What it doesn't do: it holds no logic of its own.

fn main() -> std::process::ExitCode {
    meshtown_core::cli::run::run()
}
