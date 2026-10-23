//! The tray icon: the command channel and dispatcher, and the `TrayBackend` seam the platform crates implement.
//!
//! Files: `core.rs` — `TrayCmd`/`NodeCmd`/`CmdTx`/`Core`, `channel()`/`run()`, the dispatcher
//! `dispatch_tray_cmd()` (the single node item `MENU_NODE` goes to the poller as `NodeCmd::Start`/
//! `Stop`, instead of the former pair "Start node"/"Stop node"); `menu_build.rs` — pure
//! UI construction without `Core` state: icons by color, menu items (including `struct TrayUi` —
//! the raised icon and its parts), the click handler; `exit_menu.rs` — the submenu
//! "Exit via…" on top of muda: `ExitUi::build`/`apply_exit_menu` from the
//! `app::exit_menu::ExitMenu`;
//! `actions_ui.rs` — the action items under the submenu ("Invite a member" and the checkbox "Allow sites to open via this computer");
//! `node_cmd.rs` — `NodeCmd`, the commands to the node poller (re-exported by `core.rs`); `nets_click.rs` — item id →
//! target → page link or node commands, pure; `nets_ui.rs` — the submenus of several networks and the live rows on top of muda;
//! `clipboard.rs` — writing a string to the OS clipboard with an external program (`copy_via`);
//! `backend.rs` — the `TrayBackend` trait: the platform loop, wake-up, opening a link and the clipboard;
//! `test_tray.rs` — a backend stub for tests;
//! `spin.rs` — the spinner of a starting node (frame number, the ticker thread `tray-spin`);
//! `frame_cache.rs` — built spinner frames kept by picture;
//! `core_menu.rs` — `Core::handle_menu`, a click on a menu item (a child module of `core`);
//! the tray loop of each OS lives in its platform crate and reaches this module through `Core`.
//! Links: called from `app::tray_role`; `core` knows nothing about the node, the page or `meshtown`
//! — only `TrayCmd`, `QuitSignal`, `LinkFn`, `NodeCmdTx`; it looks at `app::view`/`app::quit`.

mod actions_ui;
pub(crate) mod app;
mod backend;
mod clipboard;
mod core;
mod exit_menu;
mod frame_cache;
mod menu_build;
mod nets_click;
mod nets_ui;
mod node_cmd;
mod spin;
#[cfg(test)]
pub(crate) mod test_tray;
mod vote_item;

pub use app::quit::exit_now;
#[cfg(target_os = "macos")]
pub use app::tray_texts::{
    hidden_icon_hint_text, hidden_icon_title_text, HIDDEN_ICON_LATER_TEXT,
    HIDDEN_ICON_OPEN_SETTINGS_TEXT,
};
pub use backend::TrayBackend;
pub use clipboard::{copy_via, ClipboardTool};
pub(crate) use core::MENU_OPEN;
pub(crate) use core::{channel, run, NodeCmd, TrayCmd};
pub use core::{log_tray, Core};
