//! Tests of building queries and parsing answers (`packet.rs`).

use super::*;
use data_encoding::BASE64URL_NOPAD;
use simple_dns::rdata::{A, AAAA, CNAME};
use simple_dns::ResourceRecord;
use std::net::Ipv6Addr;

fn reply(domain: &str, addrs: &[Ipv4Addr], rcode: RCODE) -> Vec<u8> {
    let mut packet = Packet::new_query(0);
    packet.set_flags(PacketFlag::RESPONSE);
    *packet.rcode_mut() = rcode;
    for ip in addrs {
        packet.answers.push(ResourceRecord::new(
            Name::new(domain).unwrap(),
            CLASS::IN,
            60,
            RData::A(A {
                address: u32::from(*ip),
            }),
        ));
    }
    packet.build_bytes_vec().unwrap()
}

/// A `NoError` answer with ready records (a CNAME chain and A records with different TTLs).
fn reply_records(records: Vec<ResourceRecord<'static>>) -> Vec<u8> {
    let mut packet = Packet::new_query(0);
    packet.set_flags(PacketFlag::RESPONSE);
    packet.answers.extend(records);
    packet.build_bytes_vec().unwrap()
}

fn cname_record(from: &'static str, to: &'static str, ttl: u32) -> ResourceRecord<'static> {
    let target = CNAME(Name::new(to).unwrap());
    ResourceRecord::new(
        Name::new(from).unwrap(),
        CLASS::IN,
        ttl,
        RData::CNAME(target),
    )
}

fn a_record(name: &'static str, ip: Ipv4Addr, ttl: u32) -> ResourceRecord<'static> {
    let a = A {
        address: u32::from(ip),
    };
    ResourceRecord::new(Name::new(name).unwrap(), CLASS::IN, ttl, RData::A(a))
}

fn aaaa_record(name: &'static str, ip: Ipv6Addr, ttl: u32) -> ResourceRecord<'static> {
    let aaaa = AAAA {
        address: u128::from(ip),
    };
    ResourceRecord::new(Name::new(name).unwrap(), CLASS::IN, ttl, RData::AAAA(aaaa))
}

const V6: Ipv6Addr = Ipv6Addr::new(0x2606, 0x4700, 0, 0, 0, 0, 0, 0x1111);

#[test]
fn query_asks_a_record_with_recursion() {
    let bytes = build_query("example.com", TYPE::A).unwrap();
    let packet = Packet::parse(&bytes).unwrap();
    assert!(packet.has_flags(PacketFlag::RECURSION_DESIRED));
    assert_eq!(packet.questions.len(), 1);
    assert_eq!(packet.questions[0].qtype, QTYPE::TYPE(TYPE::A));
    assert_eq!(packet.questions[0].qclass, QCLASS::CLASS(CLASS::IN));
    assert_eq!(
        BASE64URL_NOPAD
            .decode(BASE64URL_NOPAD.encode(&bytes).as_bytes())
            .unwrap(),
        bytes
    );
}

#[test]
fn answer_gives_addresses() {
    let ip = Ipv4Addr::new(93, 184, 216, 34);
    let bytes = reply("example.com", &[ip], RCODE::NoError);
    let parsed = parse_answer(&bytes, "example.com").unwrap();
    assert_eq!(parsed.v4, vec![ip]);
    assert_eq!(parsed.ttl, Some(60));
}

#[test]
fn missing_name_gives_no_addresses_and_failure_is_an_error() {
    let none = reply("example.com", &[], RCODE::NameError);
    let parsed = parse_answer(&none, "example.com").unwrap();
    assert_eq!(parsed.v4, Vec::<Ipv4Addr>::new());
    assert_eq!(parsed.ttl, None);
    let empty = reply("example.com", &[], RCODE::NoError);
    let parsed = parse_answer(&empty, "example.com").unwrap();
    assert_eq!(parsed.v4, Vec::<Ipv4Addr>::new());
    assert_eq!(parsed.ttl, None);
    let broken = reply("example.com", &[], RCODE::ServerFailure);
    assert!(parse_answer(&broken, "example.com").is_err());
}

#[test]
fn ttl_is_minimum_over_whole_chain() {
    let first = Ipv4Addr::new(1, 2, 3, 4);
    let second = Ipv4Addr::new(5, 6, 7, 8);
    let bytes = reply_records(vec![
        cname_record("www.example.com", "a.example.net", 300),
        cname_record("a.example.net", "cdn.example.net", 10),
        a_record("cdn.example.net", first, 60),
        a_record("cdn.example.net", second, 30),
    ]);
    let parsed = parse_answer(&bytes, "www.example.com").unwrap();
    assert_eq!(parsed.v4, vec![first, second]);
    assert_eq!(parsed.ttl, Some(10));
}

#[test]
fn cname_without_a_gives_no_ttl() {
    let bytes = reply_records(vec![cname_record("www.example.com", "a.example.net", 60)]);
    let parsed = parse_answer(&bytes, "www.example.com").unwrap();
    assert!(parsed.v4.is_empty());
    assert_eq!(parsed.ttl, None);
}

#[test]
fn query_asks_aaaa_record_with_recursion() {
    let bytes = build_query("example.com", TYPE::AAAA).unwrap();
    let packet = Packet::parse(&bytes).unwrap();
    assert!(packet.has_flags(PacketFlag::RECURSION_DESIRED));
    assert_eq!(packet.questions.len(), 1);
    assert_eq!(packet.questions[0].qtype, QTYPE::TYPE(TYPE::AAAA));
}

#[test]
fn answer_gives_ipv6_addresses() {
    let bytes = reply_records(vec![aaaa_record("example.com", V6, 300)]);
    let parsed = parse_answer(&bytes, "example.com").unwrap();
    assert_eq!(parsed.v6, vec![V6]);
    assert!(parsed.v4.is_empty());
    assert_eq!(parsed.ttl, Some(300));
    assert!(!parsed.nx);
}

#[test]
fn aaaa_ttl_is_minimum_over_cname_chain() {
    let bytes = reply_records(vec![
        cname_record("www.example.com", "cdn.example.net", 50),
        aaaa_record("cdn.example.net", V6, 300),
    ]);
    let parsed = parse_answer(&bytes, "www.example.com").unwrap();
    assert_eq!(parsed.v6, vec![V6]);
    assert_eq!(parsed.ttl, Some(50));
}

#[test]
fn empty_aaaa_answer_gives_no_ttl() {
    let bytes = reply_records(Vec::new());
    let parsed = parse_answer(&bytes, "example.com").unwrap();
    assert!(parsed.v4.is_empty() && parsed.v6.is_empty());
    assert_eq!(parsed.ttl, None);
}

#[test]
fn nxdomain_sets_nx_and_skips_aaaa() {
    let missing = reply("example.com", &[], RCODE::NameError);
    assert!(parse_answer(&missing, "example.com").unwrap().nx);
    let no_data = reply("example.com", &[], RCODE::NoError);
    assert!(!parse_answer(&no_data, "example.com").unwrap().nx);
}
