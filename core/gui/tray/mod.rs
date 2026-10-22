//! The tray icon: the command channel and dispatcher, platform loops per OS.
//!
//! Files: `core.rs` — `TrayCmd`/`NodeCmd`/`CmdTx`/`Core`, `channel()`/`run()`, the dispatcher
//! `dispatch_tray_cmd()` (the single node item `MENU_NODE` goes to the poller as `NodeCmd::Start`/
//! `Stop`, instead of the former pair "Start node"/"Stop node"); `menu_build.rs` — pure
//! UI construction without `Core` state: icons by color, menu items (including `struct TrayUi` —
//! the raised icon and its parts), the click handler; `exit_menu.rs` — the submenu
//! "Exit via…" on top of muda: `ExitUi::build`/`apply_exit_menu` from the
//! `app::exit_menu::ExitMenu`;
//! `actions_ui.rs` — the action items under the submenu ("Invite a member" and the checkbox "Allow sites to open via this computer");
//! `clipboard.rs` — writing a string to the OS clipboard with an external program (`copy_to_clipboard`);
//! `spin.rs` — the spinner of a starting node (frame number, the ticker thread `tray-spin`);
//! `frame_cache.rs` — built spinner frames kept by picture;
//! `core_menu.rs` — `Core::handle_menu`, a click on a menu item (a child module of `core`);
//! `linux.rs`/`macos.rs`/`windows.rs` — the tray loop of its own OS; `open_url.rs` —
//! opening a link with OS means (the "Open" click); `macos_hidden.rs` — the "icon is hidden" check
//! and a window with a settings button (macOS).
//! Links: called from `app::tray_role`; `core` knows nothing about the node, the page or `meshtown`
//! — only `TrayCmd`, `QuitSignal`, `LinkFn`, `NodeCmdTx`; it looks at `app::view`/`app::quit`.

mod actions_ui;
pub mod app;
mod clipboard;
mod core;
mod exit_menu;
mod frame_cache;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod macos_hidden;
mod menu_build;
mod open_url;
mod spin;
mod vote_item;
#[cfg(target_os = "windows")]
mod windows;

pub(crate) use clipboard::copy_to_clipboard;
use core::log_tray;
pub(crate) use core::MENU_OPEN;
pub use core::{channel, run, Core, NodeCmd, TrayCmd};
