//! `network create`: makes a network on the running node, or with `--key-only` only prints a new key.

use serde_json::{json, Value};

use super::key_form::raw_form;
use super::key_only::key_only;
use crate::cli::args::{Global, Globals};
use crate::cli::commands::client::{print_json, ApiError, Node};
use crate::cli::exit::UsageError;
use crate::cli::program_name::program_name;
use crate::members::network_key::normalize_label;

/// A request function: method, path and body; the reply body or an error.
type Call<'a> = dyn FnMut(&str, &str, Option<&Value>) -> anyhow::Result<Value> + 'a;

/// A network the node made.
#[derive(Debug)]
struct Created {
    /// The reply of the last `network/create` request.
    body: Value,
    key: String,
    /// The slot added because the chosen one already had a network.
    new_slot: Option<String>,
}

pub(super) fn run(
    label: &[String],
    raw: bool,
    key_only_mode: bool,
    g: &Globals,
) -> anyhow::Result<()> {
    match key_only_mode {
        true => {
            g.allow("network create", &[])?;
            key_only(label, raw, g.config.as_deref())
        }
        false => {
            g.allow("network create", &[Global::Net, Global::Json])?;
            on_node(label, raw, g)
        }
    }
}

fn on_node(label: &[String], raw: bool, g: &Globals) -> anyhow::Result<()> {
    let label = checked_label(label)?;
    refuse_raw_json(raw, g)?;
    let node = Node::connect(g)?;
    let mut call = |method: &str, path: &str, body: Option<&Value>| node.call(method, path, body);
    let created = create_in(&mut call, &node.base, node.explicit_net, &label)?;
    print_created(&created, &label, raw, g)
}

/// The label as the node will take it; none, or one the node would refuse, is a usage error.
fn checked_label(words: &[String]) -> anyhow::Result<String> {
    let prog = program_name();
    if words.is_empty() {
        return Err(usage(format!(
            "a network needs a label: `{prog} network create <label>`. To only print a key, add --key-only"
        )));
    }
    normalize_label(&words.join(" ")).map_err(|e| usage(format!("{e}")))
}

fn refuse_raw_json(raw: bool, g: &Globals) -> anyhow::Result<()> {
    match raw && g.json {
        true => Err(usage(format!(
            "--json prints the node's answer as it is, so --raw can't go with it. See: {} network create --help",
            program_name()
        ))),
        false => Ok(()),
    }
}

fn usage(text: String) -> anyhow::Error {
    anyhow::Error::new(UsageError(text))
}

/// Creates the network in the chosen slot. A slot that has a network is not touched: without `--net` a new slot
/// is added and the network goes there.
fn create_in(
    call: &mut Call<'_>,
    base: &str,
    explicit_net: bool,
    label: &str,
) -> anyhow::Result<Created> {
    match post_create(call, base, label) {
        Ok(body) => created(body, None),
        Err(e) if !is_present(&e) => Err(e),
        Err(_) if explicit_net => {
            anyhow::bail!("that network already exists; leave out --net to create a new one")
        }
        Err(_) => in_new_slot(call, label),
    }
}

fn in_new_slot(call: &mut Call<'_>, label: &str) -> anyhow::Result<Created> {
    let added = call("POST", "/api/nets", None)?;
    let slot = added["slot"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("the node added a network slot but did not name it"))?
        .to_string();
    let body = post_create(call, &format!("/api/net/{slot}"), label)?;
    created(body, Some(slot))
}

fn post_create(call: &mut Call<'_>, base: &str, label: &str) -> anyhow::Result<Value> {
    call(
        "POST",
        &format!("{base}/network/create"),
        Some(&json!({"label": label})),
    )
}

fn created(body: Value, new_slot: Option<String>) -> anyhow::Result<Created> {
    let key = body["key"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("the node's answer has no key"))?
        .to_string();
    Ok(Created {
        body,
        key,
        new_slot,
    })
}

fn is_present(e: &anyhow::Error) -> bool {
    let refused = e.chain().find_map(|c| c.downcast_ref::<ApiError>());
    refused.is_some_and(|a| a.code == "network_present")
}

fn print_created(c: &Created, label: &str, raw: bool, g: &Globals) -> anyhow::Result<()> {
    match (g.json, raw) {
        (true, _) => print_json(&c.body),
        (_, true) => println!("{}", raw_form(&c.key)?),
        _ => println!("{}", c.key),
    }
    let shown = c.body["label"].as_str().unwrap_or(label);
    eprintln!("Network \"{shown}\" created, this computer is its administrator.");
    eprintln!("Invite others: {} invite [--net <slot>]", program_name());
    if let Some(slot) = &c.new_slot {
        eprintln!("Created in a new slot: {slot}");
    }
    Ok(())
}

#[cfg(test)]
#[path = "create_tests.rs"]
mod tests;
