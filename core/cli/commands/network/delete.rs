//! `network delete`: deletes the network for all its members (administrators only).

use serde_json::json;

use crate::cli::args::{Global, Globals};
use crate::cli::commands::client::Node;

pub(super) fn run(g: &Globals) -> anyhow::Result<()> {
    g.allow("network delete", &[Global::Net, Global::Yes])?;
    g.require_yes("network delete")?;
    let node = Node::connect(g)?;
    node.require_net("network delete")?;
    node.call(
        "POST",
        &node.net_path("network/delete"),
        Some(&json!({"confirm": true})),
    )?;
    eprintln!("Network deleted for everyone.");
    Ok(())
}
