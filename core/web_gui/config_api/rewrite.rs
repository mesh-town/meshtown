//! Writes of a slot's settings file that the page makes on its own initiative (not for a request of that slot):
//! the exit cleanup of a slot that gives up the proxy, and moving the applied-socks baseline.

use crate::config::config_file::{LoadedSettings, ProcessSettings, SaveError, Settings};
use crate::web_gui::{now_unix, NodeControl};

use super::state::ConfigState;

impl ConfigState {
    /// Applies `edit` to the settings in the file and saves the result at the current revision, then hands
    /// the new route exits to the node like a `PUT` would. `Ok(false)`: the edit changed nothing and
    /// the file was not touched. A missing or unreadable file, or a revision that moved, is an `Err`.
    pub(crate) fn rewrite_settings(
        &mut self,
        node: &dyn NodeControl,
        edit: impl FnOnce(&mut Settings),
    ) -> Result<bool, String> {
        let Some(file) = self.file.clone() else {
            return Err("the settings file is not used".to_string());
        };
        let LoadedSettings::Parsed { rev, settings } = file.load(&self.file_settings) else {
            return Err("the settings file can't be read".to_string());
        };
        let mut new = settings.clone();
        edit(&mut new);
        if new == settings {
            return Ok(false);
        }
        let saved = file.save(Some(rev), now_unix(), &self.secret, &new);
        let new_rev = saved.map_err(|e| match e {
            SaveError::StaleRev(_) => "the settings file changed during the write".to_string(),
            SaveError::Io(text) => text,
        })?;
        self.file_settings = new;
        self.file_report.rev = new_rev;
        node.apply_config(&self.file_settings, &["route.exits"]);
        Ok(true)
    }

    /// Records that `applied` socks settings are what the process runs now, so they stop counting as
    /// waiting for a restart.
    pub(crate) fn settle_socks_baseline(&mut self, applied: &ProcessSettings) {
        let boot = &mut self.boot_file_settings.process;
        boot.socks_enabled = applied.socks_enabled;
        boot.socks_port = applied.socks_port;
        boot.socks_access = applied.socks_access;
        boot.socks_ifaces = applied.socks_ifaces.clone();
    }
}

#[cfg(test)]
#[path = "rewrite_tests.rs"]
mod tests;
