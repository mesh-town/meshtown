//! Device list and address lookup.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::config::config_file::NetSlotId;
use crate::members::host_label;
use crate::transport::net::PeerId;

use super::host::MeshHost;

/// Shortest and longest hex tail of a disambiguated address.
const MIN_TAIL: usize = 4;
const MAX_TAIL: usize = 12;
/// Number of "did you mean" labels and the shared start that makes a label close.
const SUGGESTIONS: usize = 3;
const SHARED_START: usize = 4;

/// One member name as one network knows it.
pub(crate) struct NameRow {
    pub net: NetSlotId,
    pub peer: PeerId,
    pub name: String,
    pub online: bool,
    pub allowed: bool,
}

/// One device, merged over all networks that list it.
#[derive(Clone, Debug)]
pub(crate) struct Device {
    pub net: NetSlotId,
    pub peer: PeerId,
    pub name: String,
    pub label: String,
    pub online: bool,
}

/// Keeps allowed rows and merges rows of one peer: the online one wins, else the lower slot.
pub(crate) fn devices(rows: Vec<NameRow>) -> Vec<Device> {
    let mut best: BTreeMap<PeerId, NameRow> = BTreeMap::new();
    for row in rows.into_iter().filter(|row| row.allowed) {
        let keep_old = best.get(&row.peer).is_some_and(|old| !preferred(&row, old));
        if !keep_old {
            best.insert(row.peer, row);
        }
    }
    let mut out: Vec<Device> = best.into_values().map(device_of).collect();
    out.sort_by(|a, b| (&a.label, a.peer).cmp(&(&b.label, b.peer)));
    out
}

fn preferred(new: &NameRow, old: &NameRow) -> bool {
    if new.online != old.online {
        return new.online;
    }
    new.net < old.net
}

fn device_of(row: NameRow) -> Device {
    let label = host_label(&row.name);
    Device { net: row.net, peer: row.peer, name: row.name, label, online: row.online }
}

/// Address of every device: its label when unique, else `<label>-<start of the id in hex>`.
pub(crate) fn addresses(devices: &[Device]) -> HashMap<PeerId, String> {
    let labels: BTreeSet<&str> = devices.iter().map(|d| d.label.as_str()).collect();
    let mut groups: BTreeMap<&str, Vec<PeerId>> = BTreeMap::new();
    for device in devices {
        groups.entry(device.label.as_str()).or_default().push(device.peer);
    }
    let mut out = HashMap::new();
    for (label, peers) in groups {
        for peer in &peers {
            out.insert(*peer, address_of(label, *peer, &peers, &labels));
        }
    }
    out
}

fn address_of(label: &str, peer: PeerId, group: &[PeerId], labels: &BTreeSet<&str>) -> String {
    if group.len() == 1 {
        return label.to_string();
    }
    let hex = peer.to_string();
    let mut tail = shortest_tail(&hex, peer, group);
    while tail < MAX_TAIL && labels.contains(format!("{label}-{}", &hex[..tail]).as_str()) {
        tail += 1;
    }
    format!("{label}-{}", &hex[..tail])
}

/// Shortest hex start (at least `MIN_TAIL`, at most `MAX_TAIL`) that no other peer of the group shares.
fn shortest_tail(hex: &str, peer: PeerId, group: &[PeerId]) -> usize {
    let others: Vec<String> = group.iter().filter(|p| **p != peer).map(|p| p.to_string()).collect();
    let clash = |n: usize| others.iter().any(|o| o[..n] == hex[..n]);
    (MIN_TAIL..=MAX_TAIL).find(|n| !clash(*n)).unwrap_or(MAX_TAIL)
}

/// Result of resolving a `.meshtown` host.
#[derive(Debug)]
pub(crate) enum Lookup {
    Found(Device),
    Ambiguous(Vec<Device>),
    NotFound,
}

/// Resolves a host: an issued address first, then the label, then `<label>-<id start>`.
pub(crate) fn lookup(devices: &[Device], host: &MeshHost) -> Lookup {
    if host.label.is_empty() || host.label.contains('.') {
        return Lookup::NotFound;
    }
    let by_address = by_address(devices, &host.label);
    if by_address.len() == 1 {
        return Lookup::Found(by_address[0].clone());
    }
    let mut found: Vec<Device> = devices.iter().filter(|d| matches_name(d, &host.label)).cloned().collect();
    found.sort_by(|a, b| (&a.label, a.peer).cmp(&(&b.label, b.peer)));
    match found.len() {
        0 => Lookup::NotFound,
        1 => Lookup::Found(found.remove(0)),
        _ => Lookup::Ambiguous(found),
    }
}

fn by_address<'a>(devices: &'a [Device], host: &str) -> Vec<&'a Device> {
    let issued = addresses(devices);
    devices.iter().filter(|d| issued.get(&d.peer).is_some_and(|a| a == host)).collect()
}

fn matches_name(device: &Device, host: &str) -> bool {
    if device.label == host {
        return true;
    }
    let Some(tail) = host.strip_prefix(device.label.as_str()).and_then(|r| r.strip_prefix('-')) else {
        return false;
    };
    tail.len() >= MIN_TAIL && device.peer.to_string().starts_with(tail)
}

/// Closest labels (edit distance at most 2, or a shared start of 4+ characters), best first, up to three.
pub(crate) fn suggest_labels(devices: &[Device], slug: &str) -> Vec<String> {
    if slug.is_empty() || slug.contains('.') || slug.starts_with("xn--") {
        return Vec::new();
    }
    let labels: BTreeSet<&str> = devices.iter().map(|d| d.label.as_str()).collect();
    let mut close: Vec<(usize, &str)> = labels.into_iter().filter_map(|l| closeness(l, slug)).collect();
    close.sort();
    close.into_iter().take(SUGGESTIONS).map(|(_, label)| label.to_string()).collect()
}

fn closeness<'a>(label: &'a str, slug: &str) -> Option<(usize, &'a str)> {
    let distance = levenshtein(label, slug);
    let shared = label.chars().zip(slug.chars()).take_while(|(a, b)| a == b).count();
    (distance <= 2 || shared >= SHARED_START).then_some((distance, label))
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (diagonal + usize::from(ca != *cb)).min(above + 1).min(row[j] + 1);
            diagonal = above;
        }
    }
    row[b.len()]
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
