//! The command line of the `network` group: the parsed types.

/// `network` and its action.
#[derive(clap::Args)]
pub(crate) struct NetworkArgs {
    #[command(subcommand)]
    pub(crate) cmd: NetworkCmd,
}

#[derive(clap::Subcommand)]
pub(crate) enum NetworkCmd {
    /// List the networks of this computer
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown network list")]
    List,
    /// Create a network with this computer as its administrator (a second one goes to a new slot)
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown network create [--raw] [--key-only] <LABEL>…")]
    Create {
        /// A network label, words separated by spaces: meshtown network create our game.
        #[arg(value_name = "LABEL", help_heading = "Options")]
        label: Vec<String>,
        /// Print the machine key mesh2-… instead of the key with words.
        #[arg(long)]
        raw: bool,
        /// Only print a new key, with no running node: the settings file gives the administrator.
        #[arg(long)]
        key_only: bool,
    },
    /// Print the key of the network, to hand to a person who joins with it
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown network key [--raw]")]
    Key {
        /// Print the machine key mesh2-… instead of the key as it is saved.
        #[arg(long)]
        raw: bool,
    },
    /// Show what a key or an invite is: the network label, its id and where it leads
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown network inspect <KEY_OR_INVITE>")]
    Inspect {
        /// A network key (words or mesh2-…) or an invite.
        #[arg(value_name = "KEY_OR_INVITE", help_heading = "Options")]
        text: String,
    },
    /// Delete the network for everyone (administrators only)
    #[command(help_template = crate::cli::args::HELP, next_help_heading = "Options", override_usage = "meshtown network delete --yes [--net <NETWORK>]")]
    Delete,
}
