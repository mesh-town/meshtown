//! `access page <mode>` and `access proxy <mode>`: who can reach the page and the proxy. A running node takes
//! the change through `PUT /api/config`; without a node the settings file is edited.

use serde_json::{json, Value};

use super::settings_io::{edit_file, restart_needed, route, Route};
use crate::cli::args::Globals;
use crate::cli::commands::client::{ApiError, Node};
use crate::config::config_file::{ListenAccess, Settings};
use crate::node::output::NET_NAME;

/// One of the two settings this command changes.
pub(super) struct Field {
    /// The word in messages: "page" or "proxy".
    label: &'static str,
    section: &'static str,
    /// The path the node names in `restart_pending`.
    path: &'static str,
    set: fn(&mut Settings, ListenAccess),
}

const PAGE: Field = Field {
    label: "page",
    section: "web",
    path: "web.access",
    set: |s, m| s.process.web_access = m,
};

const PROXY: Field = Field {
    label: "proxy",
    section: "socks",
    path: "socks.access",
    set: |s, m| s.process.socks_access = m,
};

/// The settings patch that touches only this field.
pub(super) fn patch(field: &Field, mode: ListenAccess) -> Value {
    json!({ field.section: { "access": mode.as_str() } })
}

pub(super) fn page(mode: ListenAccess, g: &Globals) -> anyhow::Result<()> {
    g.allow("access page", &[])?;
    match route(g)? {
        Route::File(path) => offline(&path, &PAGE, mode),
        Route::Live(node) => page_live(&node, mode),
    }
}

pub(super) fn proxy(mode: ListenAccess, g: &Globals) -> anyhow::Result<()> {
    g.allow("access proxy", &[])?;
    match route(g)? {
        Route::File(path) => offline(&path, &PROXY, mode),
        Route::Live(node) => proxy_live(&node, mode),
    }
}

fn offline(path: &std::path::Path, field: &Field, mode: ListenAccess) -> anyhow::Result<()> {
    edit_file(path, "access", |s| (field.set)(s, mode))?;
    eprintln!(
        "{} access: {} ({NET_NAME} is not running: applies at startup).",
        field.label,
        mode.as_str()
    );
    Ok(())
}

/// The page listens on the new address at once only when the status already says so.
fn page_live(node: &Node, mode: ListenAccess) -> anyhow::Result<()> {
    node.put_config(&patch(&PAGE, mode))?;
    let status = node.call("GET", "/api/auth/status", None)?;
    let name = mode.as_str();
    match status["listening"] == name {
        true => eprintln!("page access: {name} (takes effect immediately)."),
        false => eprintln!("page access: {name} (saved; takes effect after {NET_NAME} restarts)."),
    }
    Ok(())
}

/// The proxy restarts itself when the stored value differs from the one it runs with.
fn proxy_live(node: &Node, mode: ListenAccess) -> anyhow::Result<()> {
    let name = mode.as_str();
    let reply = node.put_config(&patch(&PROXY, mode))?;
    if !restart_needed(&reply, &[PROXY.path]) {
        eprintln!("proxy access: {name} (unchanged).");
        return Ok(());
    }
    if let Err(e) = node.call("POST", &node.net_path("actions/restart"), None) {
        let why = match e.downcast_ref::<ApiError>() {
            Some(refused) if !refused.message.is_empty() => refused.message.clone(),
            _ => e.to_string(),
        };
        anyhow::bail!(
            "the setting is saved but the proxy was not restarted: {}. Restart {NET_NAME} to apply it.",
            why.trim_end_matches('.')
        );
    }
    eprintln!("{}", proxy_restart_text(name));
    Ok(())
}

/// The restart is asked for, not finished: the node answers before the proxy comes back.
fn proxy_restart_text(name: &str) -> String {
    format!("proxy access: {name} (proxy restarting).")
}

#[cfg(test)]
#[path = "mode_tests.rs"]
mod tests;
