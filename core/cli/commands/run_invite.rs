//! Joining by invite in the `run` command: decides whether to spend the invite, waits for the network key from
//! members and writes the received key to the settings file.
//! What it does not do: start the node or parse the invite string (that is `crate::invite`).

use portable_atomic::AtomicU64;
use std::path::Path;
use std::sync::atomic::Ordering;

use crate::config::config_file::{self, LoadedConfigFile, RunConfig, SaveError, Settings};
use crate::invite::redeem::{redeem, Progress, RedeemError, RedeemOpts};
use crate::invite::{check_fresh, same_network, ParsedInvite};
use crate::node::output::{say, NET_NAME};

/// What to do with the invite in this run.
pub(crate) enum InviteStep {
    /// There is no invite.
    None,
    /// The key of this network is already there (`--key` or the settings file): the invite is not spent.
    KeepKey,
    /// There is no key: get it from the members by the invite.
    Redeem(ParsedInvite),
}

const TEXT_WAITING: &str =
    "Waiting for network members: as soon as anyone from the invite is online, \
the node will join the network.";

/// Decides the fate of the invite before any trip to the network. A refusal does not spend the invite.
pub(crate) fn plan_invite(
    inv: Option<ParsedInvite>,
    cli_key: Option<&str>,
    file_key: Option<&str>,
    save_config: bool,
    now: u64,
) -> anyhow::Result<InviteStep> {
    let Some(inv) = inv else {
        return Ok(InviteStep::None);
    };
    if key_is_known(&inv, cli_key, file_key, save_config)? {
        return Ok(InviteStep::KeepKey);
    }
    check_fresh(&inv.head, now)?;
    Ok(InviteStep::Redeem(inv))
}

/// Whether the key of the same network as in the invite is already known; the key of another network, which cannot be replaced, is a refusal.
fn key_is_known(
    inv: &ParsedInvite,
    cli_key: Option<&str>,
    file_key: Option<&str>,
    save_config: bool,
) -> anyhow::Result<bool> {
    if let Some(key) = cli_key {
        return require_cli_key_matches_invite(inv, key);
    }
    let Some(key) = file_key else {
        return Ok(false);
    };
    if same_network(&inv.head, key).unwrap_or(false) {
        return Ok(true);
    }
    if save_config {
        return Ok(false);
    }
    anyhow::bail!(
        "the settings file has the key of another {NET_NAME} network; to join with the invite, add \
        --save-config (the key in the file will be replaced); the invite was not spent"
    )
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
/// Returns the parsed string: the label and the members' addresses for the first join are taken from it.
pub(crate) async fn settle_invite(
    inv: Option<ParsedInvite>,
    cli_key: Option<&str>,
    run: &mut RunConfig,
    save_config: bool,
    now: u64,
) -> anyhow::Result<Option<ParsedInvite>> {
    let file_key = run.file_settings.net.network_key.clone();
    let step = plan_invite(inv.clone(), cli_key, file_key.as_deref(), save_config, now)?;
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
    }
    Ok(inv)
}

/// Waits for the key from the members, puts it into the run settings and into the settings file.
pub(crate) async fn redeem_for_run(
    p: &ParsedInvite,
    run: &mut RunConfig,
    now: u64,
) -> anyhow::Result<()> {
    say("invite", TEXT_WAITING);
    let opts = RedeemOpts::for_head(&p.head, run.effective_settings.net.uses_relay());
    let last = AtomicU64::new(0);
    let note = move |pr: Progress| report_progress(&last, pr);
    let transport = crate::node::bind_ephemeral(None)
        .await
        .map_err(|e| RedeemError::Bind(format!("{e:#}")))?;
    let key = redeem(&p.head, &p.peers, &opts, &note, transport, &run.secret).await?;
    let text = p.head.key_text(&key);
    run.effective_settings.net.network_key = Some(text.clone());
    say("invite", "network key received, joining the network");
    persist_key(run, &text, now)
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

fn save_text(e: SaveError) -> String {
    match e {
        SaveError::StaleRev(_) => "file changed during the write".to_string(),
        SaveError::Io(t) => t,
    }
}

#[cfg(test)]
#[path = "run_invite_tests.rs"]
mod tests;
