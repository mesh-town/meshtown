use super::*;
use crate::iface_bind::test_support::{loopback_name, MISSING_IFACE};
use crate::iface_bind::IfaceState;
use socket2::{Domain, SockRef, Socket, Type};
use std::io;

mod bind_socket {
    use super::*;

    // given an IPv4 socket, when bound to loopback, then the kernel reports that device
    #[test]
    fn device_reads_back() {
        let sock = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap();
        let name = loopback_name();
        if !bound(&sock, &name, false) {
            return;
        }
        assert_eq!(
            SockRef::from(&sock).device().unwrap(),
            Some(name.into_bytes())
        );
    }

    // given an IPv6 socket, when bound to loopback, then the kernel reports that device
    #[test]
    fn device_reads_back_v6() {
        let Ok(sock) = Socket::new(Domain::IPV6, Type::STREAM, None) else {
            eprintln!("skipped: IPv6 sockets are not available");
            return;
        };
        let name = loopback_name();
        if !bound(&sock, &name, true) {
            return;
        }
        assert_eq!(
            SockRef::from(&sock).device().unwrap(),
            Some(name.into_bytes())
        );
    }

    // given a missing interface, when binding, then the error kind is NotFound
    #[test]
    fn missing_device_is_not_found() {
        let sock = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap();
        let err = match bind_socket(&sock, MISSING_IFACE, false) {
            Ok(()) => panic!("binding to a missing interface must fail"),
            Err(e) => e,
        };
        if err.kind() == io::ErrorKind::PermissionDenied {
            eprintln!("skipped: binding to a device needs CAP_NET_RAW on this kernel");
            return;
        }
        assert_eq!(err.kind(), io::ErrorKind::NotFound, "{err:?}");
    }
}

mod state {
    use super::*;

    // given the loopback interface, when asked for its state, then it is up
    #[test]
    fn loopback_is_up() {
        assert_eq!(state_of(&loopback_name()), IfaceState::Up);
    }
}

/// Binds the socket; false (test skipped) on kernels that demand CAP_NET_RAW.
fn bound(sock: &Socket, name: &str, ipv6: bool) -> bool {
    match bind_socket(sock, name, ipv6) {
        Ok(()) => true,
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            eprintln!("skipped: binding to a device needs CAP_NET_RAW on this kernel");
            false
        }
        Err(e) => panic!("bind_socket({name}) failed: {e}"),
    }
}
