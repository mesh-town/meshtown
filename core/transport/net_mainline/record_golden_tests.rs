//! Golden tests of the DHT record format (`peer_record.rs`): the expectations are literals recomputed outside the code
//! (`b3sum --derive-key` for the keys, ChaCha20-Poly1305 per RFC 8439 for the sealed bytes).

use super::*;
use crate::members::network_key::NetworkKey;
use crate::transport::net::endpoint_shim::EndpointAddr;
use crate::transport::net::NodeSecret;
use ed25519_dalek::SigningKey;

fn roster_0_1_20() -> Roster {
    Roster {
        version: 1,
        entries: vec![RosterEntry {
            id: [0x33; 32],
            relay: Some("https://relay.example/".to_string()),
            addrs: vec!["203.0.113.5:4000".parse().unwrap()],
            seen_unix: 1_700_000_000,
        }],
    }
}

#[test]
fn sealed_roster_bytes_are_stable() {
    let key = [0x11u8; 32];
    let sealed = seal(&key, &roster_0_1_20(), [0x22; 12]).unwrap();
    const SEALED: &str =
        "2222222222222222222222228474e62d0ea39e44ee51c2cfc831f95c14ce72ab68e1e1c54d6e1cd286a79964791aa36ebcd83fb5f5a4fa719a7969965cee62ba13b932b201d4f0a1ba0635d940373c0b4ce71db417fc25e2330444237e9f45f17f3f46";
    assert_eq!(data_encoding::HEXLOWER.encode(&sealed), SEALED);
    let literal = data_encoding::HEXLOWER.decode(SEALED.as_bytes()).unwrap();
    assert_eq!(open(&key, &literal).unwrap(), roster_0_1_20());
}

#[test]
fn dht_key_and_salt_from_subkeys_are_stable() {
    let sk = NetworkKey::from_bytes([0x44; 32]).subkeys();
    let verifying = SigningKey::from_bytes(&sk.dht_signing_seed)
        .verifying_key()
        .to_bytes();
    assert_eq!(
        data_encoding::HEXLOWER.encode(&verifying),
        "4c917f03ce60ddf2999ac56efaf5e4dfb7776017dcabcf21218cdc1100dc9092"
    );
    assert_eq!(
        data_encoding::HEXLOWER.encode(&sk.dht_salt),
        "a2924737f374bf800fd0e48b56089256"
    );
    assert_eq!(
        data_encoding::HEXLOWER.encode(&sk.roster_key),
        "17a5dac5ca436a4d8a9fff438e1e44365d7990bcbc2ee51bc1eac75f273e3355"
    );
    assert_eq!(RECORD_VERSION, 1);
    assert_eq!(ENTRY_TTL_SECS, 900);
    assert_eq!(MAX_SEALED_BYTES, 990);
    assert_eq!(MAX_ADDR_ROSTER_ENTRIES, 8);
    assert_eq!(MAX_ADDRS_PER_ENTRY, 4);
    assert_eq!(ROSTER_AAD, b"meshtown roster v1");
    assert!(ROSTER_AAD.starts_with(b"meshtown ") && ROSTER_AAD.ends_with(b" v1"));
}

fn entry_of(relay: Option<&str>, ips: &[&str]) -> RosterEntry {
    let id = NodeSecret::from_bytes(&[9; 32]).peer_id();
    let mut addr = EndpointAddr::new(id);
    if let Some(url) = relay {
        addr = addr.with_relay_url(url.parse().unwrap());
    }
    for ip in ips {
        addr = addr.with_ip_addr(ip.parse().unwrap());
    }
    entry_from_addr(
        &crate::transport::net::endpoint_shim::from_endpoint_addr(&addr),
        1_700_000_000,
        &PrimaryIps::NONE,
    )
}

#[test]
fn entry_from_addr_golden() {
    let a = entry_of(
        Some("https://relay.example/"),
        &["10.0.0.5:1", "[2001:db8::1]:2", "203.0.113.5:3"],
    );
    let id = [
        253, 23, 36, 56, 90, 160, 199, 91, 100, 251, 120, 205, 96, 47, 161, 217, 145, 253, 235,
        247, 107, 19, 197, 142, 215, 2, 234, 200, 53, 233, 246, 24,
    ];
    assert_eq!(
        a,
        RosterEntry {
            id,
            relay: Some("https://relay.example/".to_string()),
            addrs: vec![
                "203.0.113.5:3".parse().unwrap(),
                "[2001:db8::1]:2".parse().unwrap(),
                "10.0.0.5:1".parse().unwrap()
            ],
            seen_unix: 1_700_000_000,
        }
    );
    let b = entry_of(
        None,
        &[
            "10.0.0.5:1",
            "192.168.1.2:2",
            "172.16.0.3:3",
            "10.1.1.1:4",
            "100.64.0.9:5",
        ],
    );
    assert_eq!(
        b,
        RosterEntry {
            id,
            relay: None,
            addrs: vec![
                "10.0.0.5:1".parse().unwrap(),
                "10.1.1.1:4".parse().unwrap(),
                "172.16.0.3:3".parse().unwrap(),
                "100.64.0.9:5".parse().unwrap()
            ],
            seen_unix: 1_700_000_000,
        }
    );
}
