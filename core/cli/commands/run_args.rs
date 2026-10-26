//! Arguments of the `run` subcommand (see `commands.rs`, `cli.rs`).

/// Arguments of the `run` subcommand, parsed by `clap` directly (they used to be fields of `cli::CliCommand::Run`,
/// now the variant's only field is `Run(RunArgs)`).
#[derive(clap::Args)]
pub(crate) struct RunArgs {
    /// Invite to a network: a single-use string mesh-invite2-… from a running member (can be set with the MESHTOWN_INVITE variable).
    #[arg(long, env = "MESHTOWN_INVITE", hide_env = true, value_name = "TEXT")]
    pub(crate) invite: Option<String>,
    /// Network key: "label--word-word-group" or mesh1-… (can be set with the MESHTOWN_KEY variable) (default: from the settings file).
    #[arg(long, env = "MESHTOWN_KEY", hide_env = true, value_name = "KEY")]
    pub(crate) key: Option<String>,
    /// Settings file (JSON); can be set with the MESHTOWN_CONFIG variable (default: next to the program or in the standard location).
    #[arg(long, value_name = "PATH", env = "MESHTOWN_CONFIG", hide_env = true)]
    pub(crate) config: Option<std::path::PathBuf>,
    /// Do not read or write the settings file: only defaults and flags.
    #[arg(long, conflicts_with = "save_config")]
    pub(crate) no_config: bool,
    /// Write the flags you typed to the settings file and keep running.
    #[arg(long)]
    pub(crate) save_config: bool,
    /// How often to test speed to members, in seconds (default: 300).
    #[arg(long, value_name = "SECONDS", value_parser = clap::value_parser!(u64).range(5..=86400))]
    pub(crate) probe_interval: Option<u64>,
    /// Test speed with 1 MiB to all members, not only the chosen exit (for testing).
    #[arg(long)]
    pub(crate) probe_full_cycle: bool,
    /// Add a site to your list (for example example.com); it is checked at startup. Can be repeated.
    #[arg(long = "domain", value_name = "SITE")]
    pub(crate) initial_domains: Vec<String>,
    /// Interval of the site check, in seconds (default: 900).
    #[arg(long, value_name = "SECONDS", value_parser = clap::value_parser!(u64).range(30..=86400))]
    pub(crate) check_interval: Option<u64>,
    /// DNS-over-HTTPS server for site checks, ADDR:PORT (default: 1.1.1.1:443).
    #[arg(long, value_name = "ADDR:PORT")]
    pub(crate) check_doh: Option<std::net::SocketAddr>,
    /// Your ISP's DNS server, used to detect spoofed site addresses, ADDR:PORT.
    #[arg(long, value_name = "ADDR:PORT")]
    pub(crate) provider_dns: Option<std::net::SocketAddr>,
    /// Check whether sites from your list are blocked.
    #[arg(long, overrides_with = "no_checks")]
    pub(crate) checks: bool,
    /// Do not check sites, only keep the list.
    #[arg(long, overrides_with = "checks")]
    pub(crate) no_checks: bool,
    /// Port of the local proxy (SOCKS5 and HTTP on one port) on 127.0.0.1 or in the local network, see --socks-access (default: 1080).
    #[arg(long, value_name = "PORT", value_parser = clap::value_parser!(u16).range(1024..))]
    pub(crate) socks_port: Option<u16>,
    /// Who can reach the proxy: `localhost` (this computer only) or `localnet` (the local network too).
    #[arg(long, value_name = "MODE", value_parser = crate::cli::args::parse_access)]
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
    #[arg(long = "block-domain", value_name = "SITE")]
    pub(crate) blocked_domains: Vec<String>,
    /// Port of the Mesh Town page on 127.0.0.1 (default: 7777).
    #[arg(long, value_name = "PORT")]
    pub(crate) web_port: Option<u16>,
}
