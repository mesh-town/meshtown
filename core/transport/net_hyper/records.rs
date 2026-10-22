//! DHT record store over the townjump node: the same node that listens and dials.

use crate::transport::net::error::NetResult;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net_hyper::transport::dht_error_to_net_error;
use crate::transport::net_hyperdht::{Record, RecordKey, RecordStore};
use std::sync::Arc;
use townjump_dht::Dht;
use townjump_dht_peeroxide::PeeroxideDht;
use townjump_keys::key_pair_from_seed;

/// Records on the shared DHT node of the `hyper` transport.
pub(crate) struct HyperRecords {
    pub(crate) node: Arc<PeeroxideDht>,
}

impl RecordStore for HyperRecords {
    fn get(&self, key: &RecordKey) -> BoxFuture<'_, NetResult<Option<Record>>> {
        let pk = key_pair_from_seed(&key.seed()).public_key;
        Box::pin(async move {
            let got = self
                .node
                .mutable_get(pk, None)
                .await
                .map_err(dht_error_to_net_error)?;
            Ok(got.map(|r| Record {
                seq: r.seq,
                value: r.value,
            }))
        })
    }

    fn put(&self, key: &RecordKey, value: Vec<u8>, seq: u64) -> BoxFuture<'_, NetResult<()>> {
        let kp = key_pair_from_seed(&key.seed());
        Box::pin(async move {
            self.node
                .mutable_put(&kp, &value, Some(seq))
                .await
                .map_err(dht_error_to_net_error)?;
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "records_tests.rs"]
mod tests;
