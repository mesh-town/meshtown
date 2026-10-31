//! Arguments of the `run` subcommand (see `commands.rs`, `cli.rs`).

use crate::cli::args::Globals;

/// Arguments of the `run` subcommand, parsed by `clap` directly (they used to be fields of `cli::CliCommand::Run`,
/// now the variant's only field is `Run(RunArgs)`).
#[derive(clap::Args)]
pub(crate) struct RunArgs {
    /// Invite to a network: a single-use string mesh-invite2-… from a running member (can be set with the MESHTOWN_INVITE variable).
    #[arg(long, env = "MESHTOWN_INVITE", hide_env = true, value_name = "TEXT")]
    pub(crate) invite: Option<String>,
    /// Network key: a words key or mesh2-… (can be set with the MESHTOWN_KEY variable) (default: from the settings file).
    #[arg(long, env = "MESHTOWN_KEY", hide_env = true, value_name = "KEY")]
    pub(crate) key: Option<String>,
    /// The settings file: filled from the global `--config` by `with_globals`.
    #[arg(skip)]
    pub(crate) config: Option<std::path::PathBuf>,
    /// Do not read or write the settings file: only defaults and flags.
    #[arg(long)]
    pub(crate) no_config: bool,
    /// Add a site to your list (for example example.com); it is checked at startup. Can be repeated.
    #[arg(long = "site", value_name = "SITE")]
    pub(crate) initial_domains: Vec<String>,
    /// Port of the local proxy (SOCKS5 and HTTP on one port) on 127.0.0.1 or in the local network, see --socks-access (default: 1080).
    #[arg(long, value_name = "PORT", value_parser = clap::value_parser!(u16).range(1024..))]
    pub(crate) socks_port: Option<u16>,
    /// Who can reach the proxy: `localhost` (this computer only) or `localnet` (the local network too).
    #[arg(long, value_name = "MODE", value_parser = crate::cli::commands::access::parse_access)]
    pub(crate) socks_access: Option<crate::config::config_file::ListenAccess>,
    /// Start the local proxy (SOCKS5 and HTTP).
    #[arg(long, overrides_with = "no_socks")]
    pub(crate) socks: bool,
    /// Do not start the local proxy (SOCKS5 and HTTP).
    #[arg(long, overrides_with = "socks")]
    pub(crate) no_socks: bool,
    /// Let other members open sites via this computer (be an exit).
    #[arg(long, overrides_with = "no_exit")]
    pub(crate) exit: bool,
    /// Don't let other members open sites via this computer.
    #[arg(long, overrides_with = "exit")]
    pub(crate) no_exit: bool,
    /// Treat a site as blocked for you, without checking. Can be repeated.
    #[arg(long = "block-site", value_name = "SITE")]
    pub(crate) blocked_domains: Vec<String>,
    /// Set a setting for this run, repeatable (quote strings that look like JSON). The tray reads the proxy port from --socks-port only.
    #[arg(long = "set", value_name = "PATH=VALUE")]
    pub(crate) set: Vec<String>,
    /// The page port: filled from the global `--web-port` by `with_globals`.
    #[arg(skip)]
    pub(crate) web_port: Option<u16>,
}

impl RunArgs {
    /// Takes the settings file and the page port from the global options.
    pub(crate) fn with_globals(self, g: &Globals) -> RunArgs {
        RunArgs {
            config: g.config.clone(),
            web_port: g.web_port,
            ..self
        }
    }
}
