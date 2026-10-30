//! The node console: lines from the standard input of a running `run`.
//!
//! What it does: reads lines in a separate thread and dispatches them to commands — `invite` (an invite
//! string), the check commands (`checks`), the proxy (`proxy`) and the domain list (`domains`).
//! What it doesn't do: it doesn't execute commands itself, it only calls the owner modules.
//! Links: started from `node::run_cycles` on the first cycle; repeats `checks::spawn_console`.

use crate::node::output::say;
use crate::proxy::domains::{self, DomainList};
use crate::site_checks::checks::{parse_check_line, CheckCommand, Checks};

/// Starts the stdin console of a running node: a thread reading lines plus their dispatch to commands.
pub fn spawn(
    list: DomainList,
    checks: Checks,
    console: crate::proxy::ProxyConsole,
    invite: crate::invite::Source,
) {
    let mut rx = spawn_stdin_reader();
    crate::node::output::spawn(async move {
        while let Some(line) = rx.recv().await {
            dispatch_console_line(&line, &list, &checks, &console, &invite).await;
        }
        say("console", "input closed, console commands unavailable");
    });
}

/// Starts the thread reading stdin lines and returns the receiving end of its channel.
fn spawn_stdin_reader() -> tokio::sync::mpsc::UnboundedReceiver<String> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::stdin().lock().lines() {
            match line {
                Ok(line) => {
                    if tx.send(line).is_err() {
                        return;
                    }
                }
                Err(_) => return,
            }
        }
    });
    rx
}

/// Handles one console line: the invite command first, then the check, proxy and domain commands.
async fn dispatch_console_line(
    line: &str,
    list: &DomainList,
    checks: &Checks,
    console: &crate::proxy::ProxyConsole,
    invite: &crate::invite::Source,
) {
    if let Some(for_whom) = parse_invite_line(line) {
        print_invite(invite, for_whom.as_deref()).await;
        return;
    }
    match parse_check_line(line) {
        Some(Ok(CheckCommand::Table)) => checks.print_table().await,
        Some(Ok(CheckCommand::Check(domain))) => {
            if let Err(e) = checks.check_now(&domain).await {
                say("check", format!("{domain}: check did not run: {e:#}"));
            }
        }
        Some(Err(e)) => say("console", format!("{e:#}")),
        None => dispatch_other_line(line, list, console).await,
    }
}

/// Handles a line that is not an invite or check command: the proxy commands, then the domain list.
async fn dispatch_other_line(line: &str, list: &DomainList, console: &crate::proxy::ProxyConsole) {
    match crate::proxy::parse_proxy_line(line) {
        Some(Ok(cmd)) => crate::proxy::execute_proxy_command(console, cmd).await,
        Some(Err(e)) => say("console", format!("{e:#}")),
        None => match domains::parse_domain_console_line(line) {
            Ok(None) => {}
            Ok(Some(cmd)) => domains::execute_domain_command(list, cmd).await,
            Err(e) => say("console", format!("{e:#}")),
        },
    }
}

/// A console line is the `invite` command: `Some(None)` without "for whom", `Some(Some(text))` with it.
pub fn parse_invite_line(line: &str) -> Option<Option<String>> {
    let rest = line.trim().strip_prefix("invite")?;
    if rest.is_empty() {
        return Some(None);
    }
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    Some(Some(rest.trim().to_string()))
}

/// Builds an invite, writes the issue into the network document and prints a line.
async fn print_invite(invite: &crate::invite::Source, for_whom: Option<&str>) {
    match issue_invite(invite, for_whom).await {
        Ok(dealt) => print_dealt_invite(&dealt),
        Err(e) => say("invite", format!("couldn't build the invite: {e:#}")),
    }
}

/// Builds an invite and commits it to the network document.
async fn issue_invite(
    invite: &crate::invite::Source,
    for_whom: Option<&str>,
) -> anyhow::Result<crate::invite::Dealt> {
    match crate::invite::make(invite, for_whom) {
        Ok(issued) => issued.commit().await,
        Err(e) => Err(e),
    }
}

/// Prints the notes and the string of an issued invite.
fn print_dealt_invite(dealt: &crate::invite::Dealt) {
    say(
        "invite",
        "the invite string below is single-use and valid for a day; send it to the member in person",
    );
    say("invite", dealt.reach().console_line());
    println!("{}", dealt.text);
}

#[cfg(test)]
#[path = "console_tests.rs"]
mod tests;
