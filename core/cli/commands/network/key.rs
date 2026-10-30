//! `network key`: prints the key of the network, so a person can join with it.

use serde_json::Value;

use super::key_form::raw_form;
use crate::cli::args::{Global, Globals};
use crate::cli::commands::client::{print_json, Node};
use crate::cli::exit::UsageError;
use crate::cli::program_name::program_name;

pub(super) fn run(raw: bool, g: &Globals) -> anyhow::Result<()> {
    g.allow("network key", &[Global::Net, Global::Json])?;
    refuse_raw_json(raw, g)?;
    let node = Node::connect(g)?;
    let reply = node.call("GET", &node.net_path("network/key"), None)?;
    let key = key_of(&reply)?;
    match (g.json, raw) {
        (true, _) => print_json(&reply),
        (_, true) => println!("{}", raw_form(&key)?),
        _ => println!("{key}"),
    }
    Ok(())
}

/// `--raw` changes the key and `--json` prints the node's reply as it is: both cannot hold.
fn refuse_raw_json(raw: bool, g: &Globals) -> anyhow::Result<()> {
    match raw && g.json {
        true => Err(anyhow::Error::new(UsageError(format!(
            "--json prints the node's answer as it is, so --raw can't go with it. See: {} network key --help",
            program_name()
        )))),
        false => Ok(()),
    }
}

/// The key of the reply; a network that has none is an error.
fn key_of(reply: &Value) -> anyhow::Result<String> {
    match reply["key"].as_str() {
        Some(key) => Ok(key.to_string()),
        None => anyhow::bail!(
            "this node has no network yet: create one with `{0} network create <label>` or join with `{0} join`",
            program_name()
        ),
    }
}
