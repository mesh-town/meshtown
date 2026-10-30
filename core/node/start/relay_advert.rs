//! Publishing this node's own relay (key and public address) to the network: the record `relay/v1` in the shared replica.

use std::sync::Arc;

use tokio::sync::watch;

use crate::node::output::{say, NET_NAME};
use crate::node::start::relay_candidates::{encode_relay_value, RELAY_ADVERT_KEY};
use crate::transport::net::{NetResult, Replica};
use crate::transport::net_hyper::RelayAdvert;

/// Keeps the advert in step with the relay: `Some` writes it, `None` retracts it. Ends when the sender is
/// dropped. Replica errors are logged; the next change writes again.
pub(crate) async fn advertise_relay(
    mut adverts: watch::Receiver<Option<RelayAdvert>>,
    replica: Arc<dyn Replica>,
) {
    let mut prev = None;
    loop {
        let advert = *adverts.borrow_and_update();
        if let Some(line) = relay_line(prev, advert) {
            say("relay", line);
        }
        prev = advert;
        let done = apply(advert, &*replica).await;
        if let Err(e) = done {
            tracing::warn!("relay: updating the advert failed: {e}");
        }
        if adverts.changed().await.is_err() {
            return;
        }
    }
}

/// The log line for the relay appearing or going; `None` while the node stays in the same state
/// (a new address alone is not announced).
fn relay_line(prev: Option<RelayAdvert>, now: Option<RelayAdvert>) -> Option<String> {
    match (prev, now) {
        (None, Some(_)) => Some(format!(
            "serving as relay for members of network {NET_NAME}"
        )),
        (Some(_), None) => Some("stopped serving as relay".to_string()),
        _ => None,
    }
}

/// Writes or retracts the advert for the current relay.
async fn apply(advert: Option<RelayAdvert>, replica: &dyn Replica) -> NetResult<()> {
    match advert {
        Some(a) => {
            replica
                .set_own(RELAY_ADVERT_KEY, encode_relay_value(&a))
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
