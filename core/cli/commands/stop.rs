//! The `stop` subcommand: stops the running node and waits until it is gone.

use crate::cli::args::Globals;

/// `stop` has no options of its own.
#[derive(clap::Args)]
pub(crate) struct StopArgs {}

pub(crate) async fn run(args: StopArgs, g: &Globals) -> anyhow::Result<()> {
    g.allow("stop", &[])?;
    let _ = args;
    anyhow::bail!("not implemented")
}
