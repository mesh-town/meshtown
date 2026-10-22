//! Format and encryption of the DHT record — the list of network members (no networking).
//!
//! What it does: keeps the member list (`Roster`), merges fresh information about itself
//! with the old record, encrypts and decrypts the record for storage in the DHT,
//! converts a node address (`PeerAddr`) into a record and back.
//! What it does not do: it neither reads nor writes the DHT — `dht` does that.
//! Links: `dht` calls `merge`, `seal`, `open`, `entry_from_addr`, `entry_to_addr`;
//! `addr_order` sorts the addresses before slicing.

use std::net::SocketAddr;

use crate::transport::net::PeerId;
use crate::transport::net::{AddrHint, PeerAddr, RelayRef};
use anyhow::{anyhow, bail, Context, Result};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305};
use serde::{Deserialize, Serialize};

use crate::node::output::NET_NAME;
use crate::transport::addr_order::{order_direct, truncate_ranked, PrimaryIps};

/// Record format version.
const RECORD_VERSION: u8 = 1;
/// Version of the keys-only record (without relay and addresses) — for the `hyperdht` source.
pub const RECORD_VERSION_KEYS: u8 = 2;
/// Maximum entries in a key roster (654 bytes sealed at a limit of 990).
pub const MAX_KEY_ENTRIES: usize = 16;
/// Limit on the size of the encrypted record (in BEP44 the value is at most 1000 bytes).
const MAX_SEALED_BYTES: usize = 990;
/// Maximum member entries in one DHT record.
const MAX_ADDR_ROSTER_ENTRIES: usize = 8;
/// Maximum addresses in one member entry.
const MAX_ADDRS_PER_ENTRY: usize = 4;
/// Lifetime of a member entry without an update.
pub const ENTRY_TTL_SECS: u64 = 900;
/// Additional authenticated data for record encryption.
const ROSTER_AAD: &[u8] = b"meshtown roster v1";

/// List of network members — what is stored (encrypted) in the DHT.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Roster {
    pub version: u8,
    pub entries: Vec<RosterEntry>,
}

/// One network member's entry.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RosterEntry {
    pub id: [u8; 32],
    pub relay: Option<String>,
    pub addrs: Vec<SocketAddr>,
    pub seen_unix: u64,
}

/// Merges our own record with the old record: drops stale entries and our own from the old one,
/// puts ourselves first, limits the number of addresses and the number of entries.
pub fn merge(prev: Option<Roster>, me: RosterEntry, now_unix: u64) -> Roster {
    merge_capped(prev, me, now_unix, MAX_ADDR_ROSTER_ENTRIES)
}

fn merge_capped(prev: Option<Roster>, me: RosterEntry, now_unix: u64, cap: usize) -> Roster {
    let mut entries: Vec<RosterEntry> = prev
        .map(|r| r.entries)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.id != me.id && e.seen_unix.saturating_add(ENTRY_TTL_SECS) >= now_unix)
        .collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.seen_unix));
    let mut seen_ids = std::collections::HashSet::new();
    entries.retain(|e| seen_ids.insert(e.id));
    entries.insert(0, me);
    for e in entries.iter_mut() {
        // there is no route information here: the entries were already sorted and sliced at creation
        truncate_ranked(&mut e.addrs, MAX_ADDRS_PER_ENTRY, &PrimaryIps::NONE);
    }
    entries.truncate(cap);
    Roster {
        version: RECORD_VERSION,
        entries,
    }
}

/// Like `merge`, but for a key roster: relay and addresses are erased in all entries, our own entry
/// (only id, `seen_unix = now_unix`) goes first, version 2, ceiling `MAX_KEY_ENTRIES`.
pub fn merge_keys(prev: Option<Roster>, me: [u8; 32], now_unix: u64) -> Roster {
    let mine = RosterEntry {
        id: me,
        relay: None,
        addrs: Vec::new(),
        seen_unix: now_unix,
    };
    let mut roster = merge_capped(prev, mine, now_unix, MAX_KEY_ENTRIES);
    for e in roster.entries.iter_mut() {
        e.relay = None;
        e.addrs.clear();
    }
    roster.version = RECORD_VERSION_KEYS;
    roster
}

/// Encrypts the record; if the result does not fit the limit, drops the oldest
/// entry (except our own — it is first) and tries again.
pub fn seal(key: &[u8; 32], roster: &Roster, nonce: [u8; 12]) -> Result<Vec<u8>> {
    let mut roster = roster.clone();
    loop {
        let plain = postcard::to_allocvec(&roster)?;
        let unbound = UnboundKey::new(&CHACHA20_POLY1305, key)
            .map_err(|_| anyhow!("could not create the record encryption key"))?;
        let sealing_key = LessSafeKey::new(unbound);
        let mut buf = plain;
        sealing_key
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(ROSTER_AAD),
                &mut buf,
            )
            .map_err(|_| anyhow!("could not encrypt the record"))?;
        let mut out = Vec::with_capacity(12 + buf.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&buf);
        if out.len() <= MAX_SEALED_BYTES {
            return Ok(out);
        }
        if roster.entries.len() > 1 {
            roster.entries.pop();
        } else {
            bail!("record does not fit into 990 bytes");
        }
    }
}

/// Decrypts a v1 record, checks the version.
pub fn open(key: &[u8; 32], sealed: &[u8]) -> Result<Roster> {
    open_as(key, sealed, RECORD_VERSION)
}

/// Decrypts the record and requires the given format version.
pub fn open_as(key: &[u8; 32], sealed: &[u8], version: u8) -> Result<Roster> {
    if sealed.len() < 28 {
        bail!("record is too short: {} bytes", sealed.len());
    }
    let (nonce_bytes, ciphertext) = sealed.split_at(12);
    let nonce: [u8; 12] = nonce_bytes.try_into().unwrap();
    let mut buf = ciphertext.to_vec();
    let unbound = UnboundKey::new(&CHACHA20_POLY1305, key)
        .map_err(|_| anyhow!("could not create the record encryption key"))?;
    let opening_key = LessSafeKey::new(unbound);
    let plain = opening_key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(ROSTER_AAD),
            &mut buf,
        )
        .map_err(|_| {
            anyhow!("record did not decrypt — key of a different network {NET_NAME} or corruption")
        })?;
    let roster: Roster = postcard::from_bytes(plain)?;
    if roster.version != version {
        bail!("unknown record version: {}", roster.version);
    }
    Ok(roster)
}

/// Builds a member entry from a `PeerAddr`: the first `Relay(Url)` goes into `relay`, all `Ip` into `addrs`
/// (sorted and sliced); `Relay(Peer)` and `Opaque` do not go into the entry.
pub fn entry_from_addr(addr: &PeerAddr, now_unix: u64, ips: &PrimaryIps) -> RosterEntry {
    let relay = addr.hints.iter().find_map(relay_url);
    let mut addrs: Vec<SocketAddr> = addr.hints.iter().filter_map(ip_of).collect();
    order_direct(&mut addrs, ips);
    truncate_ranked(&mut addrs, MAX_ADDRS_PER_ENTRY, ips);
    RosterEntry {
        id: *addr.id.as_bytes(),
        relay,
        addrs,
        seen_unix: now_unix,
    }
}

fn relay_url(hint: &AddrHint) -> Option<String> {
    match hint {
        AddrHint::Relay(RelayRef::Url(u)) => Some(u.clone()),
        _ => None,
    }
}

fn ip_of(hint: &AddrHint) -> Option<SocketAddr> {
    match hint {
        AddrHint::Ip(a) => Some(*a),
        _ => None,
    }
}

/// Builds a `PeerAddr` from a member entry: `Relay` first, then `Ip` ascending without repeats
/// (the order of the former path through the node address).
pub fn entry_to_addr(e: &RosterEntry) -> Result<PeerAddr> {
    let mut addr = PeerAddr::new(PeerId::from_bytes(&e.id)?);
    if let Some(relay) = &e.relay {
        let url = url::Url::parse(relay).context("Failed to parse relay URL")?;
        addr.hints
            .push(AddrHint::Relay(RelayRef::Url(url.to_string())));
    }
    let mut ips: Vec<SocketAddr> = e.addrs.clone();
    ips.sort();
    ips.dedup();
    addr.hints.extend(ips.into_iter().map(AddrHint::Ip));
    Ok(addr)
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "record_golden_tests.rs"]
mod golden_tests;

#[cfg(test)]
#[path = "record_v2_tests.rs"]
mod v2_tests;
