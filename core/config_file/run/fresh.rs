//! Creating a settings file "from scratch": the shared path for the first `run` and for `ifaces --if-unset`
//! on a fresh install. Permissions and directory (0700/0600) come from `save_config_file`.

use crate::net::NodeSecret;
use std::path::Path;

use super::super::save::{save_config_file, SaveError};
use super::super::settings::Settings;
use super::helpers::new_secret;
use crate::words::random_node_name;

/// What came of creation: the settings and secret that were tried to write, and the write outcome
/// (a revision or a refusal).
pub struct Fresh {
    pub settings: Settings,
    pub secret: NodeSecret,
    pub saved: Result<u64, SaveError>,
}

/// Defaults adjusted by `tweak`; a random two-word node name if `tweak` did not set one;
/// a new secret; written as revision 1. An existing file is not checked — that is the caller's job.
pub fn create_fresh(
    path: &Path,
    now_unix: u64,
    tweak: impl FnOnce(&mut Settings),
) -> anyhow::Result<Fresh> {
    let mut settings = Settings::default();
    tweak(&mut settings);
    if settings.net.node_name.is_none() {
        settings.net.node_name = Some(random_node_name()?);
    }
    let secret = new_secret()?;
    let saved = save_config_file(path, None, now_unix, &secret, &settings);
    Ok(Fresh {
        settings,
        secret,
        saved,
    })
}

#[cfg(test)]
#[path = "fresh_tests.rs"]
mod tests;
