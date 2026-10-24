//! The bridge between the node and the web listener: `NodeCtl` implements `web::NodeControl`, a background task
//! accumulates the node snapshot, `run_node_with_ctl` (in `node.rs`) runs the cycle "start → work → stop or
//! restart into a new network" in one process.
//! What it doesn't do: it doesn't build the real `NodeSnapshot.transport/peers/domains` —
//! `node_ctl::snapshot::collect` fills only part of it (`members`, `socks`, `exit`); it doesn't
//! start the web server itself — that is another module, it is just handed an `Arc<NodeCtl>`.
//! Links: `node::run_node_with_ctl`/`node::run` hold an `Arc<NodeCtl>` and `Relaunch`, call
//! `attach`/`detach`/`next_command`/`stop_on_os_signal`/`replace_effective_settings`; `web::NodeControl` —
//! the only entry from outside the module.
//! Files: `boot_stamp.rs` — the cycle stamp (`boot_id`, `uptime_sec`); `guard_link.rs` — the link to the process memory guard; `proxy_link.rs` — the link to the process proxy; `closed_feed.rs` — the slot's "network closed" channel; `state.rs` — the phase, commands, `NodeCtl` and the lifecycle (`attach`/`detach`/gates);
//! `control.rs` — the page actions (`impl web_gui::NodeControl`); `invite_wait.rs` — waiting for
//! a sign-in by invite; `task.rs` — the background snapshot collection;
//! `live_paths.rs` — which settings paths take effect without a restart; `relaunch.rs` — rebuilding
//! `NodeConfig` when the network changes; `peer_addr.rs` — member address strings and their parsing;
//! `snapshot.rs` — collecting `NodeSnapshot` from a running node.

mod boot_stamp;
mod closed_feed;
mod control;
mod control_domains;
mod control_members;
mod control_node;
mod guard_link;
mod invite_wait;
mod leave_net;
mod live_paths;
mod members_seen;
mod peer_addr;
mod proxy_link;
mod relaunch;
mod state;
mod task;

pub(crate) mod snapshot;

pub(crate) use live_paths::apply_log_capacity;
pub use peer_addr::peer_lines;
pub use relaunch::Relaunch;
pub(crate) use relaunch::apply_key_input;
pub(crate) use state::Cmd;
pub use state::NodeCtl;
