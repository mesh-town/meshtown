//! Console commands about checks (`check <domain>`, `checks`) and the node console launch: an OS thread
//! reads stdin line by line, while parsing and running the commands is a tokio task.

use anyhow::Result;

use crate::domains::{self, DomainList};
use crate::output::say;

use super::state::Checks;

/// A console command that relates to checks.
#[derive(Debug, PartialEq, Eq)]
pub enum CheckCommand {
    Check(String),
    Table,
}

/// Parses a console line if it is about checks; otherwise `None`.
pub fn parse_check_line(line: &str) -> Option<Result<CheckCommand>> {
    let words: Vec<&str> = line.split_whitespace().collect();
    match words.as_slice() {
        ["checks"] => Some(Ok(CheckCommand::Table)),
        ["check", d] => Some(Ok(CheckCommand::Check((*d).to_string()))),
        ["check"] | ["check", _, ..] | ["checks", _, ..] => Some(Err(anyhow::anyhow!(
            "available: check <domain>, checks"
        ))),
        _ => None,
    }
}

/// Starts the node console: an OS thread reads stdin line by line, a tokio task runs the commands.
/// It is an OS thread rather than `tokio::io::stdin` so that waiting for input does not delay the node's shutdown.
pub fn spawn_console(list: DomainList, checks: Checks, console: crate::proxy::ProxyConsole) {
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
            match parse_check_line(&line) {
                Some(Ok(CheckCommand::Table)) => checks.print_table().await,
                Some(Ok(CheckCommand::Check(domain))) => {
                    if let Err(e) = checks.check_now(&domain).await {
                        say("check", format!("{domain}: check failed: {e:#}"));
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

#[cfg(test)]
#[path = "console_tests.rs"]
mod tests;
