//! `network inspect`: tells what a key or an invite is, without joining anything.

use serde_json::{json, Value};

use crate::cli::args::{Global, Globals};
use crate::cli::commands::client::{print_json, Node};

pub(super) fn run(text: &str, g: &Globals) -> anyhow::Result<()> {
    g.allow("network inspect", &[Global::Json])?;
    let node = Node::connect(g)?;
    let reply = node.call("POST", "/api/network/inspect", Some(&json!({"key": text})))?;
    match g.json {
        true => print_json(&reply),
        false => lines(&reply).iter().for_each(|l| println!("{l}")),
    }
    Ok(())
}

/// `name: value` lines: the kind, the label (`-` when there is none) and the id; an invite adds where it goes.
fn lines(reply: &Value) -> Vec<String> {
    let kind = if reply.get("place").is_some() {
        "invite"
    } else {
        "key"
    };
    let label = reply["label"].as_str().unwrap_or("-");
    let id = reply["id_hex"].as_str().unwrap_or("-");
    let mut lines = vec![
        format!("kind: {kind}"),
        format!("label: {label}"),
        format!("id: {id}"),
    ];
    for name in ["place", "slot"] {
        if let Some(value) = reply[name].as_str() {
            lines.push(format!("{name}: {value}"));
        }
    }
    lines
}

#[cfg(test)]
#[path = "inspect_tests.rs"]
mod tests;
