//! `join` when no node answers on the page port: the key or the invite goes straight into the settings file.
//! The page port is held for the whole exchange, so a node that starts meanwhile finds it busy instead of
//! working beside this command with the same identity.
//! What is not here: the live node (`join.rs`) and the exchange itself (`run_invite.rs`).

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cli::args::Globals;
use crate::cli::program_name::program_name;
use crate::config::config_file::{
    load_config_file, CliOverrides, ConfigSource, LoadedConfigFile, Settings,
};
use crate::invite::{check_fresh, ParsedInvite};
use crate::members::network_key::parse_user_input;
use crate::web_gui::auth::access::bind_ip;

use super::client::Target;
use super::run_invite::{persist_key, plan_invite, InviteStep};
use super::run_invite_addnet::{
    in_slots, peek_file_key, plan_cli_key, same_key_network, saved_slot_keys, KeyNet,
};
use super::{now_unix, prepare_run, StartInput};

/// What the person typed after `join`, already parsed.
pub(super) enum JoinText {
    Invite(ParsedInvite),
    Key(String),
}

/// The outcome of a join into the settings file.
pub(super) struct Joined {
    pub label: String,
    pub path: PathBuf,
    pub already: bool,
}

impl JoinText {
    /// The label of the network, or the first eight hex digits of its identifier when it has none.
    pub(super) fn label(&self) -> String {
        match self {
            JoinText::Invite(inv) => label_or_id(inv.head.label.clone(), &inv.head.network_id),
            JoinText::Key(key) => match parse_user_input(key) {
                Ok(p) => label_or_id(p.label, &p.key.subkeys().network_id),
                Err(_) => String::new(),
            },
        }
    }
}

fn label_or_id(label: Option<String>, network_id: &[u8; 4]) -> String {
    label.unwrap_or_else(|| data_encoding::HEXLOWER.encode(network_id))
}

/// Joins without a node and prints the result: the label on stdout, the next step on stderr.
pub(super) async fn run_offline(g: &Globals, text: JoinText, timeout: u64) -> anyhow::Result<()> {
    crate::node::output::log_to_stderr();
    let target = Target::resolve(g.config.clone(), g.web_port).map_err(anyhow::Error::msg)?;
    let source = super::resolve_source(false, g.config.as_deref());
    let now = now_unix();
    if let JoinText::Invite(inv) = &text {
        check_fresh(&inv.head, now)?;
    }
    let _page_port = hold_page_port(&target.path, target.port)?;
    let limit = Duration::from_secs(timeout);
    let joined = match tokio::time::timeout(limit, join_offline(&source, text, now)).await {
        Ok(done) => done?,
        Err(_) => anyhow::bail!(
            "no network member answered within {timeout} s; try again when one is online"
        ),
    };
    report(&joined);
    Ok(())
}

fn report(joined: &Joined) {
    if joined.already {
        eprintln!("already in this network");
        return;
    }
    println!("{}", joined.label);
    eprintln!(
        "Saved to {}. Start Mesh Town: {} run, the service, or the icon.",
        joined.path.display(),
        program_name()
    );
}

/// Takes the page port for the exchange. A busy port means a node is starting or running there.
pub(super) fn hold_page_port(path: &Path, port: u16) -> anyhow::Result<TcpListener> {
    let access = match load_config_file(path) {
        LoadedConfigFile::Parsed(p) => p.settings.process.web_access,
        _ => Settings::default().process.web_access,
    };
    TcpListener::bind((bind_ip(access), port)).map_err(|e| match e.kind() {
        std::io::ErrorKind::AddrInUse => anyhow::anyhow!(
            "port {port} is in use: Mesh Town may be starting; run join again in a minute"
        ),
        _ => anyhow::anyhow!("cannot hold port {port} while joining: {e}"),
    })
}

/// Puts the network into the settings file: the main network when the file has none, otherwise a network slot.
pub(super) async fn join_offline(
    source: &ConfigSource,
    text: JoinText,
    now: u64,
) -> anyhow::Result<Joined> {
    let path = file_path(source)?.to_path_buf();
    require_readable(&path)?;
    let label = text.label();
    let already = match text {
        JoinText::Key(key) => join_key(source, &path, &key, now).await?,
        JoinText::Invite(inv) => join_invite(source, &path, inv, now).await?,
    };
    Ok(Joined {
        label,
        path,
        already,
    })
}

fn file_path(source: &ConfigSource) -> anyhow::Result<&Path> {
    match source {
        ConfigSource::File { path, .. } => Ok(path),
        ConfigSource::Nowhere(why) => anyhow::bail!("no place for a settings file: {why}"),
        ConfigSource::Disabled => anyhow::bail!("no place for a settings file: it is turned off"),
    }
}

/// A file that exists but cannot be read is left as it is: the join would have to overwrite it.
fn require_readable(path: &Path) -> anyhow::Result<()> {
    match load_config_file(path) {
        LoadedConfigFile::Unreadable(why) | LoadedConfigFile::Invalid(why) => anyhow::bail!(
            "the settings file {} cannot be read ({why}); fix or move it, then run join again",
            path.display()
        ),
        _ => Ok(()),
    }
}

/// Whether the network of `key` is already in the file or in a slot.
fn key_known(path: &Path, key: &str) -> bool {
    key_saved(path, key) || in_slots(key, &saved_slot_keys(path))
}

/// Whether the settings file holds the key of the same network as `key`.
fn key_saved(path: &Path, key: &str) -> bool {
    peek_file_key(path).is_some_and(|k| same_key_network(&k, key))
}

/// A key: returns whether the network was already there.
async fn join_key(source: &ConfigSource, path: &Path, key: &str, now: u64) -> anyhow::Result<bool> {
    if key_known(path, key) {
        return Ok(true);
    }
    let net = plan_cli_key(source, Some(key))?;
    anyhow::ensure!(net != KeyNet::Unreadable, "the network key does not parse");
    let overrides = CliOverrides {
        network_key: Some(key.to_string()),
        ..CliOverrides::default()
    };
    let (mut run, _, _) = prepare_run(&StartInput::JOIN, overrides, None, source, now).await?;
    if net == KeyNet::Main {
        persist_key(&mut run, key, now)?;
        anyhow::ensure!(
            key_saved(path, key),
            "the network key was not saved to {}",
            path.display()
        );
    }
    Ok(false)
}

/// An invite: returns whether the network was already there (the invite is then not spent).
async fn join_invite(
    source: &ConfigSource,
    path: &Path,
    inv: ParsedInvite,
    now: u64,
) -> anyhow::Result<bool> {
    let step = plan_invite(
        Some(inv.clone()),
        None,
        peek_file_key(path).as_deref(),
        &saved_slot_keys(path),
        now,
    )?;
    if matches!(step, InviteStep::KeepKey) {
        return Ok(true);
    }
    prepare_run(
        &StartInput::JOIN,
        CliOverrides::default(),
        Some(inv.clone()),
        source,
        now,
    )
    .await?;
    let kept = plan_invite(
        Some(inv),
        None,
        peek_file_key(path).as_deref(),
        &saved_slot_keys(path),
        now,
    );
    anyhow::ensure!(
        matches!(kept, Ok(InviteStep::KeepKey)),
        "the network key was not saved to {}; the invite is spent — ask for a new one",
        path.display()
    );
    Ok(false)
}

#[cfg(test)]
#[path = "join_offline_tests.rs"]
mod tests;
