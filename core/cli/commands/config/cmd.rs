//! The command line of the `config` group: the parsed types.

/// `config` and its action.
#[derive(clap::Args)]
pub(crate) struct ConfigArgs {
    #[command(subcommand)]
    pub(crate) cmd: ConfigCmd,
}

#[derive(clap::Subcommand)]
pub(crate) enum ConfigCmd {
    /// Print the path to the settings file
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown config path [--config <PATH>]")]
    Path,
    /// Show one setting or all of them, from the running Mesh Town or from the settings file
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown config get [<PATH>] [--net <NETWORK>] [--json]")]
    Get {
        /// A setting such as probe.interval_sec; without it, all settings.
        #[arg(value_name = "PATH", help_heading = "Options")]
        path: Option<String>,
    },
    /// Change one setting in the running Mesh Town, or in the settings file when it is not running
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown config set <PATH> <VALUE> [--net <NETWORK>] [--json]")]
    Set {
        /// The setting, such as probe.interval_sec.
        #[arg(value_name = "PATH", help_heading = "Options")]
        path: String,
        /// The new value; quote strings that look like JSON.
        #[arg(
            value_name = "VALUE",
            help_heading = "Options",
            allow_hyphen_values = true
        )]
        value: String,
    },
}
