//! UDP port publishing: datagrams as length-prefixed frames over one stream per local sender.
//!
//! What it does: the stream wrapper with idle limit and slot (`session`), the one file that names the
//! framing crate (`pipe`), the publishing-side loop (`serve`), the crate log bridge (`log_bridge`).
//! What it does not do: it does not decide who may use a port; `mod.rs` has no logic.

mod log_bridge;
mod pipe;
mod serve;
mod session;

pub(crate) use pipe::{open_udp_listener, UdpListenerHandle};
pub(crate) use serve::serve_udp_stream;
pub(crate) use session::{
    UdpSession, MAX_SENDERS_PER_OPEN, MAX_UDP_SESSIONS_SERVED, OPEN_IDLE, SERVE_IDLE, UDP_SESSION_MARK,
};
