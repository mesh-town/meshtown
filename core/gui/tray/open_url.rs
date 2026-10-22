//! Opens a link on the node page through the OS: no shared HTTP client and no new
//! crates — each OS uses its own system call.
//!
//! What it does: `open(url)` returns `Ok(())` on success and, when the OS refuses, a short reason without the link itself.
//! What it does not do: it does not build the "the browser did not open: …" text — that is up to the caller
//! (`Core::dispatch_tray_cmd`); the function only returns a short reason.
//! Relations: `Core::dispatch_tray_cmd` (the "Open" click).

/// Opens `url` in the default browser (macOS — `NSWorkspace`).
#[cfg(target_os = "macos")]
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

/// Opens `url` in the default browser (Windows — `ShellExecuteW`, success is a result `> 32`,
/// as documented for `ShellExecuteW`).
#[cfg(target_os = "windows")]
pub fn open(url: &str) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let operation = wide("open");
    let file = wide(url);
    // SAFETY: both strings are valid NUL-terminated UTF-16 buffers that live until the end of the call;
    // the other parameters are null per the ShellExecuteW documentation (no parent window, no
    // arguments and no working directory).
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if (result as isize) > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecuteW returned {}", result as isize))
    }
}

/// Opens `url` in the default browser (Linux — `xdg-open`, a reaper on a separate thread so that
/// no zombie is left behind).
#[cfg(target_os = "linux")]
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
