//! `ConfigState::save_network_key` — changing the network key from the page: without a settings file it lives until the
//! end of the run, with a file it is written to disk the same way as `put` (see `put.rs`).

use std::path::Path;

use axum::response::Response;

use crate::config::config_file::{self, FileState, LoadedConfigFile, SaveError, Settings};
use crate::node::output::NET_NAME;
use crate::web_gui::now_unix;

use super::state::ConfigState;

impl ConfigState {
    pub(crate) fn save_network_key(&mut self, key: &str, keep_ports: bool) -> Result<(), Response> {
        let Some(path) = self.config_file_path.clone() else {
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

        match write_network_key(self, &path, key, keep_ports) {
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

/// Writing the new network key; the method depends on what lies in the file now.
fn write_network_key(
    state: &ConfigState,
    path: &Path,
    key: &str,
    keep_ports: bool,
) -> Result<(u64, Settings), SaveError> {
    let loaded = config_file::load_config_file(path);
    match &loaded {
        LoadedConfigFile::Parsed(p) => {
            let settings = switched_settings(&p.settings, key, keep_ports);
            config_file::save_config_file(path, Some(p.rev), now_unix(), &state.secret, &settings)
                .map(|rev| (rev, settings))
        }
        LoadedConfigFile::Absent => {
            let settings = switched_settings(&state.file_settings, key, keep_ports);
            config_file::save_with_floor(
                path,
                None,
                state.file_report.rev,
                now_unix(),
                &state.secret,
                &settings,
            )
            .map(|rev| (rev, settings))
        }
        LoadedConfigFile::Unreadable(_) | LoadedConfigFile::Invalid(_) => {
            write_network_key_after_bad_file(state, path, key, keep_ports)
        }
    }
}

/// The file is read as damaged: first set aside, then written as new.
fn write_network_key_after_bad_file(
    state: &ConfigState,
    path: &Path,
    key: &str,
    keep_ports: bool,
) -> Result<(u64, Settings), SaveError> {
    let dest = config_file::set_aside_bad(path, now_unix()).map_err(SaveError::Io)?;
    crate::node::output::say(
        "web",
        format!(
            "page: broken settings file moved aside to {}",
            dest.display()
        ),
    );
    let settings = switched_settings(&state.file_settings, key, keep_ports);
    config_file::save_with_floor(
        path,
        None,
        state.file_report.rev,
        now_unix(),
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
