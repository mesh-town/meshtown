//! The narrow selection from the `GET /api/state` snapshot that the tray reads — parsed once, at the boundary.
//!
//! What it does: `types` holds `TraySnapshot` and the nested structs, `lenient` holds the lenient field readers.
//! What it does not do: it has no logic; it does not repeat the server's snapshot type — only what the tray sees.
//! Links: the boundary is built by `tray_role::views::views_from_state_json`; the color, network, ring and exit submenu
//! (`view`, `view_net`, `view_ring`, `exit_menu`) read fields, not paths into a `Value`.

mod types;
pub(crate) mod lenient;

// The whole public set of selection types; `TrayNode`/`TrayOwnExit` are read only through the snapshot fields.
#[allow(unused_imports)]
pub use types::{TrayNodeStatus, TrayChosenExit, TrayOwnExit, TrayNode, TrayPeer, TrayRoute, TrayMembers, TraySnapshot, TraySocks};

#[cfg(test)]
#[path = "roundtrip_tests.rs"]
mod roundtrip_tests;
