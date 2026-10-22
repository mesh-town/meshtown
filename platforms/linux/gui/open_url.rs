//! Opens a link on the node page through the OS: no shared HTTP client and no new
//! crates — each OS uses its own system call.
//!
//! What it does: `open(url)` returns `Ok(())` on success and, when the OS refuses, a short reason without the link itself.
//! What it does not do: it does not build the "the browser did not open: …" text — that is up to the caller
//! (`Core::dispatch_tray_cmd`); the function only returns a short reason.
//! Relations: `Core::dispatch_tray_cmd` (the "Open" click).

/// Opens `url` in the default browser (Linux — `xdg-open`, a reaper on a separate thread so that
/// no zombie is left behind).
pub fn open(url: &str) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let mut child = Command::new("xdg-open")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
