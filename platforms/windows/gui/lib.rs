//! The windows desktop tray: implements `TrayBackend` for the core and exports the backend handle `TRAY`.

#![cfg(target_os = "windows")]

mod backend;
mod clipboard;
mod open_url;
mod tray;

pub use backend::{WindowsTray, TRAY};
