//! Tray code files included in the example as is, without copying or edits.
//!
//! What it does: assembles under the names `app::{view_ring_types, view_ring_paint}` the same
//! `core/gui/tray/app/` sources that go into the binary. Both are leaf files (they depend on nothing else in the tray),
//! so the example compiles exactly what it uses.
//! What it doesn't do: it doesn't change the sources and doesn't pull in the rest of `app` (`snapshot`, `view`, `tray_role`, `local_api`).
//! Links: `main.rs` takes `view_ring_types::{Ring, RingRole}` and `view_ring_paint::ring_rgba_lit` from here.

#[path = "../../gui/tray/app/view_ring_paint.rs"]
pub mod view_ring_paint;
#[path = "../../gui/tray/app/view_ring_types.rs"]
pub mod view_ring_types;
