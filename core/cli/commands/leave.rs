//! The `leave` subcommand: leaves a network and removes its data from this computer.

use crate::cli::args::{Global, Globals};

/// `leave` has no options of its own: the network is `--net`, the confirmation is `--yes`.
#[derive(clap::Args)]
pub(crate) struct LeaveArgs {}

pub(crate) async fn run(args: LeaveArgs, g: &Globals) -> anyhow::Result<()> {
    g.allow("leave", &[Global::Net, Global::Json, Global::Yes])?;
    g.require_yes("leave")?;
    let _ = args;
    anyhow::bail!("not implemented")
}
