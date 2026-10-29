//! Path observer of a relayed session: reports the one change that can happen, the move from the relay to the
//! direct path.

use crate::transport::net::link::{PathChange, PathKind};
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::transport::PathWatch;
use tokio::sync::watch;
use townjump_dht::RelayPath;

/// Watches the relay path of one connection; `None` source means the session never ran through a relay.
pub(crate) struct RelayPathWatch {
    source: Option<watch::Receiver<RelayPath>>,
}

impl RelayPathWatch {
    pub(crate) fn new(source: Option<watch::Receiver<RelayPath>>) -> Self {
        Self { source }
    }

    async fn wait_direct(&mut self) -> Option<PathChange> {
        let mut source = self.source.take()?;
        source.wait_for(|p| *p == RelayPath::Direct).await.ok()?;
        Some(PathChange {
            path: PathKind::Direct,
            rtt: None,
            log_line: "moved from the relay to a direct path".to_string(),
        })
    }
}

impl PathWatch for RelayPathWatch {
    fn next(&mut self) -> BoxFuture<'_, Option<PathChange>> {
        Box::pin(self.wait_direct())
    }
}

#[cfg(test)]
#[path = "path_watch_tests.rs"]
mod tests;
