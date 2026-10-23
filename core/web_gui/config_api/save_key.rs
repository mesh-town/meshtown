//! `ConfigState::save_network_key` — changing the network key from the page: without a settings file it lives until the
//! end of the run, with a file it is written to disk the same way as `put` (see `put.rs`).

use axum::response::Response;

use crate::transport::net::NodeSecret;

use crate::config::config_file::{FileState, LoadedSettings, SaveError, Settings, SettingsFile};
use crate::node::output::NET_NAME;

use super::settings_access::write_loaded;
use super::state::ConfigState;

impl ConfigState {
    pub(crate) fn save_network_key(&mut self, key: &str, keep_ports: bool) -> Result<(), Response> {
        let Some(file) = self.file.clone() else {
            crate::node::output::say(
                "web",
                format!("page: there is no settings file, the new network key {NET_NAME} lives until the end of this run"),
            );
            self.current_network_key = Some(key.to_string());
            return Ok(());
        };
        if self.file_report.overwrite_refused {
            let msg = self.file_report.message.clone().unwrap_or_else(|| {
                "the settings file was left untouched: overwriting would lose data".to_string()
            });
            return Err(self.config_write_failed(&msg));
        }

        match write_network_key(self, file.as_ref(), key, keep_ports) {
            Ok((new_rev, settings)) => {
                self.file_settings = settings;
                self.file_report.state = FileState::Ok;
                self.file_report.message = None;
                self.file_report.field_errors.clear();
                self.file_report.rev = new_rev;
                self.file_report.persistent_identity = true;
                self.file_report.overwrite_refused = false;
                self.current_network_key = Some(key.to_string());
                Ok(())
            }
            Err(SaveError::StaleRev(_)) => {
                Err(self
                    .config_write_failed("the settings file changed during the write, try again"))
            }
            Err(SaveError::Io(t)) => Err(self.config_write_failed(&t)),
        }
    }
}

impl ConfigState {
    /// The main network was left: no key, no route exits, no published ports, and the new node secret. The file is
    /// rewritten the same way as for a key change; without a file, or when `keep_file` is set (the old files could
    /// not be set aside), only the memory changes. Returns the new file revision when the file was written.
    pub(crate) fn drop_network(
        &mut self,
        secret: NodeSecret,
        _now_unix: u64,
        keep_file: bool,
    ) -> Result<Option<u64>, SaveError> {
        let mut settings = self.file_settings.clone();
        settings.net.network_key = None;
        settings.net.route_exits.clear();
        settings.net.ports_published.clear();
        let written = match self.file.clone().filter(|_| !keep_file) {
            Some(file) => Some(write_dropped(self, file.as_ref(), &secret, &settings)?),
            None => None,
        };
        self.secret = secret;
        self.file_settings = settings;
        self.current_network_key = None;
        if let Some(rev) = written {
            self.file_report.rev = rev;
        }
        Ok(written)
    }
}

fn write_dropped(
    state: &ConfigState,
    file: &dyn SettingsFile,
    secret: &NodeSecret,
    settings: &Settings,
) -> Result<u64, SaveError> {
    let loaded = file.load(&state.file_settings);
    let expected_rev = match &loaded {
        LoadedSettings::Parsed { rev, .. } => Some(*rev),
        _ => None,
    };
    let rev_floor = state.file_report.rev;
    write_loaded(file, &loaded, expected_rev, rev_floor, secret, settings)
}

/// Writing the new network key; the method depends on what lies in the file now.
fn write_network_key(
    state: &ConfigState,
    file: &dyn SettingsFile,
    key: &str,
    keep_ports: bool,
) -> Result<(u64, Settings), SaveError> {
    let loaded = file.load(&state.file_settings);
    let (base, expected_rev) = match &loaded {
        LoadedSettings::Parsed { rev, settings } => (settings, Some(*rev)),
        _ => (&state.file_settings, None),
    };
    let settings = switched_settings(base, key, keep_ports);
    let rev_floor = state.file_report.rev;
    write_loaded(
        file,
        &loaded,
        expected_rev,
        rev_floor,
        &state.secret,
        &settings,
    )
    .map(|rev| (rev, settings))
}

/// Settings after a network change: the new key; `ports.published` is cleared unless the owner did not
/// keep the publications, `ports.opened` stays; `route.exits` is always cleared.
fn switched_settings(base: &Settings, key: &str, keep_ports: bool) -> Settings {
    let mut settings = base.clone();
    settings.net.network_key = Some(key.to_string());
    clear_published(&mut settings, keep_ports);
    clear_route_exits(&mut settings);
    settings
}

fn clear_published(settings: &mut Settings, keep_ports: bool) {
    if keep_ports || settings.net.ports_published.is_empty() {
        return;
    }
    crate::node::output::say(
        "web",
        format!(
            "page: network changed without publications, ports removed: {}",
            settings.net.ports_published.len()
        ),
    );
    settings.net.ports_published.clear();
}

fn clear_route_exits(settings: &mut Settings) {
    if settings.net.route_exits.is_empty() {
        return;
    }
    crate::node::output::say(
        "web",
        format!(
            "page: network changed, route exits removed: {}",
            settings.net.route_exits.len()
        ),
    );
    settings.net.route_exits.clear();
}

#[cfg(test)]
#[path = "save_key_tests.rs"]
mod tests;
