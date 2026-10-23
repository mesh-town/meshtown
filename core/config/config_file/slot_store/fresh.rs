//! Building the start settings of a network slot that has no file yet: it is added at run time and
//! gets its file with the first network key.

use std::sync::Arc;

use super::layout::net_slot_path;
use super::settings_file::NetFile;
use super::slot_id::NetSlotId;
use crate::config::config_file::{
    new_secret, FileReport, FileState, NetSettings, RunConfig, Settings,
};

/// Copies the settings every network of the process shares on the page: the probe interval and the
/// site checks (switch, interval, DoH server, provider DNS). Everything else stays as `to` has it.
pub fn copy_shared_net(from: &NetSettings, to: &mut NetSettings) {
    to.probe_interval_sec = from.probe_interval_sec;
    to.checks_enabled = from.checks_enabled;
    to.check_interval_sec = from.check_interval_sec;
    to.doh_server = from.doh_server;
    to.provider_dns = from.provider_dns;
}

/// The report of a file that does not exist yet: the first key save creates it.
fn unwritten_report(path: std::path::PathBuf) -> FileReport {
    FileReport {
        path: Some(path),
        state: FileState::Created,
        writable: true,
        message: None,
        field_errors: Vec::new(),
        rev: 0,
        persistent_identity: true,
        identity_declined: false,
        overwrite_refused: false,
    }
}

/// Builds the run settings of a new slot without a key: the process block from the main run (SOCKS off),
/// default network settings with the shared ones copied from `shared`, a new secret and the path of the
/// slot's future file. Nothing is written. `Err` only when no secret can be made or the main run has no file.
pub fn fresh_net_slot(
    main: &RunConfig,
    shared: &NetSettings,
    id: &NetSlotId,
    now_unix: u64,
) -> anyhow::Result<RunConfig> {
    let main_path = main
        .file
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("the main run has no settings file"))?
        .path();
    let path = net_slot_path(main_path, id)
        .ok_or_else(|| anyhow::anyhow!("the main slot has no slot file"))?;
    let mut net = NetSettings::default();
    copy_shared_net(shared, &mut net);
    let mut process = main.effective_settings.process.clone();
    process.socks_enabled = false;
    Ok(RunConfig {
        effective_settings: Settings {
            process,
            net: net.clone(),
        },
        file_settings: Settings {
            process: main.file_settings.process.clone(),
            net,
        },
        secret: new_secret()?,
        file_report: unwritten_report(path.clone()),
        cli_override_paths: super::prepare::process_flag_paths(main),
        file: Some(Arc::new(NetFile {
            path,
            created_unix: now_unix,
        })),
    })
}

#[cfg(test)]
#[path = "fresh_tests.rs"]
mod tests;
