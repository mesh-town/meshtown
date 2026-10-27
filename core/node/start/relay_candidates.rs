//! Relay candidates for a dial: members of this network that advertise a relay node in the shared replica.
//!
//! An advert is the record `relay/v1` of the member in the network's replica: the first 32 bytes of the value are
//! the key of the member's relay node, a longer tail is ignored. Only live, admitted members count, so a relay of
//! another network or of a departed node is never offered.

use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;

use crate::members::membership::Members;
use crate::node::node_state::read_values;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::{PeerId, Replica};
use crate::transport::net_hyper::{RelayCandidate, RelaySource};

/// Key of the relay advert in the network's shared replica.
pub(crate) const RELAY_ADVERT_KEY: &[u8] = b"relay/v1";

/// The relay key in an advert value: the first 32 bytes; a shorter value is not an advert.
fn relay_key_of(value: &Bytes) -> Option<[u8; 32]> {
    value.get(..32)?.try_into().ok()
}

/// Candidates for a dial to `target`, sorted by member: live admitted members other than `me` and `target`
/// that advertise a relay.
pub(crate) fn relay_candidates(
    members: &Members,
    adverts: &HashMap<PeerId, Bytes>,
    me: PeerId,
    target: PeerId,
) -> Vec<RelayCandidate> {
    let mut list: Vec<RelayCandidate> = adverts
        .iter()
        .filter(|(author, _)| **author != me && **author != target)
        .filter(|(author, _)| members.is_member(**author))
        .filter_map(|(author, value)| {
            let relay = relay_key_of(value)?;
            Some(RelayCandidate {
                member: *author,
                relay,
            })
        })
        .collect();
    list.sort_by_key(|c| c.member);
    list
}

/// The candidate source of a dial: reads the adverts of the given replica on every call.
pub(crate) fn relay_source(members: Members, replica: Arc<dyn Replica>, me: PeerId) -> RelaySource {
    Arc::new(move |target| -> BoxFuture<'static, Vec<RelayCandidate>> {
        let members = members.clone();
        let replica = replica.clone();
        Box::pin(async move {
            match read_values(&*replica, RELAY_ADVERT_KEY).await {
                Ok(adverts) => relay_candidates(&members, &adverts, me, target),
                Err(e) => {
                    tracing::debug!("relay: reading adverts failed: {e}");
                    Vec::new()
                }
            }
        })
    })
}

#[cfg(test)]
#[path = "relay_candidates_tests.rs"]
mod tests;
