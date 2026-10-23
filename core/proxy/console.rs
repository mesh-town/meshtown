//! Console commands: proxy status, route selection, `get`, `exit on/off`.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::node::output::{say, short_peer_id};
use crate::proxy::route::{self, Route, RouteCtx};

use super::front::ProxyFront;
use super::get::get_via_socks;

/// Console commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyCommand {
    Socks,
    Route(String),
    Get(String),
    Exit(bool),
}

/// Parses a console line if it is about the proxy; otherwise `None`.
pub fn parse_proxy_line(line: &str) -> Option<Result<ProxyCommand>> {
    let words: Vec<&str> = line.split_whitespace().collect();
    match words.as_slice() {
        ["socks"] => Some(Ok(ProxyCommand::Socks)),
        ["route", d] => Some(Ok(ProxyCommand::Route((*d).to_string()))),
        ["get", d] => Some(Ok(ProxyCommand::Get((*d).to_string()))),
        ["exit", "on"] => Some(Ok(ProxyCommand::Exit(true))),
        ["exit", "off"] => Some(Ok(ProxyCommand::Exit(false))),
        ["socks", ..] | ["route", ..] | ["get", ..] | ["exit", ..] => Some(Err(anyhow!(
            "available: socks, route <domain>, get <domain>, exit on, exit off"
        ))),
        _ => None,
    }
}

/// What the console knows about this node's proxy.
#[derive(Clone)]
pub struct ProxyConsole {
    /// The process proxy entry; when it is `None`, the older `socks` and `route` fields are used.
    pub front: Option<ProxyFront>,
    /// Where the SOCKS5 server listens; `None` — disabled by the `--no-socks` flag or the `socks.enabled: false` setting.
    pub socks: Option<SocketAddr>,
    /// Whether the node works as an exit for others.
    pub exit_on: Arc<AtomicBool>,
    pub route: RouteCtx,
    pub user_agent: String,
}

impl ProxyConsole {
    /// The listening address: from the process entry if there is one, else the node's own.
    fn listen_addr(&self) -> Option<SocketAddr> {
        match &self.front {
            Some(front) => front.addr(),
            None => self.socks,
        }
    }

    /// The route context now; `None` — the proxy has no network to route through.
    fn route_now(&self) -> Option<RouteCtx> {
        match &self.front {
            Some(front) => front.current_route(),
            None => Some(self.route.clone()),
        }
    }
}

/// Runs the route choice for a domain and prints the outcome.
async fn say_route(console: &ProxyConsole, domain: &str) {
    let Some(route_ctx) = console.route_now() else {
        say(
            "route",
            format!("{domain} → refused: no network for the proxy"),
        );
        return;
    };
    match route_ctx.choose_now().await {
        Ok(Route::Direct) => say("route", format!("{domain} → direct")),
        Ok(Route::Exits(candidates)) => {
            let first = route::first_exit(&candidates);
            say(
                "route",
                format!("{domain} → exit {}", short_peer_id(&first)),
            );
        }
        Ok(Route::Refuse(reason)) => say("route", format!("{domain} → refused: {reason}")),
        Err(e) => say("route", format!("{domain}: {e:#}")),
    }
}

/// Prints the SOCKS listening address and whether the exit is on for others.
fn say_socks(console: &ProxyConsole) {
    let listening = match console.listen_addr() {
        Some(addr) => addr.to_string(),
        None => "off".to_string(),
    };
    let exit_status_word = if console.exit_on.load(Ordering::Relaxed) {
        "on"
    } else {
        "off"
    };
    say(
        "socks",
        format!("listening: {listening}; exit for others: {exit_status_word}"),
    );
}

/// Executes a console command.
pub async fn execute_proxy_command(console: &ProxyConsole, cmd: ProxyCommand) {
    match cmd {
        ProxyCommand::Socks => say_socks(console),
        ProxyCommand::Route(domain) => say_route(console, &domain).await,
        ProxyCommand::Get(domain) => {
            let Some(proxy) = console.listen_addr() else {
                say("get", "SOCKS5 server is off, the command is unavailable");
                return;
            };
            if let Err(e) = get_via_socks(proxy, &domain, &console.user_agent).await {
                say("get", format!("{domain}: {e:#}"));
            }
        }
        ProxyCommand::Exit(on) => {
            console.exit_on.store(on, Ordering::Relaxed);
            say(
                "exit",
                if on {
                    format!(
                        "serving as exit for members of network {}",
                        crate::node::output::NET_NAME
                    )
                } else {
                    "no longer serving as exit".to_string()
                },
            );
        }
    }
}

#[cfg(test)]
#[path = "console_tests.rs"]
mod tests;
