//! Member address strings for display on the page, and parsing them back into a node address.

use std::net::SocketAddr;

use crate::transport::net::{AddrHint, PeerAddr, PeerId, RelayRef};

/// Member address strings for display on the page: id — `Display` (64 hex), first
/// `<id>@<ip:port>` in ascending order without repeats, then `<id>@<url>` for relay hints with a URL;
/// other hints are skipped. Does not trust the input order: sorts itself.
pub fn peer_lines(addr: &PeerAddr) -> Vec<String> {
    let id = addr.id.to_string();
    let mut ips: Vec<SocketAddr> = addr.hints.iter().filter_map(ip_of).collect();
    ips.sort();
    ips.dedup();
    let urls = addr.hints.iter().filter_map(url_of);
    let ips = ips.into_iter().map(|ip| format!("{id}@{ip}"));
    ips.chain(urls.map(|url| format!("{id}@{url}"))).collect()
}

fn ip_of(hint: &AddrHint) -> Option<SocketAddr> {
    match hint {
        AddrHint::Ip(ip) => Some(*ip),
        _ => None,
    }
}

fn url_of(hint: &AddrHint) -> Option<&str> {
    match hint {
        AddrHint::Relay(RelayRef::Url(url)) => Some(url),
        _ => None,
    }
}

/// Parses a string typed on the page back into a node address. Errors are in plain English,
/// without low-level parsing details.
pub(crate) fn parse_peer_line(line: &str) -> Result<PeerAddr, String> {
    let line = line.trim();
    let (id_part, addr_part) = line
        .split_once('@')
        .ok_or_else(|| "no @ separator".to_string())?;
    if id_part.is_empty() {
        return Err("no identifier before @".to_string());
    }
    let id: PeerId = id_part
        .parse()
        .map_err(|_| "couldn't parse the identifier: a full node id is needed".to_string())?;
    let mut addr = PeerAddr::new(id);
    if let Ok(sock) = addr_part.parse::<SocketAddr>() {
        addr.hints.push(AddrHint::Ip(sock));
        return Ok(addr);
    }
    if addr_part.starts_with("http://") || addr_part.starts_with("https://") {
        let url = url::Url::parse(addr_part).map_err(|_| {
            "couldn't parse the address: an ip:port or a relay url is needed".to_string()
        })?;
        addr.hints
            .push(AddrHint::Relay(RelayRef::Url(url.to_string())));
        return Ok(addr);
    }
    Err("couldn't parse the address: an ip:port or a relay url is needed".to_string())
}

#[cfg(test)]
#[path = "peer_addr_tests.rs"]
mod tests;
