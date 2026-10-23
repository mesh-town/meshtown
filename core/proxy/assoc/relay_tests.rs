//! The relay against an exit gateway stream made of an in-memory pipe; frames go through the real codec.

use std::future::pending;
use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{duplex, split, AsyncReadExt, AsyncWriteExt, DuplexStream, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use super::super::upstream::{ExitRx, ExitTx};
use super::*;

use crate::exit::udpgw_codec::{self as codec, Frame};
use crate::proxy::socks5::{encode_udp_request, Target};

const WAIT: Duration = Duration::from_secs(5);

struct Rig {
    /// The client side of the control connection.
    control: TcpStream,
    /// The client datagram socket.
    client: UdpSocket,
    relay_addr: SocketAddr,
    /// The exit side of the gateway stream.
    exit_read: ReadHalf<DuplexStream>,
    exit_write: WriteHalf<DuplexStream>,
    task: JoinHandle<RelayStats>,
}

async fn rig_with(
    client_ip: &str,
    idle: Duration,
    stop: impl Future<Output = ()> + Send + 'static,
) -> Rig {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let control = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (served, _) = listener.accept().await.unwrap();
    let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = udp.local_addr().unwrap();
    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let (near, far) = duplex(1 << 20);
    let (near_read, near_write) = split(near);
    let (exit_read, exit_write) = split(far);
    let (tx, rx) = (ExitTx(Box::new(near_write)), ExitRx(Box::new(near_read)));
    let ip = client_ip.parse().unwrap();
    let task =
        crate::node::output::spawn(relay(served, udp, ip, tx, rx, stop, RelayLimits { idle }));
    Rig {
        control,
        client,
        relay_addr,
        exit_read,
        exit_write,
        task,
    }
}

async fn rig() -> Rig {
    rig_with("127.0.0.1", Duration::from_secs(60), pending()).await
}

async fn send_datagram(rig: &Rig, target: &Target, port: u16, data: &[u8]) {
    let bytes = encode_udp_request(target, port, data).unwrap();
    rig.client.send_to(&bytes, rig.relay_addr).await.unwrap();
}

async fn next_frame(rig: &mut Rig) -> Frame {
    timeout(WAIT, codec::read_frame(&mut rig.exit_read))
        .await
        .expect("a frame in time")
        .expect("a valid frame")
}

async fn finished(rig: Rig) -> RelayStats {
    timeout(WAIT, rig.task)
        .await
        .expect("relay ended in time")
        .unwrap()
}

mod forwarding {
    use super::*;

    #[tokio::test]
    async fn client_datagram_reaches_exit_as_frame() {
        let mut rig = rig().await;
        send_datagram(
            &rig,
            &Target::Ip("192.0.2.1".parse().unwrap()),
            53,
            b"query",
        )
        .await;
        let frame = next_frame(&mut rig).await;
        assert_eq!(frame.conn_id, 0);
        assert_eq!(
            frame.target,
            Some(codec::Target::Ip("192.0.2.1:53".parse().unwrap()))
        );
        assert_eq!(frame.data, b"query");
    }

    #[tokio::test]
    async fn ipv6_datagram_uses_conn_id_1() {
        let mut rig = rig().await;
        send_datagram(&rig, &Target::Ip("2001:db8::1".parse().unwrap()), 53, b"q").await;
        assert_eq!(next_frame(&mut rig).await.conn_id, 1);
    }

    #[tokio::test]
    async fn exit_reply_reaches_client_with_source_address() {
        let mut rig = rig().await;
        send_datagram(
            &rig,
            &Target::Ip("192.0.2.1".parse().unwrap()),
            53,
            b"query",
        )
        .await;
        next_frame(&mut rig).await;
        let from: SocketAddr = "192.0.2.1:53".parse().unwrap();
        let reply = codec::encode(&Frame::data(0, from, b"answer".to_vec())).unwrap();
        rig.exit_write.write_all(&reply).await.unwrap();
        let mut buf = [0u8; 512];
        let (n, _) = timeout(WAIT, rig.client.recv_from(&mut buf))
            .await
            .unwrap()
            .unwrap();
        let packet = crate::proxy::socks5::parse_udp_datagram(&buf[..n]).unwrap();
        assert_eq!(
            (packet.target, packet.port, packet.data),
            (Target::Ip(from.ip()), 53, b"answer".to_vec())
        );
    }
}

mod filtering {
    use super::*;

    #[tokio::test]
    async fn datagram_from_another_port_after_first_is_dropped() {
        let mut rig = rig().await;
        let target = Target::Ip("192.0.2.1".parse().unwrap());
        send_datagram(&rig, &target, 53, b"first").await;
        next_frame(&mut rig).await;
        let other = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        other
            .send_to(
                &encode_udp_request(&target, 53, b"intruder").unwrap(),
                rig.relay_addr,
            )
            .await
            .unwrap();
        send_datagram(&rig, &target, 53, b"second").await;
        assert_eq!(next_frame(&mut rig).await.data, b"second");
        rig.control.shutdown().await.unwrap();
        assert_eq!(finished(rig).await.foreign, 1);
    }

    #[tokio::test]
    async fn datagram_from_another_ip_is_dropped() {
        let mut rig = rig_with("192.0.2.1", Duration::from_secs(60), pending()).await;
        send_datagram(&rig, &Target::Ip("192.0.2.9".parse().unwrap()), 53, b"x").await;
        assert!(timeout(
            Duration::from_millis(300),
            codec::read_frame(&mut rig.exit_read)
        )
        .await
        .is_err());
        drop(rig.control);
        assert_eq!(timeout(WAIT, rig.task).await.unwrap().unwrap().foreign, 1);
    }

    #[tokio::test]
    async fn fragmented_datagram_is_dropped() {
        let mut rig = rig().await;
        let mut bytes =
            encode_udp_request(&Target::Ip("192.0.2.1".parse().unwrap()), 53, b"x").unwrap();
        bytes[2] = 1;
        rig.client.send_to(&bytes, rig.relay_addr).await.unwrap();
        assert!(timeout(
            Duration::from_millis(300),
            codec::read_frame(&mut rig.exit_read)
        )
        .await
        .is_err());
        drop(rig.control);
        assert_eq!(
            timeout(WAIT, rig.task).await.unwrap().unwrap().fragmented,
            1
        );
    }
}

mod lifetime {
    use super::*;

    #[tokio::test]
    async fn closing_control_tcp_ends_relay() {
        let rig = rig().await;
        drop(rig.control);
        timeout(WAIT, rig.task)
            .await
            .expect("relay ended in time")
            .unwrap();
    }

    #[tokio::test]
    async fn closing_exit_stream_closes_control_tcp() {
        let mut rig = rig().await;
        drop(rig.exit_write);
        drop(rig.exit_read);
        let mut buf = [0u8; 1];
        let n = timeout(WAIT, rig.control.read(&mut buf))
            .await
            .expect("control closed in time")
            .unwrap_or(0);
        assert_eq!(n, 0);
    }

    #[tokio::test]
    async fn stop_signal_ends_relay() {
        let rig = rig_with("127.0.0.1", Duration::from_secs(60), std::future::ready(())).await;
        timeout(WAIT, rig.task)
            .await
            .expect("relay ended in time")
            .unwrap();
        drop(rig.control);
    }

    #[tokio::test]
    async fn idle_association_ends() {
        let rig = rig_with("127.0.0.1", Duration::from_millis(200), pending()).await;
        timeout(WAIT, rig.task)
            .await
            .expect("relay ended in time")
            .unwrap();
        drop(rig.control);
    }
}

mod accounting {
    use super::super::super::upstream::{Delivery, UpstreamTx};
    use super::*;
    use crate::proxy::socks5::UdpPacket;

    /// A loopback datagram cannot exceed the frame, so the limit is checked at the upstream the relay counts from.
    #[tokio::test]
    async fn oversize_datagram_is_counted_not_sent() {
        let (near, mut far) = duplex(1 << 20);
        let mut tx = ExitTx(Box::new(near));
        let big = UdpPacket {
            target: Target::Ip("192.0.2.1".parse().unwrap()),
            port: 53,
            data: vec![0; 70_000],
        };
        assert!(matches!(tx.send(big).await, Ok(Delivery::Oversize)));
        drop(tx);
        let mut rest = Vec::new();
        far.read_to_end(&mut rest).await.unwrap();
        assert!(rest.is_empty());
    }

    #[tokio::test]
    async fn sent_payload_is_counted() {
        let mut rig = rig().await;
        send_datagram(
            &rig,
            &Target::Ip("192.0.2.1".parse().unwrap()),
            53,
            b"after",
        )
        .await;
        next_frame(&mut rig).await;
        drop(rig.control);
        let stats = timeout(WAIT, rig.task).await.unwrap().unwrap();
        assert_eq!(stats.up_bytes, 5);
    }
}
