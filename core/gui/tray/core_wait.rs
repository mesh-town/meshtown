//! Waiting with command handling: the pause between icon raise attempts (a child module of `core`).
//!
//! What it does: `Core::recv_for` handles commands until a deadline or `QuitWithCode`;
//! `Core` is an `IconHost` for `raise_with_retry`.
//! Links: `icon_retry.rs`, `core.rs` (`dispatch_tray_cmd`, `attach_ui`).

use std::time::{Duration, Instant};

use super::{log_tray, Core};
use crate::gui::tray::icon_retry::IconHost;

impl Core {
    /// Handles commands for up to `pause`; `Some(code)` as soon as `QuitWithCode` arrives, `None` at the deadline.
    pub fn recv_for(&mut self, pause: Duration) -> Option<i32> {
        let deadline = Instant::now() + pause;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let cmd = self.rx.recv_timeout(left).ok()?;
            if let Some(code) = self.dispatch_tray_cmd(cmd) {
                return Some(code);
            }
        }
    }
}

impl IconHost for Core {
    fn try_attach(&mut self) -> Result<(), String> {
        self.attach_ui()
    }

    fn wait(&mut self, pause: Duration) -> Option<i32> {
        self.recv_for(pause)
    }

    fn log(&mut self, text: &str) {
        log_tray(text);
    }
}

#[cfg(test)]
#[path = "core_wait_tests.rs"]
mod tests;
