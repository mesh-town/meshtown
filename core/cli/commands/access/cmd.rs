//! The command line of the `access` group: the parsed types and the mode parser.

use crate::config::config_file::ListenAccess;

/// `access` with an optional action: no action shows the current values.
#[derive(clap::Args)]
pub(crate) struct AccessArgs {
    #[command(subcommand)]
    pub(crate) cmd: Option<AccessCmd>,
}

#[derive(clap::Subcommand)]
pub(crate) enum AccessCmd {
    /// Who can open the Mesh Town page: localhost (this computer only) or localnet (the local network too)
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown access page <localhost|localnet>")]
    Page {
        /// Mode: localhost or localnet.
        #[arg(value_name = "MODE", value_parser = parse_access, help_heading = "Options")]
        mode: ListenAccess,
    },
    /// Who can use the Mesh Town proxy: localhost (this computer only) or localnet (the local network too)
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown access proxy <localhost|localnet>")]
    Proxy {
        /// Mode: localhost or localnet.
        #[arg(value_name = "MODE", value_parser = parse_access, help_heading = "Options")]
        mode: ListenAccess,
    },
    /// Which networks Mesh Town accepts connections from: interface names, auto (automatic) or '*' (no restriction)
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown access ifaces <NAME>... [--if-unset]")]
    Ifaces {
        /// Interface names (br-lan), the single word auto (automatic) or '*' (no restriction).
        #[arg(value_name = "NAME", required = true, num_args = 1.., help_heading = "Options")]
        names: Vec<String>,
        /// Write only if the file has neither socks.ifaces nor web.ifaces yet (for autostart).
        #[arg(long)]
        if_unset: bool,
    },
}

/// Parses an access mode: the error names the allowed values.
pub(crate) fn parse_access(s: &str) -> Result<ListenAccess, String> {
    ListenAccess::parse(s).ok_or_else(|| "allowed: localhost, localnet".to_string())
}

#[cfg(test)]
#[path = "cmd_tests.rs"]
mod tests;
