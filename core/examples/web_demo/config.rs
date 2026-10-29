//! Preparation of the demo node's settings file (seven steps).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use meshtown_core::config::config_file::{self, CliOverrides, ConfigSource, PathRule};
use meshtown_core::members::network_key::format_managed_key;
use meshtown_core::members::words;

use crate::args::{Args, FileState, IdentityMode};
use crate::managed::demo_root;
use crate::overrides::parse_cli_overrides;

// ============================================================================================
// Preparing the settings file (7 steps)
// ============================================================================================

/// A `path` of the form `"section.field"` → a broken value from the `--field-error` table. `None` for
/// an unknown path (the caller must exit with code 2).
fn bad_value_for(path: &str) -> Option<(&'static str, &'static str, Value)> {
    match path {
        "socks.port" => Some(("socks", "port", json!(80))),
        "checks.interval_sec" => Some(("checks", "interval_sec", json!(5))),
        "probe.interval_sec" => Some(("probe", "interval_sec", json!(1))),
        "log.ring_lines" => Some(("log", "ring_lines", json!(10))),
        "exit.max_streams" => Some(("exit", "max_streams", json!(-1))),
        "exit.interface" => Some(("exit", "interface", json!(""))),
        "node.name" => Some(("node", "name", json!(""))),
        "checks.doh_server" => Some(("checks", "doh_server", json!("not-an-address"))),
        "exit.enabled" => Some(("exit", "enabled", json!("yes"))),
        "socks.enabled" => Some(("socks", "enabled", json!("yes"))),
        "socks.strict" => Some(("socks", "strict", json!("yes"))),
        "checks.enabled" => Some(("checks", "enabled", json!("yes"))),
        "checks.provider_dns" => Some(("checks", "provider_dns", json!("x"))),
        _ => None,
    }
}

/// Step 1: --identity declined — no file at all; the network (if needed) comes only through CliOverrides.
fn prepare_declined_config(
    args: &Args,
    cli: &mut CliOverrides,
    now: u64,
) -> anyhow::Result<(config_file::RunConfig, Option<PathBuf>)> {
    if !args.no_network {
        cli.network_key = Some(format_managed_key(
            Some("demo-network"),
            &words::generate_tail()?,
            &demo_root(),
        ));
    }
    let run = config_file::prepare(&ConfigSource::Disabled, cli, false, now)?;
    Ok((run, None))
}

/// Step 2: the first run creates a file with defaults.
fn create_default_config_file(config_path: &Path, now: u64) -> anyhow::Result<()> {
    let _ = config_file::prepare(
        &ConfigSource::File {
            path: config_path.to_path_buf(),
            may_create: true,
            rule: PathRule::Explicit,
        },
        &CliOverrides::default(),
        false,
        now,
    )?;
    Ok(())
}

/// Part of step 3: --field-error is applied to the already-read document (the same "config" root
/// as node.name/network.key — otherwise the parser sees the field as unknown).
fn apply_field_errors(doc: &mut Value, args: &Args) {
    for path in &args.field_error {
        match bad_value_for(path) {
            Some((section, field, value)) => {
                doc["config"][section][field] = value;
            }
            None => {
                eprintln!("web_demo: --field-error: unknown path \"{path}\"");
                std::process::exit(2);
            }
        }
    }
}

/// Step 3: read as a Value, set node.name/network.key, apply --field-error, write back.
fn write_config_fields(config_path: &Path, args: &Args) -> anyhow::Result<()> {
    let text = std::fs::read_to_string(config_path)?;
    let mut doc: Value = serde_json::from_str(&text)?;
    // The real fields live under the "config" root (see config_file::parse_text: `cfg =
    // root_obj.get("config")`) — a path without this prefix is silently dropped by the parser as
    // an unknown field (log: "unknown fields skipped... network, node"), so we write here,
    // not to `doc["node"]`/`doc["network"]` at the document root.
    doc["config"]["node"]["name"] = json!("finch-unwary");
    // The demo node does not open the page to the local network: e2e only connects from this machine.
    doc["config"]["web"]["access"] = json!("localhost");
    if !args.no_network {
        doc["config"]["network"]["key"] = json!(format_managed_key(
            Some("demo-network"),
            &words::generate_tail()?,
            &demo_root(),
        ));
    }
    apply_field_errors(&mut doc, args);
    std::fs::write(config_path, serde_json::to_string_pretty(&doc)?)?;
    Ok(())
}

/// Step 4: --identity file_preserved — identity disappears from the root, socks.port is corrupted,
/// the directory becomes unwritable (0555): on the next read the node identity from the
/// file must be preserved, even though the file can't be rewritten.
#[cfg_attr(not(unix), allow(unused_variables))] // only the unix branch uses the paths
fn apply_file_preserved_identity(
    dir: &Path,
    config_path: &Path,
    args: &Args,
) -> anyhow::Result<()> {
    if args.identity != IdentityMode::FilePreserved {
        return Ok(());
    }
    let text2 = std::fs::read_to_string(config_path)?;
    let mut doc2: Value = serde_json::from_str(&text2)?;
    if let Value::Object(ref mut map) = doc2 {
        map.remove("identity");
    }
    // "socks" is a setting and lives under the "config" root (like node/network above), while "identity"
    // is a separate root key (config_file.rs: `root.get("identity")`), so it was
    // removed on the root directly, and this field is not.
    doc2["config"]["socks"]["port"] = json!(80);
    std::fs::write(config_path, serde_json::to_string_pretty(&doc2)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555))?;
    }
    Ok(())
}

/// Step 5: --file-state (and --identity file_problem — the same effect as --file-state unreadable).
#[cfg_attr(not(unix), allow(unused_variables))] // only the unix branch uses the paths
fn apply_file_state(dir: &Path, config_path: &Path, args: &Args) -> anyhow::Result<()> {
    let want_invalid = args.file_state == Some(FileState::Invalid);
    let want_unreadable = args.file_state == Some(FileState::Unreadable)
        || args.identity == IdentityMode::FileProblem;
    let want_readonly_dir = args.file_state == Some(FileState::Readonly);
    if want_invalid {
        std::fs::write(config_path, b"{ broken")?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if want_unreadable {
            std::fs::set_permissions(config_path, std::fs::Permissions::from_mode(0o000))?;
        }
        if want_readonly_dir {
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555))?;
        }
    }
    Ok(())
}

/// Step 6/7: the remaining --cli-override values are already in `cli`; the second run uses CliOverrides and a file
/// that is corrupted (or not), then node.name (it has no field of its own in CliOverrides) is added to the
/// "touched by CLI" list by hand if the flag was passed.
fn load_config_with_overrides(
    config_path: &Path,
    cli: &CliOverrides,
    args: &Args,
    now: u64,
) -> anyhow::Result<config_file::RunConfig> {
    let mut run = config_file::prepare(
        &ConfigSource::File {
            path: config_path.to_path_buf(),
            may_create: true,
            rule: PathRule::Explicit,
        },
        cli,
        false,
        now,
    )?;
    let node_name_flagged = args
        .cli_override
        .iter()
        .any(|e| e.split_once('=').map(|(p, _)| p).unwrap_or(e.as_str()) == "node.name");
    if node_name_flagged {
        run.cli_override_paths.push("node.name");
    }
    Ok(run)
}

/// Seven steps of preparing the demo settings file. Returns the ready `RunConfig` and, if a
/// temporary directory with the file was created, its path (to clean up on exit).
pub(crate) fn prepare_demo_config(
    args: &Args,
    now: u64,
) -> anyhow::Result<(config_file::RunConfig, Option<PathBuf>)> {
    let mut cli = parse_cli_overrides(&args.cli_override);

    if args.identity == IdentityMode::Declined {
        return prepare_declined_config(args, &mut cli, now);
    }

    let dir = std::env::temp_dir().join(format!("mesh-web-demo-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let config_path = dir.join("config.json");

    create_default_config_file(&config_path, now)?;
    write_config_fields(&config_path, args)?;
    apply_file_preserved_identity(&dir, &config_path, args)?;
    apply_file_state(&dir, &config_path, args)?;

    let run = load_config_with_overrides(&config_path, &cli, args, now)?;
    Ok((run, Some(dir)))
}
