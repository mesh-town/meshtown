//! Command line of the console app: argument parsing, subcommands, the interactive console and the process entry.
//!
//! - `args` - the clap argument definitions and the default subcommand.
//! - `cli_errors` - user-facing texts for command line errors.
//! - `commands` - the executors of the subcommands (`run`, `network`, `config`, ...).
//! - `exit` - exit codes and the one form of error text.
//! - `console` - the node console: commands from standard input.
//! - `os_console` - console output by process role and OS.
//! - `program_name` - the name the program was started under.
//! - `runtime` - the tokio runtime of the process entry, with a bounded exit.
//! - `run` - the process entry called by the `meshtown` binary (`main.rs`).

#[cfg(feature = "net-hyper")]
pub(crate) mod args;
#[cfg(feature = "net-hyper")]
pub(crate) mod cli_errors;
#[cfg(feature = "net-hyper")]
pub(crate) mod commands;
#[cfg(feature = "net-hyper")]
pub mod console;
#[cfg(feature = "net-hyper")]
pub(crate) mod exit;
#[cfg(feature = "net-hyper")]
pub(crate) mod os_console;
pub mod program_name;
#[cfg(feature = "net-hyper")]
pub mod run;
#[cfg(feature = "net-hyper")]
pub(crate) mod runtime;
#[cfg(feature = "net-hyper")]
pub(crate) mod stop_signals;
#[cfg(all(test, feature = "net-hyper"))]
pub(crate) mod test_args;
