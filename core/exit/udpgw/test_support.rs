//! Shared fixtures of the gateway tests: a client end of an in-memory stream and an exit configuration.

use super::codec::{encode, read_frame, Frame};
use super::session::{run, Limits};
use crate::exit::clients::Clients;
use crate::exit::server::ExitConfig;
use crate::exit::traffic::ExitTraffic;
use crate::members::membership::Members;
use crate::proxy::domains::DomainList;
use crate::site_checks::doh::Resolver;
use crate::site_checks::http_min::HttpLimits;
use crate::transport::net::facade::test_support::pid;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::io::{split, AsyncReadExt, DuplexStream, ReadHalf, WriteHalf};
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;
use tokio::time::timeout;

pub(super) const WAIT: Duration = Duration::from_secs(5);

pub(super) fn cfg_with(overrides: HashMap<String, SocketAddr>) -> ExitConfig {
    let stub = SocketAddr::from(([127, 0, 0, 1], 1));
    let resolver =
        Resolver::new(stub, HttpLimits::default(), "meshtown-test".to_string()).expect("resolver");
    ExitConfig {
        enabled: Arc::new(AtomicBool::new(true)),
        members: Members::default(),
        list: Arc::new(OnceLock::<DomainList>::new()),
        resolver: Arc::new(resolver),
        host_overrides: Arc::new(overrides),
        clients: Clients::default(),
        traffic: ExitTraffic::default(),
        limits: crate::exit::ExitLimits::unlimited(),
    }
}

/// A UDP echo server on the loopback.
pub(super) async fn spawn_echo() -> SocketAddr {
    let sock = UdpSocket::bind("127.0.0.1:0").await.expect("echo socket");
    let addr = sock.local_addr().expect("echo address");
    crate::node::output::spawn(async move {
        let mut buf = [0u8; 2048];
        while let Ok((n, from)) = sock.recv_from(&mut buf).await {
            let _ = sock.send_to(&buf[..n], from).await;
        }
    });
    addr
}

pub(super) struct Client {
    to: WriteHalf<DuplexStream>,
    from: ReadHalf<DuplexStream>,
}

impl Client {
    pub async fn send(&mut self, frame: &Frame) {
        self.to
            .write_all(&encode(frame).expect("fits"))
            .await
            .expect("frame sent");
    }

    pub async fn next(&mut self) -> Frame {
        timeout(WAIT, read_frame(&mut self.from))
            .await
            .expect("in time")
            .expect("frame")
    }

    pub async fn send_raw(&mut self, bytes: &[u8]) {
        self.to.write_all(bytes).await.expect("bytes sent");
    }

    /// The wire bytes of the next whole frame, length field included.
    pub async fn next_raw(&mut self) -> Vec<u8> {
        let read = async {
            let mut len = [0u8; 2];
            self.from.read_exact(&mut len).await.expect("length");
            let mut out = len.to_vec();
            out.resize(2 + usize::from(u16::from_be_bytes(len)), 0);
            self.from.read_exact(&mut out[2..]).await.expect("body");
            out
        };
        timeout(WAIT, read).await.expect("in time")
    }

    /// No frame within a short while.
    pub async fn silent(&mut self) -> bool {
        timeout(Duration::from_millis(300), self.from.read_u8())
            .await
            .is_err()
    }
}

pub(super) fn start(cfg: ExitConfig, limits: Limits) -> (Client, JoinHandle<()>) {
    let (client_end, server_end) = tokio::io::duplex(1 << 20);
    let (server_read, server_write) = split(server_end);
    let task = crate::node::output::spawn(run(
        cfg,
        pid(1),
        Box::new(server_write),
        Box::new(server_read),
        limits,
    ));
    let (from, to) = split(client_end);
    (Client { to, from }, task)
}
