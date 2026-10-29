//! The windows desktop tray: implements `TrayBackend` for the core and exports the backend handle `TRAY`.

#![cfg(target_os = "windows")]

mod autostart;
mod backend;
mod clipboard;
mod notify;
mod open_url;
mod tray;

pub use autostart::{WindowsAutostart, AUTOSTART};
pub use backend::{WindowsTray, TRAY};
