//! Self-check of the `core/tests/lossy_link/` proxy: forwarding both ways, counters, releasing the port, blocking an address (`block_addr`).
//!
//! The shared harness is `core/tests/common/lossy_link.rs`. There is no QUIC stack here — the congestion
//! measurement lives in `tests/congestion_lossy.rs`.

#[path = "common/lossy_link.rs"]
mod common;
mod lossy_link;

#[cfg(target_os = "macos")]
use std::net::SocketAddr;
use std::time::Duration;

#[cfg(target_os = "macos")]
use common::collect_raw;
use common::{collect, packet, rig};
#[cfg(target_os = "macos")]
use lossy_link::lan::{lan_delivery_works, local_lan_ip, wait_blocked};
#[cfg(target_os = "macos")]
use lossy_link::{LinkConfig, LossyLink};
use tokio::net::UdpSocket;
use tokio::time::sleep;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plain_link_forwards_both_ways_and_counts() {
    let rig = rig(|_| {}).await;
    rig.send_c2s(7, 100).await;
    let got = collect(&rig.target, rig.epoch, Duration::from_millis(200)).await;
    assert_eq!(got.len(), 1);
    assert_eq!((got[0].seq, got[0].len), (7, 100));
    let back_socket = got[0].from;
    assert_ne!(
        back_socket,
        rig.link.front_addr(),
        "the back socket is its own, not the front one"
    );
    rig.target
        .send_to(&packet(8, 50, rig.epoch), back_socket)
        .await
        .unwrap();
    let reply = collect(&rig.client, rig.epoch, Duration::from_millis(200)).await;
    assert_eq!(reply.len(), 1);
    assert_eq!(
        reply[0].from,
        rig.link.front_addr(),
        "the reply leaves from the front socket"
    );
    let s = rig.link.stats();
    assert_eq!(
        (
            s.c2s.received,
            s.c2s.delivered,
            s.c2s.lost_model,
            s.c2s.dropped_queue
        ),
        (1, 1, 0, 0)
    );
    assert_eq!(
        (
            s.s2c.received,
            s.s2c.delivered,
            s.s2c.lost_model,
            s.s2c.dropped_queue
        ),
        (1, 1, 0, 0)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_link_frees_front_port() {
    let rig = rig(|_| {}).await;
    let front = rig.link.front_addr();
    drop(rig.link);
    sleep(Duration::from_millis(100)).await;
    UdpSocket::bind(front)
        .await
        .expect("the front port must be released");
}

/// This is how the proxy closes the bypass in the measurement: the server listens on `0.0.0.0:P`, the client knocks on
/// `<interface ip>:P` — the packet must be eaten by the proxy, while on `127.0.0.1:P` everything arrives as before.
/// If even the probe datagram to the LAN address did not arrive (the macOS firewall, a verdict by Mach-O UUID),
/// `SKIP`; the test is red only with a wrong `block_addr` (the probe arrived, the sink is empty).
#[cfg(target_os = "macos")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn block_addr_swallows_datagrams_sent_to_wildcard_held_address() {
    let Some(lan) = local_lan_ip() else { return };
    if !lan_delivery_works(lan).await {
        eprintln!("SKIP: the macOS firewall does not deliver datagrams to the LAN address for this binary");
        return;
    }
    let wide = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    let port = wide.local_addr().unwrap().port();
    let mut link = LossyLink::start(LinkConfig::plain(SocketAddr::from(([127, 0, 0, 1], port))));
    link.block_addr(SocketAddr::new(lan, port)).unwrap();
    let client = UdpSocket::bind("0.0.0.0:0").await.unwrap();
    client
        .send_to(b"blocked", SocketAddr::new(lan, port))
        .await
        .unwrap();
    client
        .send_to(b"open", SocketAddr::from(([127, 0, 0, 1], port)))
        .await
        .unwrap();
    let mut buf = [0u8; 16];
    let got = collect_raw(&wide, &mut buf).await;
    assert_eq!(
        got,
        vec![b"open".to_vec()],
        "nothing must reach the occupied address"
    );
    // Mesh Town: the sink counts in its own task — wait for the counter, do not read it instantly
    assert_eq!(wait_blocked(&link, 1).await, 1);
}
