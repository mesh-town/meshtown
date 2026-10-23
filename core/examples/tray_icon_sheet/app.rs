//! Tray code files included in the example as is, without copying or edits.
//!
//! What it does: assembles under the names `app::{exit_menu, nets_menu, nets_texts, overview, proxy_state, snapshot, tray_texts, view, view_disk, view_ring, view_ring_paint}` the same
//! `core/gui/tray/app/` sources that go into the binary; `crate::gui::tray::app::view` in `exit_menu.rs` finds them
//! here too. Whatever the example doesn't call is silenced with `dead_code`.
//! What it doesn't do: it doesn't change the sources and doesn't pull in the rest of `app` (`tray_role`, `local_api`).
//! Links: `main.rs` takes `view_ring::{Ring, RingRole}` and `view_ring_paint::ring_rgba_lit` from here.

#![allow(dead_code)]

#[path = "../../gui/tray/app/exit_menu.rs"]
pub mod exit_menu;
#[path = "../../gui/tray/app/nets_menu.rs"]
pub mod nets_menu;
#[path = "../../gui/tray/app/nets_texts.rs"]
pub mod nets_texts;
#[path = "../../gui/tray/app/overview.rs"]
pub mod overview;
#[path = "../../gui/tray/app/proxy_state.rs"]
pub mod proxy_state;
#[path = "../../gui/tray/app/snapshot/mod.rs"]
pub mod snapshot;
#[path = "../../gui/tray/app/tray_texts.rs"]
pub mod tray_texts;
#[path = "../../gui/tray/app/view.rs"]
pub mod view;
#[path = "../../gui/tray/app/view_disk.rs"]
pub mod view_disk;
#[path = "../../gui/tray/app/view_ring.rs"]
pub mod view_ring;
#[path = "../../gui/tray/app/view_ring_paint.rs"]
pub mod view_ring_paint;
