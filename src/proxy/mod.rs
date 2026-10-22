//! The local proxy (SOCKS5 and HTTP on one port) and execution of the chosen path.
//!
//! What it does: listens on `127.0.0.1` (or `0.0.0.0` in the `socks.access: localnet` mode), accepts SOCKS5 and HTTP requests (`CONNECT` and a request with an absolute URI) and serves the PAC (`GET /proxy.pac`), asks `crate::route`
//! for a path, connects directly or through a network member, copies bytes; it also holds the
//! console commands and the `get` client that goes through our own SOCKS5 server.
//! What it does not do: it does not listen on public addresses (`admit` closes foreign ones), does not authenticate the client, does not intercept
//! system traffic (that is TUN, a separate stage), does not parse what is inside the tunnel.
//! Links: frames — `crate::socks5`, path choice — `crate::route`, exit — `crate::exit`,
//! addresses — `crate::doh`; started from `crate::node::start_node`.
//!
//! Files: `admit` — client admission by address; `serve` — the listening socket and SOCKS5 serving; `path` — the shared path (direct or through
//! a member); `http` and `http_head` — the HTTP face and head parsing; `pac` and `pac.js` — the proxy autoconfiguration file; `outcome` — the serving outcome for the log line; `get` — the `get` command
//! (an HTTP client over our own SOCKS5); `console` — console commands.

mod admit;
mod console;
mod get;
mod http;
mod http_head;
mod outcome;
mod pac;
mod path;
mod serve;

pub use admit::Admission;
pub use console::{execute_proxy_command, parse_proxy_line, ProxyCommand, ProxyConsole};
pub use get::get_via_socks;
pub use outcome::{outcome_line, ProxyOutcome};
pub use pac::pac_url;
pub use serve::{bind_proxy_listener, listen_ip, serve_proxy, BindError, ProxyConfig, DEFAULT_SOCKS_PORT, DIRECT_TCP_CONNECT_TIMEOUT};
