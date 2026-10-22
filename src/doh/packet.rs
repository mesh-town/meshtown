//! Building DNS queries and parsing DNS answers (RFC 1035 packets through `simple-dns`); no networking.

use std::net::Ipv4Addr;

use anyhow::{bail, Context, Result};
use simple_dns::rdata::RData;
use simple_dns::{Name, Packet, PacketFlag, Question, CLASS, QCLASS, QTYPE, RCODE, TYPE};

/// A parsed answer: the A addresses and the minimum TTL of the answer section (`None` if there is no A).
pub(super) struct Records {
    pub addrs: Vec<Ipv4Addr>,
    pub ttl: Option<u32>,
}

/// Builds an A-record query with the "recursion desired" flag, id 0 (RFC 8484).
pub(super) fn build_query(domain: &str) -> Result<Vec<u8>> {
    let mut packet = Packet::new_query(0);
    packet.set_flags(PacketFlag::RECURSION_DESIRED);
    let name = Name::new(domain).with_context(|| format!("name \"{domain}\" couldn't be parsed"))?;
    packet.questions.push(Question::new(
        name,
        QTYPE::TYPE(TYPE::A),
        QCLASS::CLASS(CLASS::IN),
        false,
    ));
    packet.build_bytes_vec().context("building the DNS query")
}

/// Extracts the addresses and the minimum TTL of the answer section (CNAME and A together — a "name → addresses"
/// binding lives no longer than the shortest-lived record of the chain; `None` if there are no addresses). An empty
/// list is a complete "no addresses" answer: the name doesn't exist (`NameError`) or the name
/// has no A records. Such a domain gets the "unreachable" class, so this is not
/// treated as an error. It is an error only when the answer can't be parsed or the resolver itself refused
/// (`ServerFailure`, `Refused` and other codes).
pub(super) fn parse_answer(bytes: &[u8], domain: &str) -> Result<Records> {
    let packet = Packet::parse(bytes).context("couldn't parse the DNS answer")?;
    match packet.rcode() {
        RCODE::NoError => {}
        RCODE::NameError => {
            return Ok(Records {
                addrs: Vec::new(),
                ttl: None,
            })
        }
        other => bail!("DNS answered {other:?} for \"{domain}\""),
    }
    let mut addrs = Vec::new();
    for answer in &packet.answers {
        if let RData::A(a) = &answer.rdata {
            addrs.push(Ipv4Addr::from(a.address));
        }
    }
    let ttl = packet.answers.iter().map(|answer| answer.ttl).min();
    Ok(Records {
        ttl: ttl.filter(|_| !addrs.is_empty()),
        addrs,
    })
}

#[cfg(test)]
#[path = "packet_tests.rs"]
mod tests;
