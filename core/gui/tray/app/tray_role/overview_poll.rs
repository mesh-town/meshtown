//! Polling the node state: the summary of all networks first, the single-network state when the node
//! does not know the summary (an older node answers 404).

use std::path::Path;

use super::api::{fetch_reply, NodeRequest};
use crate::gui::tray::app::local_api::{ApiError, Reply};

/// How many calls skip the summary after a node answered 404 to it.
const SKIPS_AFTER_NOT_FOUND: u8 = 5;

/// Remembers that the node has no summary, so an old node is not asked for it on every call.
pub(super) struct OverviewGate {
    skip: u8,
}

impl OverviewGate {
    pub(super) fn new() -> Self {
        Self { skip: 0 }
    }

    /// Whether this call should ask for the summary; a skipped call counts down.
    pub(super) fn want_overview(&mut self) -> bool {
        if self.skip > 0 {
            self.skip -= 1;
            return false;
        }
        true
    }

    pub(super) fn saw_not_found(&mut self) {
        self.skip = SKIPS_AFTER_NOT_FOUND;
    }

    pub(super) fn saw_overview(&mut self) {
        self.skip = 0;
    }
}

/// The state reply for `enter` and the views: the summary, or `GET /api/state` as a fallback.
pub(super) async fn fetch_node_reply(
    port: u16,
    token_path: &Path,
    gate: &mut OverviewGate,
) -> Result<Reply, ApiError> {
    if !gate.want_overview() {
        return fetch_reply(port, token_path, NodeRequest::State, None).await;
    }
    let reply = fetch_reply(port, token_path, NodeRequest::Overview, None).await;
    if matches!(reply, Ok(Reply::OtherStatus(404))) {
        gate.saw_not_found();
        return fetch_reply(port, token_path, NodeRequest::State, None).await;
    }
    if matches!(reply, Ok(Reply::Ok(_))) {
        gate.saw_overview();
    }
    reply
}

#[cfg(test)]
#[path = "overview_poll_tests.rs"]
mod tests;
