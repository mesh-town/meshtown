//! Membership list ("who may be a neighbour"): record format, tally, verdict view, boot,
//! watcher and member actions.
//!
//! What it does: declares the modules of the list.
//! What it doesn't do: holds no logic.

pub mod abilities;
pub mod actions;
mod actions_managed;
pub mod authority;
pub mod boot;
mod boot_plan;
mod deny;
mod managed;
mod reasons;
pub mod record;
mod record_input;
mod record_io;
mod record_keys;
pub mod report;
pub mod runtime;
mod subjects;
pub mod tally;
pub mod view;
mod view_diff;
pub mod watch;
