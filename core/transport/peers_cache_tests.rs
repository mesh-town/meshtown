//! Tests of the cache of last known member addresses (`peers_cache.rs`).

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use crate::transport::net::{AddrHint, NodeSecret, PeerAddr, PeerId, RelayRef};

use super::*;
use crate::transport::net_mainline::record::RosterEntry;

#[path = "peers_cache_load_tests.rs"]
mod load;

#[path = "peers_cache_remember_tests.rs"]
mod remember;

// ---------- building a record (`entry_for`) ----------

#[test]
fn entry_for_puts_selected_ip_first_and_caps_addrs() {
    let known_addrs: Vec<SocketAddr> = (0..5u8).map(sock_addr).collect();
    let known = PeerAddr {
        id: endpoint_id(1),
        hints: known_addrs.iter().map(|a| AddrHint::Ip(*a)).collect(),
    };
    let selected = known_addrs[4];

    let entry = entry_for(
        Some(&known),
        endpoint_id(1),
        Some(&AddrHint::Ip(selected)),
        1000,
    );
    assert_eq!(
        entry.addrs.first(),
        Some(&selected),
        "the chosen address must be first"
    );
    assert_eq!(
        entry.addrs.len(),
        MAX_ADDRS,
        "the total number of addresses must be at most MAX_ADDRS"
    );
    assert_eq!(entry.relay, None);
}

#[test]
fn entry_for_uses_selected_relay_over_known() {
    let relay_a = RelayRef::Url("https://relay-a.example./".to_string());
    let relay_b = RelayRef::Url("https://relay-b.example./".to_string());
    let known = PeerAddr {
        id: endpoint_id(1),
        hints: vec![AddrHint::Relay(relay_a)],
    };

    let entry = entry_for(
        Some(&known),
        endpoint_id(1),
        Some(&AddrHint::Relay(relay_b)),
        1000,
    );
    assert_eq!(
        entry.relay,
        Some("https://relay-b.example./".to_string()),
        "the chosen relay must win"
    );
}

#[test]
fn entry_for_without_known_uses_only_selected() {
    let addr = sock_addr(1);
    let entry = entry_for(None, endpoint_id(1), Some(&AddrHint::Ip(addr)), 1000);
    assert_eq!(entry.addrs, vec![addr]);
    assert_eq!(entry.relay, None);
}

// ---------- writing and reading back ----------

#[test]
fn save_then_load_roundtrip_same_network() {
    let dir = scratch("roundtrip");
    let file = dir.join(PEERS_CACHE_FILE_NAME);
    let mut cache = PeersCache::load(Some(&file), "net", 1000);
    let e1 = entry_for(
        None,
        endpoint_id(1),
        Some(&AddrHint::Ip(sock_addr(1))),
        1000,
    );
    let e2 = entry_for(
        None,
        endpoint_id(2),
        Some(&AddrHint::Ip(sock_addr(2))),
        1000,
    );
    assert!(cache.remember(e1, 1000));
    assert!(cache.remember(e2, 1000));
    cache.save().expect("save must succeed");

    let loaded = PeersCache::load(Some(&file), "net", 1000);
    let addrs = loaded.addrs();
    let mut ids: Vec<[u8; 32]> = addrs.iter().map(|a| *a.id.as_bytes()).collect();
    ids.sort();
    let mut expected = vec![id_bytes(1), id_bytes(2)];
    expected.sort();
    assert_eq!(
        ids, expected,
        "addrs() after a round trip must match by members"
    );
    for a in &addrs {
        let want = if *a.id.as_bytes() == id_bytes(1) {
            sock_addr(1)
        } else {
            sock_addr(2)
        };
        assert_eq!(ip_hints(a), vec![want]);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file)
            .expect("file metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "permissions of the cache file {mode:o}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_without_directory_returns_error_with_path_not_panic() {
    let dir = scratch("no-dir");
    let file = dir.join("no-such-directory").join(PEERS_CACHE_FILE_NAME);
    let cache = PeersCache::load(Some(&file), "net", 1000);

    let err = cache
        .save()
        .expect_err("no directory — save must return Err");
    assert!(
        err.contains("no-such-directory") || err.contains(&file.display().to_string()),
        "the error text must contain the path: {err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------- `peers_cache_path_beside` ----------

#[test]
fn path_beside_replaces_config_file_name() {
    let config = Path::new("/a/b/config.json");
    let cache_path = peers_cache_path_beside(config).expect("the path has a parent");
    assert_eq!(cache_path, PathBuf::from("/a/b/peers-cache.json"));
}

// ---------- helpers ----------

/// A temporary directory of the test; at the end of the test the caller itself calls `remove_dir_all`.
fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-peers-cache-{name}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("could not create the test's temporary directory");
    dir
}

/// 32-byte public key of member `n` — as the member id in records and in `PeerId`.
fn id_bytes(n: u8) -> [u8; 32] {
    *NodeSecret::from_bytes(&[n; 32]).peer_id().as_bytes()
}

/// `PeerId` of member `n`, for calls to `entry_for`/`remember`/`touch`.
fn endpoint_id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

/// Member address of member `n` — different ports on the local host, so the addresses do not collide.
fn sock_addr(n: u8) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 10_000 + n as u16))
}

/// Record of member `n` for the file-reading tests — directly, without `entry_for`.
fn sample_entry(n: u8, seen_unix: u64) -> RosterEntry {
    RosterEntry {
        id: id_bytes(n),
        relay: None,
        addrs: vec![sock_addr(n)],
        seen_unix,
    }
}

/// Writes a test cache file in the `CacheFile` format directly — for the cases of corruption, version, network.
fn write_cache_file(path: &Path, version: u8, network_id: &str, peers: Vec<RosterEntry>) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create the test directory");
    }
    let content = serde_json::to_string(&CacheFile {
        version,
        network_id: network_id.to_string(),
        peers,
    })
    .expect("serialize the test cache file");
    std::fs::write(path, content).expect("write the test cache file");
}

/// Direct addresses from the hints of `addr`, in the hint order.
fn ip_hints(addr: &PeerAddr) -> Vec<SocketAddr> {
    let ips = addr.hints.iter().filter_map(|h| match h {
        AddrHint::Ip(a) => Some(*a),
        _ => None,
    });
    ips.collect()
}

/// Sorted list of member ids whose addresses `addrs()` returned.
fn ids_in(cache: &PeersCache) -> Vec<[u8; 32]> {
    let mut ids: Vec<[u8; 32]> = cache.addrs().iter().map(|a| *a.id.as_bytes()).collect();
    ids.sort();
    ids
}

/// Record of member `n` in the closed `peers` field of the cache; to check `seen_unix` and addresses
/// without rebuilding through `addrs()`.
fn entry_in(cache: &PeersCache, n: u8) -> &RosterEntry {
    cache
        .peers
        .iter()
        .find(|e| e.id == id_bytes(n))
        .unwrap_or_else(|| panic!("member {n} should be in the cache"))
}
