//! Tests of forwarding on `NetStack` over `MemNet`: refusals close TCP quickly (no member,
//! deadline expired). Other `forward_to_peer_port` cases are in `dial_forward_tests.rs`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::transport::net::facade::test_support::{pid, secret, Probe};
use crate::transport::net::{MemNet, NetStack, NetStackBuilder, PeerId, ServiceHandler, PORT};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};

use super::*;
use crate::ports_sharing::types::Proto;

/// A TCP pair: the client half and the accepted one.
async fn pair() -> (TcpStream, TcpStream) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let c = TcpStream::connect(l.local_addr().unwrap()).await.unwrap();
    (c, l.accept().await.unwrap().0)
}

async fn assert_eof(mut client: TcpStream) {
    let mut buf = [0u8; 8];
    let r = tokio::time::timeout(Duration::from_secs(1), client.read(&mut buf))
        .await
        .expect("the client did not see the close");
    assert!(matches!(r, Ok(0) | Err(_)));
}

fn target(id: PeerId) -> Opened {
    Opened {
        proto: Proto::Tcp,
        node: id,
        port: 80,
        local: 1,
    }
}

#[tokio::test]
async fn non_member_closes_fast() {
    let net = MemNet::new();
    let stack = node(&net, 5, Probe::new());
    let id = pid(6);
    let (client, accepted) = pair().await;
    let t = Instant::now();
    let _ = forward_to_peer_port(
        &stack,
        &PeerConnPool::new(PORT),
        &Members::default(),
        &target(id),
        accepted,
    )
    .await;
    assert!(t.elapsed() <= Duration::from_secs(1));
    assert_eof(client).await;
}

#[tokio::test]
async fn unreachable_member_closes_in_deadline() {
    // the peer accepts the connection (Probe on PORT) but does not serve the stream: the deadline runs for real
    let net = MemNet::new();
    let (stack, _peer) = linked(&net, Probe::new());
    let id = pid(2);
    let members = Members::default();
    members.add_verified_conn(id);
    let (client, accepted) = pair().await;
    let t = Instant::now();
    let pool = PeerConnPool::new(PORT);
    forward_to_peer_port_within(
        &stack,
        &pool,
        &members,
        &target(id),
        accepted,
        Duration::from_millis(300),
    )
    .await;
    assert!(t.elapsed() <= Duration::from_millis(1300));
    assert!(
        t.elapsed() >= Duration::from_millis(250),
        "the deadline closed it, not NoRoute"
    );
    assert_eof(client).await;
}

/// Node `seed` on network `net` with handler `handler` on `PORT`.
fn node(net: &MemNet, seed: u8, handler: Arc<dyn ServiceHandler>) -> NetStack {
    NetStackBuilder::new(secret(seed))
        .transport(net.transport(&secret(seed)))
        .serve(PORT, handler)
        .build()
        .expect("stack was built")
}

/// A (seed 1) knows the address of B (seed 2, `handler` on `PORT`).
fn linked(net: &MemNet, handler: Arc<dyn ServiceHandler>) -> (NetStack, NetStack) {
    let (a, b) = (node(net, 1, Probe::new()), node(net, 2, handler));
    a.add_hints(b.invite_addrs()[0].clone());
    (a, b)
}
