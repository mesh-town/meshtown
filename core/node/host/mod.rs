//! Process host: what the process owns once, however many nodes it starts and stops.
//! - `run` — the assembly point "services → page → body → page shutdown";
//! - `process` — the process services: the memory guard;
//! - `add_slot` — adding a network at run time;
//! - `proxy_claim` — choosing the network whose exit the proxy uses;
//! - `retire` — leaving a network: setting its folder or files aside;
//! - `saved_slots` — starting the networks saved under `nets/`;
//! - `slots` — the map of network slots the page and the signal reach the nodes through;
//! - `signal` — the Ctrl-C/SIGTERM stop signal.

mod add_slot;
mod process;
mod proxy_claim;
mod retire;
mod run;
mod saved_slots;
pub(crate) mod signal;
mod slots;

pub use crate::config::config_file::NetSlotId;
pub use process::ProcessServices;
pub use run::{Host, PageRole};
pub use saved_slots::MAX_NETS;
