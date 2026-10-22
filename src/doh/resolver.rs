//! The resolver: DoH queries, the address cache for the exit, and the separate query to the ISP's resolver.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use data_encoding::BASE64URL_NOPAD;
use rustls::pki_types::ServerName;
use rustls::ClientConfig;
use simple_dns::TYPE;
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;
use tokio_rustls::TlsConnector;

use crate::http_min::{self, HttpGetRequest, HttpLimits};
use crate::tcp_tune::disable_nagle;

use super::cache::{cache_key, clamp_ttl, DnsCache, CACHE_ENTRIES, MIN_TTL};
use super::packet::{build_query, parse_answer, Records};
use super::{DNS_MESSAGE, DOH_PATH};

/// A resolver answer.
#[derive(Debug, Clone)]
pub struct DnsAnswer {
    pub addrs: Vec<Ipv4Addr>,
    pub elapsed: Duration,
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
        let records = self.query_doh(domain, TYPE::A).await?;
        Ok(DnsAnswer {
            addrs: records.v4,
            elapsed: started.elapsed(),
        })
    }

    /// A domain's IPv4 addresses for the exit: a live cache entry — no network; a miss — a trip to DoH and a write for
    /// the answer's TTL. A DoH error goes up as is and is not written to the cache.
    pub async fn resolve_cached(&self, domain: &str) -> Result<Vec<Ipv4Addr>> {
        Ok(self.resolve_cached_nx(domain).await?.0)
    }

    /// Like `resolve_cached`, and also says whether DoH answered "the name does not exist" (then there is nothing to
    /// ask for any other record type). A cache hit never reports it: the cache doesn't keep empty answers.
    pub async fn resolve_cached_nx(&self, domain: &str) -> Result<(Vec<Ipv4Addr>, bool)> {
        let key = cache_key(domain);
        let hit = self.cache().get(&key, Instant::now());
        if let Some(addrs) = hit {
            return Ok((addrs, false));
        }
        let records = self.query_doh(domain, TYPE::A).await?;
        self.remember(key, &records);
        Ok((records.v4, records.nx))
    }

    /// A domain's IPv6 addresses for the exit: a live cache entry that knows the family — no network; otherwise a
    /// trip to DoH for AAAA and a write next to the live entry. A DoH error goes up and is not written to the cache.
    pub async fn resolve_cached_v6(&self, domain: &str) -> Result<Vec<Ipv6Addr>> {
        let key = cache_key(domain);
        let hit = self.cache().get_v6(&key, Instant::now());
        if let Some(addrs) = hit {
            return Ok(addrs);
        }
        let records = self.query_doh(domain, TYPE::AAAA).await?;
        let ttl = records.ttl.map_or(MIN_TTL, clamp_ttl);
        let addrs = records.v6.clone();
        self.cache().insert_v6(key, addrs, ttl, Instant::now());
        Ok(records.v6)
    }

    /// Forgets a domain's addresses of both families: the next `resolve_cached` will ask DoH again.
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

    /// Puts both families of a domain into the cache for 60 s (an empty `v4` next to a non-empty `v6` is a "v6-only name") —
    /// only for tests of neighboring modules.
    #[cfg(all(test, feature = "net-hyper"))]
    pub(crate) fn seed_cache_dual_for_tests(
        &self,
        domain: &str,
        v4: Vec<Ipv4Addr>,
        v6: Vec<Ipv6Addr>,
    ) {
        let (key, ttl, now) = (cache_key(domain), Duration::from_secs(60), Instant::now());
        let mut cache = self.cache();
        cache.insert(key.clone(), v4, ttl, now);
        cache.insert_v6(key.clone(), v6, ttl, now);
        cache.note_empty_v4(&key, now);
    }

    /// A live cache entry for a domain — only for tests of neighboring modules (removed through `forget`).
    #[cfg(all(test, feature = "net-hyper"))]
    pub(crate) fn cached_for_tests(&self, domain: &str) -> Option<Vec<Ipv4Addr>> {
        self.cache().get(&cache_key(domain), Instant::now())
    }

    /// Puts an A answer into the cache for its TTL; an answer with no addresses (no TTL) is only noted as an empty
    /// IPv4 next to a live IPv6 entry.
    fn remember(&self, key: String, records: &Records) {
        let Some(ttl) = records.ttl else {
            self.cache().note_empty_v4(&key, Instant::now());
            return;
        };
        let addrs = records.v4.clone();
        self.cache()
            .insert(key, addrs, clamp_ttl(ttl), Instant::now());
    }

    /// The cache lock. Only a lookup or insert happens under it, and it is not held across `.await`; a poisoned
    /// lock is taken as is — the cache has no invariants a panic could break.
    fn cache(&self) -> MutexGuard<'_, DnsCache> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// One trip to the DoH server for a record type: connection, TLS, GET, parsing the answer.
    async fn query_doh(&self, domain: &str, qtype: TYPE) -> Result<Records> {
        let query = build_query(domain, qtype)?;
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
        let query = build_query(domain, TYPE::A)?;
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
        let addrs = parse_answer(&buf[..n], domain)?.v4;
        Ok(DnsAnswer {
            addrs,
            elapsed: started.elapsed(),
        })
    }
}

#[cfg(test)]
#[path = "resolver_tests.rs"]
mod tests;
