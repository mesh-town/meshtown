//! Shared console output for a process role: role parsing, the log directory and log
//! rotation. Platform behaviour is documented in the `///` above `init` in its own file (Windows — `windows.rs`, macOS
//! — `macos.rs`; Linux — an empty function here, since the repository has no
//! way to start the tray other than from a terminal).

#[cfg(any(test, windows, target_os = "macos"))]
use std::path::Path;
#[cfg(any(windows, target_os = "macos"))]
use std::path::PathBuf;

#[cfg(any(test, windows, target_os = "macos"))]
use std::ffi::OsStr;
#[cfg(any(windows, target_os = "macos"))]
use std::ffi::OsString;

/// The process role after the default subcommand is substituted (the second argv element): `tray` when
/// a tray backend is present (`has_tray`), `run` without it. Both platforms call it.
#[cfg(any(windows, target_os = "macos"))]
pub(super) fn process_role(has_tray: bool) -> Option<OsString> {
    crate::cli::args::with_default_subcommand(std::env::args_os(), has_tray)
        .into_iter()
        .nth(1)
}

/// Whether to release the process console: only the `tray` role, and only if the console has at most one process
/// (0 means no console at all). A pure function without Windows types, so it is tested on every OS.
#[cfg(any(test, windows))]
pub(super) fn should_free_console(role: Option<&OsStr>, console_process_count: u32) -> bool {
    role == Some(OsStr::new("tray")) && console_process_count <= 1
}

/// Whether to redirect stdio to a file on macOS: only the `tray` role and only if stderr is not a terminal. A launch via
/// LaunchServices and a launch from a terminal with redirection (`meshtown tray 2>file`, `| tee`) take
/// the same branch; the check cannot tell them apart (accepted).
#[cfg(any(test, target_os = "macos"))]
pub(super) fn should_redirect_stdio(role: Option<&OsStr>, stderr_is_terminal: bool) -> bool {
    role == Some(OsStr::new("tray")) && !stderr_is_terminal
}

/// The log directory: the parent of the standard config file.
#[cfg(any(windows, target_os = "macos"))]
pub(super) fn log_dir() -> std::io::Result<PathBuf> {
    let env = crate::config::config_file::current_env();
    let config = crate::config::config_file::standard_path(&env.as_path_env())
        .map_err(std::io::Error::other)?;
    config
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| std::io::Error::other("no config directory"))
}

/// The tray log (`meshtown-tray.log`) in the standard config directory.
#[cfg(any(windows, target_os = "macos"))]
pub(super) fn open_tray_log() -> std::io::Result<std::fs::File> {
    open_log(&log_dir()?, "meshtown-tray.log")
}

/// Opens a fresh log `name` in `dir`: the directory is created and the previous log moves to `.1`
/// (an old `.1` is replaced; a failure to move it is ignored).
#[cfg(any(test, windows, target_os = "macos"))]
pub(super) fn open_log(dir: &Path, name: &str) -> std::io::Result<std::fs::File> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(name);
    rotate_old_log(&path);
    std::fs::File::create(path)
}

/// Moves an existing log to `<path>.1` (an old `.1` is replaced; a failure is ignored).
#[cfg(any(test, windows, target_os = "macos"))]
fn rotate_old_log(path: &Path) {
    if path.exists() {
        let mut old = path.as_os_str().to_owned();
        old.push(".1");
        let _ = std::fs::rename(path, old);
    }
}

/// The node log `meshtown.log` (the same rotation in the standard config directory) that the
/// `tray` process opens for its `run` child.
#[cfg(all(feature = "gui", any(windows, target_os = "macos")))]
pub fn open_node_log() -> std::io::Result<std::fs::File> {
    open_log(
        &log_dir()?,
        crate::gui::tray::app::start_note::NODE_LOG_NAME,
    )
}

#[cfg(all(feature = "gui", any(windows, target_os = "macos")))]
const NODE_LOG_TAIL_BYTES: u64 = 8192;

/// The last `max_bytes` bytes of the file (`from_utf8_lossy`); `None` if the file is missing or reading fails.
#[cfg(any(test, all(feature = "gui", any(windows, target_os = "macos"))))]
pub(super) fn read_tail(path: &Path, max_bytes: u64) -> Option<String> {
    use std::io::{Read as _, Seek as _, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(max_bytes)))
        .ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The tail of the node log `meshtown.log`; `None` means there is no log (Linux, where the tray's stderr is a terminal
/// and the file is not read).
#[cfg(all(feature = "gui", any(windows, target_os = "macos")))]
pub fn node_log_tail() -> Option<String> {
    child_needs_node_log().then_some(())?;
    let path = log_dir()
        .ok()?
        .join(crate::gui::tray::app::start_note::NODE_LOG_NAME);
    read_tail(&path, NODE_LOG_TAIL_BYTES)
}

/// Outside Windows/macOS there is no node log.
#[cfg(all(feature = "gui", not(any(windows, target_os = "macos"))))]
pub fn node_log_tail() -> Option<String> {
    None
}

/// Whether the `run` child needs a separate log `meshtown.log`: on Windows always (after
/// `FreeConsole` no inherited parent handles remain), on macOS only if the tray's stderr
/// is not a terminal (the same check that decides the redirect in `init`). The only caller is
/// `tray_role::run::attach_node_log`.
#[cfg(all(feature = "gui", any(windows, target_os = "macos")))]
pub fn child_needs_node_log() -> bool {
    #[cfg(windows)]
    {
        true
    }
    #[cfg(target_os = "macos")]
    {
        use std::io::IsTerminal as _;
        !std::io::stderr().is_terminal()
    }
}

/// Outside Windows/macOS: the console is either the process's own or does not exist (Linux) — nothing to do.
#[cfg(not(any(windows, target_os = "macos")))]
pub fn init(_has_tray: bool) {}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
