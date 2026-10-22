//! The `access` subcommand: who can open the node page (`web.access`). If the node is running, it edits
//! the setting through `PUT /api/config` and uses `GET /api/auth/status` to say whether it applies at once;
//! if it is not running, it writes the field into the settings file itself.

use crate::config::config_file::{
    self, ListenAccess, LoadedConfigFile, ParsedConfigFile, SecretRead,
};
use serde_json::{json, Value};

use super::node_link::{LinkError, Target};

/// Entry point from `main`: finds the node and sets the access mode.
pub(crate) fn run_access_command(
    config: Option<std::path::PathBuf>,
    web_port: Option<u16>,
    mode: ListenAccess,
) -> anyhow::Result<()> {
    let target = Target::resolve(config, web_port).map_err(anyhow::Error::msg)?;
    set(&target, mode).map_err(anyhow::Error::msg)
}

/// Sets the access mode.
fn set(t: &Target, mode: ListenAccess) -> Result<(), String> {
    match live(t, mode) {
        Err(LinkError::NotRunning) => offline(t, mode),
        Err(LinkError::Other(e)) => Err(e),
        Ok(applied) => {
            report_live(mode, applied);
            Ok(())
        }
    }
}

fn report_live(mode: ListenAccess, applied: bool) {
    let name = mode.as_str();
    if applied {
        println!("Node page access: {name} (takes effect immediately).");
        return;
    }
    println!("Node page access: {name} (saved; takes effect after the node restarts).");
}

fn json_of(call: Result<(u16, String), LinkError>, what: &str) -> Result<Value, LinkError> {
    let (code, body) = call?;
    if code != 200 {
        return Err(LinkError::Other(format!(
            "node refused ({what}, code {code})"
        )));
    }
    serde_json::from_str(&body)
        .map_err(|_| LinkError::Other(format!("node did not reply with JSON ({what})")))
}

/// The node is running: `true` means the mode already applies, `false` means it waits for a restart.
fn live(t: &Target, mode: ListenAccess) -> Result<bool, LinkError> {
    let rev = json_of(t.call("GET", "/api/config", None), "reading settings")?["rev"].as_u64();
    let rev =
        rev.ok_or_else(|| LinkError::Other("the node reply has no settings revision".into()))?;
    let body = json!({"rev": rev, "config": {"web": {"access": mode.as_str()}}}).to_string();
    json_of(
        t.call("PUT", "/api/config", Some(&body)),
        "writing settings",
    )?;
    let status = json_of(
        t.call("GET", "/api/auth/status", None),
        "reading sign-in status",
    )?;
    Ok(status["listening"] == mode.as_str())
}

/// The node is not running: reads the file, changes the field, writes with a revision check.
fn offline(t: &Target, mode: ListenAccess) -> Result<(), String> {
    match config_file::load_config_file(&t.path) {
        LoadedConfigFile::Absent => {
            Err("no settings file: start the node once, then change the access".into())
        }
        LoadedConfigFile::Unreadable(why) | LoadedConfigFile::Invalid(why) => {
            Err(format!("settings file is unusable: {why}"))
        }
        LoadedConfigFile::Parsed(p) => write_field(t, p, mode),
    }
}

fn write_field(t: &Target, p: ParsedConfigFile, mode: ListenAccess) -> Result<(), String> {
    let SecretRead::Valid(secret) = p.secret else {
        return Err("the settings file has no valid node key: start the node once".into());
    };
    let mut settings = p.settings;
    settings.process.web_access = mode;
    let now = super::now_unix();
    config_file::save_config_file(&t.path, Some(p.rev), now, &secret, &settings)
        .map_err(|e| format!("settings file was not saved: {e:?}"))?;
    println!(
        "Node page access: {} (node is not running: applies at startup).",
        mode.as_str()
    );
    Ok(())
}
