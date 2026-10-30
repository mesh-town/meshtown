//! The `status` subcommand: the networks of the running node and whether they are online.

use crate::cli::args::{Global, Globals};

/// Arguments of `status`.
#[derive(clap::Args)]
pub(crate) struct StatusArgs {
    /// Wait until the network is online.
    #[arg(long)]
    pub(crate) wait_online: bool,
    /// How long to wait for the network, in seconds (default: 60).
    #[arg(long, value_name = "SECONDS", default_value_t = 60, value_parser = clap::value_parser!(u64).range(1..=3600), requires = "wait_online")]
    pub(crate) timeout: u64,
}

pub(crate) async fn run(args: StatusArgs, g: &Globals) -> anyhow::Result<()> {
    g.allow("status", &[Global::Net, Global::Json])?;
    let _ = (args.wait_online, args.timeout);
    anyhow::bail!("not implemented")
}
