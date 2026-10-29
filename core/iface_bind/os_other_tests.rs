use super::*;
use crate::iface_bind::IfaceState;
use socket2::{Domain, Socket, Type};
use std::io;

mod unsupported_system {
    use super::*;

    // given a system without interface binding, when asked for any state, then it is unsupported
    #[test]
    fn always_unsupported() {
        for name in ["lo", "wg0", "mtnone0"] {
            assert_eq!(state_of(name), IfaceState::Unsupported, "{name}");
        }
    }

    // given a system without interface binding, when binding a socket, then it fails as unsupported
    #[test]
    fn binding_is_refused() {
        let sock = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap();
        let err = bind_socket(&sock, "lo", false).expect_err("binding must be refused");
        assert_eq!(err.kind(), io::ErrorKind::Unsupported, "{err:?}");
    }
}
