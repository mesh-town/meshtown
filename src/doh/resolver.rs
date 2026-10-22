//! The resolver: DoH queries, the address cache for the exit, and the separate query to the ISP's resolver.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use data_encoding::BASE64URL_NOPAD;
use rustls::pki_types::ServerName;
use rustls::ClientConfig;
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;
use tokio_rustls::TlsConnector;

use crate::http_min::{self, HttpLimits, HttpGetRequest};
use crate::tcp_tune::disable_nagle;

use super::cache::{cache_key, clamp_ttl, DnsCache, CACHE_ENTRIES};
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

#[cfg(test)]
#[path = "resolver_tests.rs"]
mod tests;
