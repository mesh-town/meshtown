//! The `status` subcommand: the networks of the running node and whether they are online.

use std::time::{Duration, Instant};

use serde_json::Value;

use super::client::{print_json, Node};
use crate::cli::args::{Global, Globals};
use crate::cli::exit::NotRunning;
use crate::cli::program_name::program_name;

/// Arguments of `status`.
#[derive(clap::Args)]
pub(crate) struct StatusArgs {
    /// Wait until the network is online.
    #[arg(long)]
    pub(crate) wait_online: bool,
    /// How long to wait for the network, in seconds.
    #[arg(long, value_name = "SECONDS", default_value_t = 60, value_parser = clap::value_parser!(u64).range(1..=3600), requires = "wait_online")]
    pub(crate) timeout: u64,
}

/// What one look at the node means for `--wait-online`.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum WaitStep {
    Done,
    Wait,
    Fail(String),
}

pub(crate) async fn run(args: StatusArgs, g: &Globals) -> anyhow::Result<()> {
    g.allow("status", &[Global::Net, Global::Json])?;
    let node = match args.wait_online {
        true => wait_online(g, args.timeout).await?,
        false => Node::connect(g)?,
    };
    show(&node, g)
}

fn show(node: &Node, g: &Globals) -> anyhow::Result<()> {
    let overview = node.call("GET", "/api/overview", None)?;
    match g.json {
        true => print_json(&overview),
        false => print_lines(&overview, chosen_slot(node)),
    }
    Ok(())
}

/// The slot `--net` picked, `None` for all networks.
fn chosen_slot(node: &Node) -> Option<&str> {
    node.base
        .strip_prefix("/api/net/")
        .filter(|_| node.explicit_net)
}

fn print_lines(overview: &Value, slot: Option<&str>) {
    let nets = shown_nets(overview, slot);
    for line in status_lines(&nets) {
        println!("{line}");
    }
    if nets.iter().any(|n| n["id_hex"].is_null()) {
        eprintln!("Not in a network yet: {} join \"<invite>\"", program_name());
    }
}

fn shown_nets(overview: &Value, slot: Option<&str>) -> Vec<Value> {
    let all = overview["nets"].as_array().map_or(&[][..], Vec::as_slice);
    let wanted = |n: &&Value| slot.is_none_or(|s| n["id"].as_str() == Some(s));
    all.iter().filter(wanted).cloned().collect()
}

/// One tab-separated line per network: the label, the status, the peers online, then `exit` and `proxy` when set.
pub(super) fn status_lines(nets: &[Value]) -> Vec<String> {
    nets.iter().map(status_line).collect()
}

fn status_line(net: &Value) -> String {
    let label = net["label"].as_str().unwrap_or("(no network)");
    let status = net["status"].as_str().unwrap_or("unknown");
    let online = net["peers_online"].as_u64().unwrap_or(0);
    let total = net["peers_total"].as_u64().unwrap_or(0);
    let flags = [("exit_enabled", "exit"), ("proxy", "proxy")]
        .into_iter()
        .filter(|(key, _)| net[*key].as_bool() == Some(true))
        .map(|(_, word)| format!("\t{word}"))
        .collect::<String>();
    format!("{label}\t{status}\t{online}/{total} online{flags}")
}

/// Looks at the node once: is the wanted network online, still coming up, or never going to be.
pub(super) fn wait_step(nets: Result<&Value, &anyhow::Error>, slot: Option<&str>) -> WaitStep {
    match nets {
        Err(e) if e.chain().any(|c| c.is::<NotRunning>()) => WaitStep::Wait,
        Err(e) => WaitStep::Fail(e.to_string()),
        Ok(nets) => net_step(nets, slot),
    }
}

fn net_step(nets: &Value, slot: Option<&str>) -> WaitStep {
    let list = nets["nets"].as_array().map_or(&[][..], Vec::as_slice);
    let net = list
        .iter()
        .find(|n| slot.is_none_or(|s| n["id"].as_str() == Some(s)));
    match net {
        Some(n) if n["id_hex"].is_null() => WaitStep::Fail("no network to wait for".into()),
        Some(n) if n["status"].as_str() == Some("online") => WaitStep::Done,
        Some(_) => WaitStep::Wait,
        None => WaitStep::Fail("no network to wait for".into()),
    }
}

/// Polls once a second until the network is online. A node that never answered is `NotRunning`.
async fn wait_online(g: &Globals, seconds: u64) -> anyhow::Result<Node> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut seen = Seen::default();
    loop {
        let looked = Node::connect(g);
        let slot = looked.as_ref().ok().and_then(chosen_slot);
        match wait_step(looked.as_ref().map(|n| &n.nets), slot) {
            WaitStep::Done => return looked,
            WaitStep::Fail(why) => anyhow::bail!(why),
            WaitStep::Wait => seen.note(looked),
        }
        if Instant::now() >= deadline {
            return Err(seen.timed_out(seconds));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// What the polling saw so far: the status of the network, or why the node did not answer.
#[derive(Default)]
struct Seen {
    status: Option<String>,
    refused: Option<anyhow::Error>,
}

impl Seen {
    fn note(&mut self, looked: anyhow::Result<Node>) {
        match looked {
            Ok(node) => self.status = Some(status_text(&node)),
            Err(e) => self.refused = Some(e),
        }
    }

    fn timed_out(self, seconds: u64) -> anyhow::Error {
        match (self.status, self.refused) {
            (None, Some(e)) => e,
            (status, _) => anyhow::anyhow!(
                "not online after {seconds} s (status: {})",
                status.unwrap_or_else(|| "unknown".into())
            ),
        }
    }
}

fn status_text(node: &Node) -> String {
    let slot = chosen_slot(node);
    let nets = node.nets["nets"].as_array().map_or(&[][..], Vec::as_slice);
    let net = nets
        .iter()
        .find(|n| slot.is_none_or(|s| n["id"].as_str() == Some(s)));
    net.and_then(|n| n["status"].as_str())
        .unwrap_or("unknown")
        .to_string()
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
