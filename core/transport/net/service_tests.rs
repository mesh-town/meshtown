//! Tests for `net/service.rs`: `ServiceId` and the service constants.
//! The constants must match the protocol ALPNs (the `*_ALPN` values).

use super::*;
use std::collections::HashSet;

mod constants_match_the_alpn_of_the_protocols {
    use super::*;

    #[test]
    fn auth_is_the_handshake_alpn() {
        assert_eq!(AUTH.as_bytes(), crate::members::handshake::AUTH_ALPN);
    }

    #[test]
    fn invite_is_the_invite_alpn() {
        assert_eq!(INVITE.as_bytes(), crate::invite::wire::INVITE_ALPN);
    }

    #[test]
    fn exit_is_the_exit_alpn() {
        // `exit::frame` is private, the constant is reached through the `exit` re-export
        assert_eq!(EXIT.as_bytes(), crate::exit::EXIT_ALPN);
    }

    #[test]
    fn port_is_the_ports_alpn() {
        assert_eq!(PORT.as_bytes(), crate::ports_sharing::frame::PORT_ALPN);
    }
}

mod constants_text {
    use super::*;

    #[test]
    fn are_the_versioned_names() {
        assert_eq!(AUTH.as_bytes(), b"meshtown/auth/1");
        assert_eq!(EXIT.as_bytes(), b"meshtown/exit/1");
        assert_eq!(PORT.as_bytes(), b"meshtown/port/1");
        assert_eq!(INVITE.as_bytes(), b"meshtown/invite/1");
    }

    #[test]
    fn alpn_all_meshtown_v1() {
        let all = [AUTH, EXIT, PORT, INVITE, SYNC, REPLICA];
        for id in all {
            let text = std::str::from_utf8(id.as_bytes()).unwrap();
            assert!(text.starts_with("meshtown/"), "{text}");
            assert!(text.ends_with("/1"), "{text}");
        }
        let set: HashSet<ServiceId> = all.into_iter().collect();
        assert_eq!(set.len(), 6);
        assert_eq!(AUTH.as_bytes(), crate::members::handshake::AUTH_ALPN);
        assert_eq!(PORT.as_bytes(), crate::ports_sharing::frame::PORT_ALPN);
        assert_eq!(EXIT.as_bytes(), crate::exit::EXIT_ALPN);
        assert_eq!(INVITE.as_bytes(), crate::invite::wire::INVITE_ALPN);
    }

    #[test]
    fn are_distinct_from_each_other() {
        let set: HashSet<ServiceId> = [AUTH, EXIT, PORT, INVITE].into_iter().collect();
        assert_eq!(set.len(), 4, "four services — four different identifiers");
    }
}

mod service_id {
    use super::*;

    #[test]
    fn as_bytes_returns_what_new_got() {
        assert_eq!(ServiceId::new(b"test/svc/1").as_bytes(), b"test/svc/1");
    }

    #[test]
    fn new_is_usable_in_a_const() {
        const CUSTOM: ServiceId = ServiceId::new(b"test/const/1");
        assert_eq!(CUSTOM.as_bytes(), b"test/const/1");
    }

    #[test]
    fn ids_with_equal_bytes_are_equal_and_hash_alike() {
        let a = ServiceId::new(b"test/svc/1");
        let b = ServiceId::new(b"test/svc/1");
        assert_eq!(a, b);
        let set: HashSet<ServiceId> = [a, b].into_iter().collect();
        assert_eq!(set.len(), 1, "equal identifiers collapse in the set");
    }

    #[test]
    fn ids_with_different_bytes_are_different() {
        assert_ne!(ServiceId::new(b"test/svc/1"), ServiceId::new(b"test/svc/2"));
    }
}
