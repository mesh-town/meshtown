//! An opened port of a neighbor counts the bytes of its dials as traffic "through the peer".

use crate::exit::ExitTraffic;
use crate::members::membership::Members;
use crate::ports_sharing::open::Opener;
use crate::ports_sharing::test_support::{linked, rate_of, serving_on, tcp_echo};
use crate::ports_sharing::{Opened, Proto};
use crate::transport::net::facade::test_support::{pid, within};
use crate::transport::net::MemNet;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::test]
async fn opened_port_dial_counts_as_through_peer() {
    // given: B publishes an echo port; A opens it locally with a registry
    let echo = tcp_echo().await;
    let net = MemNet::new();
    let (a, _b) = linked(&net, serving_on(Proto::Tcp, echo));
    let members = Members::default();
    members.add_verified_conn(pid(2));
    let traffic = ExitTraffic::default();
    let opener = Opener::new_counted(a, members, traffic.clone());
    let local = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    opener.reconcile_listeners(vec![Opened {
        proto: Proto::Tcp,
        node: pid(2),
        port: echo,
        local,
    }]);

    // when: bytes go through the local listener and back
    let mut tcp = within("local connect", TcpStream::connect(("127.0.0.1", local)))
        .await
        .expect("connect");
    tcp.write_all(&[7u8; 10_000]).await.unwrap();
    let mut back = vec![0u8; 10_000];
    within("echo", tcp.read_exact(&mut back))
        .await
        .expect("echo");

    // then: the neighbor's speed shows up as "through the peer" only
    let rate = rate_of(&traffic, pid(2)).await;
    assert!(rate.through_peer_bps > 0, "no speed: {rate:?}");
    assert_eq!(rate.through_us_bps, 0);
}
