//! The seam between the tray logic and a platform: the platform crate implements `TrayBackend` and hands
//! a `&'static` reference to the entry point.

use super::core::Core;

/// What the tray needs from a platform.
pub trait TrayBackend: Sync {
    /// Runs the platform event loop on the main thread: raises the icon through `Core::attach_ui`, handles
    /// the queued commands and never returns (the process ends through `exit_now`).
    fn run(&self, core: Core) -> !;

    /// Wakes the event loop so it picks up a queued command. Called from any thread.
    fn wake(&self);

    /// Opens a link with the default browser. Called from the event loop thread on a menu click.
    fn open_url(&self, url: &str) -> Result<(), String>;

    /// Writes `text` to the OS clipboard, blocking until done. Called from a worker thread. The error is a
    /// ready-made reason that does not contain the text.
    fn copy_to_clipboard(&self, text: &str) -> Result<(), &'static str>;

    /// Shows a system notification. Called from a short-lived worker thread, never from the event loop;
    /// may block. The error is a ready-made reason for the log.
    fn notify(&self, title: &str, body: &str) -> Result<(), &'static str>;
}
