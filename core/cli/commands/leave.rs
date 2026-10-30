//! The `leave` subcommand: leaves a network and removes its data from this computer.

use serde_json::{json, Value};

use super::client::{print_json, Node};
use crate::cli::args::{Global, Globals};

/// `leave` has no options of its own: the network is `--net`, the confirmation is `--yes`.
#[derive(clap::Args)]
pub(crate) struct LeaveArgs {}

pub(crate) async fn run(args: LeaveArgs, g: &Globals) -> anyhow::Result<()> {
    g.allow("leave", &[Global::Net, Global::Json, Global::Yes])?;
    g.require_yes("leave")?;
    let _ = args;
    let node = Node::connect(g)?;
    node.require_net("leave")?;
    let label = net_label(&node);
    let reply = node.call(
        "POST",
        &node.net_path("members/leave"),
        Some(&json!({"confirm": true})),
    )?;
    match g.json {
        true => print_json(&reply),
        false => eprintln!("Left {label}."),
    }
    Ok(())
}

/// The label of the network being left: the one `--net` named, otherwise the first.
fn net_label(node: &Node) -> String {
    let slot = node
        .base
        .strip_prefix("/api/net/")
        .filter(|_| node.explicit_net);
    let nets = node.nets["nets"].as_array().map_or(&[][..], Vec::as_slice);
    let net = nets
        .iter()
        .find(|n| slot.is_none_or(|s| n["id"].as_str() == Some(s)));
    label_text(net)
}

fn label_text(net: Option<&Value>) -> String {
    let label = net.and_then(|n| n["label"].as_str());
    label.map_or_else(|| "the network".to_string(), str::to_string)
}

#[cfg(test)]
#[path = "leave_tests.rs"]
mod tests;
