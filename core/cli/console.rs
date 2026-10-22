//! The node console: lines from the standard input of a running `run`.
//!
//! What it does: reads lines in a separate thread and dispatches them to commands — `invite` (an invite
//! string), the check commands (`checks`), the proxy (`proxy`) and the domain list (`domains`).
//! What it doesn't do: it doesn't execute commands itself, it only calls the owner modules.
//! Links: started from `node::run_cycles` on the first cycle; repeats `checks::spawn_console`,
//! which stays in the code until the owner of `checks.rs` edits it.

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
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
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
    tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            if let Some(for_whom) = parse_invite_line(&line) {
                print_invite(&invite, for_whom.as_deref()).await;
                continue;
            }
            match parse_check_line(&line) {
                Some(Ok(CheckCommand::Table)) => checks.print_table().await,
                Some(Ok(CheckCommand::Check(domain))) => {
                    if let Err(e) = checks.check_now(&domain).await {
                        say("check", format!("{domain}: check did not run: {e:#}"));
                    }
                }
                Some(Err(e)) => say("console", format!("{e:#}")),
                None => match crate::proxy::parse_proxy_line(&line) {
                    Some(Ok(cmd)) => crate::proxy::execute_proxy_command(&console, cmd).await,
                    Some(Err(e)) => say("console", format!("{e:#}")),
                    None => match domains::parse_domain_console_line(&line) {
                        Ok(None) => {}
                        Ok(Some(cmd)) => domains::execute_domain_command(&list, cmd).await,
                        Err(e) => say("console", format!("{e:#}")),
                    },
                },
            }
        }
        say("console", "input closed, console commands unavailable");
    });
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
    let issued = match crate::invite::make(invite, for_whom) {
        Ok(issued) => issued.commit().await,
        Err(e) => Err(e),
    };
    match issued {
        Ok(dealt) => {
            say(
                "invite",
                "the invite string below is single-use and valid for a day; send it to the member in person",
            );
            say("invite", dealt.reach().console_line());
            println!("{}", dealt.text);
        }
        Err(e) => say("invite", format!("couldn't build the invite: {e:#}")),
    }
}

#[cfg(test)]
#[path = "console_tests.rs"]
mod tests;
