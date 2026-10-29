//! Start at login on macOS: a LaunchAgent property list in `~/Library/LaunchAgents`.

use std::path::{Path, PathBuf};

use meshtown_core::autostart::{
    launch_agent_path, launch_agent_plist, launch_agent_switched_off, mac_launch, AutostartBackend,
    EntryState, FileEntry,
};
use objc2_foundation::NSBundle;

use crate::notify::in_bundle;

/// The macos start-at-login backend.
pub struct MacosAutostart;

/// The handle to pass to `meshtown_core::autostart::install`.
pub static AUTOSTART: MacosAutostart = MacosAutostart;

impl AutostartBackend for MacosAutostart {
    fn state(&self) -> Result<EntryState, String> {
        entry()?.state()
    }

    fn enable(&self) -> Result<(), String> {
        entry()?.enable()
    }

    fn disable(&self) -> Result<(), String> {
        entry()?.disable()
    }
}

/// The entry for this process: the bundle (if any), the binary and `HOME` are read now.
fn entry() -> Result<FileEntry, String> {
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .ok_or("HOME is not set")?;
    let exe = std::env::current_exe().map_err(|e| format!("cannot find the program: {e}"))?;
    let bundle = current_bundle();
    let bundle = bundle
        .as_ref()
        .map(|(app, id)| (app.as_path(), id.as_str()));
    Ok(entry_in(&home, bundle, &exe))
}

/// The `.app` path and identifier of the running bundle, or `None` for a bare binary.
fn current_bundle() -> Option<(PathBuf, String)> {
    let bundle = NSBundle::mainBundle();
    let path = bundle.bundlePath().to_string();
    let id = bundle.bundleIdentifier().map(|s| s.to_string());
    if !in_bundle(&path, id.as_deref()) {
        return None;
    }
    id.map(|id| (PathBuf::from(path), id))
}

fn entry_in(home: &Path, bundle: Option<(&Path, &str)>, exe: &Path) -> FileEntry {
    let (label, launch) = mac_launch(bundle, exe);
    FileEntry {
        path: launch_agent_path(home, &label),
        text: launch_agent_plist(&label, &launch),
        switched_off: launch_agent_switched_off,
    }
}

#[cfg(test)]
#[path = "autostart_tests.rs"]
mod tests;
