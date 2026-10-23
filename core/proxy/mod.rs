//! The local proxy (SOCKS5 and HTTP on one port) and execution of the chosen path.
//!
//! What it does: listens on `127.0.0.1` (or `0.0.0.0` in the `socks.access: localnet` mode), accepts SOCKS5 and HTTP requests (`CONNECT` and a request with an absolute URI) and serves the PAC (`GET /proxy.pac`), asks `crate::proxy::route`
//! for a path, connects directly or through a network member, copies bytes; it also holds the
//! console commands and the `get` client that goes through our own SOCKS5 server.
//! What it does not do: it does not listen on public addresses (`admit` closes foreign ones), does not authenticate the client, does not intercept
//! system traffic (that is TUN, a separate stage), does not parse what is inside the tunnel.
//! Links: frames — `crate::proxy::socks5`, path choice — `crate::proxy::route`, exit — `crate::exit`,
//! addresses — `crate::site_checks::doh`; started from `crate::node::start_node`.
//!
//! Files: `front` — the one process-wide listener (`ProxyFront`), its published state (`ProxyFace`) and the choice of the serving network (`ProxySeat`); `admit` — client admission by address; `serve` — the listening socket and SOCKS5 serving; `assoc` — UDP ASSOCIATE; `path` — the shared path (direct or through
//! a member); `http` and `http_head` — the HTTP face and head parsing; `pac` and `pac.js` — the proxy autoconfiguration file; `outcome` — the serving outcome for the log line; `get` — the `get` command
//! (an HTTP client over our own SOCKS5); `console` — console commands.

#[cfg(feature = "net-hyper")]
pub mod domains;
#[cfg(feature = "net-hyper")]
pub mod route;
pub mod socks5;

#[cfg(feature = "net-hyper")]
mod admit;
#[cfg(feature = "net-hyper")]
mod assoc;
#[cfg(feature = "net-hyper")]
mod console;
#[cfg(feature = "net-hyper")]
mod front;
#[cfg(feature = "net-hyper")]
mod get;
#[cfg(feature = "net-hyper")]
mod http;
#[cfg(feature = "net-hyper")]
mod http_head;
#[cfg(feature = "net-hyper")]
mod outcome;
#[cfg(feature = "net-hyper")]
mod pac;
#[cfg(feature = "net-hyper")]
mod path;
#[cfg(feature = "net-hyper")]
mod serve;

#[cfg(feature = "net-hyper")]
pub use admit::Admission;
#[cfg(feature = "net-hyper")]
pub use assoc::udp_slots;
#[cfg(feature = "net-hyper")]
pub use console::{execute_proxy_command, parse_proxy_line, ProxyCommand, ProxyConsole};
#[cfg(feature = "net-hyper")]
pub use front::{ProxyFace, ProxyFront, ProxySeat};
#[cfg(feature = "net-hyper")]
pub use get::get_via_socks;
#[cfg(feature = "net-hyper")]
pub use outcome::{outcome_line, ProxyOutcome};
#[cfg(feature = "net-hyper")]
pub use pac::pac_url;
#[cfg(feature = "net-hyper")]
pub use serve::{
    bind_proxy_listener, listen_ip, BindError, ProxyConfig, DEFAULT_SOCKS_PORT,
    DIRECT_TCP_CONNECT_TIMEOUT, NO_PROXY_NETWORK,
};
