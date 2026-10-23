//! The file seam: where a slot re-reads its settings from and where it writes them, so no caller
//! branches on the kind of file.

use std::path::{Path, PathBuf};

use crate::config::config_file::{
    load_config_file, load_net_file, save_config_file, save_net_file, save_with_floor,
    LoadedConfigFile, LoadedNetFile, SaveError, Settings,
};
use crate::transport::net::NodeSecret;

/// What a settings file holds right now, as settings and a revision. One value at a time and short-lived,
/// so the large variant is not boxed.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum LoadedSettings {
    Absent,
    Unreadable(String),
    Invalid(String),
    Parsed { rev: u64, settings: Settings },
}

/// The settings file behind a network slot.
pub trait SettingsFile: Send + Sync {
    fn path(&self) -> &Path;

    /// Reads the file again. `Err` is the reason the caller reports before it keeps `prev`.
    fn reload(&self, prev: &Settings) -> Result<Settings, String>;

    /// Reads the file with its revision. `base` supplies the part the file does not hold.
    fn load(&self, base: &Settings) -> LoadedSettings;

    /// Writes the settings; returns the new revision.
    fn save(
        &self,
        expected_rev: Option<u64>,
        now_unix: u64,
        secret: &NodeSecret,
        s: &Settings,
    ) -> Result<u64, SaveError>;

    /// Like `save`, but a new file (`expected_rev` of `None`) gets a revision above `rev_floor`
    /// where the file kind keeps one.
    fn save_with_floor(
        &self,
        expected_rev: Option<u64>,
        rev_floor: u64,
        now_unix: u64,
        secret: &NodeSecret,
        s: &Settings,
    ) -> Result<u64, SaveError>;
}

/// The main settings file.
pub struct MainFile(pub PathBuf);

impl SettingsFile for MainFile {
    fn path(&self) -> &Path {
        &self.0
    }

    fn reload(&self, _prev: &Settings) -> Result<Settings, String> {
        match load_config_file(&self.0) {
            LoadedConfigFile::Parsed(p) => Ok(p.settings),
            LoadedConfigFile::Absent => Err("no file".to_string()),
            LoadedConfigFile::Unreadable(r) | LoadedConfigFile::Invalid(r) => Err(r),
        }
    }

    fn load(&self, _base: &Settings) -> LoadedSettings {
        match load_config_file(&self.0) {
            LoadedConfigFile::Parsed(p) => LoadedSettings::Parsed {
                rev: p.rev,
                settings: p.settings,
            },
            LoadedConfigFile::Absent => LoadedSettings::Absent,
            LoadedConfigFile::Unreadable(r) => LoadedSettings::Unreadable(r),
            LoadedConfigFile::Invalid(r) => LoadedSettings::Invalid(r),
        }
    }

    fn save(
        &self,
        expected_rev: Option<u64>,
        now_unix: u64,
        secret: &NodeSecret,
        s: &Settings,
    ) -> Result<u64, SaveError> {
        save_config_file(&self.0, expected_rev, now_unix, secret, s)
    }

    fn save_with_floor(
        &self,
        expected_rev: Option<u64>,
        rev_floor: u64,
        now_unix: u64,
        secret: &NodeSecret,
        s: &Settings,
    ) -> Result<u64, SaveError> {
        save_with_floor(&self.0, expected_rev, rev_floor, now_unix, secret, s)
    }
}

/// The file of an added network slot: only the network block is read and written, the process
/// block always comes from the main file.
pub struct NetFile {
    pub path: PathBuf,
    pub created_unix: u64,
}

impl SettingsFile for NetFile {
    fn path(&self) -> &Path {
        &self.path
    }

    fn reload(&self, prev: &Settings) -> Result<Settings, String> {
        match load_net_file(&self.path) {
            LoadedNetFile::Parsed(p) => Ok(Settings {
                process: prev.process.clone(),
                net: p.net,
            }),
            LoadedNetFile::Absent => Err("no file".to_string()),
            LoadedNetFile::Unreadable(r) | LoadedNetFile::Invalid(r) => Err(r),
        }
    }

    fn load(&self, base: &Settings) -> LoadedSettings {
        match load_net_file(&self.path) {
            LoadedNetFile::Parsed(p) => LoadedSettings::Parsed {
                rev: p.rev,
                settings: Settings {
                    process: base.process.clone(),
                    net: p.net,
                },
            },
            LoadedNetFile::Absent => LoadedSettings::Absent,
            LoadedNetFile::Unreadable(r) => LoadedSettings::Unreadable(r),
            LoadedNetFile::Invalid(r) => LoadedSettings::Invalid(r),
        }
    }

    fn save_with_floor(
        &self,
        expected_rev: Option<u64>,
        _rev_floor: u64,
        now_unix: u64,
        secret: &NodeSecret,
        s: &Settings,
    ) -> Result<u64, SaveError> {
        self.save(expected_rev, now_unix, secret, s)
    }

    fn save(
        &self,
        expected_rev: Option<u64>,
        now_unix: u64,
        secret: &NodeSecret,
        s: &Settings,
    ) -> Result<u64, SaveError> {
        save_net_file(
            &self.path,
            expected_rev,
            now_unix,
            self.created_unix,
            secret,
            &s.net,
        )
    }
}
