//! Building DNS queries and parsing DNS answers (RFC 1035 packets through `simple-dns`); no networking.

use std::net::{Ipv4Addr, Ipv6Addr};

use anyhow::{bail, Context, Result};
use simple_dns::rdata::RData;
use simple_dns::{Name, Packet, PacketFlag, Question, CLASS, QCLASS, QTYPE, RCODE, TYPE};

/// A parsed answer: the A and AAAA addresses, the minimum TTL of the answer section (`None` if there are no
/// addresses) and whether the name does not exist at all (`nx`: `NameError`, as opposed to an empty answer).
pub(super) struct Records {
    pub v4: Vec<Ipv4Addr>,
    pub v6: Vec<Ipv6Addr>,
    pub ttl: Option<u32>,
    pub nx: bool,
}

/// Builds a query of the given record type with the "recursion desired" flag, id 0 (RFC 8484).
pub(super) fn build_query(domain: &str, qtype: TYPE) -> Result<Vec<u8>> {
    let mut packet = Packet::new_query(0);
    packet.set_flags(PacketFlag::RECURSION_DESIRED);
    let name =
        Name::new(domain).with_context(|| format!("name \"{domain}\" couldn't be parsed"))?;
    packet.questions.push(Question::new(
        name,
        QTYPE::TYPE(qtype),
        QCLASS::CLASS(CLASS::IN),
        false,
    ));
    packet.build_bytes_vec().context("building the DNS query")
}

/// Extracts the addresses and the minimum TTL of the answer section (CNAME, A and AAAA together — a "name → addresses"
/// binding lives no longer than the shortest-lived record of the chain; `None` if there are no addresses). An empty
/// list is a complete "no addresses" answer: the name doesn't exist (`NameError`) or the name
/// has no records of the asked type. Such a domain gets the "unreachable" class, so this is not
/// treated as an error. It is an error only when the answer can't be parsed or the resolver itself refused
/// (`ServerFailure`, `Refused` and other codes).
pub(super) fn parse_answer(bytes: &[u8], domain: &str) -> Result<Records> {
    let packet = Packet::parse(bytes).context("couldn't parse the DNS answer")?;
    match packet.rcode() {
        RCODE::NoError => {}
        RCODE::NameError => {
            return Ok(Records {
                v4: Vec::new(),
                v6: Vec::new(),
                ttl: None,
                nx: true,
            })
        }
        other => bail!("DNS answered {other:?} for \"{domain}\""),
    }
    let mut v4 = Vec::new();
    let mut v6 = Vec::new();
    for answer in &packet.answers {
        match &answer.rdata {
            RData::A(a) => v4.push(Ipv4Addr::from(a.address)),
            RData::AAAA(a) => v6.push(Ipv6Addr::from(a.address)),
            _ => {}
        }
    }
    let ttl = packet.answers.iter().map(|answer| answer.ttl).min();
    let has_addrs = !v4.is_empty() || !v6.is_empty();
    Ok(Records {
        ttl: ttl.filter(|_| has_addrs),
        v4,
        v6,
        nx: false,
    })
}

#[cfg(test)]
#[path = "packet_tests.rs"]
mod tests;
