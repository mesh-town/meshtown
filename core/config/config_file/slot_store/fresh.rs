//! Building the start settings of a network slot that has no file yet: it is added at run time and
//! gets its file with the first network key.

use std::path::Path;

use super::ids::new_slot_id;
use super::layout::net_slot_path;
use super::prepare::slot_run;
use super::settings_file::NetFile;
use super::slot_id::NetSlotId;
use crate::config::config_file::{new_secret, FileReport, FileState, NetSettings, RunConfig};

const ID_ATTEMPTS: usize = 8;

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
    let file = NetFile {
        path: path.clone(),
        created_unix: now_unix,
    };
    Ok(slot_run(
        main,
        net,
        new_secret()?,
        unwritten_report(path),
        file,
    ))
}

/// Picks a free id (not `taken`, a few tries) and builds the settings of a new slot beside the main
/// file at `main_path`; see `fresh_net_slot`. Nothing is written.
pub fn new_net_slot(
    main: &RunConfig,
    main_path: &Path,
    shared: &NetSettings,
    taken: &dyn Fn(&NetSlotId) -> bool,
    now_unix: u64,
) -> anyhow::Result<(NetSlotId, RunConfig)> {
    for _ in 0..ID_ATTEMPTS {
        let id = new_slot_id(main_path)?;
        if !taken(&id) {
            let run = fresh_net_slot(main, shared, &id, now_unix)?;
            return Ok((id, run));
        }
    }
    Err(anyhow::anyhow!("couldn't find a free network id"))
}

#[cfg(test)]
#[path = "fresh_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "fresh_new_tests.rs"]
mod new_tests;
