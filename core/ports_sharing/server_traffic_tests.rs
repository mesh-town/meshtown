//! The publishing side counts a neighbor's bytes on a published port as traffic "through us".

use crate::exit::ExitTraffic;
use crate::members::membership::Members;
use crate::ports_sharing::test_support::{node, rate_of, tcp_echo};
use crate::ports_sharing::{
    encode_port_request, port_service_counted, Audience, PortRequest, Proto, Published,
    PublishedPortsTable,
};
use crate::transport::net::facade::test_support::{pid, within, Probe};
use crate::transport::net::{MemNet, PORT};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn served_port_connection_counts_as_through_us() {
    // given: B publishes an echo port with a shared registry; A is its member
    let echo = tcp_echo().await;
    let members = Members::default();
    members.add_verified_conn(pid(1));
    let row = Published {
        proto: Proto::Tcp,
        port: echo,
        name: "echo".into(),
        audience: Audience::All,
    };
    let traffic = ExitTraffic::default();
    let handler = port_service_counted(
        members,
        PublishedPortsTable::new(vec![row]),
        traffic.clone(),
    );
    let net = MemNet::new();
    let (a, b) = (node(&net, 1, Probe::new()), node(&net, 2, handler));
    a.add_hints(b.invite_addrs()[0].clone());
    let conn = within("A connects to B", a.connect(pid(2), PORT))
        .await
        .expect("connect");

    // when: A opens the port and bytes go both ways
    let mut pair = within("open", conn.open()).await.expect("stream");
    let req = PortRequest {
        proto: Proto::Tcp,
        port: echo,
    };
    pair.send
        .write_all(&encode_port_request(req))
        .await
        .unwrap();
    pair.send.write_all(&[7u8; 10_000]).await.unwrap();
    let mut back = vec![0u8; 2 + 10_000];
    within("echo", pair.recv.read_exact(&mut back))
        .await
        .expect("echo");

    // then: the peer's speed shows up as "through us" only
    let rate = rate_of(&traffic, pid(1)).await;
    assert!(rate.through_us_bps > 0, "no speed: {rate:?}");
    assert_eq!(rate.through_peer_bps, 0);
}
