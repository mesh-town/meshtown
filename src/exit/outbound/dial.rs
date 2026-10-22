//! From a name or an IP literal to an open TCP stream: address choice, own-address check, connection.
//!
//! A name is tried over IPv4 first; IPv6 is asked for only when no IPv4 connection opened and time is left.
//! Addresses of one family are tried one after another: every attempt but the last one is capped, the last one
//! takes the rest of the time budget.

use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::time::timeout;

use super::address_guard::is_own_address;
use super::refusal::Refusal;
use super::target::{literal_address, v4_answer, v6_answer, NameAnswer, AAAA_LOOKUP_TIMEOUT};
use crate::exit::frame::EXIT_TCP_CONNECT_TIMEOUT;
use crate::exit::server::ExitConfig;

/// How long the connection phase takes in all and how long one attempt may take unless it is the last one.
#[derive(Clone, Copy)]
pub(in crate::exit) struct Budget {
    pub total: Duration,
    pub attempt: Duration,
}

/// A non-last connection attempt is dropped after this long.
pub(in crate::exit) const EXIT_TCP_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(3);

pub(in crate::exit) const BUDGET: Budget = Budget {
    total: EXIT_TCP_CONNECT_TIMEOUT,
    attempt: EXIT_TCP_ATTEMPT_TIMEOUT,
};

/// How many attempts were made and the last address tried.
#[derive(Default)]
struct Attempts {
    count: usize,
    last: Option<SocketAddr>,
}

impl Attempts {
    fn then(self, later: Attempts) -> Attempts {
        Attempts {
            count: self.count + later.count,
            last: later.last.or(self.last),
        }
    }
}

/// An open stream to the destination of a request: the real network.
pub(in crate::exit) async fn open_target(
    cfg: &ExitConfig,
    target_host: &str,
    port: u16,
) -> std::result::Result<TcpStream, Refusal> {
    open_target_with(cfg, target_host, port, BUDGET, TcpStream::connect).await
}

/// The same with the connection made by `connect`. The test table `host_overrides` is used as is; an IP literal
/// goes through the filter (the client sends the address in the request frame as a name) and the own-address
/// check; otherwise the resolver cache, on a miss DoH, and the same filter on every address of the answer.
pub(in crate::exit) async fn open_target_with<S, C, F>(
    cfg: &ExitConfig,
    target_host: &str,
    port: u16,
    budget: Budget,
    connect: C,
) -> std::result::Result<S, Refusal>
where
    C: Fn(SocketAddr) -> F,
    F: Future<Output = io::Result<S>>,
{
    if let Some(addr) = cfg.host_overrides.get(target_host) {
        return open_one(cfg, target_host, *addr, budget, &connect).await;
    }
    if let Ok(ip) = target_host.parse::<IpAddr>() {
        let addr = literal_target(ip, target_host, port)?;
        return open_one(cfg, target_host, addr, budget, &connect).await;
    }
    let v4 = v4_answer(&cfg.resolver, target_host, port).await?;
    open_answers(cfg, target_host, port, budget, &connect, v4).await
}

/// The IPv4 answer is known: connect over it, then, if nothing opened, over IPv6 from AAAA. A name that does not
/// exist has no AAAA either. The clock starts after the IPv4 answer.
async fn open_answers<S, C, F>(
    cfg: &ExitConfig,
    target_host: &str,
    port: u16,
    budget: Budget,
    connect: &C,
    v4: NameAnswer,
) -> std::result::Result<S, Refusal>
where
    C: Fn(SocketAddr) -> F,
    F: Future<Output = io::Result<S>>,
{
    let deadline = Instant::now() + budget.total;
    let first = match connect_in_order(&v4.open, deadline, budget.attempt, connect).await {
        Ok(stream) => return Ok(stream),
        Err(attempts) => attempts,
    };
    let left = time_left(deadline);
    if v4.nx || left.is_zero() {
        return Err(refusal(cfg, target_host, &first, v4.any));
    }
    let v6 = v6_answer(&cfg.resolver, target_host, port, left.min(AAAA_LOOKUP_TIMEOUT)).await;
    match connect_in_order(&v6.open, deadline, budget.attempt, connect).await {
        Ok(stream) => Ok(stream),
        Err(more) => Err(refusal(cfg, target_host, &first.then(more), v4.any || v6.any)),
    }
}

/// A literal checked against the guard and the node's own addresses.
fn literal_target(ip: IpAddr, target_host: &str, port: u16) -> std::result::Result<SocketAddr, Refusal> {
    let addr = literal_address(ip, target_host, port)?;
    if is_own_address(addr.ip()) {
        return Err(Refusal::forbidden(target_host));
    }
    Ok(addr)
}

/// One address, the whole budget: a literal or a table entry.
async fn open_one<S, C, F>(
    cfg: &ExitConfig,
    target_host: &str,
    addr: SocketAddr,
    budget: Budget,
    connect: &C,
) -> std::result::Result<S, Refusal>
where
    C: Fn(SocketAddr) -> F,
    F: Future<Output = io::Result<S>>,
{
    let deadline = Instant::now() + budget.total;
    match connect_in_order(&[addr], deadline, budget.attempt, connect).await {
        Ok(stream) => Ok(stream),
        Err(attempts) => Err(refusal(cfg, target_host, &attempts, true)),
    }
}

/// Addresses one after another: an attempt takes `attempt` at most, the last one — all that is left until `deadline`.
async fn connect_in_order<S, C, F>(
    addrs: &[SocketAddr],
    deadline: Instant,
    attempt: Duration,
    connect: &C,
) -> std::result::Result<S, Attempts>
where
    C: Fn(SocketAddr) -> F,
    F: Future<Output = io::Result<S>>,
{
    let mut done = Attempts::default();
    for (i, addr) in addrs.iter().enumerate() {
        let left = time_left(deadline);
        if left.is_zero() {
            break;
        }
        let cap = if i + 1 == addrs.len() { left } else { left.min(attempt) };
        done = Attempts { count: done.count + 1, last: Some(*addr) };
        if let Ok(Ok(stream)) = timeout(cap, connect(*addr)).await {
            return Ok(stream);
        }
    }
    Err(done)
}

fn time_left(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

/// The refusal when nothing opened: a failed attempt wins, then "all closed", then "no address". The name is
/// forgotten after a failed attempt: its addresses may be stale in the cache.
fn refusal(cfg: &ExitConfig, target_host: &str, tried: &Attempts, any_address: bool) -> Refusal {
    if let Some(last) = tried.last {
        cfg.resolver.forget(target_host);
        return Refusal::connect_failed(target_host, last, tried.count);
    }
    if any_address {
        return Refusal::forbidden(target_host);
    }
    Refusal::no_address(format!("{target_host} — no usable address"))
}

#[cfg(test)]
#[path = "dial_tests.rs"]
mod tests;
