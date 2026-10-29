//! Shared fixtures of exit tests.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use super::clients::Clients;
use super::outbound::Refusal;
use super::traffic::ExitTraffic;
use super::ExitConfig;
use crate::members::membership::Members;
use crate::site_checks::doh::Resolver;
use crate::site_checks::http_min::HttpLimits;
use crate::transport::net::{BoxFuture, PeerConn, ServiceHandler};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A scripted exit: reads the whole request frame, writes `reply` and closes the stream.
/// `hits` counts the requests it has received.
pub(crate) struct Scripted {
    reply: &'static [u8],
    pub hits: Arc<AtomicUsize>,
}

impl Scripted {
    pub fn new(reply: &'static [u8]) -> Scripted {
        Scripted {
            reply,
            hits: Arc::default(),
        }
    }
}

impl ServiceHandler for Scripted {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let reply = self.reply;
        let hits = self.hits.clone();
        Box::pin(async move {
            while let Ok(mut pair) = conn.accept().await {
                let mut head = [0u8; 2];
                if pair.recv.read_exact(&mut head).await.is_err() {
                    continue;
                }
                let mut tail = vec![0u8; usize::from(head[1]) + 2];
                let _ = pair.recv.read_exact(&mut tail).await;
                hits.fetch_add(1, Ordering::SeqCst);
                let _ = pair.send.write_all(reply).await;
                let _ = pair.send.shutdown().await;
            }
        })
    }
}

/// `ExitConfig` with a resolver pointing at a closed loopback port: an ordinary domain name
/// fails on trying to reach it rather than quietly getting an answer — this is how a test catches an accidental
/// DNS query for an IP literal.
pub(in crate::exit) fn exit_config_with_dead_resolver() -> ExitConfig {
    exit_config(HashMap::new())
}

/// The same with the test table `host_overrides`: entries bypass the address filter and the own-address probe.
pub(in crate::exit) fn exit_config(hosts: HashMap<String, SocketAddr>) -> ExitConfig {
    exit_config_at(SocketAddr::from(([127, 0, 0, 1], 1)), hosts)
}

/// `ExitConfig` whose resolver talks to a "silent" server: the listener accepts connections into its backlog and
/// never answers, so a DoH query hangs until its caller gives up. The listener must live as long as the test, or
/// the resolver would see a closed port instead.
pub(in crate::exit) fn exit_config_with_silent_resolver() -> (ExitConfig, std::net::TcpListener) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("silent DoH listener");
    let server = listener.local_addr().expect("silent DoH address");
    (exit_config_at(server, HashMap::new()), listener)
}

fn exit_config_at(server: SocketAddr, hosts: HashMap<String, SocketAddr>) -> ExitConfig {
    let resolver = Resolver::new(server, HttpLimits::default(), "meshtown-test".to_string())
        .expect("resolver with a stub address");

    ExitConfig {
        enabled: Arc::new(AtomicBool::new(true)),
        members: Members::default(),
        list: Arc::new(OnceLock::new()),
        resolver: Arc::new(resolver),
        host_overrides: Arc::new(hosts),
        clients: Clients::default(),
        traffic: ExitTraffic::default(),
        limits: crate::exit::ExitLimits::unlimited(),
        iface: crate::iface_bind::OutIface::default(),
    }
}

/// A refusal from the result; success panics with the name of what was asked.
pub(in crate::exit) fn refusal_of<T>(
    result: std::result::Result<T, Refusal>,
    what: &str,
) -> Refusal {
    match result {
        Ok(_) => panic!("{what}: a refusal was expected, but an address was given"),
        Err(refusal) => refusal,
    }
}
