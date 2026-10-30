//! The `invite` subcommand: prints a single-use invite to the network.

use crate::cli::args::{Global, Globals};

/// Arguments of `invite`.
#[derive(clap::Args)]
pub(crate) struct InviteArgs {
    /// A note for yourself about who the invite is for; it is shown in the list of invites.
    #[arg(long = "for", value_name = "NOTE")]
    pub(crate) note: Option<String>,
}

pub(crate) async fn run(args: InviteArgs, g: &Globals) -> anyhow::Result<()> {
    g.allow("invite", &[Global::Net, Global::Json])?;
    let _ = args.note;
    anyhow::bail!("not implemented")
}
