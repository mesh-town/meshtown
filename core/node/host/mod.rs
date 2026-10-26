//! Process host: what the process owns once, however many nodes it starts and stops.
//! - `run` — the assembly point "services → page → body → page shutdown";
//! - `process` — the process services: the memory guard;
//! - `add_slot` — adding a network at run time;
//! - `closed_watch` — leaving a network that an administrator closed;
//! - `control` — the page's view of the slot map (`HostControl`);
//! - `launch` — starting one network slot;
//! - `notices` — what the page is told and must acknowledge (a deleted network);
//! - `proxy_claim` — choosing the network whose exit the proxy uses;
//! - `retire` — leaving a network: setting its folder or files aside;
//! - `saved_slots` — starting the networks saved under `nets/`;
//! - `slots` — the map of network slots the page and the signal reach the nodes through;
//! - `signal` — the Ctrl-C/SIGTERM stop signal;
//! - `test_keys` — key texts for the tests.

mod add_slot;
mod closed_watch;
mod control;
mod launch;
mod notices;
mod process;
mod proxy_claim;
mod retire;
mod run;
mod saved_slots;
pub(crate) mod signal;
mod slots;
#[cfg(test)]
pub(crate) mod test_keys;

pub use crate::config::config_file::NetSlotId;
pub use notices::Notice;
pub use process::ProcessServices;
pub use run::{Host, PageRole};
pub use saved_slots::MAX_NETS;
