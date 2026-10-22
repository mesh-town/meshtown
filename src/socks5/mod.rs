//! SOCKS5 frames (RFC 1928): parsing the client's request and building the reply, without external crates.
//!
//! What it does: reads the greeting and the connect request, builds the ten-byte server reply,
//! and can also do the reverse — act as a client through a foreign SOCKS5 server (the console command
//! `get`).
//! What it does not do: it does not authenticate (RFC 1929), does not support the `BIND` command
//! and `UDP ASSOCIATE`, does not connect to IPv6 addresses, and dials nowhere on its own except `client_connect`.
//! Links: the server on top of these frames is `crate::proxy`; the request frame to a network member is different,
//! it is in `crate::exit`.
//!
//! Files: `frame` — codes, parsing and building the frame (pure functions); `wire` — the server's I/O
//! (greeting, request, reply); `client` — the `client_connect` client.

mod client;
mod frame;
mod wire;

pub use client::client_connect;
pub use frame::{
    encode_socks5_reply, encode_socks5_request, parse_socks5_request, ConnectRequest, RequestError, Target,
    ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6, CMD_CONNECT, MAX_NAME_LEN, METHOD_NO_ACCEPTABLE, METHOD_NO_AUTH,
    REP_ATYP_NOT_SUPPORTED, REP_CMD_NOT_SUPPORTED, REP_GENERAL, REP_HOST_UNREACHABLE, REP_OK,
    VERSION,
};
pub use wire::{read_greeting, read_request_bytes, write_reply};
