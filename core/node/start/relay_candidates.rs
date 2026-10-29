//! Relay candidates for a dial: members of this network that advertise a relay node in the shared replica.
//!
//! An advert is the record `relay/v1` of the member in the network's replica: the first 32 bytes of the value are
//! the key of the member's relay node, the next 6 bytes are its public IPv4 address and port, a longer tail is
//! ignored. Only live, admitted members count, so a relay of
//! another network or of a departed node is never offered.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;

use bytes::Bytes;

use crate::members::membership::Members;
use crate::node::node_state::read_values;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::{PeerId, Replica};
use crate::transport::net_hyper::{RelayAdvert, RelayCandidate, RelaySource};

/// Key of the relay advert in the network's shared replica.
pub(crate) const RELAY_ADVERT_KEY: &[u8] = b"relay/v1";

/// Length of an advert value written by this version: key, IPv4 address, UDP port.
const ADVERT_LEN: usize = 38;

/// The advert value: the relay key, then its IPv4 address and big-endian UDP port.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn encode_relay_value(advert: &RelayAdvert) -> Bytes {
    let mut value = Vec::with_capacity(ADVERT_LEN);
    value.extend_from_slice(&advert.key);
    value.extend_from_slice(&advert.address.ip().octets());
    value.extend_from_slice(&advert.address.port().to_be_bytes());
    Bytes::from(value)
}

/// The relay key and address in an advert value. A value shorter than 32 bytes is not an advert; a value
/// without a usable address (short tail, `0.0.0.0`, port 0) gives the key alone; a tail past 38 bytes is ignored.
pub(crate) fn decode_relay_value(value: &[u8]) -> Option<([u8; 32], Option<SocketAddrV4>)> {
    let key: [u8; 32] = value.get(..32)?.try_into().ok()?;
    Some((key, address_of(value)))
}

fn address_of(value: &[u8]) -> Option<SocketAddrV4> {
    let ip: [u8; 4] = value.get(32..36)?.try_into().ok()?;
    let port = u16::from_be_bytes(value.get(36..ADVERT_LEN)?.try_into().ok()?);
    let ip = Ipv4Addr::from(ip);
    (!ip.is_unspecified() && port != 0).then(|| SocketAddrV4::new(ip, port))
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
            let (relay, address) = decode_relay_value(value)?;
            Some(RelayCandidate {
                member: *author,
                relay,
                address,
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

#[cfg(test)]
#[path = "relay_value_tests.rs"]
mod relay_value_tests;
