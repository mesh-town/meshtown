//! Golden test of the `peers-cache.json` format (a member id is an array of 32 numbers): the expectations are
//! literals, so they survive a change of the id type. A separate neighbor file: `peers_cache_tests.rs` is at the size ceiling.

use super::*;

use crate::net::{AddrHint, NodeSecret, PeerId, RelayRef};
use crate::net::endpoint_shim::from_endpoint_addr;
use std::net::SocketAddr;

fn id_of(seed_hex: &str) -> PeerId {
    let seed: [u8; 32] = data_encoding::HEXLOWER
        .decode(seed_hex.as_bytes())
        .expect("seed — hex")
        .try_into()
        .expect("seed is 32 bytes");
    NodeSecret::from_bytes(&seed).peer_id()
}

#[test]
fn golden_cache_file_stores_id_as_32_number_array() {
    let id = id_of("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
    // RFC 8032 §7.1 anchor
    assert_eq!(
        id.to_string(),
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
    );
    let dir = std::env::temp_dir().join(format!("mesh-peers-cache-golden-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("test directory");
    let file = dir.join(PEERS_CACHE_FILE_NAME);
    let addr: SocketAddr = "127.0.0.1:4000".parse().expect("address");

    let mut cache = PeersCache::load(Some(&file), "net", 1000);
    let entry = entry_for(None, id, Some(&AddrHint::Ip(addr)), 1000);
    assert!(cache.remember(entry, 1000));
    cache.save().expect("save");

    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("file")).expect("JSON");
    let want = serde_json::json!({
        "version": 1,
        "network_id": "net",
        "peers": [{
            "id": [215, 90, 152, 1, 130, 177, 10, 183, 213, 75, 254, 211, 201, 100, 7, 58,
                   14, 225, 114, 243, 218, 166, 35, 37, 175, 2, 26, 104, 247, 7, 81, 26],
            "relay": null,
            "addrs": ["127.0.0.1:4000"],
            "seen_unix": 1000
        }]
    });
    assert_eq!(doc, want);

    // reading: a literal file (an array of numbers) returns the same id
    let literal = r#"{"version":1,"network_id":"net","peers":[{"id":[215,90,152,1,130,177,10,183,213,75,254,211,201,100,7,58,14,225,114,243,218,166,35,37,175,2,26,104,247,7,81,26],"relay":null,"addrs":["127.0.0.1:4000"],"seen_unix":1000}]}"#;
    std::fs::write(&file, literal).expect("write the literal");
    let loaded = PeersCache::load(Some(&file), "net", 1000);
    assert!(loaded.seen().contains_key(&id), "id of vector 1 was not read");
    let _ = std::fs::remove_dir_all(&dir);
}

fn vec1() -> PeerId {
    id_of("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60")
}

fn temp_file(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("mesh-peers-cache-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("test directory");
    let file = dir.join(PEERS_CACHE_FILE_NAME);
    (dir, file)
}

#[test]
fn file_written_by_0_1_20_loads() {
    use crate::net::PeerAddr;
    let (dir, file) = temp_file("load");
    let literal = r#"{"version":1,"network_id":"net","peers":[{"id":[215,90,152,1,130,177,10,183,213,75,254,211,201,100,7,58,14,225,114,243,218,166,35,37,175,2,26,104,247,7,81,26],"relay":"https://relay.example/","addrs":["203.0.113.5:4000","[2001:db8::5]:4000"],"seen_unix":1000},{"id":[129,57,119,14,168,125,23,95,86,163,84,102,195,76,126,204,203,141,138,145,180,238,55,162,93,246,15,91,143,201,179,148],"relay":null,"addrs":["198.51.100.7:4001"],"seen_unix":900},{"id":[237,73,40,198,40,209,194,198,234,233,3,56,144,89,149,97,41,89,39,58,92,99,249,54,54,193,70,20,172,135,55,209],"relay":"not a url","addrs":[],"seen_unix":800}]}"#;
    std::fs::write(&file, literal).expect("write the literal");
    let cache = PeersCache::load(Some(&file), "net", 1000);
    assert_eq!(cache.len(), 3);
    let seen = cache.seen();
    assert_eq!(seen[&vec1()], 1000);
    assert_eq!(seen[&NodeSecret::from_bytes(&[2u8; 32]).peer_id()], 900);
    assert_eq!(seen[&NodeSecret::from_bytes(&[3u8; 32]).peer_id()], 800);
    let ip = |s: &str| AddrHint::Ip(s.parse().expect("address"));
    let want = vec![
        PeerAddr {
            id: vec1(),
            hints: vec![
                AddrHint::Relay(RelayRef::Url("https://relay.example/".to_string())),
                ip("203.0.113.5:4000"),
                ip("[2001:db8::5]:4000"),
            ],
        },
        PeerAddr {
            id: NodeSecret::from_bytes(&[2u8; 32]).peer_id(),
            hints: vec![ip("198.51.100.7:4001")],
        },
    ];
    let got: Vec<PeerAddr> = cache.addrs();
    assert_eq!(got, want);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The file that 0.1.20 writes for two members (a literal, captured by running the baseline).
const FILE_0_1_20: &str = r#"{
  "version": 1,
  "network_id": "net",
  "peers": [
    {
      "id": [
        215,
        90,
        152,
        1,
        130,
        177,
        10,
        183,
        213,
        75,
        254,
        211,
        201,
        100,
        7,
        58,
        14,
        225,
        114,
        243,
        218,
        166,
        35,
        37,
        175,
        2,
        26,
        104,
        247,
        7,
        81,
        26
      ],
      "relay": "https://relay.example/",
      "addrs": [
        "[2001:db8::5]:4000",
        "203.0.113.5:4000"
      ],
      "seen_unix": 1000
    },
    {
      "id": [
        129,
        57,
        119,
        14,
        168,
        125,
        23,
        95,
        86,
        163,
        84,
        102,
        195,
        76,
        126,
        204,
        203,
        141,
        138,
        145,
        180,
        238,
        55,
        162,
        93,
        246,
        15,
        91,
        143,
        201,
        179,
        148
      ],
      "relay": "https://relay-b.example/",
      "addrs": [],
      "seen_unix": 900
    }
  ]
}"#;

#[test]
fn saved_file_is_byte_equal_to_0_1_20() {
    let (dir, file) = temp_file("save");
    let a = vec1();
    let b = NodeSecret::from_bytes(&[2u8; 32]).peer_id();
    let known_a = crate::net::endpoint_shim::EndpointAddr::new(a)
        .with_relay_url("https://relay.example/".parse().expect("url"))
        .with_ip_addr("203.0.113.5:4000".parse().expect("address"))
        .with_ip_addr("[2001:db8::5]:4000".parse().expect("address"));
    let known_a = from_endpoint_addr(&known_a);
    let v6_a: SocketAddr = "[2001:db8::5]:4000".parse().expect("address");
    let relay_b = AddrHint::Relay(RelayRef::Url("https://relay-b.example/".to_string()));
    let mut cache = PeersCache::load(Some(&file), "net", 1000);
    assert!(cache.remember(entry_for(Some(&known_a), a, Some(&AddrHint::Ip(v6_a)), 1000), 1000));
    assert!(cache.remember(entry_for(None, b, Some(&relay_b), 900), 900));
    cache.save().expect("save");
    assert_eq!(std::fs::read_to_string(&file).expect("file"), FILE_0_1_20);
    let _ = std::fs::remove_dir_all(&dir);
}
