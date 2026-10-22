//! Library of Mesh Town: transport, membership and the domain list.
//!
//! What it does: assembles the node modules — unified output, network key, DHT record format,
//! DHT entry, handshake on the secret, member bookkeeping, volume probe, path and NAT report,
//! node assembly.
//! What it doesn't do: it holds no CLI logic — that is `main.rs`.
//! Links: `main.rs` calls `node::run_node_with_ctl`, `network_key`, `words` and `invite`.
//!
//! Module map:
//! - `output` — unified formatted output to stdout.
//! - `network_key` — the network key and derived subkeys.
//! - `words` — the EFF word list, the random part of the key and its check.
//! - `net_mainline::record` — format and encryption of the DHT record.
//! - `peers_cache` — cache of members' last working addresses on disk (`peers-cache.json`).
//! - `peer_names` — last known names of other members on disk (`peer-names.json`).
//! - `docs_store` — path of the domain list storage directory next to the config (`docs-store/`); read only by `sync::replica::import`.
//! - `invite` — network invite string: building it by the console command `invite` and parsing the `--invite` flag.
//! - `handshake` — handshake on the network secret.
//! - `sync` — own synchronization on top of `net`: topic membership by flooding over the `SYNC` service (`sync::flood`) and own replica of records with the author's signature, reconciliation over `REPLICA` and import of `docs-store/` (`sync::replica`).
//! - `membership` — network members and gossip events.
//! - `access` — the membership list: signed admit and exclude records in the shared replica, the vote tally, the verdict view used by the handshake and the watcher.
//! - `transfer_probe` — volume probe over a connection.
//! - `net_mainline` — roster record format and the clock (the network part is removed).
//! - `domains` — the node's own domain list (no reconciliation) and console commands.
//! - `http_min` — minimal HTTP/1.1 client and TLS setup for checks.
//! - `doh` — own resolver for checks (DNS over HTTPS).
//! - `reachability` — step-by-step domain check and result classes.
//! - `checks` — `chk/` records and the check schedule.
//! - `console` — node console: commands from standard input (`invite`, checks, proxy, domains).
//! - `socks5` — SOCKS5 frames for the local proxy server.
//! - `route` — path choice for a domain: directly or through a network member.
//! - `exit` — the exit protocol `meshtown/exit/1` and byte forwarding.
//! - `ports` — publishing own TCP ports to network members and opening others': protocol `meshtown/port/1`, records `port/tcp/`.
//! - `proxy` — local SOCKS5 server and execution of the chosen path.
//! - `config_file` — settings file on disk: lookup, reading, merging with flags, writing.
//! - `node` — node assembly from all modules.
//! - `telemetry` — live node snapshot for the page: relay, NAT, DHT, members.
//! - `node_ctl` — the `web::NodeControl` trait for a real node, restart on network change.
//! - `web` — node page: built-in HTTP server and its API.
//!
//! Modules of two blocks. The core is modules without foreign network crates (built with `--no-default-features`);
//! the node layer is the rest, behind the `net-hyper` feature. A new module goes into the core block only if
//! it does not refer to the node layer; otherwise into the node layer block.

pub mod addr_order;
pub mod doh;
pub mod http_min;
#[cfg(any(test, all(target_arch = "arm", target_env = "musl")))]
pub mod libm_shim;
pub mod net;
pub mod reachability;
pub mod socks5;
pub mod sync;
pub mod tcp_tune;

#[cfg(feature = "net-hyper")]
pub mod access;
#[cfg(feature = "net-hyper")]
pub mod checks;
#[cfg(feature = "net-hyper")]
pub mod config_file;
#[cfg(feature = "net-hyper")]
pub mod console;
#[cfg(feature = "net-hyper")]
pub mod docs_store;
#[cfg(feature = "net-hyper")]
pub mod domains;
#[cfg(feature = "net-hyper")]
pub mod exit;
#[cfg(feature = "net-hyper")]
pub mod geoip;
#[cfg(feature = "net-hyper")]
pub mod handshake;
#[cfg(feature = "net-hyper")]
pub mod host;
#[cfg(feature = "net-hyper")]
pub mod invite;
#[cfg(feature = "net-hyper")]
pub mod membership;
#[cfg(feature = "net-hyper")]
pub mod net_hyper;
#[cfg(feature = "net-hyper")]
pub mod net_mainline;
#[cfg(feature = "net-hyper")]
pub mod net_hyperdht;
#[cfg(feature = "net-hyper")]
pub mod network_key;
#[cfg(feature = "net-hyper")]
pub mod node;
#[cfg(feature = "net-hyper")]
pub mod node_ctl;
#[cfg(feature = "net-hyper")]
pub mod node_name;
#[cfg(feature = "net-hyper")]
pub mod node_state;
pub mod program_name;
#[cfg(feature = "net-hyper")]
pub mod output;
#[cfg(feature = "net-hyper")]
pub mod peer_names;
#[cfg(feature = "net-hyper")]
pub mod peers_cache;
#[cfg(feature = "net-hyper")]
pub mod ports;
#[cfg(feature = "net-hyper")]
pub mod program_version;
#[cfg(feature = "net-hyper")]
pub mod proxy;
#[cfg(feature = "net-hyper")]
pub mod route;
#[cfg(feature = "net-hyper")]
pub mod stale_peers;
#[cfg(feature = "net-hyper")]
pub mod stale_prune;
#[cfg(feature = "net-hyper")]
pub mod telemetry;
#[cfg(feature = "net-hyper")]
pub mod transfer_probe;
#[cfg(feature = "net-hyper")]
pub mod web;
#[cfg(feature = "net-hyper")]
pub mod words;
