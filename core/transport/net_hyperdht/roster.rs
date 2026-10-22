//! A key roster round: read the record from the store, merge our own key, seal and write.

use super::keys::RosterKeys;
use super::store::RecordStore;
use crate::net::{NetError, NetResult, PeerId};
use crate::net_mainline::record::{merge_keys, open_as, seal, Roster, RECORD_VERSION_KEYS};
use crate::network_key::fill_random;

/// Next `seq`: greater than the previous one and not less than the current milliseconds.
pub(super) fn next_seq(prev: Option<u64>, now_ms: u64) -> u64 {
    prev.map_or(now_ms, |p| p.saturating_add(1).max(now_ms))
}

/// Reads the roster: no record, a foreign version or it did not open — `Ok(None)`; a store error — `Err`.
pub(super) async fn read(
    store: &dyn RecordStore,
    keys: &RosterKeys,
) -> NetResult<Option<(Roster, u64)>> {
    let Some(rec) = store.get(&keys.record_key).await? else {
        return Ok(None);
    };
    match open_as(&keys.seal_key, &rec.value, RECORD_VERSION_KEYS) {
        Ok(roster) => Ok(Some((roster, rec.seq))),
        Err(e) => {
            tracing::debug!("hyperdht: record did not open, treating as empty: {e}");
            Ok(None)
        }
    }
}

/// One publish round: read, merge our own key, seal, write. Returns the written roster.
pub(super) async fn publish_round(
    store: &dyn RecordStore,
    keys: &RosterKeys,
    me: PeerId,
    now_unix: u64,
    now_ms: u64,
) -> NetResult<Roster> {
    let prev = read(store, keys).await?;
    let seq = next_seq(prev.as_ref().map(|p| p.1), now_ms);
    let merged = merge_keys(prev.map(|p| p.0), *me.as_bytes(), now_unix);
    let mut nonce = [0u8; 12];
    fill_random(&mut nonce).map_err(NetError::Other)?;
    let sealed = seal(&keys.seal_key, &merged, nonce).map_err(NetError::Other)?;
    store.put(&keys.record_key, sealed, seq).await?;
    Ok(merged)
}

#[cfg(test)]
#[path = "roster_tests.rs"]
mod tests;
