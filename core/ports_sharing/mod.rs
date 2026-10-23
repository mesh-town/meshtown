//! Publishing our own TCP ports to network members and opening others': the `meshtown/port/1` protocol,
//! `port/tcp/` entries in the shared document.
//!
//! What it does: the publication vocabulary (types, entry key and value, protocol frame, table
//! of our own publications, reading and writing the document); the other modules build on this vocabulary.
//! What it does not do: it does not decide who is a member and does not copy bytes itself; `mod.rs` has no logic.
//! Links: the right — `crate::members::membership`, the document — `crate::proxy::domains::DomainList`, byte copying
//! — `crate::exit`.
//!
//! Files: `types` — types; `record` — entry key and value; `frame` — protocol frame;
//! `table` — table of our own publications; `store` — the document; `server`, `announce`, `open`,
//! `dial` — the publishing and opening sides (imported by full path).

pub mod announce;
pub mod dial;
pub mod frame;
pub mod open;
pub mod record;
pub mod server;
pub mod store;
pub mod table;
pub mod types;

pub use dial::{dial_peer_port, PortDialError, PortReach, PortStream};
pub use frame::{
    encode_port_reply, encode_port_request, parse_port_reply, parse_port_request, port_code_text,
    PORT_ALPN, PORT_CODE_BAD_REQUEST, PORT_CODE_CONNECT_FAILED, PORT_CODE_NOT_PUBLISHED,
    PORT_CODE_OK, PORT_CODE_REFUSED, PORT_FRAME_VERSION,
};
pub use record::{
    decode_port_record, encode_port_record, port_from_key, port_key, PortRecord, PORT_PREFIX,
    PORT_RECORD_VERSION,
};
pub use server::port_service;
pub use store::{PortView, PortsDoc};
pub use table::{LocalState, PublishedPortsTable};
pub use types::{Audience, Opened, Published, MAX_NAME_CHARS, MAX_PORT_LIST_ENTRIES};
