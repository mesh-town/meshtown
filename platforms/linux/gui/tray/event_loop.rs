//! The real tray loop for Linux: ksni through `tray-icon` (statically, without GTK).
//!
//! What it does: raises the icon, repeating with growing pauses while the StatusNotifierWatcher is absent
//! (a failure is not fatal — the node works without an icon; commands are handled during the pauses), then
//! handles commands in a blocking way until `QuitWithCode` and ends the process.
//! What it does not do: it does not call `Core::drop_ui` — on Linux the icon's `Drop` waits for the ksni thread, and
//! `process::exit` does not run destructors, so there is no need to remove the icon explicitly.
//! Links: `Core`, `raise_with_retry`, `exit_now`.

use meshtown_core::gui::tray::{exit_now, raise_with_retry, Core, Raise, RetryPlan};

pub(crate) fn run(mut core: Core) -> ! {
    if let Raise::Quit(code) = raise_with_retry(&mut core, RetryPlan::LINUX) {
        exit_now(code);
    }
    let code = core.recv_blocking();
    exit_now(code);
}

/// No wakeup is needed: `recv()` is woken by `send()` itself — ksni keeps its own D-Bus thread,
/// so no OS nudge is needed.
pub(crate) fn wake() {}
