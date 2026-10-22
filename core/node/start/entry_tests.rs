//! Tests for `entry.rs`: the heap-profile dump directory.

use super::*;
use std::path::PathBuf;

fn cfg() -> NodeConfig {
    NodeConfig::for_loopback_test(crate::members::network_key::NetworkKey::generate().unwrap())
}

#[test]
fn heap_dump_dir_beside_peers_cache() {
    let mut c = cfg();
    c.peers_cache_path = Some(PathBuf::from("/data/a/peers.json"));
    c.peer_names_path = Some(PathBuf::from("/data/b/names.json"));
    assert_eq!(heap_dump_dir(&c), PathBuf::from("/data/a"));
}

#[test]
fn heap_dump_dir_falls_back_to_names() {
    let mut c = cfg();
    c.peers_cache_path = None;
    c.peer_names_path = Some(PathBuf::from("/data/b/names.json"));
    assert_eq!(heap_dump_dir(&c), PathBuf::from("/data/b"));
}

#[test]
fn heap_dump_dir_defaults_to_current() {
    let mut c = cfg();
    c.peers_cache_path = None;
    c.peer_names_path = None;
    assert_eq!(heap_dump_dir(&c), PathBuf::from("."));
}
