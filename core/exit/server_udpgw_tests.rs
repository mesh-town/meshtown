//! The conventional UDP gateway target in `serve_exit_stream`: recognized before any address lookup,
//! ordinary targets keep their behavior.

use super::*;
use crate::exit::frame::{encode_exit_request, EXIT_CODE_ADDRESS_FORBIDDEN};
use crate::exit::udpgw::{UDPGW_HOST, UDPGW_PORT};
use crate::transport::net::facade::test_support::pid;
use std::time::Duration;
use tokio::io::{duplex, split};
use tokio::net::{TcpListener, UdpSocket};

fn cfg(overrides: HashMap<String, SocketAddr>) -> ExitConfig {
    let stub = SocketAddr::from(([127, 0, 0, 1], 1));
    let resolver = Resolver::new(
        stub,
        crate::site_checks::http_min::HttpLimits::default(),
        "meshtown-test".to_string(),
    )
    .expect("resolver");
    let cfg = ExitConfig {
        enabled: Arc::new(AtomicBool::new(true)),
        members: Members::default(),
        list: Arc::new(OnceLock::new()),
        resolver: Arc::new(resolver),
        host_overrides: Arc::new(overrides),
        clients: Clients::default(),
        traffic: ExitTraffic::default(),
    };
    cfg.members.add_verified_conn(pid(1));
    cfg
}

/// Runs `serve_exit_stream` on an in-memory stream, returns the client side and the finishing task.
fn open(
    cfg: &ExitConfig,
) -> (
    tokio::io::ReadHalf<tokio::io::DuplexStream>,
    tokio::io::WriteHalf<tokio::io::DuplexStream>,
    tokio::task::JoinHandle<()>,
) {
    let (client_end, server_end) = duplex(1 << 20);
    let (server_read, server_write) = split(server_end);
    let conn = cfg.clients.register_conn(pid(1)).conn();
    let cfg = cfg.clone();
    let task = crate::node::output::spawn(async move {
        let _ = serve_exit_stream(
            cfg,
            pid(1),
            conn,
            Box::new(server_write),
            Box::new(server_read),
        )
        .await;
    });
    let (from, to) = split(client_end);
    (from, to, task)
}

async fn echo_udp() -> SocketAddr {
    let sock = UdpSocket::bind("127.0.0.1:0").await.expect("echo");
    let addr = sock.local_addr().expect("addr");
    crate::node::output::spawn(async move {
        let mut buf = [0u8; 1024];
        while let Ok((n, from)) = sock.recv_from(&mut buf).await {
            let _ = sock.send_to(&buf[..n], from).await;
        }
    });
    addr
}

#[tokio::test]
async fn gateway_target_gets_ok_and_carries_a_datagram_both_ways() {
    let echo = echo_udp().await;
    let cfg = cfg(HashMap::from([("echo.test".to_string(), echo)]));
    let (mut from, mut to, task) = open(&cfg);
    to.write_all(&encode_exit_request(UDPGW_HOST, UDPGW_PORT).expect("frame"))
        .await
        .expect("request");
    let mut reply = [0u8; 2];
    from.read_exact(&mut reply).await.expect("reply");
    assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_OK]);
    // frame: LEN FLAGS(data) CONN_ID(7) ATYP(domain) 9 "echo.test" PORT payload
    let mut body = vec![0x02, 0, 7, 3, 9];
    body.extend_from_slice(b"echo.test");
    body.extend_from_slice(&[0, 9]);
    body.extend_from_slice(b"hi");
    let mut wire = (body.len() as u16).to_be_bytes().to_vec();
    wire.extend_from_slice(&body);
    to.write_all(&wire).await.expect("datagram");
    let mut head = [0u8; 2];
    tokio::time::timeout(Duration::from_secs(5), from.read_exact(&mut head))
        .await
        .expect("in time")
        .expect("len");
    let mut answer = vec![0u8; usize::from(u16::from_be_bytes(head))];
    from.read_exact(&mut answer).await.expect("body");
    assert_eq!(&answer[..3], &[0x02, 0, 7]);
    assert!(answer.ends_with(b"hi"));
    drop((from, to));
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("stream ended")
        .expect("no panic");
}

#[tokio::test]
async fn gateway_target_is_refused_for_a_non_member() {
    let cfg = cfg(HashMap::new());
    cfg.enabled.store(false, Ordering::Relaxed);
    let (mut from, mut to, _task) = open(&cfg);
    to.write_all(&encode_exit_request(UDPGW_HOST, UDPGW_PORT).expect("frame"))
        .await
        .expect("request");
    let mut reply = [0u8; 2];
    from.read_exact(&mut reply).await.expect("reply");
    assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_REFUSED]);
}

#[tokio::test]
async fn same_host_on_another_port_is_still_a_closed_tcp_target() {
    let cfg = cfg(HashMap::new());
    let (mut from, mut to, _task) = open(&cfg);
    to.write_all(&encode_exit_request(UDPGW_HOST, UDPGW_PORT + 1).expect("frame"))
        .await
        .expect("request");
    let mut reply = [0u8; 2];
    from.read_exact(&mut reply).await.expect("reply");
    assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_ADDRESS_FORBIDDEN]);
}

#[tokio::test]
async fn ordinary_tcp_target_still_connects() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let site = listener.local_addr().expect("addr");
    crate::node::output::spawn(async move {
        let _ = listener.accept().await;
        std::future::pending::<()>().await;
    });
    let cfg = cfg(HashMap::from([("site.test".to_string(), site)]));
    let (mut from, mut to, _task) = open(&cfg);
    to.write_all(&encode_exit_request("site.test", 80).expect("frame"))
        .await
        .expect("request");
    let mut reply = [0u8; 2];
    from.read_exact(&mut reply).await.expect("reply");
    assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_OK]);
}
