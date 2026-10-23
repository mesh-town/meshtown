//! Joining by invite in the `run` command: decides whether to spend the invite, waits for the network key from
//! members and writes the received key to the settings file.
//! What it does not do: start the node or parse the invite string (that is `crate::invite`).

use portable_atomic::AtomicU64;
use std::path::Path;
use std::sync::atomic::Ordering;

use crate::config::config_file::{self, LoadedConfigFile, RunConfig, SaveError, Settings};
use crate::invite::redeem::{redeem, Progress, RedeemError, RedeemOpts};
use crate::invite::{check_fresh, same_network, ParsedInvite};
use crate::node::host::MAX_NETS;
use crate::node::output::{say, NET_NAME};
use crate::transport::net::NodeSecret;

use super::run_invite_addnet::{
    net_limit_reached, prepare_net_slot, save_prepared_slot, saved_slot_keys,
};

/// What to do with the invite in this run.
pub(crate) enum InviteStep {
    /// There is no invite.
    None,
    /// The key of this network is already there (`--key`, the settings file or a network slot): the invite is not spent.
    KeepKey,
    /// There is no key: get it from the members by the invite and save it in the settings file.
    Redeem(ParsedInvite),
    /// The node has the key of another network: get the key by the invite and keep it as a separate network.
    AddNet(ParsedInvite),
}

const TEXT_WAITING: &str =
    "Waiting for network members: as soon as anyone from the invite is online, \
the node will join the network.";

/// Decides the fate of the invite before any trip to the network. A refusal does not spend the invite.
pub(crate) fn plan_invite(
    inv: Option<ParsedInvite>,
    cli_key: Option<&str>,
    file_key: Option<&str>,
    slot_keys: &[Option<String>],
    now: u64,
) -> anyhow::Result<InviteStep> {
    let Some(inv) = inv else {
        return Ok(InviteStep::None);
    };
    if let Some(key) = cli_key {
        require_cli_key_matches_invite(&inv, key)?;
        return Ok(InviteStep::KeepKey);
    }
    let Some(file_key) = file_key else {
        check_fresh(&inv.head, now)?;
        return Ok(InviteStep::Redeem(inv));
    };
    if same_network(&inv.head, file_key).unwrap_or(false) || known_in_slots(&inv, slot_keys) {
        return Ok(InviteStep::KeepKey);
    }
    if net_limit_reached(slot_keys) {
        anyhow::bail!("too many networks (limit {MAX_NETS}); the invite was not spent");
    }
    check_fresh(&inv.head, now)?;
    Ok(InviteStep::AddNet(inv))
}

fn known_in_slots(inv: &ParsedInvite, slot_keys: &[Option<String>]) -> bool {
    slot_keys
        .iter()
        .flatten()
        .any(|k| same_network(&inv.head, k).unwrap_or(false))
}

fn require_cli_key_matches_invite(inv: &ParsedInvite, key: &str) -> anyhow::Result<bool> {
    if same_network(&inv.head, key)? {
        return Ok(true);
    }
    anyhow::bail!(
        "the key from --key (or MESHTOWN_KEY) and the invite are from different {NET_NAME} networks; the invite was not spent"
    )
}

/// Everything about the invite in `run`: the decision, joining or the "key already known" mark, the log notes.
/// Returns the parsed string: the label and the members' addresses for the first join are taken from it;
/// `None` when the invite went to another network slot, which the main network has no use for.
pub(crate) async fn settle_invite(
    inv: Option<ParsedInvite>,
    cli_key: Option<&str>,
    run: &mut RunConfig,
    typed_domains: &[String],
    now: u64,
) -> anyhow::Result<Option<ParsedInvite>> {
    let file_key = run.file_settings.net.network_key.clone();
    let slot_keys = run
        .file_report
        .path
        .as_deref()
        .map(saved_slot_keys)
        .unwrap_or_default();
    let step = plan_invite(inv.clone(), cli_key, file_key.as_deref(), &slot_keys, now)?;
    for note in inv.iter().flat_map(|p| p.warnings.iter()) {
        say("invite", note);
    }
    match step {
        InviteStep::None => {}
        InviteStep::KeepKey => say(
            "invite",
            "this network's key is already known; the invite was not spent",
        ),
        InviteStep::Redeem(p) => redeem_for_run(&p, run, now).await?,
        InviteStep::AddNet(p) => {
            add_redeemed_net(&p, run, typed_domains, now).await?;
            return Ok(None);
        }
    }
    Ok(inv)
}

/// Joins by the invite and keeps the network as a slot; a failed save is a warning, the run goes on without it.
async fn add_redeemed_net(
    p: &ParsedInvite,
    run: &RunConfig,
    typed_domains: &[String],
    now: u64,
) -> anyhow::Result<()> {
    let slot = prepare_net_slot(run, now)?;
    let text = redeem_text(p, &slot.run.secret).await?;
    say("invite", "network key received, adding the network");
    if let Err(why) = save_prepared_slot(run, slot, &text, typed_domains, &p.peers, now) {
        say(
            "invite",
            format!("network key not saved: {why:#}; the next run needs a new invite"),
        );
    }
    Ok(())
}

/// Waits for the key from the members, puts it into the run settings and into the settings file.
pub(crate) async fn redeem_for_run(
    p: &ParsedInvite,
    run: &mut RunConfig,
    now: u64,
) -> anyhow::Result<()> {
    let text = redeem_text(p, &run.secret).await?;
    run.effective_settings.net.network_key = Some(text.clone());
    say("invite", "network key received, joining the network");
    persist_key(run, &text, now)
}

/// Waits for the key from the members, redeeming the invite with `secret`, and returns it as the text a settings file holds.
async fn redeem_text(
    p: &ParsedInvite,
    secret: &NodeSecret,
) -> anyhow::Result<String> {
    say("invite", TEXT_WAITING);
    let opts = RedeemOpts::for_head(&p.head);
    let last = AtomicU64::new(0);
    let note = move |pr: Progress| report_progress(&last, pr);
    let transport = crate::node::bind_ephemeral(None)
        .await
        .map_err(|e| RedeemError::Bind(format!("{e:#}")))?;
    let key = redeem(&p.head, &p.peers, &opts, &note, transport, secret).await?;
    Ok(p.head.key_text(&key))
}

/// A progress line: only when the pause between rounds has changed.
fn report_progress(last: &AtomicU64, pr: Progress) {
    let Progress::Waiting(pause) = pr else {
        return;
    };
    if last.swap(pause.as_secs(), Ordering::Relaxed) == pause.as_secs() {
        return;
    }
    say(
        "invite",
        format!(
            "members haven't answered yet; next attempt in {} s",
            pause.as_secs()
        ),
    );
}

const TEXT_NO_FILE: &str = "network key not saved: running without a settings file; the next \
run needs a new invite";

/// Writes the received key into the settings file itself (only `network.key` changes, the run flags
/// do not get into the file) and updates `run.file_settings` and the revision. A failure is a warning, the run goes on.
fn persist_key(run: &mut RunConfig, key_text: &str, now: u64) -> anyhow::Result<()> {
    let Some(path) = run.file_report.path.clone() else {
        say("invite", TEXT_NO_FILE);
        return Ok(());
    };
    match write_key(run, &path, key_text, now) {
        Ok((rev, settings)) => {
            run.file_settings = settings;
            run.file_report.rev = rev;
        }
        Err(why) => say(
            "invite",
            format!(
                "network key not saved to the settings file {}: {why}; the next \
                run needs a new invite",
                path.display()
            ),
        ),
    }
    Ok(())
}

fn write_key(
    run: &RunConfig,
    path: &Path,
    key_text: &str,
    now: u64,
) -> Result<(u64, Settings), String> {
    if run.file_report.overwrite_refused {
        return Err("file left untouched: overwriting would lose data".to_string());
    }
    let saved = match config_file::load_config_file(path) {
        LoadedConfigFile::Parsed(p) => {
            let s = with_key(p.settings, key_text);
            config_file::save_config_file(path, Some(p.rev), now, &run.secret, &s).map(|r| (r, s))
        }
        LoadedConfigFile::Absent => {
            let s = with_key(run.file_settings.clone(), key_text);
            let floor = run.file_report.rev;
            config_file::save_with_floor(path, None, floor, now, &run.secret, &s).map(|r| (r, s))
        }
        LoadedConfigFile::Unreadable(m) | LoadedConfigFile::Invalid(m) => {
            return Err(format!("file is unreadable: {m}"))
        }
    };
    saved.map_err(save_text)
}

fn with_key(mut settings: Settings, key_text: &str) -> Settings {
    settings.net.network_key = Some(key_text.to_string());
    settings
}

pub(super) fn save_text(e: SaveError) -> String {
    match e {
        SaveError::StaleRev(_) => "file changed during the write".to_string(),
        SaveError::Io(t) => t,
    }
}

#[cfg(test)]
#[path = "run_invite_tests.rs"]
mod tests;
