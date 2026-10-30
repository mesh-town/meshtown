//! `network list`: the networks of this computer as a table, or the node's reply with `--json`.

use serde_json::Value;

use crate::cli::args::{Global, Globals};
use crate::cli::commands::client::{print_json, print_table, Node};

const HEAD: [&str; 6] = ["SLOT", "LABEL", "ID", "STATUS", "ROLE", "ONLINE"];

pub(super) fn run(g: &Globals) -> anyhow::Result<()> {
    g.allow("network list", &[Global::Json])?;
    let node = Node::connect(g)?;
    let reply = node.call("GET", "/api/nets", None)?;
    match g.json {
        true => print_json(&reply),
        false => print_table(&HEAD, &rows(&reply)?),
    }
    Ok(())
}

/// The table rows of a `GET /api/nets` reply.
fn rows(nets: &Value) -> anyhow::Result<Vec<Vec<String>>> {
    let list = nets["nets"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("the reply has no list of networks"))?;
    Ok(list.iter().map(row).collect())
}

fn row(net: &Value) -> Vec<String> {
    vec![
        text(&net["id"]),
        text(&net["label"]),
        short_id(&net["id_hex"]),
        status(net),
        role(net),
        format!(
            "{}/{}",
            count(&net["peers_online"]),
            count(&net["peers_total"])
        ),
    ]
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or("-").to_string()
}

fn count(v: &Value) -> u64 {
    v.as_u64().unwrap_or(0)
}

fn short_id(id_hex: &Value) -> String {
    id_hex
        .as_str()
        .map_or("-".to_string(), |h| h.chars().take(8).collect())
}

/// The first that applies: broken, excluded, pending, joining, otherwise the node's own word.
fn status(net: &Value) -> String {
    let flag = |key: &str| net[key].as_bool().unwrap_or(false);
    let own = net["status"].as_str().unwrap_or("-");
    let word = match (
        own == "broken",
        flag("me_excluded"),
        flag("me_pending"),
        flag("joining"),
    ) {
        (true, ..) => "broken",
        (_, true, ..) => "excluded",
        (_, _, true, _) => "pending",
        (_, _, _, true) => "joining",
        _ => own,
    };
    word.to_string()
}

/// `admin` or `member`; `-` for a slot with no network or a broken one.
fn role(net: &Value) -> String {
    let none = net["id_hex"].is_null() || net["status"] == "broken";
    match (none, net["me_admin"].as_bool().unwrap_or(false)) {
        (true, _) => "-",
        (_, true) => "admin",
        _ => "member",
    }
    .to_string()
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
