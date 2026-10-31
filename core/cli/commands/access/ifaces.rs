//! `access ifaces`: which networks (interfaces) the node accepts connections from — `socks.ifaces` and
//! `web.ifaces` as one value. If the node is running, it edits through `PUT /api/config`; if not, it writes
//! the settings file itself. `--if-unset` is a one-time seed (an OpenWrt init script): if there is no file, it creates
//! it with the names; if the key is already there or the file is unreadable, it changes nothing and exits with code 0.

use std::path::Path;

use serde_json::{json, Value};

use super::settings_io::{edit_file, restart_needed, route, Route};
use crate::cli::args::Globals;
use crate::cli::exit::UsageError;
use crate::config::config_file::{self, IfaceFilter};
use crate::node::output::NET_NAME;

const EXPECTED: &str = "a list of interface names without repeats was expected (auto — automatic, '*' — no restriction)";

/// What to do with `--if-unset` given the state of the file.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Seed {
    Skip(&'static str),
    Write,
    /// There is no file at all: create it with the defaults and the interface names.
    Create,
}

/// What was found in place of the settings file.
pub(super) enum FileRead {
    Absent,
    Unreadable,
    Text(String),
}

fn read_file(path: &std::path::Path) -> FileRead {
    match std::fs::read_to_string(path) {
        Ok(text) => FileRead::Text(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => FileRead::Absent,
        Err(_) => FileRead::Unreadable,
    }
}

/// `access ifaces`: names are checked before anything is read, then the file decides what to do.
pub(super) fn run(names: Vec<String>, if_unset: bool, g: &Globals) -> anyhow::Result<()> {
    g.allow("access ifaces", &[])?;
    let filter = parse_names(&names).map_err(|why| {
        let prog = crate::cli::program_name::program_name();
        anyhow::Error::new(UsageError(format!(
            "{why}. See: {prog} access ifaces --help"
        )))
    })?;
    let path = super::settings_io::settings_path(g)?;
    match seed_decision(if_unset, &read_file(&path)) {
        Seed::Skip(why) => {
            eprintln!("Interface list unchanged: {why}.");
            Ok(())
        }
        Seed::Create => create_seeded(&path, &filter),
        Seed::Write => write(g, &path, &filter),
    }
}

/// A fresh install: there is no file, so we create it the same way as the first `run`, right away with the interface names.
/// A node cannot exist without a file, so we do not look for one. An existing file is not overwritten:
/// `save_config_file` without an expected revision writes over it silently.
pub(super) fn create_seeded(path: &Path, filter: &IfaceFilter) -> anyhow::Result<()> {
    if path.symlink_metadata().is_ok() {
        anyhow::bail!("settings file already exists: creation cancelled");
    }
    let now = crate::cli::commands::now_unix();
    let fresh = config_file::create_fresh(path, now, |s| {
        s.process.socks_ifaces = filter.clone();
        s.process.web_ifaces = filter.clone();
    })?;
    fresh
        .saved
        .map_err(|e| anyhow::anyhow!("settings file was not created: {e:?}"))?;
    eprintln!(
        "Settings file created: {}; interfaces for the proxy and the page: {}.",
        path.display(),
        describe(filter)
    );
    Ok(())
}

/// Names from the command line → a filter: `auto`, `'*'` or a list.
pub(super) fn parse_names(names: &[String]) -> Result<IfaceFilter, String> {
    if names == ["auto"] {
        return Ok(IfaceFilter::Auto);
    }
    let mixed = names.iter().any(|n| n == "auto");
    let parsed = IfaceFilter::from_names(names.to_vec()).filter(|_| !mixed);
    parsed.ok_or_else(|| EXPECTED.to_string())
}

/// Without `--if-unset` we always write; with it we create if there is no file, skip an unreadable one,
/// and for an existing one write only if none of the keys is in it.
pub(super) fn seed_decision(if_unset: bool, file: &FileRead) -> Seed {
    if !if_unset {
        return Seed::Write;
    }
    match file {
        FileRead::Absent => Seed::Create,
        FileRead::Unreadable => Seed::Skip("settings file is unreadable"),
        FileRead::Text(text) => seed_for_text(text),
    }
}

fn seed_for_text(text: &str) -> Seed {
    let set =
        config_file::has_field(text, "socks.ifaces") || config_file::has_field(text, "web.ifaces");
    if set {
        return Seed::Skip("the interface list is already set in the file");
    }
    Seed::Write
}

fn write(g: &Globals, path: &Path, filter: &IfaceFilter) -> anyhow::Result<()> {
    match route(g)? {
        Route::File(_) => offline(path, filter),
        Route::Live(node) => {
            let names = filter.parts().1;
            let reply =
                node.put_config(&json!({"socks": {"ifaces": names}, "web": {"ifaces": names}}))?;
            eprintln!("{}", saved_text(&reply, filter));
            Ok(())
        }
    }
}

fn saved_text(reply: &Value, filter: &IfaceFilter) -> String {
    let what = describe(filter);
    match restart_tail(reply) {
        Some(tail) => format!("Interface list saved: {what} ({tail} after {NET_NAME} restarts)."),
        None => format!("Interface list saved: {what}."),
    }
}

/// Who picks the list up only after a restart, from the fields the reply lists; none when nothing waits.
fn restart_tail(reply: &Value) -> Option<&'static str> {
    let page = restart_needed(reply, &["web.ifaces"]);
    let proxy = restart_needed(reply, &["socks.ifaces"]);
    match (page, proxy) {
        (false, false) => None,
        (true, false) => Some("the page picks it up"),
        (false, true) => Some("the proxy picks it up"),
        (true, true) => Some("the page and the proxy pick it up"),
    }
}

/// The filter in words: `automatic`, `all interfaces` or `only en0, wlan0`.
fn describe(filter: &IfaceFilter) -> String {
    match filter {
        IfaceFilter::Auto => "automatic".to_string(),
        IfaceFilter::Any => "all interfaces".to_string(),
        IfaceFilter::Only(names) => format!("only {}", names.join(", ")),
    }
}

/// The node is not running: reads the file, changes the fields, writes with a revision check. If there is no file, it does not create one.
pub(super) fn offline(path: &Path, filter: &IfaceFilter) -> anyhow::Result<()> {
    edit_file(path, "interface list", |s| {
        s.process.socks_ifaces = filter.clone();
        s.process.web_ifaces = filter.clone();
    })?;
    eprintln!(
        "Interface list saved: {} ({NET_NAME} is not running: applies at startup).",
        describe(filter)
    );
    Ok(())
}

#[cfg(test)]
#[path = "ifaces_tests.rs"]
mod tests;
