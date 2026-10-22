//! The Windows console by process role: the binary is a console binary in every build — there is no attribute to switch the
//! subsystem to a windowless one. `init()` (any build, not just release) looks at the
//! role (the first argument after the default substitution) and at the number of processes on the current console
//! (`GetConsoleProcessList`): for the `tray` role with at most one process on the console (0 means no console
//! at all, a service; 1 means this process is the last one, typically Explorer or autostart) it releases the console
//! (`FreeConsole`) and redirects stdout/stderr to the log `meshtown-tray.log` in the standard
//! config directory (the previous log moves to `.log.1`); from a terminal (2 or more processes on the console) and for any
//! other role it does nothing. A flash of a window when `tray` starts outside a terminal is the price.

use super::log;

/// Windows, any build: under the `tray` role, if the console is the process's own or absent, release it and send
/// stdout/stderr to the log; for another role or a launch from a terminal, leave the parent's handles alone.
pub fn init() {
    use std::os::windows::io::IntoRawHandle;

    use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Console::{
        FreeConsole, GetConsoleProcessList, GetStdHandle, SetStdHandle, STD_ERROR_HANDLE,
        STD_HANDLE, STD_OUTPUT_HANDLE,
    };

    const IDS: [STD_HANDLE; 2] = [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE];

    let role = log::process_role();
    let mut buf = [0u32; 1];
    // SAFETY: the buffer holds one element; if the console has more than one process, the function just
    // returns the required size without writing to the buffer (the usual Win32 "ask for the size" behaviour).
    let count = unsafe { GetConsoleProcessList(buf.as_mut_ptr(), 1) };
    if !log::should_free_console(role.as_deref(), count) {
        return;
    }

    // SAFETY: no pointers; a failure (there was no console) does not stop the files below from being handed out.
    unsafe {
        FreeConsole();
    }

    fn busy(id: STD_HANDLE) -> bool {
        // SAFETY: no pointers; on error it returns `INVALID_HANDLE_VALUE`.
        let h = unsafe { GetStdHandle(id) };
        !h.is_null() && h != INVALID_HANDLE_VALUE
    }
    // The handle is the only one for both free ids; the file is deliberately not closed until the end of the
    // process.
    fn set_free(h: HANDLE) {
        for id in IDS {
            if !busy(id) {
                // SAFETY: `h` is an open descriptor handed to the process for good.
                unsafe {
                    SetStdHandle(id, h);
                }
            }
        }
    }

    if let Ok(file) = log::open_tray_log() {
        set_free(file.into_raw_handle());
    }
}
