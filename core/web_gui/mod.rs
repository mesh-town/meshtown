//! Listener for the node page: an embedded HTTP server (the bind address depends on the `web.access` mode) that serves the page
//! for controlling the node and its API. The stack is HTTP/1.1 on axum 0.8 + tower-http.
//! The contract is described in
//! `docs/WEB-API.md`; the listener sees the node only through the `NodeControl` trait.
//!
//! Module files: `options.rs` — settings and handles (`WebOptions`, `WebHandle`, `WebShared`);
//! `host_control.rs` — `HostControl` (the slots the page addresses) and `single_slot_host`; `control.rs` — the `NodeControl` trait and errors (`ActionError`, `WebStartError`); `snapshot.rs` —
//! `NodeSnapshot` and nested structs; `listener.rs` — `start_web_page`/`accept_loop`/`shutdown`;
//! `routes/` — routes and handlers; `config_api/` — config state and API; `guard.rs` —
//! request checks; `http.rs` — connection-serving server wrapper; `ring.rs` — log ring;
//! `token.rs` — page token; `cookie.rs` — session cookie; `auth/` — sign-in: password hash and file,
//! temporary tokens and sessions, attempt limiter, address admission, sign-in state (no `App`).

// A ready-made axum response (`Response`, >=128 bytes) as `Err` is the usual handler idiom: the error
// goes straight to the client, and a `Box` would add a memory allocation on every refusal for no gain.
#![allow(clippy::result_large_err, clippy::large_enum_variant)]

pub(crate) mod auth;
mod config_api;
mod control;
mod cookie;
mod guard;
mod host_control;
mod http;
mod listener;
mod members;
mod net_brief;
mod options;
mod ring;
mod routes;
mod snapshot;
#[cfg(test)]
pub(crate) mod test_host;
mod token;

pub use auth::password_file::{remove_password_file, PASSWORD_FILE_NAME};
#[doc(hidden)]
pub use config_api::ConfigState;
pub use control::{
    ActionError, InviteJoinState, InviteJoinView, MemberActionReq, MemberError, MemberFuture,
    NodeControl, PendingInvite, SaveKey, WebStartError,
};
pub use host_control::{
    single_slot_host, single_slot_host_with_front, AddedSlot, HostControl, HostError, SlotHandle,
};
pub use listener::start_web_page;
pub use members::dto::{
    AdmissionInfo, Counts, ExcludedInfo, ForgetInfo, MemberRow, MembersReply, RefusalInfo, VoteInfo,
};
pub use options::{WebHandle, WebOptions, WebShared, DEFAULT_DRAIN, TOKEN_FILE_NAME};
pub use ring::LogRing;
pub use snapshot::*;

use std::path::PathBuf;
use std::sync::Arc;

pub(crate) struct App {
    pub shared: Arc<WebShared>,
    pub host: Arc<dyn HostControl>,
    pub token_dir: Option<PathBuf>,
    pub stopping: std::sync::atomic::AtomicBool,
}

pub(crate) fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
