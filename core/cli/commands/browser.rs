//! Starts the system browser on a URL. Each OS uses its own system opener; the child is not waited for.

use std::process::{Command, Stdio};

/// Opens `url` in the default browser; on failure returns a short reason without the link.
pub(super) fn launch(url: &str) -> Result<(), String> {
    let mut child = opener(url)
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

#[cfg(target_os = "macos")]
fn opener(url: &str) -> Command {
    let mut c = Command::new("/usr/bin/open");
    c.arg(url);
    c
}

#[cfg(windows)]
fn opener(url: &str) -> Command {
    let mut c = Command::new("rundll32");
    c.arg("url.dll,FileProtocolHandler").arg(url);
    c
}

#[cfg(not(any(target_os = "macos", windows)))]
fn opener(url: &str) -> Command {
    let mut c = Command::new("xdg-open");
    c.arg(url);
    c
}
