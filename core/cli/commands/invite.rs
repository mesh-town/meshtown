//! The `invite` subcommand: prints a single-use invite to the network.

use super::client::{print_json, query_escape, Node};
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
    let node = Node::connect(g)?;
    let reply = node.call("GET", &invite_path(&node, args.note.as_deref()), None)?;
    match g.json {
        true => print_json(&reply),
        false => print_invite(&reply)?,
    }
    Ok(())
}

/// `{base}/network/invite`, with the note as `for` when there is one.
fn invite_path(node: &Node, note: Option<&str>) -> String {
    let path = node.net_path("network/invite");
    match note {
        Some(note) => format!("{path}?for={}", query_escape(note)),
        None => path,
    }
}

fn print_invite(reply: &serde_json::Value) -> anyhow::Result<()> {
    let invite = reply["invite"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("the reply has no invite"))?;
    println!("{invite}");
    eprintln!("Single-use, valid for a day: hand it to one person.");
    Ok(())
}

#[cfg(test)]
#[path = "invite_tests.rs"]
mod tests;
