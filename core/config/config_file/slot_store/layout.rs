//! Where slot folders live: `nets/<id>/config.json` beside the main settings file.

use std::path::{Path, PathBuf};

use super::slot_id::NetSlotId;
use crate::config::config_file::FILE_NAME;

/// Name of the folder that holds the slot folders.
pub(super) const NETS_DIR: &str = "nets";

/// Prefix of a folder that was set aside: `left-<id>-<stamp>`.
pub(super) const LEFT_PREFIX: &str = "left-";

/// The folder beside the main file (`.` for a bare file name).
fn main_dir(main: &Path) -> &Path {
    match main.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    }
}

/// `<folder of main>/nets`.
pub(super) fn nets_dir(main: &Path) -> PathBuf {
    main_dir(main).join(NETS_DIR)
}

/// `<folder of main>/nets/<id>/config.json`; the main slot has no such file — `None`.
pub fn net_slot_path(main: &Path, id: &NetSlotId) -> Option<PathBuf> {
    (*id != NetSlotId::main()).then(|| nets_dir(main).join(id.as_str()).join(FILE_NAME))
}
