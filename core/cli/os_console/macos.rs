//! The tray log on macOS: the `tray` role, after the default substitution, with stderr not
//! a terminal (a launch via LaunchServices — probe: fd 1/2 are a pipe, not a terminal), redirects
//! stdout/stderr to `meshtown-tray.log` via `dup2`; from a terminal without redirection it does nothing.
//! A launch from a terminal with redirection (`meshtown tray 2>file`, `| tee`) takes the same branch:
//! the "stderr is not a terminal" check cannot tell it apart from LaunchServices (accepted behavior).

use std::io::IsTerminal as _;
use std::os::unix::io::AsRawFd as _;

use super::log;

/// Redirects stdout/stderr to `meshtown-tray.log` if the role is `tray` and stderr is not a terminal; otherwise
/// does nothing. A failure to open the file is silent (there is nowhere to report it: there was no output before this point,
/// and handing out stdout/stderr for a message before `dup2` would be premature).
pub fn init(has_tray: bool) {
    let role = log::process_role(has_tray);
    let terminal = std::io::stderr().is_terminal();
    if !log::should_redirect_stdio(role.as_deref(), terminal) {
        return;
    }
    let Ok(file) = log::open_tray_log() else {
        return;
    };
    let fd = file.as_raw_fd();
    // SAFETY: `fd` is the descriptor of the file just opened; 1 and 2 belong to no
    // Rust object (`Stdout`/`Stderr` write by descriptor number and do not own it), and there was no output before this
    // point (`console::init()` is the first line of `main`, before `init_tracing` and any
    // output). The result of `dup2` is not checked; the original `file` closes at the end of the function — the copies
    // on 1 and 2 keep working (the kernel tracks them with a separate reference count).
    unsafe {
        libc::dup2(fd, 1);
        libc::dup2(fd, 2);
    }
}
