//! The `join` subcommand: joins a network with an invite or a network key, on the running node or in the settings file.

use crate::cli::args::Globals;

/// Arguments of `join`.
#[derive(clap::Args)]
pub(crate) struct JoinArgs {
    /// A member's invite (mesh-invite2-…) or a network key (words or mesh2-…).
    #[arg(value_name = "INVITE_OR_KEY", help_heading = "Options")]
    pub(crate) text: String,
    /// How long to wait for a member to answer an invite, in seconds (default: 120).
    #[arg(long, value_name = "SECONDS", default_value_t = 120, value_parser = clap::value_parser!(u64).range(1..=3600))]
    pub(crate) timeout: u64,
}

pub(crate) async fn run(args: JoinArgs, g: &Globals) -> anyhow::Result<()> {
    g.allow("join", &[])?;
    let _ = (args.text, args.timeout);
    anyhow::bail!("not implemented")
}
