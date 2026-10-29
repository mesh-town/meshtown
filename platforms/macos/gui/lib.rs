//! The macos desktop tray: implements `TrayBackend` for the core and exports the backend handle `TRAY`.

#![cfg(target_os = "macos")]

mod autostart;
mod backend;
mod clipboard;
mod notify;
mod notify_un;
mod open_url;
mod tray;

pub use autostart::{MacosAutostart, AUTOSTART};
pub use backend::{MacosTray, TRAY};
