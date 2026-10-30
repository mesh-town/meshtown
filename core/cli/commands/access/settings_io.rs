//! Shared input and output of the `access` commands: the file edit used when no node is running and the
//! choice between the node and the file.

use std::path::{Path, PathBuf};

use crate::cli::args::Globals;
use crate::cli::commands::client::{Node, Target};
use crate::cli::exit::NotRunning;
use crate::config::config_file::{self, LoadedConfigFile, ParsedConfigFile, SecretRead, Settings};
use crate::node::output::NET_NAME;

/// The settings file that `--config` or the standard place names; none is an error with the reason.
pub(super) fn settings_path(g: &Globals) -> anyhow::Result<PathBuf> {
    let target = Target::resolve(g.config.clone(), g.web_port).map_err(anyhow::Error::msg)?;
    Ok(target.path)
}

/// Whether a failed connection means "no node on the page port" (and only that).
pub(super) fn node_is_absent(e: &anyhow::Error) -> bool {
    e.chain().any(|c| c.downcast_ref::<NotRunning>().is_some())
}

/// Where a command acts: on the running node, or on the settings file when no node answers.
pub(super) enum Route {
    Live(Node),
    File(PathBuf),
}

/// The settings file comes first (no place for one is an error), then the node; only "no node on the
/// port" falls back to the file, every other refusal, a wrong token included, is the answer.
pub(super) fn route(g: &Globals) -> anyhow::Result<Route> {
    let path = settings_path(g)?;
    match Node::connect(g) {
        Ok(node) => Ok(Route::Live(node)),
        Err(e) if node_is_absent(&e) => Ok(Route::File(path)),
        Err(e) => Err(e),
    }
}

/// Reads the settings file; a missing or damaged one is an error that says what to do.
pub(super) fn load(path: &Path, what: &str) -> anyhow::Result<ParsedConfigFile> {
    match config_file::load_config_file(path) {
        LoadedConfigFile::Parsed(p) => Ok(p),
        LoadedConfigFile::Absent => {
            anyhow::bail!("no settings file: start {NET_NAME} once, then change the {what}")
        }
        LoadedConfigFile::Unreadable(why) | LoadedConfigFile::Invalid(why) => {
            anyhow::bail!("settings file is unusable: {why}")
        }
    }
}

/// Reads the file, change the settings and writes them back, checking that nobody wrote in between.
/// A missing file is not created: a node that never started has no key to protect it.
pub(super) fn edit_file(
    path: &Path,
    what: &str,
    f: impl FnOnce(&mut Settings),
) -> anyhow::Result<()> {
    let parsed = load(path, what)?;
    let SecretRead::Valid(secret) = parsed.secret else {
        anyhow::bail!("the settings file has no valid key for this computer: start {NET_NAME} once")
    };
    let mut settings = parsed.settings;
    f(&mut settings);
    let now = crate::cli::commands::now_unix();
    config_file::save_config_file(path, Some(parsed.rev), now, &secret, &settings)
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("settings file was not saved: {e:?}"))
}

#[cfg(test)]
#[path = "settings_io_tests.rs"]
mod tests;
