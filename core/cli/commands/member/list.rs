//! `member list`: the rows of the table and the hints that go to stderr.

use serde_json::Value;

use crate::cli::program_name::program_name;

/// The table rows of the `members` array of `GET members`: `ID STATUS ONLINE ADMIN BLOCKED NAME`.
pub(crate) fn rows(members: &Value) -> Vec<Vec<String>> {
    let list = members.as_array().map_or(&[][..], Vec::as_slice);
    list.iter().map(row).collect()
}

fn row(m: &Value) -> Vec<String> {
    vec![
        m["id"].as_str().unwrap_or("-").to_string(),
        m["status"].as_str().unwrap_or("-").to_string(),
        yes_no(&m["online"]),
        yes_no(&m["admin"]),
        yes_no(&m["blocked_by_me"]),
        name(m),
    ]
}

fn yes_no(v: &Value) -> String {
    match v.as_bool() {
        Some(true) => "yes",
        _ => "no",
    }
    .to_string()
}

/// The name, or `-`; this computer's row is marked.
fn name(m: &Value) -> String {
    let shown = match m["name"].as_str() {
        Some(n) if !n.is_empty() => clean(n),
        _ => "-".to_string(),
    };
    match m["self"].as_bool() {
        Some(true) => format!("{shown} (this computer)"),
        _ => shown,
    }
}

/// A text from a neighbour for a terminal: control characters become `?`.
pub(crate) fn clean(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

/// The lines for stderr after the table.
pub(crate) fn hints(reply: &Value) -> Vec<String> {
    let flag = |key: &str| reply[key].as_bool().unwrap_or(false);
    let waiting = count(reply, |m| m["status"] == "pending");
    let admit = flag("me_admin") && waiting > 0;
    let lines = [
        Some(summary(reply)),
        admit.then(|| waiting_line(waiting)),
        flag("me_pending")
            .then(|| "This computer waits for an administrator to admit it".to_string()),
        flag("me_excluded")
            .then(|| "An administrator removed this computer from the network".to_string()),
    ];
    lines.into_iter().flatten().collect()
}

fn count(reply: &Value, test: impl Fn(&Value) -> bool) -> usize {
    let list = reply["members"].as_array().map_or(&[][..], Vec::as_slice);
    list.iter().filter(|m| test(m)).count()
}

fn summary(reply: &Value) -> String {
    let members = count(reply, |m| m["status"] == "member");
    let online = count(reply, |m| m["status"] == "member" && m["online"] == true);
    let noun = if members == 1 { "member" } else { "members" };
    format!("{members} {noun} ({online} online)")
}

fn waiting_line(waiting: usize) -> String {
    format!(
        "{waiting} waiting: run `{} member admit <name or id>`",
        program_name()
    )
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
