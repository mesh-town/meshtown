//! Start at login on Linux: an XDG autostart `.desktop` file in the user's config directory.

use std::ffi::OsString;
use std::path::Path;

use meshtown_core::autostart::{
    desktop_entry, desktop_entry_path, desktop_switched_off, AutostartBackend, EntryState,
    FileEntry,
};

/// The linux start-at-login backend.
pub struct LinuxAutostart;

/// The handle to pass to `meshtown_core::autostart::install`.
pub static AUTOSTART: LinuxAutostart = LinuxAutostart;

impl AutostartBackend for LinuxAutostart {
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

/// The entry for this process: the environment and the binary path are read now.
fn entry() -> Result<FileEntry, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot find the program: {e}"))?;
    entry_in(
        &|name| std::env::var_os(name).filter(|v| !v.is_empty()),
        &exe,
    )
}

fn entry_in(var: &dyn Fn(&str) -> Option<OsString>, exe: &Path) -> Result<FileEntry, String> {
    Ok(FileEntry {
        path: desktop_entry_path(var)?,
        text: desktop_entry(exe)?,
        switched_off: desktop_switched_off,
    })
}

#[cfg(test)]
#[path = "autostart_tests.rs"]
mod tests;
