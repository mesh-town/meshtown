//! Setting a slot aside under `nets/left-<id>-<stamp>`: the folder of an added slot, or the files of the main one.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::layout::{net_slot_path, nets_dir, LEFT_PREFIX};
use super::slot_id::NetSlotId;
use crate::config::config_file::{stamp_utc, FILE_NAME};

/// Caches that sit beside the main file and move with it.
const MAIN_CACHES: [&str; 4] = [
    "peers-cache.json",
    "peer-names.json",
    "docs-store",
    "replica",
];

/// `nets/left-<id>-<stamp>`.
fn left_dir(main: &Path, id: &str, now_unix: u64) -> PathBuf {
    nets_dir(main).join(format!("{LEFT_PREFIX}{id}-{}", stamp_utc(now_unix)))
}

/// Renames the folder of an added slot to `nets/left-<id>-<stamp>`. The main slot, a missing folder or a taken
/// target — `Err`, nothing is touched.
pub fn set_aside_slot(main: &Path, id: &NetSlotId, now_unix: u64) -> Result<PathBuf, String> {
    let file = net_slot_path(main, id).ok_or("the main network has no slot folder")?;
    let dir = file.parent().ok_or("the slot has no folder")?;
    let target = left_dir(main, id.as_str(), now_unix);
    if !dir.is_dir() {
        return Err(format!("folder {} does not exist", dir.display()));
    }
    if target.exists() {
        return Err(format!("{} already exists", target.display()));
    }
    std::fs::rename(dir, &target).map_err(|e| {
        format!(
            "couldn't move {} to {}: {e}",
            dir.display(),
            target.display()
        )
    })?;
    Ok(target)
}

/// Copies the main file into `dir` with mode `0600` and `fsync`.
fn copy_main_file(main: &Path, dir: &Path) -> std::io::Result<()> {
    let bytes = std::fs::read(main)?;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(dir.join(FILE_NAME))?;
    f.write_all(&bytes)?;
    f.sync_all()
}

/// Creates the folder and copies the main file into it; a failure removes the folder.
fn start_left_folder(main: &Path, target: &Path) -> Result<(), String> {
    if target.exists() {
        return Err(format!("{} already exists", target.display()));
    }
    let failed = |e: std::io::Error| {
        format!(
            "couldn't copy {} to {}: {e}",
            main.display(),
            target.display()
        )
    };
    super::super::save::create_private_dir(target).map_err(failed)?;
    copy_main_file(main, target).map_err(|e| {
        let _ = std::fs::remove_dir_all(target);
        failed(e)
    })
}

/// Moves the caches that exist beside the main file into `target`.
fn move_caches(main: &Path, target: &Path) -> Result<(), String> {
    let beside = main
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    for name in MAIN_CACHES {
        let from = beside.join(name);
        if !from.exists() {
            continue;
        }
        std::fs::rename(&from, target.join(name)).map_err(|e| {
            format!(
                "couldn't move {name} to {}: {e}; the rest stays in place",
                target.display()
            )
        })?;
    }
    Ok(())
}

/// Keeps the main network's files before it is left: a copy of the main file and its caches go to
/// `nets/left-main-<stamp>/`. The main file itself stays; the caller rewrites it. Taken target or a failed copy —
/// `Err`, the created folder is removed and the caches stay; a failed cache move — `Err`, what moved stays.
pub fn set_aside_main(main: &Path, now_unix: u64) -> Result<PathBuf, String> {
    let target = left_dir(main, "main", now_unix);
    start_left_folder(main, &target)?;
    move_caches(main, &target)?;
    Ok(target)
}
