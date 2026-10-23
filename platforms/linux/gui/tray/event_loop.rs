//! The real tray loop for Linux: ksni through `tray-icon` (statically, without GTK).
//!
//! What it does: raises the icon (an error is not fatal — the node works without it), then
//! handles commands in a blocking way until `QuitWithCode` and ends the process.
//! What it does not do: it does not call `Core::drop_ui` — on Linux the icon's `Drop` waits for the ksni thread, and
//! `process::exit` does not run destructors, so there is no need to remove the icon explicitly.
//! Links: `Core`, `log_tray`, `exit_now`.

use meshtown_core::gui::tray::{exit_now, log_tray, Core};

pub(crate) fn run(mut core: Core) -> ! {
    if let Err(e) = core.attach_ui() {
        log_tray(&format!("no tray icon: {e}"));
    }
    let code = core.recv_blocking();
    exit_now(code);
}

/// No wakeup is needed: `recv()` is woken by `send()` itself — ksni keeps its own D-Bus thread,
/// so no OS nudge is needed.
pub(crate) fn wake() {}
