//! Helpers shared by the tests of the ports modules: stacks over `MemNet`, a peer with a scripted reply,
//! the ports handler of a publishing node, a loopback UDP echo and the datagram frame.

use std::sync::Arc;

use crate::members::membership::Members;
use crate::ports_sharing::{port_service, Audience, Proto, Published, PublishedPortsTable};
use crate::transport::net::facade::test_support::{pid, secret, Probe};
use crate::transport::net::{
    BoxFuture, MemNet, NetStack, NetStackBuilder, PeerConn, ServiceHandler, StreamPair, PORT,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UdpSocket;

/// Node `seed` on network `net` with handler `handler` on `PORT`.
pub(crate) fn node(net: &MemNet, seed: u8, handler: Arc<dyn ServiceHandler>) -> NetStack {
    NetStackBuilder::new(secret(seed))
        .transport(net.transport(&secret(seed)))
        .serve(PORT, handler)
        .build()
        .expect("stack was built")
}

/// A (seed 1) knows the address of B (seed 2, `handler` on `PORT`).
pub(crate) fn linked(net: &MemNet, handler: Arc<dyn ServiceHandler>) -> (NetStack, NetStack) {
    let (a, b) = (node(net, 1, Probe::new()), node(net, 2, handler));
    a.add_hints(b.invite_addrs()[0].clone());
    (a, b)
}

/// Peer scenario: on each stream reads 3 bytes of the request, writes `reply` and closes the stream.
struct Scripted(Vec<u8>);

pub(crate) fn scripted(reply: &[u8]) -> Arc<dyn ServiceHandler> {
    Arc::new(Scripted(reply.to_vec()))
}

impl ServiceHandler for Scripted {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let reply = self.0.clone();
        Box::pin(async move {
            while let Ok(StreamPair { mut send, mut recv }) = conn.accept().await {
                let _ = recv.read_exact(&mut [0u8; 3]).await;
                let _ = send.write_all(&reply).await;
            }
        })
    }
}

/// Ports handler of B: A (seed 1) is a member, `port` is published on `proto` to everyone.
pub(crate) fn serving_on(proto: Proto, port: u16) -> Arc<dyn ServiceHandler> {
    let members = Members::default();
    members.add_verified_conn(pid(1));
    let row = Published {
        proto,
        port,
        name: "echo".into(),
        audience: Audience::All,
    };
    port_service(members, PublishedPortsTable::new(vec![row]))
}

/// UDP echo on loopback; returns the port.
pub(crate) async fn spawn_udp_echo() -> u16 {
    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = sock.local_addr().unwrap().port();
    crate::node::output::spawn(async move {
        let mut buf = vec![0u8; 65_535];
        while let Ok((n, from)) = sock.recv_from(&mut buf).await {
            let _ = sock.send_to(&buf[..n], from).await;
        }
    });
    port
}

/// A UDP port nobody listens on.
pub(crate) fn free_udp_port() -> u16 {
    let s = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    s.local_addr().unwrap().port()
}

/// The frame of one datagram in a stream: the length of the body (4 bytes, big-endian) and the body.
pub(crate) fn datagram_frame(body: &[u8]) -> Vec<u8> {
    let len = u32::try_from(body.len()).expect("a short datagram");
    [&len.to_be_bytes()[..], body].concat()
}

/// TCP echo on loopback; returns the port.
pub(crate) async fn tcp_echo() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    crate::node::output::spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            crate::node::output::spawn(async move {
                let (mut read, mut write) = tcp.split();
                let _ = tokio::io::copy(&mut read, &mut write).await;
            });
        }
    });
    port
}
