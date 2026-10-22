//! Own resolver for the checks: DNS over HTTPS and a separate query to the ISP's resolver.
//!
//! What it does: builds and parses DNS packets, asks a DoH server given by IP address for a domain's address,
//! and — only as a spoofing signal — asks the ISP's resolver over UDP-53;
//! drops addresses that must not be used (own fake-ip pool, loopback, "zero", link-local).
//! Caches addresses only for the exit (`resolve_cached`, answer TTL within 30–120 s, up to
//! 1024 names); reachability checks query around the cache (`resolve`).
//! What it doesn't do: doesn't use the system resolver, doesn't remember failures and empty answers,
//! doesn't query AAAA records, doesn't verify DNSSEC signatures.
//! Related: transport is `crate::http_min` over TLS; used by `crate::reachability` and
//! `crate::exit`; the cache is in `doh/cache.rs`.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use data_encoding::BASE64URL_NOPAD;
use rustls::pki_types::ServerName;
use rustls::ClientConfig;
use simple_dns::rdata::RData;
use simple_dns::{Name, Packet, PacketFlag, Question, CLASS, QCLASS, QTYPE, RCODE, TYPE};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;
use tokio_rustls::TlsConnector;

use crate::http_min::{self, HttpLimits, HttpGetRequest};
use crate::tcp_tune::disable_nagle;

mod cache;

use cache::{cache_key, clamp_ttl, DnsCache, CACHE_ENTRIES};

/// Default DoH server: Cloudflare, given by address — no name needs to be resolved.
pub const DEFAULT_DOH: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 443);
/// DoH request path (RFC 8484).
pub const DOH_PATH: &str = "/dns-query";
/// Content type of the DoH request and response.
pub const DNS_MESSAGE: &str = "application/dns-message";

/// A resolver answer.
#[derive(Debug, Clone)]
pub struct DnsAnswer {
    pub addrs: Vec<Ipv4Addr>,
    pub elapsed: Duration,
}

/// A parsed answer: the A addresses and the minimum TTL of the answer section (`None` if there is no A).
struct Records {
    addrs: Vec<Ipv4Addr>,
    ttl: Option<u32>,
}

/// An address from the fake-ip pool (RFC 2544, `198.18.0.0/15`): issued by our own TUN.
pub fn is_fake_ip(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 198 && (o[1] == 18 || o[1] == 19)
}

/// An address a check must not use: our own TUN, loopback, "zero", link-local, broadcast.
pub fn is_unusable(ip: Ipv4Addr) -> bool {
    is_fake_ip(ip)
        || ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_multicast()
}

/// The checks resolver.
#[derive(Clone)]
pub struct Resolver {
    server: SocketAddr,
    tls: Arc<ClientConfig>,
    limits: HttpLimits,
    user_agent: String,
    /// Address cache for `resolve_cached`; clones of the resolver share it.
    cache: Arc<Mutex<DnsCache>>,
}

impl Resolver {
    /// Creates a resolver for the DoH server given by address.
    pub fn new(server: SocketAddr, limits: HttpLimits, user_agent: String) -> Result<Resolver> {
        Ok(Resolver {
            server,
            tls: Arc::new(http_min::tls_client_config()?),
            limits,
            user_agent,
            cache: Arc::new(Mutex::new(DnsCache::new(CACHE_ENTRIES))),
        })
    }

    /// The DoH server address.
    pub fn server(&self) -> SocketAddr {
        self.server
    }

    /// Asks the DoH server for a domain's addresses, always a fresh answer around the cache: the time and addresses
    /// are written by the checks. The system resolver is not involved: the server address is known, and the name in TLS
    /// is the same address, so no SNI is sent and there is nothing to resolve.
    pub async fn resolve(&self, domain: &str) -> Result<DnsAnswer> {
        let started = Instant::now();
        let records = self.query_doh(domain).await?;
        Ok(DnsAnswer {
            addrs: records.addrs,
            elapsed: started.elapsed(),
        })
    }

    /// A domain's addresses for the exit: a live cache entry — no network; a miss — a trip to DoH and a write for
    /// the answer's TTL. A DoH error goes up as is and is not written to the cache.
    pub async fn resolve_cached(&self, domain: &str) -> Result<Vec<Ipv4Addr>> {
        let key = cache_key(domain);
        let hit = self.cache().get(&key, Instant::now());
        if let Some(addrs) = hit {
            return Ok(addrs);
        }
        let records = self.query_doh(domain).await?;
        self.remember(key, &records);
        Ok(records.addrs)
    }

    /// Forgets a domain's addresses: the next `resolve_cached` will ask DoH again.
    pub fn forget(&self, domain: &str) {
        self.cache().remove(&cache_key(domain));
    }

    /// Puts a domain's addresses into the cache for 60 s — only for tests of neighboring modules, to check
    /// filtering on a DoH answer without a network (`exit::server`).
    #[cfg(all(test, feature = "net-hyper"))]
    pub(crate) fn seed_cache_for_tests(&self, domain: &str, addrs: Vec<Ipv4Addr>) {
        self.cache().insert(
            cache_key(domain),
            addrs,
            Duration::from_secs(60),
            Instant::now(),
        );
    }

    /// A live cache entry for a domain — only for tests of neighboring modules (removed through `forget`).
    #[cfg(all(test, feature = "net-hyper"))]
    pub(crate) fn cached_for_tests(&self, domain: &str) -> Option<Vec<Ipv4Addr>> {
        self.cache().get(&cache_key(domain), Instant::now())
    }

    /// Puts an answer into the cache for its TTL; an answer with no addresses (no TTL) is not cached.
    fn remember(&self, key: String, records: &Records) {
        let Some(ttl) = records.ttl else {
            return;
        };
        let addrs = records.addrs.clone();
        self.cache()
            .insert(key, addrs, clamp_ttl(ttl), Instant::now());
    }

    /// The cache lock. Only a lookup or insert happens under it, and it is not held across `.await`; a poisoned
    /// lock is taken as is — the cache has no invariants a panic could break.
    fn cache(&self) -> MutexGuard<'_, DnsCache> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// One trip to the DoH server: connection, TLS, GET, parsing the answer.
    async fn query_doh(&self, domain: &str) -> Result<Records> {
        let query = build_query(domain)?;
        let path = format!("{DOH_PATH}?dns={}", BASE64URL_NOPAD.encode(&query));
        let mut tls = self.open_tls().await?;
        let host = self.server.ip().to_string();
        let req = HttpGetRequest {
            host: &host,
            path: &path,
            accept: DNS_MESSAGE,
            user_agent: &self.user_agent,
        };
        let resp = http_min::http_get(&mut tls, req, self.limits).await?;
        if resp.status != 200 {
            bail!("DoH server answered with code {}", resp.status);
        }
        parse_answer(&resp.body, domain)
    }

    /// TCP and TLS to the DoH server, each step under `limits.connect`.
    async fn open_tls(&self) -> Result<tokio_rustls::client::TlsStream<TcpStream>> {
        let tcp = timeout(self.limits.connect, TcpStream::connect(self.server))
            .await
            .context("timeout connecting to the DoH server")?
            .context("connection to the DoH server")?;
        disable_nagle(&tcp);
        let connector = TlsConnector::from(self.tls.clone());
        let name = ServerName::from(self.server.ip());
        timeout(self.limits.connect, connector.connect(name, tcp))
            .await
            .context("timeout during TLS to the DoH server")?
            .context("TLS to the DoH server")
    }

    /// Asks the same domain from the ISP's resolver over UDP port 53. The answer is only a sign of
    /// a spoofing signal: the addresses from here go nowhere, so a zero request id is acceptable.
    pub async fn resolve_udp(&self, server: SocketAddr, domain: &str) -> Result<DnsAnswer> {
        let started = Instant::now();
        let query = build_query(domain)?;
        let bind: SocketAddr = if server.is_ipv4() {
            SocketAddr::from(([0, 0, 0, 0], 0))
        } else {
            SocketAddr::from(([0u16; 8], 0))
        };
        let socket = UdpSocket::bind(bind).await.context("UDP socket for DNS")?;
        socket
            .send_to(&query, server)
            .await
            .context("query to the ISP resolver")?;
        let mut buf = vec![0u8; 1500];
        let (n, from) = timeout(self.limits.read, socket.recv_from(&mut buf))
            .await
            .context("timeout waiting for the ISP resolver's answer")?
            .context("answer from the ISP resolver")?;
        if from.ip() != server.ip() {
            bail!("DNS answer came from a foreign address {}", from.ip());
        }
        let addrs = parse_answer(&buf[..n], domain)?.addrs;
        Ok(DnsAnswer {
            addrs,
            elapsed: started.elapsed(),
        })
    }
}

/// Builds an A-record query with the "recursion desired" flag, id 0 (RFC 8484).
fn build_query(domain: &str) -> Result<Vec<u8>> {
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
fn parse_answer(bytes: &[u8], domain: &str) -> Result<Records> {
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
#[path = "doh_tests.rs"]
mod tests;
