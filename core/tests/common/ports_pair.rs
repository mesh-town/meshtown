//! Helpers for the port publishing scenarios with real nodes: a pair of nodes where A publishes and B
//! opens, free-port pickers, waiting, document and listener state readers, a UDP round trip.
//! Plugged in as `#[path = "common/ports_pair.rs"] mod ports_pair;` next to
//! `#[path = "common/socks_pair.rs"] mod common;` (the pair handshake lives there).
#![allow(dead_code)]

use std::time::Duration;

use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, NodeConfig, RunningNode};
use meshtown_core::ports_sharing::open::OpenState;
use meshtown_core::ports_sharing::{Audience, Opened, Proto, Published};
use tokio::net::{TcpListener, UdpSocket};
use tokio::time::{sleep, timeout, Instant};

use super::common::{echo_site, introduce, wait_member};

/// How often a round trip repeats its datagram: the first one of a new sender may be lost while the stream opens.
const RESEND_EVERY: Duration = Duration::from_secs(1);

/// A publication of `proto` `port` for the given circle.
pub(crate) fn published(proto: Proto, port: u16, audience: Audience) -> Published {
    Published {
        proto,
        port,
        name: "svc".to_string(),
        audience,
    }
}

/// A port of `a` opened at `127.0.0.1:local`.
pub(crate) fn opened_as(a: &RunningNode, proto: Proto, port: u16, local: u16) -> Opened {
    Opened {
        proto,
        node: a.id,
        port,
        local,
    }
}

/// A free TCP port.
pub(crate) async fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    l.local_addr().unwrap().port()
}

/// A free UDP port.
pub(crate) async fn free_udp_port() -> u16 {
    let s = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    s.local_addr().unwrap().port()
}

/// A port number that is free for TCP and for UDP at once.
pub(crate) async fn free_port_both() -> u16 {
    for _ in 0..50 {
        let port = free_port().await;
        if UdpSocket::bind(("127.0.0.1", port)).await.is_ok() {
            return port;
        }
    }
    panic!("no port free for both TCP and UDP");
}

/// A TCP echo and a UDP echo listening on the same number; returns it.
pub(crate) async fn echoes_on_one_port() -> u16 {
    for _ in 0..50 {
        let port = echo_site().await.port();
        if let Ok(sock) = UdpSocket::bind(("127.0.0.1", port)).await {
            meshtown_core::node::output::spawn(async move {
                let mut buf = vec![0u8; 2048];
                while let Ok((n, from)) = sock.recv_from(&mut buf).await {
                    let _ = sock.send_to(&buf[..n], from).await;
                }
            });
            return port;
        }
    }
    panic!("no number with both a TCP and a UDP echo");
}

/// A with the publications `published`, B without them; both know each other.
pub(crate) async fn pair_ports(published: Vec<Published>) -> (RunningNode, RunningNode) {
    let key = NetworkKey::generate().unwrap();
    let mut cfg_a = NodeConfig::for_loopback_test(key.clone());
    cfg_a.settings.exit_enabled = false;
    cfg_a.settings.ports_published = published;
    let mut cfg_b = NodeConfig::for_loopback_test(key);
    cfg_b.settings.exit_enabled = false;
    let a = node::start_node(cfg_a).await.unwrap();
    let b = node::start_node(cfg_b).await.unwrap();
    introduce(&a, &b).await;
    wait_member(&a, b.id, Duration::from_secs(20)).await;
    wait_member(&b, a.id, Duration::from_secs(20)).await;
    (a, b)
}

/// Waits until `cond` becomes true, no longer than `limit`.
pub(crate) async fn wait_for<F, Fut>(limit: Duration, what: &str, mut cond: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if cond().await {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("timed out after {limit:?}: {what}");
}

/// The `(proto, port)` pairs of author `a` that node `b` sees in the document (readable entries only), sorted.
pub(crate) async fn seen_ports(b: &RunningNode, a: &RunningNode) -> Vec<(Proto, u16)> {
    let mut v: Vec<(Proto, u16)> = b
        .ports_doc
        .read_all()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.author == a.id && p.record.is_some())
        .map(|p| (p.proto, p.port))
        .collect();
    v.sort_unstable();
    v
}

/// The state of B's listener of `proto` on `127.0.0.1:local`; `None` if there is no such listener.
pub(crate) fn listener_state(b: &RunningNode, proto: Proto, local: u16) -> Option<OpenState> {
    b.opener
        .bind_states()
        .into_iter()
        .find(|(o, _, _)| o.proto == proto && o.local == local)
        .map(|(_, state, _)| state)
}

/// The last dial failure of B's listener (`"not_opened"`, `"offline"`, ...), `None` before the first attempt
/// and after a success.
pub(crate) fn last_error(b: &RunningNode, proto: Proto, local: u16) -> Option<String> {
    let (_, _, reach) = b
        .opener
        .bind_states()
        .into_iter()
        .find(|(o, _, _)| o.proto == proto && o.local == local)?;
    let value = serde_json::to_value(reach?).unwrap();
    Some(value.as_str().unwrap().to_string())
}

/// A new sender socket sends `payload` to `127.0.0.1:local` (repeating once a second) and returns the first
/// reply, or `None` if nothing came within `wait`.
pub(crate) async fn udp_roundtrip(local: u16, payload: &[u8], wait: Duration) -> Option<Vec<u8>> {
    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let deadline = Instant::now() + wait;
    let mut buf = vec![0u8; 2048];
    while Instant::now() < deadline {
        sock.send_to(payload, ("127.0.0.1", local)).await.unwrap();
        let slice = RESEND_EVERY.min(deadline.saturating_duration_since(Instant::now()));
        if let Ok(Ok((n, _))) = timeout(slice, sock.recv_from(&mut buf)).await {
            return Some(buf[..n].to_vec());
        }
    }
    None
}
