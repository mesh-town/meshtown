//! UDP gateway ("udpgw") served inside one exit stream.
//!
//! A client that cannot open UDP associations through a SOCKS proxy wraps its datagrams into frames and
//! sends them over one ordinary TCP-like stream. The exit recognizes the stream by the conventional
//! target `UDPGW_HOST:UDPGW_PORT`, answers `EXIT_CODE_OK` and serves frames until the stream closes.
//!
//! Frames carrying none of the known flags are answered with an error frame; the stream stays open.
//!
//! Files: `codec` — frame parsing and building (pure, no networking); `session` — the per-stream
//! loop with one UDP socket per `conn_id`.
//! The frame layout follows the tun2proxy udpgw protocol (MIT), see `THIRD_PARTY_NOTICES.md`.

pub(crate) mod codec;
#[cfg(test)]
#[path = "golden_tests.rs"]
mod golden_tests;
mod session;
#[cfg(test)]
mod test_support;

pub(super) use session::serve;

/// The conventional target host that selects the UDP gateway. A class E address: it never
/// belongs to the virtual DNS pool (198.18.0.0/15) and the exit never connects to it as a TCP target.
pub const UDPGW_HOST: &str = "240.0.0.1";
/// The conventional target port that selects the UDP gateway.
pub const UDPGW_PORT: u16 = 7300;

/// Whether an exit request names the UDP gateway.
pub fn is_gateway_target(host: &str, port: u16) -> bool {
    host == UDPGW_HOST && port == UDPGW_PORT
}
