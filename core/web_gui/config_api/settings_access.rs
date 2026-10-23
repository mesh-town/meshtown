//! Writing through the `SettingsFile` of a slot: one place that picks the write method by what is on disk,
//! shared by `put` and the network key change.

use crate::config::config_file::{self, LoadedSettings, SaveError, Settings, SettingsFile};
use crate::transport::net::NodeSecret;
use crate::web_gui::now_unix;

/// Writes `new`. A readable file is replaced at `expected_rev`; a missing one is created above `rev_floor`;
/// a damaged one is first set aside under a new name, then written as new.
pub(super) fn write_loaded(
    file: &dyn SettingsFile,
    loaded: &LoadedSettings,
    expected_rev: Option<u64>,
    rev_floor: u64,
    secret: &NodeSecret,
    new: &Settings,
) -> Result<u64, SaveError> {
    match loaded {
        LoadedSettings::Parsed { .. } => file.save(expected_rev, now_unix(), secret, new),
        LoadedSettings::Absent => file.save_with_floor(None, rev_floor, now_unix(), secret, new),
        LoadedSettings::Unreadable(_) | LoadedSettings::Invalid(_) => {
            set_aside_damaged(file)?;
            file.save_with_floor(None, rev_floor, now_unix(), secret, new)
        }
    }
}

fn set_aside_damaged(file: &dyn SettingsFile) -> Result<(), SaveError> {
    let dest = config_file::set_aside_bad(file.path(), now_unix()).map_err(SaveError::Io)?;
    crate::node::output::say(
        "web",
        format!(
            "page: broken settings file moved aside to {}",
            dest.display()
        ),
    );
    Ok(())
}
