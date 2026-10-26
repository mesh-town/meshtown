//! The `keygen` command: prints a new network key whose root, the first admin, is the node identity of the
//! settings file (the same identity `run` takes from it).

use std::path::{Path, PathBuf};

use crate::config::config_file::{
    self, ConfigSource, LoadedConfigFile, ParsedConfigFile, SecretRead,
};
use crate::members::network_key::{
    format_managed_key, managed_key_with_label, normalize_label, NetworkKey,
};
use crate::members::words::generate_tail;
use crate::node::output::NET_NAME;
use crate::transport::net::PeerId;

/// Prints one key line: the words form by default, the machine form `mesh2-…` with `--raw`.
pub(crate) fn keygen(label: Vec<String>, raw: bool, config: Option<PathBuf>) -> anyhow::Result<()> {
    let root = node_identity(config.as_deref(), super::now_unix())?;
    println!("{}", new_key(&label, raw, &root)?);
    Ok(())
}

fn new_key(label: &[String], raw: bool, root: &PeerId) -> anyhow::Result<String> {
    if raw {
        return Ok(managed_key_with_label(
            None,
            &NetworkKey::generate()?.to_bytes(),
            root,
        ));
    }
    let label = match label.is_empty() {
        true => None,
        false => Some(normalize_label(&label.join(" "))?),
    };
    Ok(format_managed_key(
        label.as_deref(),
        &generate_tail()?,
        root,
    ))
}

/// The node identity of the settings file; a missing file is created, the way the first `run` does it.
fn node_identity(config: Option<&Path>, now: u64) -> anyhow::Result<PeerId> {
    let env = config_file::current_env();
    match config_file::resolve_config_source(config, &env.as_path_env()) {
        ConfigSource::File { path, .. } => identity_at(&path, now),
        ConfigSource::Nowhere(why) => anyhow::bail!("no place for the settings file: {why}"),
        ConfigSource::Disabled => {
            anyhow::bail!("no place for the settings file: the settings file is disabled")
        }
    }
}

fn identity_at(path: &Path, now: u64) -> anyhow::Result<PeerId> {
    match config_file::load_config_file(path) {
        LoadedConfigFile::Parsed(p) => parsed_identity(path, p),
        LoadedConfigFile::Absent => create_identity(path, now),
        LoadedConfigFile::Unreadable(why) | LoadedConfigFile::Invalid(why) => {
            anyhow::bail!("settings file is unusable: {why}")
        }
    }
}

fn parsed_identity(path: &Path, p: ParsedConfigFile) -> anyhow::Result<PeerId> {
    match p.secret {
        SecretRead::Valid(secret) => Ok(secret.peer_id()),
        SecretRead::Missing | SecretRead::Bad(_) => anyhow::bail!(
            "the settings file {} has no identity for this computer yet: start {NET_NAME} once ({} run), then run keygen again",
            path.display(),
            crate::cli::program_name::program_name()
        ),
    }
}

fn create_identity(path: &Path, now: u64) -> anyhow::Result<PeerId> {
    let fresh = config_file::create_fresh(path, now, |_| {})?;
    if let Err(e) = fresh.saved {
        anyhow::bail!("the settings file {} was not saved: {e:?}", path.display());
    }
    Ok(fresh.secret.peer_id())
}

#[cfg(test)]
#[path = "keygen_tests.rs"]
mod tests;
