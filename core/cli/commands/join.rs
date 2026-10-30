//! The `join` subcommand: joins a network with an invite or a network key, on the running node or in the settings file.
//! What is not here: joining without a node (`join_offline.rs`) and the exchange itself (`run_invite.rs`).

use std::time::Duration;

use serde_json::{json, Value};
use tokio::time::{sleep, Instant};

use super::client::{ApiError, Node, Target};
use super::join_offline::{busy_text, run_offline, JoinText};
use crate::cli::args::Globals;
use crate::cli::exit::{NotRunning, UsageError};
use crate::cli::program_name::program_name;

/// Arguments of `join`.
#[derive(clap::Args)]
pub(crate) struct JoinArgs {
    /// A member's invite (mesh-invite2-…) or a network key (words or mesh2-…).
    #[arg(value_name = "INVITE_OR_KEY", help_heading = "Options")]
    pub(crate) text: String,
    /// How long to wait for a member to answer an invite, in seconds.
    #[arg(long, value_name = "SECONDS", default_value_t = 120, value_parser = clap::value_parser!(u64).range(1..=3600))]
    pub(crate) timeout: u64,
}

/// How long the node is polled between two looks at the join state.
const POLL_EVERY: Duration = Duration::from_secs(1);
/// How often the wait is mentioned on stderr.
const REMIND_EVERY: u64 = 15;

pub(crate) async fn run(args: JoinArgs, g: &Globals) -> anyhow::Result<()> {
    g.allow("join", &[])?;
    let text = parse_text(&args.text)?;
    match Node::connect(g) {
        Ok(node) => Box::pin(join_live(&node, &args, &text)).await,
        Err(e) if is_not_running(&e) => Box::pin(run_offline(g, text, args.timeout)).await,
        Err(e) if is_dropped_link(&e) => Err(page_port_busy(g, e)),
        Err(e) => Err(e),
    }
}

/// An invite or a key, parsed; a text that is neither is a usage error, an old-format key keeps its own message.
fn parse_text(text: &str) -> anyhow::Result<JoinText> {
    if crate::invite::looks_like_invite(text) {
        return Ok(JoinText::Invite(crate::invite::parse(text)?));
    }
    match crate::members::network_key::parse_user_input(text) {
        Ok(_) => Ok(JoinText::Key(text.trim().to_string())),
        Err(e)
            if e.downcast_ref::<crate::members::network_key::LegacyKey>()
                .is_some() =>
        {
            Err(e)
        }
        Err(_) => Err(anyhow::Error::new(UsageError(not_joinable_text()))),
    }
}

fn not_joinable_text() -> String {
    format!(
        "this is neither an invite nor a network key. Ask a member of the network for an invite: `{} invite` on their node",
        program_name()
    )
}

/// Hex identifier of the network a key belongs to; none for an invite, whose network is known after the exchange.
fn key_network(text: &JoinText) -> Option<String> {
    match text {
        JoinText::Key(key) => crate::members::network_key::parse_user_input(key)
            .ok()
            .map(|k| k.key.subkeys().network_id_hex()),
        JoinText::Invite(_) => None,
    }
}

/// Whether a slot in the `/api/nets` reply already works with the network `net_hex`.
pub(super) fn network_listed(nets: &Value, net_hex: &str) -> bool {
    let list = nets.get("nets").unwrap_or(nets);
    let listed = list.as_array().into_iter().flatten();
    let known = listed.filter_map(|n| n["id_hex"].as_str());
    known
        .filter(|hex| !hex.is_empty())
        .any(|hex| net_hex.starts_with(hex))
}

fn already_joined(node: &Node, text: &JoinText) -> anyhow::Result<bool> {
    let Some(net) = key_network(text) else {
        return Ok(false);
    };
    let nets = node.call("GET", "/api/nets", None)?;
    Ok(network_listed(&nets, &net))
}

fn is_not_running(e: &anyhow::Error) -> bool {
    e.chain().any(|c| c.downcast_ref::<NotRunning>().is_some())
}

/// Whether the connection to the page port was reset, closed or answered by something that is not a node:
/// another `join` holds the port and has just let go of it.
fn is_dropped_link(e: &anyhow::Error) -> bool {
    let text = format!("{e:#}").to_lowercase();
    [
        "reset by peer",
        "broken pipe",
        "connection aborted",
        "did not reply over http",
    ]
    .iter()
    .any(|sign| text.contains(sign))
}

/// The error of a busy page port: the same words as when `join` cannot hold the port itself.
fn page_port_busy(g: &Globals, cause: anyhow::Error) -> anyhow::Error {
    match Target::resolve(g.config.clone(), g.web_port) {
        Ok(target) => anyhow::anyhow!(busy_text(target.port)),
        Err(_) => cause,
    }
}

/// What a refusal of the join request means for the command.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum LiveAnswer {
    /// The main tab already has another network: join in a new network slot.
    AddSlot,
    /// The network is already on the node: nothing to do.
    Already,
    /// Any other refusal.
    Fail,
}

pub(super) fn live_answer(e: &anyhow::Error) -> LiveAnswer {
    let code = e
        .chain()
        .find_map(|c| c.downcast_ref::<ApiError>())
        .map(|a| a.code.as_str());
    match code {
        Some("network_present") => LiveAnswer::AddSlot,
        Some("net_already_added") => LiveAnswer::Already,
        _ => LiveAnswer::Fail,
    }
}

/// The result of one join request.
enum Attempt {
    Accepted(Value),
    NeedSlot,
    Already,
}

fn attempt(node: &Node, path: &str, body: &Value) -> anyhow::Result<Attempt> {
    match node.call("POST", path, Some(body)) {
        Ok(reply) => Ok(Attempt::Accepted(reply)),
        Err(e) => match live_answer(&e) {
            LiveAnswer::AddSlot => Ok(Attempt::NeedSlot),
            LiveAnswer::Already => Ok(Attempt::Already),
            LiveAnswer::Fail => Err(e),
        },
    }
}

/// Sends the join to the main tab, and to a new network slot when the main tab has another network.
fn request_join(node: &Node, text: &str) -> anyhow::Result<Option<Value>> {
    let body = json!({ "key": text });
    let first = attempt(node, "/api/network/join", &body)?;
    let second = match first {
        Attempt::NeedSlot => {
            let slot = new_slot(node)?;
            let tried = attempt(node, &format!("/api/net/{slot}/network/join"), &body);
            match &tried {
                Ok(Attempt::Accepted(_)) => {}
                _ => drop_slot(node, &slot),
            }
            tried?
        }
        other => other,
    };
    match second {
        Attempt::Accepted(reply) => Ok(Some(reply)),
        Attempt::Already => Ok(None),
        Attempt::NeedSlot => anyhow::bail!("the new network slot already has a network"),
    }
}

/// Removes the empty slot made for a join that was not accepted; a failure here is not worth reporting.
fn drop_slot(node: &Node, slot: &str) {
    let path = format!("/api/net/{slot}/members/leave");
    let _ = node.call("POST", &path, Some(&json!({ "confirm": true })));
}

fn new_slot(node: &Node) -> anyhow::Result<String> {
    let reply = node.call("POST", "/api/nets", Some(&json!({})))?;
    let slot = reply["slot"].as_str();
    slot.map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("the reply has no network slot"))
}

async fn join_live(node: &Node, args: &JoinArgs, text: &JoinText) -> anyhow::Result<()> {
    if already_joined(node, text)? {
        eprintln!("already in this network");
        return Ok(());
    }
    let Some(reply) = request_join(node, args.text.trim())? else {
        eprintln!("already in this network");
        return Ok(());
    };
    let label = reply["label"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| text.label());
    match (text, reply["waiting"].as_bool()) {
        (JoinText::Invite(inv), Some(true)) => {
            let slot = reply["slot"].as_str().unwrap_or("main").to_string();
            let net = data_encoding::HEXLOWER.encode(&inv.head.network_id);
            wait_joined(node, &slot, &net, args.timeout).await?;
        }
        _ => eprintln!("Joined; Mesh Town reconnects to the new network."),
    }
    println!("{label}");
    Ok(())
}

/// What the join state says about the invite.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum PollStep {
    Wait,
    Joined,
    Fail(String),
}

/// Reads `node.invite_join` of the slot state. When it is gone the join succeeded only if the slot now holds
/// the network of the invite (`invite_net` is its hex identifier); otherwise the join was replaced or stopped.
pub(super) fn poll_step(state: &Value, nets: &Value, slot: &str, invite_net: &str) -> PollStep {
    let join = &state["node"]["invite_join"];
    match join["state"].as_str() {
        Some("refused") => PollStep::Fail(refusal_text(join)),
        Some(_) => PollStep::Wait,
        None if slot_holds(nets, slot, invite_net) => PollStep::Joined,
        None => PollStep::Fail("the join was cancelled (another join or a stop)".to_string()),
    }
}

fn refusal_text(join: &Value) -> String {
    match join["message"].as_str() {
        Some(m) => m.to_string(),
        None => "the members refused the invite".to_string(),
    }
}

fn slot_holds(nets: &Value, slot: &str, invite_net: &str) -> bool {
    let list = nets.get("nets").unwrap_or(nets);
    let found = list
        .as_array()
        .into_iter()
        .flatten()
        .find(|n| n["id"].as_str() == Some(slot));
    let hex = found.and_then(|n| n["id_hex"].as_str()).unwrap_or("");
    !invite_net.is_empty() && hex.starts_with(invite_net)
}

/// Polls the slot once a second until the node joined, refused, or `timeout` seconds passed.
async fn wait_joined(node: &Node, slot: &str, net: &str, timeout: u64) -> anyhow::Result<()> {
    eprintln!("Waiting for network members…");
    let started = Instant::now();
    let mut reminded = 0;
    loop {
        let state = node.call("GET", &format!("/api/net/{slot}/state"), None)?;
        let nets = node.call("GET", "/api/nets", None)?;
        match poll_step(&state, &nets, slot, net) {
            PollStep::Joined => return Ok(()),
            PollStep::Fail(why) => anyhow::bail!("{why}"),
            PollStep::Wait => {}
        }
        let waited = started.elapsed().as_secs();
        anyhow::ensure!(
            waited < timeout,
            "still joining in the background; see `{} status`",
            program_name()
        );
        if waited / REMIND_EVERY > reminded {
            reminded = waited / REMIND_EVERY;
            eprintln!("still waiting ({waited} s)");
        }
        sleep(POLL_EVERY).await;
    }
}

#[cfg(test)]
#[path = "join_tests.rs"]
mod tests;
