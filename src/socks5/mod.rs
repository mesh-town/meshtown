//! SOCKS5 frames (RFC 1928): parsing the client's request and building the reply, without external crates.
//!
//! What it does: reads the greeting and the request (`CONNECT` or `UDP ASSOCIATE`), builds the ten-byte server reply,
//! parses and builds the header of a UDP datagram,
//! and can also do the reverse — act as a client through a foreign SOCKS5 server (the console command
//! `get`).
//! What it does not do: it does not authenticate (RFC 1929), does not support the `BIND` command,
//! and dials nowhere on its own except `client_connect`.
//! Links: the server on top of these frames is `crate::proxy`; the request frame to a network member is different,
//! it is in `crate::exit`.
//!
//! Files: `frame` — codes, parsing and building the frame (pure functions); `udp` — the UDP datagram header; `wire` — the server's I/O
//! (greeting, request, reply); `client` — the `client_connect` client.

mod client;
mod frame;
mod udp;
mod wire;

pub use client::client_connect;
pub use frame::{
    encode_socks5_reply, encode_socks5_reply_bound, encode_socks5_request,
    encode_udp_associate_request, parse_socks5_request, Command, Request, RequestError, Target,
    ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6, CMD_CONNECT, CMD_UDP_ASSOCIATE, MAX_NAME_LEN,
    METHOD_NO_ACCEPTABLE, METHOD_NO_AUTH, REP_ATYP_NOT_SUPPORTED, REP_CMD_NOT_SUPPORTED,
    REP_GENERAL, REP_HOST_UNREACHABLE, REP_OK, VERSION,
};
pub use udp::{
    encode_udp_reply, encode_udp_request, parse_udp_datagram, UdpHeaderError, UdpPacket,
};
pub use wire::{read_greeting, read_request_bytes, write_reply, write_reply_bound};
