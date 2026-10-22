//! Core of signing in to the page: synchronous parts without network and without `App`.

pub(crate) mod access;
pub(crate) mod iface_gate;
pub(crate) mod iface_kind;
pub(crate) mod iface_nets;
pub(crate) mod iface_os;
pub(super) mod limiter;
pub(super) mod password;
pub(super) mod password_file;
pub(crate) mod state;
pub(super) mod tickets;
