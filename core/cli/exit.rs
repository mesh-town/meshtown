//! Process exit codes and the one form of error text.
//!
//! Commands return `anyhow::Result`; an error that carries a marker (`UsageError`, `NotRunning`) anywhere in its
//! chain decides the exit code: 2 for a wrong command line, 3 for no node on the page port, 1 for anything else.

use std::fmt;
use std::process::ExitCode;

use crate::node::output::NET_NAME;
use crate::proxy::BindError;

/// The command line is wrong in a way only the command knows (exit code 2). The text is complete, with the advice.
#[derive(Debug)]
pub(crate) struct UsageError(pub String);

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UsageError {}

/// Nothing answers on the page port (exit code 3).
#[derive(Debug)]
pub(crate) struct NotRunning {
    pub port: u16,
}

impl fmt::Display for NotRunning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{NET_NAME} isn't running on port {}. Start it ({} run, the service or the icon), or pass --web-port <PORT>.",
            self.port,
            crate::cli::program_name::program_name()
        )
    }
}

impl std::error::Error for NotRunning {}

/// The exit code of a failed command: the first marker in the chain decides, no marker means 1.
pub(crate) fn exit_code(e: &anyhow::Error) -> ExitCode {
    let code = e.chain().find_map(marker_code).unwrap_or(1);
    ExitCode::from(code)
}

fn marker_code(cause: &(dyn std::error::Error + 'static)) -> Option<u8> {
    match (
        cause.downcast_ref::<UsageError>(),
        cause.downcast_ref::<NotRunning>(),
    ) {
        (Some(_), _) => Some(2),
        (_, Some(_)) => Some(3),
        _ => None,
    }
}

/// The word error texts start with: a text that already begins with it gets no prefix.
pub(crate) const PREFIX: &str = "Error";

/// The report of a command error: `Error: reason: deeper`; if the proxy port is busy — a second line with advice.
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

#[cfg(test)]
#[path = "exit_tests.rs"]
mod tests;
