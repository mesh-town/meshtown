//! The `ifaces` subcommand: which networks (interfaces) the node accepts connections from — `socks.ifaces` and
//! `web.ifaces` as one value. If the node is running, it edits through `PUT /api/config`; if not, it writes
//! the settings file itself. `--if-unset` is a one-time seed (an OpenWrt init script): if there is no file, it creates
//! it with the names; if the key is already there or the file is unreadable, it changes nothing and exits with code 0.

use crate::config::config_file::{
    self, IfaceFilter, LoadedConfigFile, ParsedConfigFile, SecretRead,
};
use serde_json::{json, Value};

use super::node_link::{LinkError, Target};
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

/// Entry point from `main`.
pub(crate) fn run_ifaces_command(
    config: Option<std::path::PathBuf>,
    web_port: Option<u16>,
    names: Vec<String>,
    if_unset: bool,
) -> anyhow::Result<()> {
    let filter = parse_names(&names).map_err(anyhow::Error::msg)?;
    let target = Target::resolve(config, web_port).map_err(anyhow::Error::msg)?;
    match seed_decision(if_unset, &read_file(&target.path)) {
        Seed::Skip(why) => {
            println!("Interface list unchanged: {why}.");
            Ok(())
        }
        Seed::Create => create_seeded(&target, &filter).map_err(anyhow::Error::msg),
        Seed::Write => set(&target, &filter).map_err(anyhow::Error::msg),
    }
}

/// A fresh install: there is no file, so we create it the same way as the first `run`, right away with the interface names.
/// A node cannot exist without a file, so we do not try `live`. An existing file is not overwritten:
/// `save_config_file` without an expected revision writes over it silently.
pub(super) fn create_seeded(t: &Target, filter: &IfaceFilter) -> Result<(), String> {
    if t.path.symlink_metadata().is_ok() {
        return Err("settings file already exists: creation cancelled".into());
    }
    let fresh = config_file::create_fresh(&t.path, super::now_unix(), |s| {
        s.process.socks_ifaces = filter.clone();
        s.process.web_ifaces = filter.clone();
    })
    .map_err(|e| e.to_string())?;
    fresh
        .saved
        .map_err(|e| format!("settings file was not created: {e:?}"))?;
    println!(
        "Settings file created: {}; interfaces for the proxy and the page: {}.",
        t.path.display(),
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

fn set(t: &Target, filter: &IfaceFilter) -> Result<(), String> {
    match live(t, filter) {
        Err(LinkError::NotRunning) => offline(t, filter),
        Err(LinkError::Other(e)) => Err(e),
        Ok(()) => {
            println!(
                "Interface list saved: {} (the proxy picks it up after {NET_NAME} restarts).",
                describe(filter)
            );
            Ok(())
        }
    }
}

fn describe(filter: &IfaceFilter) -> String {
    let (label, names) = filter.parts();
    format!("{label} {}", Value::from(names))
}

fn json_of(call: Result<(u16, String), LinkError>, what: &str) -> Result<Value, LinkError> {
    let (code, body) = call?;
    if code != 200 {
        return Err(LinkError::Other(format!(
            "{NET_NAME} refused ({what}, code {code}): {body}"
        )));
    }
    serde_json::from_str(&body)
        .map_err(|_| LinkError::Other(format!("{NET_NAME} did not reply with JSON ({what})")))
}

/// The node is running: writes both fields with one PUT carrying the revision.
fn live(t: &Target, filter: &IfaceFilter) -> Result<(), LinkError> {
    let rev = json_of(t.call("GET", "/api/config", None), "reading settings")?["rev"].as_u64();
    let rev = rev.ok_or_else(|| {
        LinkError::Other(format!("the {NET_NAME} reply has no settings revision"))
    })?;
    let names = filter.parts().1;
    let config = json!({"socks": {"ifaces": names}, "web": {"ifaces": names}});
    let body = json!({"rev": rev, "config": config}).to_string();
    json_of(
        t.call("PUT", "/api/config", Some(&body)),
        "writing settings",
    )?;
    Ok(())
}

/// The node is not running: reads the file, changes the fields, writes with a revision check. If there is no file, it does not create one.
pub(super) fn offline(t: &Target, filter: &IfaceFilter) -> Result<(), String> {
    match config_file::load_config_file(&t.path) {
        LoadedConfigFile::Absent => Err(format!(
            "no settings file: start {NET_NAME} once, then change the interface list"
        )),
        LoadedConfigFile::Unreadable(why) | LoadedConfigFile::Invalid(why) => {
            Err(format!("settings file is unusable: {why}"))
        }
        LoadedConfigFile::Parsed(p) => write_fields(t, p, filter),
    }
}

fn write_fields(t: &Target, p: ParsedConfigFile, filter: &IfaceFilter) -> Result<(), String> {
    let SecretRead::Valid(secret) = p.secret else {
        return Err(format!(
            "the settings file has no valid key for this computer: start {NET_NAME} once"
        ));
    };
    let mut settings = p.settings;
    settings.process.socks_ifaces = filter.clone();
    settings.process.web_ifaces = filter.clone();
    config_file::save_config_file(&t.path, Some(p.rev), super::now_unix(), &secret, &settings)
        .map_err(|e| format!("settings file was not saved: {e:?}"))?;
    println!(
        "Interface list saved: {} ({NET_NAME} is not running: applies at startup).",
        describe(filter)
    );
    Ok(())
}

#[cfg(test)]
#[path = "ifaces_tests.rs"]
mod tests;
