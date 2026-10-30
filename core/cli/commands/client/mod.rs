//! The client side of a running node: where to find it and one request to it.
//!
//! - `target` - the settings file, the page port and the token, and the HTTP call.

mod target;

pub(crate) use target::{LinkError, Target};
