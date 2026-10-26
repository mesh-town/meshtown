//! Exit protocol `meshtown/exit/1`: a node opens a connection to a domain on behalf of a network member.
//!
//! What it does: accepts a request frame (version, domain name, port), checks permission and conditions,
//! opens a connection to the domain and shuttles bytes between the QUIC stream and the TCP socket;
//! on the other side it asks a member for the same and keeps one connection per member.
//! What it doesn't do: it doesn't parse what is inside the tunnel, doesn't limit
//! speed, doesn't build chains longer than one hop; the rules (only ports 80 and 443) and
//! (only domains from the list) have been removed — the exit owner serves any domain on any
//! port, except addresses of local and service networks and its own (`address_guard`,
//! code 7), and controls only `exit.enabled`.
//! Links: permission comes from `crate::members::membership`, the list from `crate::proxy::domains`, the address from `crate::site_checks::doh`;
//! this protocol is called by `crate::proxy`.
//!
//! Files: `outbound` — where and how the exit connects outward (the address guard: its tables are pure functions, the probe
//! of the node's own address uses a socket without packets; address choice, refusals, dialing: IPv4 first, then IPv6 from AAAA records), `clients` — the registry of exit clients (who is currently going through
//! the node), `frame` — the request/reply frame, codes and their parsing (pure functions, no networking);
//! `server` — the handler of incoming connections (`ExitConfig`, `exit_service`, `serve_exit_stream`);
//! `pool` — connections to members and the `ask_exit_to_connect` request; `splice` — shuttling bytes between
//! the network stream and TCP, shared by both sides; `report` — `[exit]` and `[exit-client]` lines every 5 s while traffic flows;
//! `gateway_memo` — exits that refused the UDP gateway target (asked again after a while);
//! `udpgw` — UDP datagrams over one exit stream (the conventional target 240.0.0.1:7300);
//! `traffic` — exit traffic speed per peer (a registry of samples for `/api/state`).

mod ask_reply;
mod clients;
mod frame;
mod gateway_memo;
mod limits;
mod outbound;
mod pool;
mod report;
mod server;
mod splice;
mod stream_tasks;
#[cfg(test)]
pub(crate) mod test_support;
pub mod traffic;
mod udpgw;

pub use ask_reply::ExitBusy;
pub use clients::{Clients, EXIT_CLIENT_LINGER};
pub use frame::{
    encode_exit_request, exit_code_text, parse_exit_request, EXIT_ALPN,
    EXIT_CODE_ADDRESS_FORBIDDEN, EXIT_CODE_BAD_REQUEST, EXIT_CODE_CONNECT_FAILED,
    EXIT_CODE_NOT_IN_LIST, EXIT_CODE_NO_ADDRESS, EXIT_CODE_OK, EXIT_CODE_PORT_NOT_ALLOWED,
    EXIT_CODE_REFUSED, EXIT_PROTOCOL_VERSION, EXIT_TCP_CONNECT_TIMEOUT,
};
pub use gateway_memo::{GatewayMemo, GATEWAY_REFUSAL_TTL};
pub(crate) use limits::rss_ceiling_kib;
pub use limits::ExitLimits;
pub use udpgw::{is_gateway_target, UDPGW_HOST, UDPGW_PORT};
// Consumed by the node-side UDP bridge.
pub use pool::{ask_exit_to_connect, PeerConnPool, StreamBusy, OPEN_STREAM_TIMEOUT};
pub use server::{exit_service, ExitConfig};
pub use splice::{splice, HALF_CLOSE_GRACE};
pub use traffic::{Direction, ExitTraffic, PeerRate};
pub(crate) use udpgw::codec as udpgw_codec;
