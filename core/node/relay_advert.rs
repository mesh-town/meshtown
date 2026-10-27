//! Publishing this node's own relay key to the network: the record `relay/v1` in the shared replica.

use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::watch;

use crate::node::start::relay_candidates::RELAY_ADVERT_KEY;
use crate::transport::net::{NetResult, Replica};

/// Keeps the advert in step with the relay keys: `Some` writes it, `None` retracts it. Ends when the sender is
/// dropped. Replica errors are logged; the next key change writes again.
pub(crate) async fn advertise_relay(
    mut keys: watch::Receiver<Option<[u8; 32]>>,
    replica: Arc<dyn Replica>,
) {
    loop {
        let key = *keys.borrow_and_update();
        let done = apply(key, &*replica).await;
        if let Err(e) = done {
            tracing::warn!("relay: updating the advert failed: {e}");
        }
        if keys.changed().await.is_err() {
            return;
        }
    }
}

/// Writes or retracts the advert for the current key.
async fn apply(key: Option<[u8; 32]>, replica: &dyn Replica) -> NetResult<()> {
    match key {
        Some(k) => {
            replica
                .set_own(RELAY_ADVERT_KEY, Bytes::copy_from_slice(&k))
                .await
        }
        None => retract_relay(replica).await,
    }
}

/// Deletes the own advert, only if a live one exists.
pub(crate) async fn retract_relay(replica: &dyn Replica) -> NetResult<()> {
    let own = replica.get(replica.me(), RELAY_ADVERT_KEY).await?;
    match own {
        Some(entry) if !entry.is_tombstone() => replica.del_own(RELAY_ADVERT_KEY).await,
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "relay_advert_tests.rs"]
mod tests;
