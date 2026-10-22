//! Raising the open-files limit at process start (unix only).
//!
//! What it does: raises the soft `RLIMIT_NOFILE` to `min(hard, CAP)` and returns the result as a log line; `report` writes it as a `[node]` line, and only `run`/`tray` call it.
//! Why: an app launched from Finder starts with a limit of 256, while NAT punching of the RANDOM class opens up to 256 UDP sockets.
//! What it does not do: it does not change the hard limit; a `setrlimit` failure is not a startup error, only a log line.

// `CAP` and `target_for` are pure logic for all OSes (the `fd_limit_tests.rs` tests run on Windows too), but only
// `mod unix` calls them: on non-unix they are dead, so `cfg_attr(not(unix), allow(dead_code))` stays.
/// Target cap: on macOS above `OPEN_MAX` `setrlimit` answers `EINVAL` even with `hard = RLIM_INFINITY`.
#[cfg(target_os = "macos")]
#[cfg_attr(not(unix), allow(dead_code))]
const CAP: u64 = 10240;
#[cfg(not(target_os = "macos"))]
#[cfg_attr(not(unix), allow(dead_code))]
const CAP: u64 = 65536;

/// New soft limit for `(soft, hard)`, or `None` if there is nothing to raise.
#[cfg_attr(not(unix), allow(dead_code))]
fn target_for(soft: u64, hard: u64, cap: u64) -> Option<u64> {
    let target = hard.min(cap);
    (soft < target).then_some(target)
}

/// Raises the limit and returns the log line; prints nothing. `None` on non-unix or when there is nothing to raise.
pub fn raise() -> Option<String> {
    #[cfg(unix)]
    return unix::raise();
    #[cfg(not(unix))]
    None
}

/// Writes the line from `raise` as a `[node]` log entry (called by `run` and `tray` only).
pub fn report(note: Option<String>) {
    if let Some(text) = note {
        meshtown::output::say("node", text);
    }
}

#[cfg(unix)]
mod unix {
    use super::{target_for, CAP};

    /// Reads the limit; `None` means it failed.
    fn current() -> Option<libc::rlimit> {
        let mut lim = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        // SAFETY: `lim` is a valid pointer to a local struct.
        let rc = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut lim) };
        (rc == 0).then_some(lim)
    }

    /// Raises the soft limit; returns a log line, or `None` if no change is needed.
    pub fn raise() -> Option<String> {
        let mut lim = current()?;
        let soft = lim.rlim_cur;
        let target = target_for(soft, lim.rlim_max, CAP)?;
        lim.rlim_cur = target;
        // SAFETY: `lim` is a valid pointer to a local struct.
        let rc = unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &lim) };
        Some(match rc {
            0 => format!("open files limit: {soft} → {target}"),
            _ => format!("open files limit {soft} not raised: {}", std::io::Error::last_os_error()),
        })
    }
}

#[cfg(test)]
#[path = "fd_limit_tests.rs"]
mod tests;
