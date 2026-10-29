use super::*;
use crate::iface_bind::test_support::{loopback_name, MISSING_IFACE};
use crate::iface_bind::IfaceState;
use socket2::{Domain, Socket, Type};
use std::io;
use std::os::windows::io::AsRawSocket;
use windows_sys::Win32::Networking::WinSock::{
    getsockopt, IPPROTO_IP, IPPROTO_IPV6, IPV6_UNICAST_IF, IP_UNICAST_IF,
};

mod bind_socket {
    use super::*;

    // given an IPv4 socket, when bound to loopback, then IP_UNICAST_IF holds the index in network order
    #[test]
    fn unicast_if_reads_back() {
        let sock = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap();
        bind_socket(&sock, &loopback_name(), false).expect("bind to loopback");
        let raw = read_option(&sock, IPPROTO_IP as i32, IP_UNICAST_IF as i32);
        assert_eq!(raw, loopback_index().to_be());
    }

    // given an IPv6 socket, when bound to loopback, then IPV6_UNICAST_IF holds the index in host order
    #[test]
    fn unicast_if_reads_back_v6() {
        let Ok(sock) = Socket::new(Domain::IPV6, Type::STREAM, None) else {
            eprintln!("skipped: IPv6 sockets are not available");
            return;
        };
        bind_socket(&sock, &loopback_name(), true).expect("bind to loopback");
        let raw = read_option(&sock, IPPROTO_IPV6 as i32, IPV6_UNICAST_IF as i32);
        assert_eq!(raw, loopback_index());
    }

    // given an alias no adapter has, when binding, then the error kind is NotFound
    #[test]
    fn unknown_alias_missing() {
        let sock = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap();
        let result = bind_socket(&sock, MISSING_IFACE, false);
        let err = result.expect_err("binding to an unknown alias must fail");
        assert_eq!(err.kind(), io::ErrorKind::NotFound, "{err:?}");
    }
}

mod state {
    use super::*;

    // given an alias no adapter has, when asked for its state, then it is missing
    #[test]
    fn unknown_alias_is_missing() {
        assert_eq!(state_of(MISSING_IFACE), IfaceState::Missing);
    }

    // given the loopback adapter, when asked for its state, then it is up
    #[test]
    fn loopback_is_up() {
        assert_eq!(state_of(&loopback_name()), IfaceState::Up);
    }
}

/// Interface index of the loopback adapter, as the system lists it.
fn loopback_index() -> u32 {
    let name = loopback_name();
    let all = if_addrs::get_if_addrs().unwrap();
    let found = all.into_iter().find(|i| i.name == name && i.ip().is_ipv4());
    found
        .and_then(|i| i.index)
        .expect("loopback interface index")
}

fn read_option(sock: &Socket, level: i32, option: i32) -> u32 {
    let mut value: u32 = 0;
    let mut len = std::mem::size_of::<u32>() as i32;
    // SAFETY: `value` and `len` are live locals of the sizes passed, the socket handle is valid.
    let rc = unsafe {
        getsockopt(
            sock.as_raw_socket() as usize,
            level,
            option,
            (&mut value as *mut u32).cast::<u8>(),
            &mut len,
        )
    };
    assert_eq!(rc, 0, "getsockopt failed: {}", io::Error::last_os_error());
    value
}
