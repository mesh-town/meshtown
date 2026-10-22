//! Node assembly: ties all modules into a working process.
//!
//! What it does: brings up the transports (`hyper`; `mem` in tests), the sync (swarm and domain list replica), availability
//! checks, the network stack with discovery (`invite`, `cache`, `hyperdht`) and the node's main loop (member discovery,
//! handshakes, invites, periodic status).
//! What it doesn't do: doesn't parse command-line arguments — that is `main.rs`.
//! Links: uses all the other library modules; `main.rs` calls `run_node_with_ctl`/`run_node_awaiting_key`.
//!
//! Files:
//! - `identity` — the node identity state (`IdentityState`, `TemporaryReason`).
//! - `config` — node settings for assembly (`NodeConfig`).
//! - `running` — the running node and its live handles (`RunningNode`).
//! - `start` — assembling and starting the node (`start_node`, `bind_ephemeral`), split into start phases.
//! - `event_loop` — the node's main loop: discovered addresses, member events, dial failures,
//!   retry deadlines and periodic tasks.
//! - `guard` — the node's memory guard: a separate OS thread measures RSS once a second, prints `[guard]` and exits before swap.
//! - `alloc` — the jemalloc allocator with a heap profile (`heap-prof` feature, Linux only) for the memory guard.
//! - `dial` — dialing a discovered member and the handshake as the initiator (`PeerDialer`).
//! - `redial` — the redial state of a not-yet-connected member, with no network (`Redial`).
//! - `lifecycle` — the stop signal and the "start → run → stop or restart" loops
//!   (`run`, `run_node_with_ctl`, `run_node_awaiting_key`).

mod alloc;
mod config;
mod dial;
mod event_loop;
mod guard;
mod identity;
mod lifecycle;
mod peer_path;
mod redial;
mod discovery_sources;
pub(crate) mod net_names;
mod rejoin;
mod doc_changes;
mod running;
mod socks_task;
mod start;

pub use config::{MemoryLimits, NodeConfig, SocksListen};
pub use identity::{IdentityState, TemporaryReason};
pub use lifecycle::{run, run_node_awaiting_key, run_node_with_ctl};
pub use doc_changes::DocChanges;
pub use running::RunningNode;
pub use start::{bind_ephemeral, start_node};
