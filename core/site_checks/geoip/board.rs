//! The "country of this node" mark in the network's shared document — a separate key, its own author.
//!
//! What it does: writes and reads the country code through the shared helpers `node_state::{read_values,
//! own_value}`; a tombstone, a missing entry and an invalid value are indistinguishable to the reader —
//! all of them are "country unknown".
//! What it doesn't do: doesn't decide when to announce and at what interval to recheck — that is
//! `geoip::task`.
//! Links: `node_ctl::snapshot::collect` calls `read_countries`, the display uses `Country::as_str`.

use std::collections::HashMap;

use crate::transport::net::{NetError, PeerId, Replica};
use anyhow::Result;
use bytes::Bytes;

use crate::node::node_state::{own_value, read_values};

use super::country::Country;

/// The country key of a node in the network's shared document.
pub const COUNTRY_KEY: &[u8] = b"node/country/";

/// Announces the country of this node as its own entry.
pub async fn announce_country(r: &dyn Replica, country: Country) -> Result<()> {
    let value = Bytes::copy_from_slice(country.as_str().as_bytes());
    r.set_own(COUNTRY_KEY, value)
        .await
        .map_err(NetError::into_anyhow)
}

/// The country of each author whose value is valid; a tombstone or an invalid value — the author does not
/// appear in the map.
pub async fn read_countries(r: &dyn Replica) -> Result<HashMap<PeerId, Country>> {
    let values = read_values(r, COUNTRY_KEY).await?;
    Ok(values
        .into_iter()
        .filter_map(|(id, bytes)| Country::from_bytes(&bytes).map(|c| (id, c)))
        .collect())
}

/// Its own country and the write time in microseconds; a tombstone, a missing entry or an invalid
/// value is `None`.
pub async fn own_country_record(replica: &dyn Replica) -> Result<Option<(Country, u64)>> {
    let Some((bytes, at)) = own_value(replica, COUNTRY_KEY).await? else {
        return Ok(None);
    };
    Ok(Country::from_bytes(&bytes).map(|c| (c, at)))
}
