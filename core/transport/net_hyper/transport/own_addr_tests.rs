//! `own_hints`: a node gives its address only when it is a persistent node that peers can reach.

use super::*;
use crate::transport::net::addr::AddrHint;
use std::net::{SocketAddr, SocketAddrV4};
use townjump_dht::NodeMode;

mod persistent_and_reachable {
    use super::*;

    #[test]
    fn gives_the_mode_address() {
        // given
        let mode = reachable();
        // when
        let hints = own_hints(&mode);
        // then
        assert_eq!(hints, vec![AddrHint::Ip(SocketAddr::V4(public()))]);
    }
}

mod firewalled {
    use super::*;

    #[test]
    fn gives_nothing() {
        let mode = NodeMode {
            firewalled: true,
            ..reachable()
        };
        assert!(own_hints(&mode).is_empty());
    }
}

mod not_persistent {
    use super::*;

    #[test]
    fn gives_nothing() {
        let mode = NodeMode {
            persistent: false,
            ..reachable()
        };
        assert!(own_hints(&mode).is_empty());
    }
}

mod no_address {
    use super::*;

    #[test]
    fn gives_nothing() {
        let mode = NodeMode {
            address: None,
            ..reachable()
        };
        assert!(own_hints(&mode).is_empty());
    }
}

mod default_mode {
    use super::*;

    #[test]
    fn gives_nothing() {
        assert!(own_hints(&NodeMode::default()).is_empty());
    }
}

fn public() -> SocketAddrV4 {
    "203.0.113.5:4000".parse().unwrap()
}

fn reachable() -> NodeMode {
    NodeMode {
        persistent: true,
        adaptive: false,
        firewalled: false,
        address: Some(public()),
    }
}
