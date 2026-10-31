//! `config set <path> <value>`: one setting, written through the running node or, when none runs, into the file.

use serde_json::{json, Value};

use super::assign::value_of;
use super::file::{place, save, saved_note};
use crate::cli::args::{Global, Globals};
use crate::cli::commands::access::settings_io::Route;
use crate::cli::commands::client::{print_json, Node};
use crate::cli::exit::UsageError;
use crate::cli::program_name::program_name;
use crate::config::config_file::{FieldSet, SetRefusal};
use crate::node::output::NET_NAME;

pub(super) fn run(path: &str, value: &str, g: &Globals) -> anyhow::Result<()> {
    g.allow("config set", &[Global::Net, Global::Json])?;
    let set = FieldSet::check(path, value_of(value)).map_err(refusal)?;
    match place(g)? {
        Route::Live(node) => live(&node, &set, g.json),
        Route::File(file) => {
            save(&file, &set)?;
            eprintln!("{}", saved_note(&set, &file));
            Ok(())
        }
    }
}

fn refusal(r: SetRefusal) -> anyhow::Error {
    let prog = program_name();
    match r {
        SetRefusal::Unknown(path) => anyhow::Error::new(UsageError(format!(
            "no such setting `{path}`. See: {prog} config get"
        ))),
        SetRefusal::ReadOnly("web.port") => {
            anyhow::anyhow!("the page port is set with --web-port, not stored")
        }
        SetRefusal::ReadOnly(path) => anyhow::anyhow!("`{path}` can't be changed"),
        SetRefusal::NetworkKey => {
            anyhow::anyhow!("the network key is not set this way: use `{prog} join <key>`")
        }
        SetRefusal::BadValue { path, expected } => {
            anyhow::anyhow!("`{path}` expects {expected}")
        }
    }
}

/// The patch the node merges: only this field, in its section.
pub(super) fn patch(set: &FieldSet) -> Value {
    let (section, key) = set.path().split_once('.').unwrap_or((set.path(), ""));
    json!({ section: { key: set.value() } })
}

/// Whether the node can take this field now; the text is the refusal.
pub(super) fn live_refusal(set: &FieldSet, current: &Value) -> Option<String> {
    let listed = current["read_only"]
        .as_array()
        .is_some_and(|l| l.iter().any(|p| p.as_str() == Some(set.path())));
    match (set.is_file_only(), listed) {
        (true, _) => Some(format!(
            "`{}` lives in the file only: stop {NET_NAME}, then run config set again",
            set.path()
        )),
        (_, true) => Some(format!(
            "`{}` is shared by all networks; run without --net",
            set.path()
        )),
        _ => None,
    }
}

fn live(node: &Node, set: &FieldSet, json: bool) -> anyhow::Result<()> {
    let current = node.call("GET", &node.net_path("config"), None)?;
    if let Some(why) = live_refusal(set, &current) {
        anyhow::bail!(why)
    }
    let reply = node.put_config(&patch(set))?;
    eprintln!("saved {}", set.path());
    report(&reply, set.path());
    if json {
        print_json(&reply);
    }
    Ok(())
}

/// The stderr notes after a save: what only takes effect later, and what this run overrides.
pub(super) fn restart_notes(reply: &Value, path: &str) -> Vec<String> {
    let listed = |key: &str| {
        reply[key]
            .as_array()
            .is_some_and(|l| l.iter().any(|p| p.as_str() == Some(path)))
    };
    let mut notes = Vec::new();
    if listed("restart_pending") {
        notes.push(format!("applies after {NET_NAME} restarts"));
    }
    if listed("cli_overrides") {
        notes.push("this run keeps the value set at startup".to_string());
    }
    notes
}

fn report(reply: &Value, path: &str) {
    for note in restart_notes(reply, path) {
        eprintln!("{note}");
    }
}

#[cfg(test)]
#[path = "set_tests.rs"]
mod tests;
