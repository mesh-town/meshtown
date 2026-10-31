//! The actions on one member: the check of `--reason`, the request body and the words of the result.

use serde_json::{json, Value};

use super::cmd::MemberCmd;
use super::list::clean;
use super::resolve::resolve_member;
use crate::cli::args::Globals;
use crate::cli::commands::client::{print_json, Node};
use crate::cli::exit::UsageError;
use crate::cli::program_name::program_name;
use crate::web_gui::members::MAX_REASON_CHARS;

/// `--reason` of `exclude`, checked before any request; other commands have none.
pub(super) fn reason_of(cmd: &MemberCmd) -> anyhow::Result<Option<String>> {
    match cmd {
        MemberCmd::Exclude { reason, .. } => {
            check_reason(reason.as_deref()).map_err(anyhow::Error::new)
        }
        _ => Ok(None),
    }
}

/// The reason without the blanks around it; none when it is blank. A control character or more than the
/// node accepts is a usage error.
pub(crate) fn check_reason(reason: Option<&str>) -> Result<Option<String>, UsageError> {
    let usage = |what: &str| {
        UsageError(format!(
            "--reason {what}. See: {} member exclude --help",
            program_name()
        ))
    };
    let text = reason.unwrap_or("");
    if text.chars().any(char::is_control) {
        return Err(usage("must not contain control characters"));
    }
    let text = text.trim();
    if text.chars().count() > MAX_REASON_CHARS {
        return Err(usage(&format!(
            "must be at most {MAX_REASON_CHARS} characters"
        )));
    }
    Ok(Some(text.to_string()).filter(|t| !t.is_empty()))
}

/// The request body of an action; the id isn't checked here.
pub(crate) fn action_body(cmd: &MemberCmd, id_full: &str) -> Value {
    match cmd {
        MemberCmd::Admin { revoke, .. } => json!({"id": id_full, "admin": !revoke}),
        MemberCmd::Exclude { reason, .. } => with_reason(id_full, reason.as_deref()),
        _ => json!({"id": id_full}),
    }
}

fn with_reason(id_full: &str, reason: Option<&str>) -> Value {
    let reason = reason.map(str::trim).filter(|r| !r.is_empty());
    match reason {
        Some(text) => json!({"id": id_full, "reason": text}),
        None => json!({"id": id_full}),
    }
}

/// The line for stderr after an action that went through; `changed: false` says it was already so.
pub(crate) fn outcome(cmd: &MemberCmd, label: &str, changed: bool) -> String {
    match changed {
        true => done_text(cmd, label),
        false => format!("Nothing changed: {}", already_text(cmd, label)),
    }
}

fn done_text(cmd: &MemberCmd, label: &str) -> String {
    match cmd {
        MemberCmd::List => String::new(),
        MemberCmd::Admit { .. } => format!("Admitted {label}."),
        MemberCmd::Exclude { .. } => format!("Removed {label} from the network."),
        MemberCmd::Unexclude { .. } => format!("Withdrew the removal of {label}."),
        MemberCmd::Block { .. } => format!("Blocked {label} on this computer."),
        MemberCmd::Unblock { .. } => format!("Unblocked {label}."),
        MemberCmd::Admin { revoke: false, .. } => format!("{label} is now an administrator."),
        MemberCmd::Admin { revoke: true, .. } => format!("{label} is no longer an administrator."),
    }
}

/// Why an action had nothing to do: the state it wanted was there already.
fn already_text(cmd: &MemberCmd, label: &str) -> String {
    match cmd {
        MemberCmd::List => String::new(),
        MemberCmd::Admit { .. } => format!("{label} is already admitted."),
        MemberCmd::Exclude { .. } => format!("{label} is already removed from the network."),
        MemberCmd::Unexclude { .. } => format!("{label} is not removed."),
        MemberCmd::Block { .. } => format!("{label} is already blocked."),
        MemberCmd::Unblock { .. } => format!("{label} is not blocked."),
        MemberCmd::Admin { revoke: false, .. } => format!("{label} is already an administrator."),
        MemberCmd::Admin { revoke: true, .. } => format!("{label} is not an administrator."),
    }
}

/// Resolves the member in the list the node just gave, sends the action and tells the result.
pub(super) fn run(
    node: &Node,
    cmd: &MemberCmd,
    wanted: &str,
    list: &Value,
    g: &Globals,
) -> anyhow::Result<()> {
    let id = resolve_member(&list["members"], wanted).map_err(anyhow::Error::msg)?;
    let body = action_body(cmd, &id);
    let path = node.net_path(&format!("members/{}", cmd.verb()));
    let reply = node.call("POST", &path, Some(&body))?;
    match g.json {
        true => print_json(&reply),
        false => eprintln!("{}", outcome(cmd, &label(list, &id), changed(&reply))),
    }
    Ok(())
}

fn changed(reply: &Value) -> bool {
    reply["changed"].as_bool().unwrap_or(true)
}

/// `<name> (<id>)`, or the id alone for a member with no name.
fn label(list: &Value, id_full: &str) -> String {
    let rows = list["members"].as_array().map_or(&[][..], Vec::as_slice);
    let row = rows.iter().find(|m| m["id_full"].as_str() == Some(id_full));
    let id = row.and_then(|m| m["id"].as_str()).unwrap_or(id_full);
    match row
        .and_then(|m| m["name"].as_str())
        .filter(|n| !n.is_empty())
    {
        Some(name) => format!("{} ({id})", clean(name)),
        None => id.to_string(),
    }
}

#[cfg(test)]
#[path = "act_tests.rs"]
mod tests;
