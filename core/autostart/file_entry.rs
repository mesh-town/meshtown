//! An entry that is a single file (macOS LaunchAgent, Linux autostart `.desktop`).

use std::io::ErrorKind;
use std::path::PathBuf;

use super::backend::{AutostartBackend, EntryState};

/// A login entry stored as a file with a known text.
pub struct FileEntry {
    pub path: PathBuf,
    /// The exact text this binary would write.
    pub text: String,
    /// Tells whether a foreign text means the user switched the entry off.
    pub switched_off: fn(&str) -> bool,
}

impl FileEntry {
    pub fn state(&self) -> Result<EntryState, String> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => Ok(self.classify(&text)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(EntryState::Absent),
            Err(e) => Err(self.describe(&e)),
        }
    }

    fn classify(&self, text: &str) -> EntryState {
        if text == self.text {
            return EntryState::Current;
        }
        if (self.switched_off)(text) {
            return EntryState::SwitchedOff;
        }
        EntryState::Stale
    }

    pub fn enable(&self) -> Result<(), String> {
        self.write_atomically().map_err(|e| self.describe(&e))
    }

    fn write_atomically(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.temp_path();
        std::fs::write(&tmp, &self.text)?;
        std::fs::rename(&tmp, &self.path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }

    fn temp_path(&self) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(".tmp");
        self.path.with_file_name(name)
    }

    pub fn disable(&self) -> Result<(), String> {
        match std::fs::remove_file(&self.path) {
            Err(e) if e.kind() != ErrorKind::NotFound => Err(self.describe(&e)),
            _ => Ok(()),
        }
    }

    fn describe(&self, e: &std::io::Error) -> String {
        format!("{}: {e}", self.path.display())
    }
}

impl AutostartBackend for FileEntry {
    fn state(&self) -> Result<EntryState, String> {
        FileEntry::state(self)
    }

    fn enable(&self) -> Result<(), String> {
        FileEntry::enable(self)
    }

    fn disable(&self) -> Result<(), String> {
        FileEntry::disable(self)
    }
}

#[cfg(test)]
#[path = "file_entry_tests.rs"]
mod tests;
