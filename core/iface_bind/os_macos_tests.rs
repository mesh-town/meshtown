use super::*;
use crate::iface_bind::test_support::{loopback_name, MISSING_IFACE};
use crate::iface_bind::IfaceState;
use socket2::{Domain, SockRef, Socket, Type};
use std::ffi::CString;
use std::io;

mod bind_socket {
    use super::*;

    // given an IPv4 socket, when bound to loopback, then the bound interface index reads back
    #[test]
    fn bound_if_reads_back() {
        let sock = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap();
        let name = loopback_name();
        bind_socket(&sock, &name, false).expect("bind to loopback");
        let got = SockRef::from(&sock).device_index_v4().unwrap();
        assert_eq!(got.map(|i| i.get()), Some(system_index(&name)));
    }

    // given an IPv6 socket, when bound to loopback, then the bound interface index reads back
    #[test]
    fn bound_if_reads_back_v6() {
        let Ok(sock) = Socket::new(Domain::IPV6, Type::STREAM, None) else {
            eprintln!("skipped: IPv6 sockets are not available");
            return;
        };
        let name = loopback_name();
        bind_socket(&sock, &name, true).expect("bind to loopback");
        let got = SockRef::from(&sock).device_index_v6().unwrap();
        assert_eq!(got.map(|i| i.get()), Some(system_index(&name)));
    }

    // given a missing interface, when binding, then the error kind is NotFound
    #[test]
    fn missing_interface_is_not_found() {
        let sock = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap();
        let result = bind_socket(&sock, MISSING_IFACE, false);
        let err = result.expect_err("binding to a missing interface must fail");
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

fn system_index(name: &str) -> u32 {
    let c = CString::new(name).unwrap();
    // SAFETY: `c` is a valid NUL-terminated string that outlives the call.
    let index = unsafe { libc::if_nametoindex(c.as_ptr()) };
    assert_ne!(index, 0, "if_nametoindex({name}) found nothing");
    index
}
