//! The external deadline of a network step in the `UDP ASSOCIATE` scenarios.
//! Plugged in as `#[path = "common/socks_udp_limit.rs"] mod udp_limit;` by the crates that wait on UDP replies.

use std::time::Duration;

/// The external deadline of a network step.
pub(crate) const UDP_LIMIT: Duration = Duration::from_secs(10);
