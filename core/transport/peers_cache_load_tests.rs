//! Tests for `peers_cache.rs`: reading the file (`load`).

use super::*;

// ---------- reading the file (`load`) ----------

#[test]
fn load_without_path_is_empty_and_save_is_ok() {
    let cache = PeersCache::load(None, "net", 1000);
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
    assert!(
        cache.save().is_ok(),
        "save of a disabled cache must not return an error"
    );
}

#[test]
fn load_missing_file_is_empty() {
    let dir = scratch("load-missing");
    let file = dir.join(PEERS_CACHE_FILE_NAME);
    let cache = PeersCache::load(Some(&file), "net", 1000);
    assert!(cache.is_empty());
    assert!(!file.exists(), "load must not create a file");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_corrupted_json_is_empty_without_panic() {
    let dir = scratch("load-corrupt");
    let file = dir.join(PEERS_CACHE_FILE_NAME);
    std::fs::write(&file, "{not json").expect("write a broken file");

    let mut cache = PeersCache::load(Some(&file), "net", 1000);
    assert!(
        cache.is_empty(),
        "corrupted JSON must be read as an empty cache"
    );

    // after remember + save the file is read normally again
    let entry = entry_for(
        None,
        endpoint_id(1),
        Some(&AddrHint::Ip(sock_addr(1))),
        1000,
    );
    assert!(cache.remember(entry, 1000));
    cache
        .save()
        .expect("save after file corruption must succeed");

    let reloaded = PeersCache::load(Some(&file), "net", 1000);
    assert_eq!(reloaded.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_rejects_other_format_version() {
    let dir = scratch("load-version");
    let file = dir.join(PEERS_CACHE_FILE_NAME);
    write_cache_file(
        &file,
        PEERS_CACHE_FORMAT_VERSION + 1,
        "net",
        vec![sample_entry(1, 1000)],
    );

    let cache = PeersCache::load(Some(&file), "net", 1000);
    assert!(
        cache.is_empty(),
        "a file of another version must be read as an empty cache"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_rejects_other_network() {
    let dir = scratch("load-network");
    let file = dir.join(PEERS_CACHE_FILE_NAME);
    write_cache_file(
        &file,
        PEERS_CACHE_FORMAT_VERSION,
        "other-network",
        vec![sample_entry(1, 1000)],
    );

    let cache = PeersCache::load(Some(&file), "net", 1000);
    assert!(
        cache.is_empty(),
        "a file of another network must be read as an empty cache"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_drops_entries_older_than_ttl() {
    let dir = scratch("load-ttl");
    let file = dir.join(PEERS_CACHE_FILE_NAME);
    let now = 10_000_000u64;
    let stale = sample_entry(1, now - TTL_SECS - 1);
    let fresh = sample_entry(2, now - 10);
    write_cache_file(&file, PEERS_CACHE_FORMAT_VERSION, "net", vec![stale, fresh]);

    let cache = PeersCache::load(Some(&file), "net", now);
    assert_eq!(
        cache.len(),
        1,
        "an entry stale by TTL must be dropped on read"
    );
    let ids = ids_in(&cache);
    assert_eq!(ids, vec![id_bytes(2)], "only the fresh entry must remain");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_caps_at_max_peers_keeping_freshest() {
    let dir = scratch("load-cap");
    let file = dir.join(PEERS_CACHE_FILE_NAME);
    let now = 10_000_000u64;
    // n=1 is the freshest (seen_unix = now-1), n=40 is the oldest (seen_unix = now-40)
    let peers: Vec<RosterEntry> = (1u8..=40)
        .map(|n| sample_entry(n, now - n as u64))
        .collect();
    write_cache_file(&file, PEERS_CACHE_FORMAT_VERSION, "net", peers);

    let cache = PeersCache::load(Some(&file), "net", now);
    assert_eq!(
        cache.len(),
        MAX_PEERS,
        "above the ceiling exactly MAX_PEERS entries must remain"
    );
    let ids = ids_in(&cache);
    assert!(
        ids.contains(&id_bytes(1)),
        "the freshest member must survive"
    );
    assert!(
        !ids.contains(&id_bytes(40)),
        "the oldest member must be evicted"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn addrs_skips_entry_with_bad_relay_url() {
    let dir = scratch("load-bad-relay");
    let file = dir.join(PEERS_CACHE_FILE_NAME);
    let now = 1000u64;
    let mut broken = sample_entry(1, now);
    broken.relay = Some("not a url".to_string());
    broken.addrs.clear();
    let good = sample_entry(2, now);
    write_cache_file(&file, PEERS_CACHE_FORMAT_VERSION, "net", vec![broken, good]);

    let cache = PeersCache::load(Some(&file), "net", now);
    assert_eq!(
        cache.len(),
        2,
        "load keeps both entries, even the corrupted one"
    );
    let addrs = cache.addrs();
    assert_eq!(
        addrs.len(),
        1,
        "addrs() must silently skip the corrupted entry"
    );
    assert_eq!(*addrs[0].id.as_bytes(), id_bytes(2));
    let _ = std::fs::remove_dir_all(&dir);
}
