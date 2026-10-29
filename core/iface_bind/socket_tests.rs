use super::*;
use crate::iface_bind::test_support::{loopback_name, MISSING_IFACE};
use crate::iface_bind::{BindError, IfaceState};
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

const WAIT: Duration = Duration::from_secs(5);
const QUIET: Duration = Duration::from_millis(300);

mod state {
    use super::*;

    // given the loopback interface, when asked for its state, then it is up
    #[test]
    fn loopback_is_up() {
        assert_eq!(state_of(&loopback_name()), IfaceState::Up);
    }

    // given a name no machine has, when asked for its state, then it is missing
    #[test]
    fn missing_is_missing() {
        assert_eq!(state_of(MISSING_IFACE), IfaceState::Missing);
    }
}

mod checking {
    use super::*;

    // given the loopback interface, when checked, then it is usable
    #[test]
    fn check_loopback_ok() {
        let result = check(&loopback_name());
        assert!(result.is_ok(), "loopback must pass the check: {result:?}");
    }

    // given a missing interface, when checked, then the error names it as missing
    #[test]
    fn check_missing_fails() {
        let err = check(MISSING_IFACE).expect_err("a missing interface must fail");
        assert!(
            matches!(&err, BindError::Missing(n) if n == MISSING_IFACE),
            "expected Missing({MISSING_IFACE}), got {err:?}"
        );
    }
}

mod classify_maps_error_kinds {
    use super::*;

    // given a permission error, when classified, then it is NoPermission with the name
    #[test]
    fn permission_denied_is_no_permission() {
        let err = classify("wg0", io::Error::from(io::ErrorKind::PermissionDenied));
        assert!(matches!(&err, BindError::NoPermission(n) if n == "wg0"), "{err:?}");
    }

    // given a not-found error, when classified, then it is Missing with the name
    #[test]
    fn not_found_is_missing() {
        let err = classify("wg0", io::Error::from(io::ErrorKind::NotFound));
        assert!(matches!(&err, BindError::Missing(n) if n == "wg0"), "{err:?}");
    }

    // given an unsupported error, when classified, then it is Unsupported
    #[test]
    fn unsupported_is_unsupported() {
        let err = classify("wg0", io::Error::from(io::ErrorKind::Unsupported));
        assert!(matches!(err, BindError::Unsupported), "{err:?}");
    }

    // given any other error, when classified, then it is Os and keeps the error
    #[test]
    fn other_is_os() {
        let err = classify("wg0", io::Error::from(io::ErrorKind::TimedOut));
        let kept = matches!(&err, BindError::Os(n, e) if n == "wg0" && e.kind() == io::ErrorKind::TimedOut);
        assert!(kept, "{err:?}");
    }
}

mod tcp {
    use super::*;

    // given no interface, when connecting to a listener, then the listener accepts
    #[tokio::test]
    async fn tcp_unbound_connects() {
        let (listener, addr) = listen_v4().await;
        let stream = connect_tcp(addr, None).await;
        assert!(stream.is_ok(), "plain connect failed: {stream:?}");
        assert!(accepts_within(listener, WAIT).await, "listener saw no connection");
    }

    // given the loopback interface, when connecting to a local listener, then the listener accepts
    #[tokio::test]
    async fn tcp_bound_to_loopback_connects() {
        let (listener, addr) = listen_v4().await;
        let stream = connect_tcp(addr, named(&loopback_name())).await;
        assert!(stream.is_ok(), "bound connect failed: {stream:?}");
        assert!(accepts_within(listener, WAIT).await, "listener saw no connection");
    }

    // given a missing interface, when connecting, then it fails and the listener sees nothing
    #[tokio::test]
    async fn tcp_bound_to_missing_never_reaches_listener() {
        let (listener, addr) = listen_v4().await;
        let result = connect_tcp(addr, named(MISSING_IFACE)).await;
        assert!(result.is_err(), "connect through a missing interface must fail");
        assert!(
            !accepts_within(listener, QUIET).await,
            "the listener must not receive a connection"
        );
    }

    // given an IPv6 listener and the loopback interface, when connecting, then it is accepted
    #[tokio::test]
    async fn tcp_v6_bound_to_loopback_connects() {
        let Ok(listener) = TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).await else {
            eprintln!("skipped: [::1] is not available");
            return;
        };
        let addr = listener.local_addr().unwrap();
        let stream = connect_tcp(addr, named(&loopback_name())).await;
        assert!(stream.is_ok(), "bound IPv6 connect failed: {stream:?}");
        assert!(accepts_within(listener, WAIT).await, "listener saw no connection");
    }
}

mod udp {
    use super::*;

    // given no interface, when exchanging datagrams with a local echo, then the reply arrives
    #[test]
    fn udp_unbound() {
        let (server, addr) = echo_server();
        let client = bind_udp(addr, None).expect("plain bind");
        assert!(client.local_addr().unwrap().ip().is_unspecified());
        assert_eq!(echo(&client, &server, addr), b"ping");
    }

    // given the loopback interface, when exchanging datagrams, then the reply arrives
    #[test]
    fn udp_bound_to_loopback() {
        let (server, addr) = echo_server();
        let client = bind_udp(addr, Some(&loopback_name())).expect("bound bind");
        assert!(client.local_addr().unwrap().ip().is_unspecified());
        assert_eq!(echo(&client, &server, addr), b"ping");
    }

    // given a missing interface, when binding, then it fails
    #[test]
    fn udp_bound_to_missing_fails() {
        let (_server, addr) = echo_server();
        let result = bind_udp(addr, Some(MISSING_IFACE));
        assert!(result.is_err(), "bind through a missing interface must fail");
    }

    // given an IPv6 destination, when binding without interface, then the socket is IPv6
    #[test]
    fn udp_family_follows_destination() {
        let dest = SocketAddr::from((Ipv6Addr::LOCALHOST, 9));
        let Ok(client) = bind_udp(dest, None) else {
            eprintln!("skipped: IPv6 sockets are not available");
            return;
        };
        assert!(client.local_addr().unwrap().is_ipv6());
    }
}

fn named(name: &str) -> Option<Arc<str>> {
    Some(Arc::from(name))
}

async fn listen_v4() -> (TcpListener, SocketAddr) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    (listener, addr)
}

/// Whether the listener accepts a connection within `limit`.
async fn accepts_within(listener: TcpListener, limit: Duration) -> bool {
    tokio::time::timeout(limit, listener.accept()).await.is_ok_and(|r| r.is_ok())
}

fn echo_server() -> (UdpSocket, SocketAddr) {
    let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    server.set_read_timeout(Some(WAIT)).unwrap();
    let addr = server.local_addr().unwrap();
    (server, addr)
}

/// Sends "ping" from `client` to `server` and returns what comes back.
fn echo(client: &UdpSocket, server: &UdpSocket, addr: SocketAddr) -> Vec<u8> {
    client.set_read_timeout(Some(WAIT)).unwrap();
    client.send_to(b"ping", addr).unwrap();
    let mut buf = [0u8; 64];
    let (n, from) = server.recv_from(&mut buf).expect("server got no datagram");
    server.send_to(&buf[..n], from).unwrap();
    let (n, _) = client.recv_from(&mut buf).expect("client got no reply");
    buf[..n].to_vec()
}
