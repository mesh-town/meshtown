//! The macos desktop tray: implements `TrayBackend` for the core and exports the backend handle `TRAY`.

#![cfg(target_os = "macos")]

mod backend;
mod clipboard;
mod notify;
mod open_url;
mod tray;

pub use backend::{MacosTray, TRAY};
