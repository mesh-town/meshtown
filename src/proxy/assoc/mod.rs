//! UDP ASSOCIATE of the SOCKS5 server: a client datagram socket relayed to the UDP gateway of an exit.
//!
//! Files: `serve` — the command: slot, local socket, route, reply, journal; `relay` — the two pumps and the
//! lifetime of an association; `stats` — counters shared by the pumps; `upstream` — the traits a relay pushes
//! datagrams through and the exit implementation; `bridge` — SOCKS datagram ↔ udpgw frame conversion.

mod bridge;
mod relay;
mod serve;
mod stats;
mod upstream;

pub(in crate::proxy) use serve::serve_associate;
pub use serve::udp_slots;
