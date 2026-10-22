//! Tests of the format and encryption of the DHT member record (`peer_record.rs`).

use super::*;
use crate::transport::addr_order::PrimaryIps;
use crate::transport::net::endpoint_shim::{EndpointAddr, TransportAddr};
use crate::transport::net::NodeSecret;

#[test]
fn seal_open_roundtrip() {
    let key = [1u8; 32];
    let roster = Roster {
        version: RECORD_VERSION,
        entries: vec![RosterEntry {
            id: [2u8; 32],
            relay: Some("https://relay.example./".to_string()),
            addrs: vec!["1.2.3.4:5".parse().unwrap()],
            seen_unix: 100,
        }],
    };
    let sealed = seal(&key, &roster, [3u8; 12]).unwrap();
    let opened = open(&key, &sealed).unwrap();
    assert_eq!(roster, opened);
}

#[test]
fn open_fails_with_other_key() {
    let key = [1u8; 32];
    let other_key = [9u8; 32];
    let roster = Roster {
        version: RECORD_VERSION,
        entries: vec![],
    };
    let sealed = seal(&key, &roster, [3u8; 12]).unwrap();
    let err = open(&other_key, &sealed).unwrap_err();
    assert!(err.to_string().contains("key of a different network"));
}

#[test]
fn open_fails_on_tampering() {
    let key = [1u8; 32];
    let roster = Roster {
        version: RECORD_VERSION,
        entries: vec![RosterEntry {
            id: [2u8; 32],
            relay: None,
            addrs: vec![],
            seen_unix: 1,
        }],
    };
    let mut sealed = seal(&key, &roster, [3u8; 12]).unwrap();
    let last = sealed.len() - 1;
    sealed[last] ^= 0xFF;
    let err = open(&key, &sealed).unwrap_err();
    assert!(err.to_string().contains("key of a different network"));
}

/// Builds the input for `merge_drops_self_stale_and_caps`: our own entry, a stale version of ourselves,
/// one entry expired by TTL and 10 live entries with different `seen_unix`, each with 6 addresses.
fn build_merge_prune_and_cap_input() -> (RosterEntry, Roster) {
    let me = RosterEntry {
        id: [0u8; 32],
        relay: None,
        addrs: vec![],
        seen_unix: 1000,
    };
    let mut prev_entries = Vec::new();
    // an old version of ourselves — must be dropped
    prev_entries.push(RosterEntry {
        id: [0u8; 32],
        relay: None,
        addrs: vec![],
        seen_unix: 999,
    });
    // a stale entry (seen_unix + TTL < now) — must be dropped
    prev_entries.push(RosterEntry {
        id: [1u8; 32],
        relay: None,
        addrs: vec![],
        seen_unix: 1000 - ENTRY_TTL_SECS - 1,
    });
    // 10 live entries with different seen_unix, each with 6 addresses (must be trimmed to 4)
    for i in 2..12u8 {
        let addrs: Vec<SocketAddr> = (0..6u16)
            .map(|p| SocketAddr::from(([127, 0, 0, i], 1000 + p)))
            .collect();
        prev_entries.push(RosterEntry {
            id: [i; 32],
            relay: None,
            addrs,
            seen_unix: 1000 + i as u64,
        });
    }
    let prev = Roster {
        version: RECORD_VERSION,
        entries: prev_entries,
    };
    (me, prev)
}

/// Checks the result of `merge()`: our own entry is first and the only one, the TTL-expired one is gone,
/// the number of entries and addresses is within limits, the order by `seen_unix` is descending.
fn assert_merge_dropped_self_stale_and_capped(merged: &Roster, me: &RosterEntry) {
    assert_eq!(merged.entries[0].id, me.id, "our own entry must be first");
    assert!(merged.entries.len() <= MAX_ADDR_ROSTER_ENTRIES);
    assert!(merged
        .entries
        .iter()
        .all(|e| e.addrs.len() <= MAX_ADDRS_PER_ENTRY));
    assert!(
        !merged.entries.iter().any(|e| e.id == [1u8; 32]),
        "the stale entry must not remain"
    );
    assert_eq!(
        merged.entries.iter().filter(|e| e.id == me.id).count(),
        1,
        "there must be only one own entry"
    );
    let rest = &merged.entries[1..];
    for pair in rest.windows(2) {
        assert!(
            pair[0].seen_unix >= pair[1].seen_unix,
            "the order must be descending"
        );
    }
}

#[test]
fn merge_drops_self_stale_and_caps() {
    let (me, prev) = build_merge_prune_and_cap_input();
    let merged = merge(Some(prev), me.clone(), 1000);
    assert_merge_dropped_self_stale_and_capped(&merged, &me);
}

#[test]
fn seal_fits_limit_by_dropping_oldest() {
    let key = [4u8; 32];
    let relay = "https://aps1-1.relay.n0.iroh.link./";
    let mut entries = Vec::new();
    for i in 0..8u8 {
        let addrs: Vec<SocketAddr> = (0..4u16)
            .map(|p| {
                SocketAddr::from((
                    std::net::Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, i as u16),
                    4000 + p,
                ))
            })
            .collect();
        entries.push(RosterEntry {
            id: [i; 32],
            relay: Some(relay.to_string()),
            addrs,
            seen_unix: 1000 + i as u64,
        });
    }
    let roster = Roster {
        version: RECORD_VERSION,
        entries,
    };
    let own_id = roster.entries[0].id;
    let sealed = seal(&key, &roster, [5u8; 12]).unwrap();
    assert!(sealed.len() <= MAX_SEALED_BYTES);
    let opened = open(&key, &sealed).unwrap();
    assert!(opened.entries.len() < 8);
    assert_eq!(opened.entries[0].id, own_id);
}

mod entry_from_addr_with_route {
    use super::*;

    fn sa(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn five_ipv4_keep_route_address_first_after_truncate() {
        // given
        let id = NodeSecret::from_bytes(&[9u8; 32]).peer_id();
        let addr = EndpointAddr::from_parts(
            id,
            [
                "10.0.0.1:7000",
                "10.0.0.2:7000",
                "10.0.0.3:7000",
                "10.0.0.4:7000",
                "192.168.88.129:7000",
            ]
            .map(|s| TransportAddr::Ip(sa(s))),
        );
        let primary = PrimaryIps {
            v4: Some("192.168.88.129".parse().unwrap()),
            v6: None,
        };
        // when
        let entry = entry_from_addr(&from_endpoint_addr(&addr), 42, &primary);
        // then
        assert_eq!(entry.addrs.len(), 4, "sliced to MAX_ADDRS_PER_ENTRY");
        assert_eq!(
            entry.addrs[0],
            sa("192.168.88.129:7000"),
            "the route address is first and not sliced off: {:?}",
            entry.addrs
        );
    }

    #[test]
    fn entry_keeps_tunnel_last_when_no_public_address() {
        // given
        let id = NodeSecret::from_bytes(&[9u8; 32]).peer_id();
        let addr = EndpointAddr::from_parts(
            id,
            [
                "10.0.0.1:7000",
                "10.0.0.2:7000",
                "10.0.0.3:7000",
                "10.0.0.4:7000",
                "100.64.0.7:7000",
            ]
            .map(|s| TransportAddr::Ip(sa(s))),
        );
        // when
        let entry = entry_from_addr(&from_endpoint_addr(&addr), 42, &PrimaryIps::NONE);
        // then
        assert_eq!(entry.addrs.len(), 4, "{:?}", entry.addrs);
        assert_eq!(entry.addrs[3], sa("100.64.0.7:7000"), "{:?}", entry.addrs);
    }

    #[test]
    fn entry_drops_tunnel_when_public_address_present() {
        // given
        let id = NodeSecret::from_bytes(&[9u8; 32]).peer_id();
        let addr = EndpointAddr::from_parts(
            id,
            [
                "10.0.0.1:7000",
                "10.0.0.2:7000",
                "10.0.0.3:7000",
                "100.64.0.7:7000",
                "93.184.216.34:7000",
            ]
            .map(|s| TransportAddr::Ip(sa(s))),
        );
        // when
        let entry = entry_from_addr(&from_endpoint_addr(&addr), 42, &PrimaryIps::NONE);
        // then
        assert_eq!(entry.addrs.len(), 4, "{:?}", entry.addrs);
        assert!(
            !entry.addrs.contains(&sa("100.64.0.7:7000")),
            "{:?}",
            entry.addrs
        );
    }
}

// --- converters over PeerAddr ---

use crate::transport::net::endpoint_shim::from_endpoint_addr;
use crate::transport::net::{AddrHint, PeerAddr, RelayRef};

fn pid(n: u8) -> crate::transport::net::PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn sample_entry() -> RosterEntry {
    RosterEntry {
        id: *pid(9).as_bytes(),
        relay: Some("https://relay.example/".to_string()),
        addrs: vec![
            "203.0.113.5:3".parse().unwrap(),
            "10.0.0.5:1".parse().unwrap(),
            "[2001:db8::1]:2".parse().unwrap(),
        ],
        seen_unix: 1_700_000_000,
    }
}

#[test]
fn entry_to_peer_golden() {
    let got = entry_to_addr(&sample_entry()).unwrap();
    let mut ips: Vec<std::net::SocketAddr> = vec![
        "203.0.113.5:3".parse().unwrap(),
        "10.0.0.5:1".parse().unwrap(),
        "[2001:db8::1]:2".parse().unwrap(),
    ];
    ips.sort();
    let mut hints = vec![AddrHint::Relay(RelayRef::Url(
        "https://relay.example/".to_string(),
    ))];
    hints.extend(ips.into_iter().map(AddrHint::Ip));
    assert_eq!(got, PeerAddr { id: pid(9), hints });
    assert_eq!(got.hints[1], AddrHint::Ip("10.0.0.5:1".parse().unwrap()));
}

#[test]
fn entry_to_peer_bad_relay_keeps_text() {
    let mut e = sample_entry();
    e.relay = Some("not a url".to_string());
    let new = entry_to_addr(&e).unwrap_err().to_string();
    assert_eq!(new, "Failed to parse relay URL");
}

#[test]
fn entry_to_peer_bad_id_is_error() {
    let bad = (0..=255u8)
        .map(|n| [n; 32])
        .find(|b| crate::transport::net::PeerId::from_bytes(b).is_err())
        .expect("there is an id off the curve");
    let mut e = sample_entry();
    e.id = bad;
    assert!(entry_to_addr(&e).is_err());
}

#[test]
fn entry_from_peer_skips_peer_relay_and_opaque() {
    let addr = PeerAddr {
        id: pid(9),
        hints: vec![
            AddrHint::Relay(RelayRef::Peer(pid(2))),
            AddrHint::Opaque {
                kind: 7,
                bytes: vec![1],
            },
            AddrHint::Relay(RelayRef::Url("https://a.example/".to_string())),
            AddrHint::Relay(RelayRef::Url("https://b.example/".to_string())),
            AddrHint::Ip("203.0.113.5:3".parse().unwrap()),
        ],
    };
    let e = entry_from_addr(&addr, 1, &PrimaryIps::NONE);
    assert_eq!(e.relay.as_deref(), Some("https://a.example/"));
    assert_eq!(e.addrs.len(), 1);
}

#[test]
fn entry_from_peer_empty_hints() {
    let e = entry_from_addr(&PeerAddr::new(pid(9)), 1, &PrimaryIps::NONE);
    assert_eq!(e.relay, None);
    assert!(e.addrs.is_empty());
    assert_eq!(e.id, *pid(9).as_bytes());
}
