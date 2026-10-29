//! The linux desktop tray: implements `TrayBackend` for the core and exports the backend handle `TRAY`.

#![cfg(target_os = "linux")]

mod autostart;
mod backend;
mod clipboard;
mod notify;
mod open_url;
mod tray;

pub use autostart::{LinuxAutostart, AUTOSTART};
pub use backend::{LinuxTray, TRAY};
