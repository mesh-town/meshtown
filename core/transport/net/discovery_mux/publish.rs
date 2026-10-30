//! Publishing the own address to all publishing discovery sources.

use super::DiscoveryMux;
use crate::transport::net::addr::PeerAddr;
use crate::transport::net::discovery::Discovery;
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::stream::BoxFuture;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;

impl DiscoveryMux {
    /// To all publishing sources at once; the error of the earliest-added one goes up.
    pub fn publish(&self, own_addr: &PeerAddr) -> BoxFuture<'_, NetResult<()>> {
        let own_addr = own_addr.clone();
        Box::pin(async move {
            let (set, index) = self.spawn_publishers(&own_addr);
            Self::first_error_by_add_order(Self::collect_publish_errors(set, index).await)
        })
    }

    /// Starts `publish_one` for every publishing source; returns the tasks and the add order of each.
    fn spawn_publishers(
        &self,
        own_addr: &PeerAddr,
    ) -> (JoinSet<NetResult<()>>, HashMap<tokio::task::Id, usize>) {
        let limit = self.timing.publish_timeout;
        let mut set = JoinSet::new();
        let mut index = HashMap::new();
        let publishers = self.sources.iter().filter(|s| s.publishes());
        for (i, src) in publishers.enumerate() {
            let job = Self::publish_one(src.clone(), own_addr.clone(), limit);
            index.insert(
                crate::transport::net::task_label::spawn_in(&mut set, job).id(),
                i,
            );
        }
        (set, index)
    }

    /// Waits for all tasks; a panic of a task is `NetError::Other`.
    async fn collect_publish_errors(
        mut set: JoinSet<NetResult<()>>,
        index: HashMap<tokio::task::Id, usize>,
    ) -> Vec<(usize, NetError)> {
        let mut errors: Vec<(usize, NetError)> = Vec::new();
        while let Some(done) = set.join_next_with_id().await {
            match done {
                Ok((_, Ok(()))) => {}
                Ok((id, Err(e))) => errors.push((index[&id], e)),
                Err(e) => errors.push((index[&e.id()], NetError::Other(e.into()))),
            }
        }
        errors
    }

    /// The error of the earliest-added source, if any source failed.
    fn first_error_by_add_order(mut errors: Vec<(usize, NetError)>) -> NetResult<()> {
        errors.sort_by_key(|(i, _)| *i);
        errors.into_iter().next().map_or(Ok(()), |(_, e)| Err(e))
    }

    async fn publish_one(
        src: Arc<dyn Discovery>,
        own_addr: PeerAddr,
        limit: Duration,
    ) -> NetResult<()> {
        let sent = tokio::time::timeout(limit, src.publish(&own_addr)).await;
        sent.unwrap_or(Err(NetError::Timeout))
    }
}
