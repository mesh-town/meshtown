//! Roster keys: the record key (signature) and the value sealing key.

use super::store::RecordKey;
use crate::network_key::SubKeys;

const CTX_RECORD: &str = "meshtown hyperdht roster signing v1";

#[derive(Clone, Debug)]
pub struct RosterKeys {
    pub record_key: RecordKey,
    pub seal_key: [u8; 32],
}

impl RosterKeys {
    pub fn from_subkeys(sub: &SubKeys) -> Self {
        let seed = blake3::derive_key(CTX_RECORD, &sub.dht_signing_seed);
        Self {
            record_key: RecordKey::from_seed(seed),
            seal_key: sub.roster_key,
        }
    }
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
