//! The application entry point and its bricks.
//!
//! What it does: `snapshot` (a narrow selection of the node's JSON snapshot, parsed once at the boundary), `view`
//! (the color and texts of the icon/menu from that selection), `view_picture` (the icon image: color, ring and disk —
//! the key and input of the raster), `view_disk` (the speed disk in the center of the icon), `view_ring` (the ring of sectors
//! by members for the icon), `view_ring_paint` (the raster of that ring), `exit_menu` (the list of the
//! "Exit via…" submenu from the same snapshot), `mode` (whether the tray is needed at all on this
//! platform) and `quit` (the "Quit" signal with a 15 s watchdog and a hard process exit) — pure
//! bricks with no node and no OS loop; `local_api` — its own HTTP client
//! to the node page: the request, response parsing and classification into `Reply`/`ApiError`;
//! `proxy_state` (the proxy menu item: the address or the reason it is "off" — the settings or a run flag), `start_note` (the reason the node didn't start and its menu lines), `tray_texts` (the dictionary: all the user-facing strings of the tray screen — menu, hints, the macOS dialog; log and error lines stay with the code), `tray_actions` (the texts of the "Invite a member" tray items),
//! `tray_role` — the `tray` subcommand: finds or starts the node over HTTP (through `local_api`) and
//! drives the icon; `console` — Windows output without a console by process
//! role.
//! What it doesn't do: it doesn't parse command line flags — that is `main.rs`, which calls
//! `tray_role::run_tray` from the `CliCommand::Tray` branch; the node itself (the `CliCommand::Run` branch) this module
//! doesn't touch at all (the node page is the code of `main.rs`/`web`/`node`, not the `page` stub).
//! Links: `view`, `view_picture`, `view_disk`, `view_ring`, `view_ring_paint` and `quit` are `pub(crate)`, `tray` reads them (a neighbor
//! at the binary root, not a descendant of `app`); `mode` is private, only `tray_role` calls it;
//! `mode`, `quit`, `view`, `view_picture`, `view_ring`, `view_ring_paint`, `tray_role` are under the build feature
//! `tray` (on by default), in a thin build `app` consists of `console` only;
//! `console` has no feature, every role needs it, `main` calls its `init()` on the first line, before any
//! printing.

pub(crate) mod console;
#[cfg(feature = "tray")]
pub(crate) mod exit_menu;
#[cfg(feature = "tray")]
mod local_api;
#[cfg(feature = "tray")]
pub(crate) mod members_note;
#[cfg(feature = "tray")]
pub(crate) mod proxy_state;
#[cfg(feature = "tray")]
mod mode;
#[cfg(feature = "tray")]
pub(crate) mod quit;
#[cfg(feature = "tray")]
pub(crate) mod snapshot;
#[cfg(feature = "tray")]
pub(crate) mod start_note;
#[cfg(feature = "tray")]
pub(crate) mod tray_actions;
#[cfg(feature = "tray")]
pub(crate) mod tray_role;
#[cfg(feature = "tray")]
pub(crate) mod tray_texts;
#[cfg(feature = "tray")]
pub(crate) mod view;
#[cfg(feature = "tray")]
pub(crate) mod view_alert;
#[cfg(feature = "tray")]
pub(crate) mod view_disk;
#[cfg(feature = "tray")]
pub(crate) mod view_picture;
#[cfg(feature = "tray")]
pub(crate) mod view_ring;
#[cfg(feature = "tray")]
pub(crate) mod view_ring_paint;
