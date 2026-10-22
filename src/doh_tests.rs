//! Tests of the DNS-over-HTTPS resolver (`doh.rs`).

use super::*;
use simple_dns::rdata::{A, CNAME};
use simple_dns::ResourceRecord;

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

/// A resolver pointed at a closed port: any trip to DoH fails at once, without a network.
fn dead_resolver() -> Resolver {
    Resolver::new(
        SocketAddr::from(([127, 0, 0, 1], 1)),
        HttpLimits::default(),
        "meshtown-test".to_string(),
    )
    .expect("resolver with a placeholder address")
}

const CACHED: Ipv4Addr = Ipv4Addr::new(93, 184, 216, 34);

/// Puts `example.com` → `CACHED` into the cache for 60 s.
fn resolver_with_cached_example() -> Resolver {
    let resolver = dead_resolver();
    let ttl = Duration::from_secs(60);
    resolver
        .cache()
        .insert(cache_key("example.com"), vec![CACHED], ttl, Instant::now());
    resolver
}

#[test]
fn query_asks_a_record_with_recursion() {
    let bytes = build_query("example.com").unwrap();
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
    assert_eq!(parsed.addrs, vec![ip]);
    assert_eq!(parsed.ttl, Some(60));
}

#[test]
fn missing_name_gives_no_addresses_and_failure_is_an_error() {
    let none = reply("example.com", &[], RCODE::NameError);
    let parsed = parse_answer(&none, "example.com").unwrap();
    assert_eq!(parsed.addrs, Vec::<Ipv4Addr>::new());
    assert_eq!(parsed.ttl, None);
    let empty = reply("example.com", &[], RCODE::NoError);
    let parsed = parse_answer(&empty, "example.com").unwrap();
    assert_eq!(parsed.addrs, Vec::<Ipv4Addr>::new());
    assert_eq!(parsed.ttl, None);
    let broken = reply("example.com", &[], RCODE::ServerFailure);
    assert!(parse_answer(&broken, "example.com").is_err());
}

#[test]
fn fake_ip_and_loopback_are_unusable() {
    assert!(is_fake_ip(Ipv4Addr::new(198, 18, 0, 1)));
    assert!(is_fake_ip(Ipv4Addr::new(198, 19, 255, 254)));
    assert!(!is_fake_ip(Ipv4Addr::new(198, 20, 0, 1)));
    assert!(is_unusable(Ipv4Addr::new(127, 0, 0, 1)));
    assert!(is_unusable(Ipv4Addr::new(0, 0, 0, 0)));
    assert!(!is_unusable(Ipv4Addr::new(93, 184, 216, 34)));
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
    assert_eq!(parsed.addrs, vec![first, second]);
    assert_eq!(parsed.ttl, Some(10));
}

#[test]
fn cname_without_a_gives_no_ttl() {
    let bytes = reply_records(vec![cname_record("www.example.com", "a.example.net", 60)]);
    let parsed = parse_answer(&bytes, "www.example.com").unwrap();
    assert!(parsed.addrs.is_empty());
    assert_eq!(parsed.ttl, None);
}

#[tokio::test]
async fn cached_answer_is_served_without_network() {
    let resolver = resolver_with_cached_example();
    let addrs = resolver.resolve_cached("Example.COM.").await.unwrap();
    assert_eq!(addrs, vec![CACHED]);
}

#[tokio::test]
async fn forget_sends_next_call_to_server() {
    let resolver = resolver_with_cached_example();
    resolver.forget("Example.COM.");
    assert!(resolver.resolve_cached("example.com").await.is_err());
}

#[tokio::test]
async fn plain_resolve_bypasses_cache() {
    let resolver = resolver_with_cached_example();
    assert!(resolver.resolve("example.com").await.is_err());
}

#[tokio::test]
async fn failed_lookup_leaves_cache_empty() {
    let resolver = dead_resolver();
    assert!(resolver.resolve_cached("example.com").await.is_err());
    assert_eq!(resolver.cache().get("example.com", Instant::now()), None);
}
