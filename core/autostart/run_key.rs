//! The Windows `Run` registry value.

use std::path::Path;

use super::backend::EntryState;

pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const APPROVED_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
pub const VALUE_NAME: &str = "Mesh Town";

/// The command stored in the `Run` value: `"<exe>" tray`.
pub fn run_value(exe: &Path) -> String {
    format!("\"{}\" tray", exe.display())
}

/// The first byte of the `StartupApproved` value is odd (`03`) when the entry is switched off.
pub fn approved_disabled(bytes: &[u8]) -> bool {
    bytes.first().is_some_and(|b| b % 2 == 1)
}

/// State from the `Run` value, the `StartupApproved` value and the running binary.
pub fn run_entry_state(value: Option<&str>, approved: Option<&[u8]>, exe: &Path) -> EntryState {
    let Some(value) = value else {
        return EntryState::Absent;
    };
    if approved.is_some_and(approved_disabled) {
        return EntryState::SwitchedOff;
    }
    if value == run_value(exe) {
        return EntryState::Current;
    }
    EntryState::Stale
}

#[cfg(test)]
#[path = "run_key_tests.rs"]
mod tests;
