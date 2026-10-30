//! The backend trait, the state of the login entry and the rule for changing it.

use std::sync::OnceLock;

/// What the system holds for this app's login entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryState {
    /// There is no entry.
    Absent,
    /// The entry exists but the user switched it off in the system.
    SwitchedOff,
    /// The entry matches what this binary would write now.
    Current,
    /// The entry is ours but differs (another path, an older text).
    Stale,
}

impl EntryState {
    /// The entry will start the app at login.
    pub fn is_on(self) -> bool {
        matches!(self, EntryState::Current | EntryState::Stale)
    }
}

/// A change to apply to the entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Enable,
    Disable,
}

/// Decides what to do so that the entry matches `desired`.
pub fn plan_change(desired: bool, state: EntryState) -> Option<Change> {
    match (state, desired) {
        (EntryState::Absent | EntryState::SwitchedOff | EntryState::Stale, true) => {
            Some(Change::Enable)
        }
        (EntryState::Current | EntryState::Stale, false) => Some(Change::Disable),
        _ => None,
    }
}

/// Reads and changes the login entry of the current user.
pub trait AutostartBackend: Sync + Send {
    /// Reads the system every time it is called.
    fn state(&self) -> Result<EntryState, String>;
    /// Creates or rewrites the entry for the running binary and clears a system switch-off.
    fn enable(&self) -> Result<(), String>;
    /// Removes the entry; an absent entry is not an error.
    fn disable(&self) -> Result<(), String>;
}

/// Rewrites a stale login entry (it points at another copy of the program) for the running
/// binary. Returns `true` if it rewrote. An absent, switched-off or current entry is left alone.
pub fn refresh_on_start(backend: &dyn AutostartBackend) -> Result<bool, String> {
    if backend.state()? != EntryState::Stale {
        return Ok(false);
    }
    backend.enable()?;
    Ok(true)
}

static INSTALLED: OnceLock<&'static dyn AutostartBackend> = OnceLock::new();

/// Registers the backend for this process; a second call is ignored.
pub fn install(backend: &'static dyn AutostartBackend) {
    let _ = INSTALLED.set(backend);
}

/// The registered backend, if the build has one.
pub fn installed() -> Option<&'static dyn AutostartBackend> {
    INSTALLED.get().copied()
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
