//! The log line of a relay that sits behind a NAT: it says it is reached by key, and a later address is silent.

use super::*;
use std::net::{Ipv4Addr, SocketAddrV4};

mod relay_line {
    use super::*;

    #[test]
    fn key_only_relay_says_reached_by_key() {
        // when
        let line = relay_line(None, Some(key_only()));
        // then
        let want = format!(
            "serving as relay for members of network {NET_NAME}, behind NAT (reached by key)"
        );
        assert_eq!(line, Some(want));
    }

    #[test]
    fn key_only_to_addressed_is_silent() {
        // when: the relay turns out to be reachable at an address
        let line = relay_line(Some(key_only()), Some(addressed()));
        // then
        assert_eq!(line, None);
    }
}

// --- helpers ---

fn key_only() -> RelayAdvert {
    RelayAdvert {
        key: [4; 32],
        address: None,
    }
}

fn addressed() -> RelayAdvert {
    RelayAdvert {
        address: Some(SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 4), 4004)),
        ..key_only()
    }
}
