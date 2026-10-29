//! The UDP gateway against the `exit.interface` setting (`session.rs`): sessions are bound to the interface,
//! an unavailable interface drops the datagram, and a live switch reopens a session instead of letting it
//! carry on through the old path.

use super::*;
use crate::exit::udpgw::test_support::{cfg_with, spawn_echo, start, Client};
use crate::iface_bind::test_support::{loopback_name, MISSING_IFACE};
use crate::iface_bind::{check, BindError, OutIface};
use crate::transport::net::facade::test_support::pid;

mod bound_to_loopback {
    use super::*;

    #[tokio::test]
    async fn session_bound_to_loopback_echoes() {
        // given: the exit is bound to the loopback interface
        let Some(name) = bindable_loopback() else {
            return;
        };
        let (mut client, _task, _iface, echo) = serve_via(Some(&name)).await;

        // when: a datagram is sent to a local echo
        client.send(&to_echo(5, b"ping")).await;

        // then: the echo comes back with the same conn_id
        assert_eq!(client.next().await, Frame::data(5, echo, b"ping".to_vec()));
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn session_socket_is_bound() {
        // given: the exit is bound to the loopback interface
        let Some(name) = bindable_loopback() else {
            return;
        };
        let (mut gateway, _queue) = gateway_via(Some(&name)).await;

        // when: a datagram opens a session
        gateway.handle(to_echo(5, b"x")).await;

        // then: the session socket itself carries the binding
        let session = gateway.sessions.get(&5).expect("the session is open");
        then_bound_to(&session.sock, &name);
    }
}

mod unavailable_interface {
    use super::*;

    #[tokio::test]
    async fn missing_interface_drops_and_reports_once() {
        // given: the exit is bound to an interface that doesn't exist
        let (mut gateway, mut queue) = gateway_via(Some(MISSING_IFACE)).await;

        // when: two datagrams arrive for the same conn_id
        gateway.handle(to_echo(7, b"one")).await;
        gateway.handle(to_echo(7, b"two")).await;

        // then: no session is opened, nothing is answered, and the conn_id is recorded as refused
        assert!(gateway.sessions.is_empty(), "no socket may open");
        assert!(queue.try_recv().is_err(), "nothing is sent back");
        assert!(
            gateway.refused.contains(&7),
            "the refusal is recorded once per conn_id"
        );
    }

    #[tokio::test]
    async fn live_switch_reopens_udp_session() {
        // given: a session opened through loopback that echoes
        let Some(name) = bindable_loopback() else {
            return;
        };
        let (mut client, _task, iface, echo) = serve_via(Some(&name)).await;
        client.send(&to_echo(5, b"one")).await;
        assert_eq!(client.next().await, Frame::data(5, echo, b"one".to_vec()));

        // when: the interface is switched to a missing one
        iface.set(Some(MISSING_IFACE));
        client.send(&to_echo(5, b"two")).await;

        // then: the old session is not reused: the datagram is dropped
        assert!(
            client.silent().await,
            "a session of the old interface must not carry on"
        );

        // when: the interface comes back
        iface.set(Some(&name));
        client.send(&to_echo(5, b"three")).await;

        // then: the session is opened again
        assert_eq!(client.next().await, Frame::data(5, echo, b"three".to_vec()));
    }
}

mod same_interface {
    use super::*;

    #[tokio::test]
    async fn same_interface_keeps_the_session() {
        // when: the same conn_id asks for a session twice without a switch, with no interface
        // then: both calls return the very same socket
        assert_session_reused(None).await;

        // and the same holds bound to loopback (skipped where binding to a device is refused)
        if let Some(name) = bindable_loopback() {
            assert_session_reused(Some(&name)).await;
        }
    }

    async fn assert_session_reused(iface: Option<&str>) {
        let (mut gateway, _queue) = gateway_via(iface).await;
        let dest: SocketAddr = "127.0.0.1:9".parse().expect("address");
        let first = gateway
            .session(5, dest)
            .expect("first session")
            .sock
            .clone();
        let second = gateway
            .session(5, dest)
            .expect("second session")
            .sock
            .clone();
        assert!(
            Arc::ptr_eq(&first, &second),
            "iface {iface:?}: the session was reopened"
        );
    }
}

/// A datagram for the local echo, named `echo.test`.
fn to_echo(conn_id: u16, data: &[u8]) -> Frame {
    Frame {
        flags: FLAG_DATA,
        conn_id,
        target: Some(Target::Domain("echo.test".into(), 9)),
        data: data.to_vec(),
    }
}

fn exit_via(iface: Option<&str>, echo: SocketAddr) -> ExitConfig {
    let cfg = cfg_with(HashMap::from([("echo.test".to_string(), echo)]));
    cfg.iface.set(iface);
    cfg
}

/// The gateway on an in-memory stream with the exit bound to `iface`; the handle switches it later.
async fn serve_via(iface: Option<&str>) -> (Client, JoinHandle<()>, OutIface, SocketAddr) {
    let echo = spawn_echo().await;
    let cfg = exit_via(iface, echo);
    let handle = cfg.iface.clone();
    let (client, task) = start(cfg, LIMITS);
    (client, task, handle, echo)
}

/// The per-stream state itself, to look at sessions and refusals; replies land in the returned queue.
async fn gateway_via(iface: Option<&str>) -> (Gateway, mpsc::Receiver<Vec<u8>>) {
    let echo = spawn_echo().await;
    let (replies, queue) = mpsc::channel(REPLY_QUEUE);
    let gateway = Gateway {
        cfg: exit_via(iface, echo),
        peer: pid(1),
        limits: LIMITS,
        replies,
        sessions: HashMap::new(),
        allowed: HashSet::new(),
        refused: HashSet::new(),
    };
    (gateway, queue)
}

/// The loopback interface name, or `None` (the test is skipped) where the kernel refuses binding to a device.
fn bindable_loopback() -> Option<String> {
    let name = loopback_name();
    match check(&name) {
        Ok(()) => Some(name),
        Err(BindError::NoPermission(_)) => {
            eprintln!("skipped: binding to a device needs CAP_NET_RAW on this kernel");
            None
        }
        Err(e) => panic!("the loopback interface {name} must be usable: {e}"),
    }
}

#[cfg(target_os = "linux")]
fn then_bound_to(sock: &UdpSocket, name: &str) {
    let device = socket2::SockRef::from(sock)
        .device()
        .expect("device option");
    assert_eq!(device, Some(name.as_bytes().to_vec()), "bound device");
}

#[cfg(target_os = "macos")]
fn then_bound_to(sock: &UdpSocket, name: &str) {
    let c_name = std::ffi::CString::new(name).expect("interface name");
    // SAFETY: `c_name` is a valid NUL-terminated string that outlives the call.
    let index = unsafe { libc::if_nametoindex(c_name.as_ptr()) };
    let bound = socket2::SockRef::from(sock)
        .device_index_v4()
        .expect("bound interface option");
    assert_eq!(bound.map(|i| i.get()), Some(index), "bound interface index");
}
