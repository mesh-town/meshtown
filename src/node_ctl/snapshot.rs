//! Entry point of the folder: collecting the node snapshot. `collect` (in `collect.rs`) reads the running
//! `RunningNode` and telemetry in one pass and puts them into `Inputs`; `build` (in `build.rs`) is a
//! pure function with no network that decides everything else (member statuses, order, route by
//! domain).

mod build;
mod collect;
mod doc_cache;
mod peers;
mod ports;
mod process;
mod route_state;

pub(crate) use collect::{collect, link_status, Collected};
pub(in crate::node_ctl) use collect::overlay_members;
pub(crate) use doc_cache::DocCache;
pub(in crate::node_ctl) use process::{overlay_process, status_of, ProcessFacts};
