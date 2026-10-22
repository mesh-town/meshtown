//! Opens a link on the node page through the OS: no shared HTTP client and no new
//! crates — each OS uses its own system call.
//!
//! What it does: `open(url)` returns `Ok(())` on success and, when the OS refuses, a short reason without the link itself.
//! What it does not do: it does not build the "the browser did not open: …" text — that is up to the caller
//! (`Core::dispatch_tray_cmd`); the function only returns a short reason.
//! Relations: `Core::dispatch_tray_cmd` (the "Open" click).

/// Opens `url` in the default browser (macOS — `NSWorkspace`).
pub fn open(url: &str) -> Result<(), String> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSString, NSURL};

    let ns_url = NSURL::URLWithString(&NSString::from_str(url))
        .ok_or_else(|| "could not build an NSURL from the link".to_string())?;
    if NSWorkspace::sharedWorkspace().openURL(&ns_url) {
        Ok(())
    } else {
        Err("NSWorkspace.openURL returned false".to_string())
    }
}
