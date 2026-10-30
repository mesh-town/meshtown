//! The client side of a running node: where to find it, one request to it and what comes back.
//!
//! - `target` - the settings file, the page port and the token, and the HTTP call.
//! - `node` - a connected node: the chosen network, requests, settings changes.
//! - `api_error` - a refusal from the node and the advice that goes with it.
//! - `print` - JSON and tables on stdout.

// Each command uses a part of what the client offers.
#![allow(dead_code, unused_imports)]

mod api_error;
mod node;
mod print;
mod target;

pub(crate) use api_error::ApiError;
pub(crate) use node::{pick_net, query_escape, Node};
pub(crate) use print::{print_json, print_table};
pub(crate) use target::{LinkError, Target};
