//! Opens a link on the node page through the OS: no shared HTTP client and no new
//! crates — each OS uses its own system call.
//!
//! What it does: `open(url)` returns `Ok(())` on success and, when the OS refuses, a short reason without the link itself.
//! What it does not do: it does not build the "the browser did not open: …" text — that is up to the caller
//! (`Core::dispatch_tray_cmd`); the function only returns a short reason.
//! Relations: `Core::dispatch_tray_cmd` (the "Open" click).

/// Opens `url` in the default browser (Windows — `ShellExecuteW`, success is a result `> 32`,
/// as documented for `ShellExecuteW`).
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
