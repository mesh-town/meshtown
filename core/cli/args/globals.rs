//! The options every command accepts after or before its name (`global = true`), and the checks a command
//! makes on the ones it doesn't use.

use std::path::PathBuf;

use crate::cli::exit::UsageError;
use crate::cli::program_name::program_name;

/// The option group heading in the help.
pub(crate) const HEADING: &str = "Global options";

/// Options shared by all commands.
#[derive(clap::Args, Clone, Debug, Default)]
pub(crate) struct Globals {
    /// Settings file (JSON); can be set with the MESHTOWN_CONFIG variable (default: next to the program or in the standard location).
    #[arg(long, global = true, value_name = "PATH", env = "MESHTOWN_CONFIG", hide_env = true, help_heading = HEADING)]
    pub(crate) config: Option<PathBuf>,
    /// Port of the Mesh Town page on 127.0.0.1 (default: from the settings file, then 7777; 0 picks a free port for run).
    #[arg(long, global = true, value_name = "PORT", help_heading = HEADING)]
    pub(crate) web_port: Option<u16>,
    /// The network to act on, by its label, its id or the start of its id (default: the first network).
    #[arg(long, global = true, value_name = "NETWORK", help_heading = HEADING)]
    pub(crate) net: Option<String>,
    /// Print the node's reply as one line of JSON.
    #[arg(long, global = true, help_heading = HEADING)]
    pub(crate) json: bool,
    /// Confirm a command that removes data.
    #[arg(long, global = true, help_heading = HEADING)]
    pub(crate) yes: bool,
}

/// The global options a command may refuse (`--config` and `--web-port` are used by every command).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Global {
    Net,
    Json,
    Yes,
}

impl Global {
    fn flag(self) -> &'static str {
        match self {
            Global::Net => "--net",
            Global::Json => "--json",
            Global::Yes => "--yes",
        }
    }
}

/// The text that sends a person to the command's help.
fn see_help(command: &str) -> String {
    let prog = program_name();
    format!("See: {prog} {command} --help")
}

impl Globals {
    fn is_set(&self, which: Global) -> bool {
        match which {
            Global::Net => self.net.is_some(),
            Global::Json => self.json,
            Global::Yes => self.yes,
        }
    }

    /// Refuses `--web-port 0` outside `run`, and any of `--net`, `--json`, `--yes` that the command doesn't use.
    pub(crate) fn allow(&self, command: &str, allowed: &[Global]) -> anyhow::Result<()> {
        self.check_web_port(command)?;
        let unused = [Global::Net, Global::Json, Global::Yes]
            .into_iter()
            .find(|g| self.is_set(*g) && !allowed.contains(g));
        match unused {
            Some(g) => Err(unused_error(g, command)),
            None => Ok(()),
        }
    }

    /// A removing command needs `--yes`; there is no question to answer.
    pub(crate) fn require_yes(&self, command: &str) -> anyhow::Result<()> {
        match self.yes {
            true => Ok(()),
            false => Err(anyhow::Error::new(UsageError(format!(
                "{} {command} removes data; add --yes to confirm. {}",
                program_name(),
                see_help(command)
            )))),
        }
    }

    fn check_web_port(&self, command: &str) -> anyhow::Result<()> {
        match (self.web_port, command) {
            (Some(0), "run") | (None | Some(1..), _) => Ok(()),
            (Some(0), _) => Err(anyhow::Error::new(UsageError(format!(
                "--web-port 0 isn't used by {} {command}: use a port from 1 to 65535. {}",
                program_name(),
                see_help(command)
            )))),
        }
    }
}

fn unused_error(g: Global, command: &str) -> anyhow::Error {
    anyhow::Error::new(UsageError(format!(
        "{} isn't used by {} {command}. {}",
        g.flag(),
        program_name(),
        see_help(command)
    )))
}

#[cfg(test)]
#[path = "globals_tests.rs"]
mod tests;
