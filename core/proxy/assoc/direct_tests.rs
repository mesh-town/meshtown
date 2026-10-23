//! The direct upstream through a real relay: a local UDP echo server stands for the target; the proxy config has a
//! dead resolver (loopback port 1) and a `host_overrides` table.

use std::collections::HashMap;
use std::future::pending;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

use super::super::relay::{relay, RelayLimits};
use super::super::stats::RelayStats;
use super::*;
use crate::exit::PeerConnPool;
use crate::proxy::route::RouteCtx;
use crate::proxy::socks5::{encode_udp_request, parse_udp_datagram};
use crate::site_checks::doh::Resolver;
use crate::site_checks::http_min::HttpLimits;

const WAIT: Duration = Duration::from_secs(5);
const SILENCE: Duration = Duration::from_millis(300);

async fn config(hosts: HashMap<String, SocketAddr>) -> ProxyConfig {
    let key = crate::members::network_key::NetworkKey::generate().unwrap();
    let node = crate::node::start_node(crate::node::NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    let resolver = Resolver::new(
        SocketAddr::from(([127, 0, 0, 1], 1)),
        HttpLimits::default(),
        "meshtown-test".to_string(),
    )
    .expect("resolver with a stub address");
    ProxyConfig {
        route: RouteCtx::new(node.members.clone(), node.domains.clone(), node.id, vec![]),
        stack: node.stack.clone(),
        pool: PeerConnPool::default(),
        resolver: Arc::new(resolver),
        host_overrides: Arc::new(hosts),
        udp_slots: crate::proxy::udp_slots(),
    }
}

/// A UDP echo server on the given loopback address, `None` if it cannot be bound.
async fn echo_on(ip: &str) -> Option<SocketAddr> {
    let sock = UdpSocket::bind((ip.parse::<IpAddr>().unwrap(), 0))
        .await
        .ok()?;
    let addr = sock.local_addr().unwrap();
    crate::node::output::spawn(async move {
        let mut buf = [0u8; 2048];
        while let Ok((n, from)) = sock.recv_from(&mut buf).await {
            let _ = sock.send_to(&buf[..n], from).await;
        }
    });
    Some(addr)
}

struct Rig {
    control: TcpStream,
    client: UdpSocket,
    relay_addr: SocketAddr,
    task: JoinHandle<RelayStats>,
}

async fn rig(hosts: HashMap<String, SocketAddr>) -> Rig {
    let cfg = config(hosts).await;
    let (tx, rx) = open_direct(&cfg).expect("direct sockets");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let control = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (served, _) = listener.accept().await.unwrap();
    let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = udp.local_addr().unwrap();
    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let ip = "127.0.0.1".parse().unwrap();
    let task = crate::node::output::spawn(relay(
        served,
        udp,
        ip,
        tx,
        rx,
        pending(),
        RelayLimits {
            idle: Duration::from_secs(60),
        },
    ));
    Rig {
        control,
        client,
        relay_addr,
        task,
    }
}

impl Rig {
    async fn send(&self, target: Target, port: u16, data: &[u8]) {
        let bytes = encode_udp_request(&target, port, data).unwrap();
        self.client.send_to(&bytes, self.relay_addr).await.unwrap();
    }

    async fn reply(&self) -> Option<(SocketAddr, Vec<u8>)> {
        let mut buf = vec![0u8; 65_535];
        let (n, _) = timeout(WAIT, self.client.recv_from(&mut buf))
            .await
            .ok()?
            .ok()?;
        let packet = parse_udp_datagram(&buf[..n]).unwrap();
        let Target::Ip(ip) = packet.target else {
            panic!("a reply carries an address")
        };
        Some((SocketAddr::new(ip, packet.port), packet.data))
    }

    /// Nothing arrives at the client for a while.
    async fn is_silent(&self) -> bool {
        let mut buf = vec![0u8; 65_535];
        timeout(SILENCE, self.client.recv_from(&mut buf))
            .await
            .is_err()
    }

    /// Closes the control connection and returns what the relay counted.
    async fn finish(self) -> RelayStats {
        sleep(SILENCE).await;
        drop(self.control);
        timeout(WAIT, self.task)
            .await
            .expect("relay ended in time")
            .unwrap()
    }
}

fn ip_target(text: &str) -> Target {
    Target::Ip(text.parse().unwrap())
}

mod targets {
    use super::*;

    #[tokio::test]
    async fn ip_target_is_echoed_directly() {
        let echo = echo_on("127.0.0.1").await.unwrap();
        let rig = rig(HashMap::new()).await;
        rig.send(Target::Ip(echo.ip()), echo.port(), b"ping").await;
        assert_eq!(rig.reply().await, Some((echo, b"ping".to_vec())));
    }

    #[tokio::test]
    async fn domain_target_uses_host_overrides() {
        let echo = echo_on("127.0.0.1").await.unwrap();
        let rig = rig(HashMap::from([("echo.test".to_string(), echo)])).await;
        rig.send(Target::Domain("echo.test".to_string()), 9, b"named")
            .await;
        assert_eq!(rig.reply().await, Some((echo, b"named".to_vec())));
    }

    #[tokio::test]
    async fn ipv6_target_is_echoed_when_loopback_v6_binds() {
        let Some(echo) = echo_on("::1").await else {
            eprintln!("no IPv6 loopback on this host: skipped");
            return;
        };
        let rig = rig(HashMap::new()).await;
        rig.send(Target::Ip(echo.ip()), echo.port(), b"six").await;
        assert_eq!(rig.reply().await, Some((echo, b"six".to_vec())));
    }
}

mod refusals {
    use super::*;

    #[tokio::test]
    async fn fake_ip_target_is_dropped() {
        let rig = rig(HashMap::new()).await;
        rig.send(ip_target("198.18.0.5"), 53, b"x").await;
        assert!(rig.is_silent().await);
        assert_eq!(rig.finish().await.unroutable, 1);
    }

    #[tokio::test]
    async fn unspecified_and_multicast_targets_are_dropped() {
        let echo = echo_on("127.0.0.1").await.unwrap();
        let rig = rig(HashMap::new()).await;
        rig.send(ip_target("0.0.0.0"), echo.port(), b"a").await;
        rig.send(ip_target("224.0.0.251"), 5353, b"b").await;
        rig.send(ip_target("255.255.255.255"), 9, b"c").await;
        rig.send(Target::Ip(echo.ip()), 0, b"d").await;
        assert!(rig.is_silent().await);
        assert_eq!(rig.finish().await.unroutable, 4);
    }

    #[tokio::test]
    async fn datagram_to_own_bnd_does_not_loop() {
        let echo = echo_on("127.0.0.1").await.unwrap();
        let rig = rig(HashMap::new()).await;
        rig.send(
            Target::Ip(rig.relay_addr.ip()),
            rig.relay_addr.port(),
            b"loop",
        )
        .await;
        rig.send(Target::Ip(echo.ip()), echo.port(), b"real").await;
        assert_eq!(rig.reply().await, Some((echo, b"real".to_vec())));
        assert!(rig.is_silent().await);
        assert_eq!(rig.finish().await.foreign, 1);
    }

    #[tokio::test]
    async fn unresolved_domain_is_dropped() {
        let echo = echo_on("127.0.0.1").await.unwrap();
        let rig = rig(HashMap::new()).await;
        rig.send(Target::Domain("nowhere.test".to_string()), 9, b"lost")
            .await;
        rig.send(Target::Ip(echo.ip()), echo.port(), b"next").await;
        assert_eq!(rig.reply().await, Some((echo, b"next".to_vec())));
        assert_eq!(rig.finish().await.unroutable, 1);
    }
}
