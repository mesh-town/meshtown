//! Tests of the list wiring (`access_wiring.rs`): the boot input is built in a stable order, and a started node
//! either runs the boot and the watcher (live) or leaves the view open (the loopback preset).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use super::*;
use crate::config::config_file::save_invited_peers;
use crate::members::access::record::BOOT_KEY;
use crate::members::network_key::NetworkKey;
use crate::transport::net::contract::pid;
use crate::transport::net::{AddrHint, PeerAddr};
use crate::transport::peers_cache::entry_for;

const LIMIT: Duration = Duration::from_secs(5);

fn cfg_with_invite(ids: &[PeerId]) -> NodeConfig {
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    cfg.initial_peers = ids.iter().map(|id| PeerAddr::new(*id)).collect();
    cfg
}

fn remember(cache: &mut PeersCache, id: PeerId, seen: u64) {
    let hint = AddrHint::Ip("127.0.0.1:9".parse::<SocketAddr>().unwrap());
    assert!(cache.remember(entry_for(None, id, Some(&hint), seen), seen));
}

#[test]
fn initial_ids_follow_the_invite_without_repeats() {
    // given: an invite that names one node twice
    let cfg = cfg_with_invite(&[pid(3), pid(2), pid(3)]);
    // when: the boot input is built
    let st = access_start(&cfg, pid(1), &PeersCache::load(None, "net", 1_000));
    // then: ids keep the invite order, once each
    assert_eq!(st.boot.initial, vec![pid(3), pid(2)]);
    assert_eq!(st.boot.me, pid(1));
}

#[test]
fn cached_ids_go_freshest_first_then_by_id() {
    // given: a cache with three nodes, two of them seen at the same time
    let mut cache = PeersCache::load(None, "net", 1_000);
    remember(&mut cache, pid(2), 1_000);
    remember(&mut cache, pid(5), 2_000);
    remember(&mut cache, pid(4), 2_000);
    // when: the boot input is built
    let st = access_start(&cfg_with_invite(&[]), pid(1), &cache);
    // then: the freshest come first, equal times by id
    let mut tie = vec![pid(4), pid(5)];
    tie.sort();
    assert_eq!(st.boot.cached, [tie, vec![pid(2)]].concat());
}

#[test]
fn the_flag_of_the_config_is_carried() {
    let cache = PeersCache::load(None, "net", 1_000);
    assert!(access_start(&cfg_with_invite(&[]), pid(1), &cache).open_list);
    let mut live = cfg_with_invite(&[]);
    live.open_list = false;
    assert!(!access_start(&live, pid(1), &cache).open_list);
}

async fn has_boot_marker(node: &crate::node::RunningNode) -> bool {
    let rows = node.domains.shared_replica().scan(BOOT_KEY).await.unwrap();
    !rows.is_empty()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn open_list_leaves_the_boot_alone_and_the_view_ready() {
    // given / when: a loopback node (open list)
    let node = crate::node::start_node(cfg_with_invite(&[])).await.unwrap();
    // then: no boot marker and the view is ready at once
    assert!(!has_boot_marker(&node).await, "the boot must not run");
    tokio::time::timeout(Duration::from_millis(200), node.members.access().ready())
        .await
        .expect("ready at once");
    node.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_live_list_runs_the_boot_and_publishes_the_view() {
    // given / when: a node with the list consulted
    let mut cfg = cfg_with_invite(&[]);
    cfg.open_list = false;
    let node = crate::node::start_node(cfg).await.unwrap();
    // then: the view is ready within the limit and the boot marker is in the shared replica
    tokio::time::timeout(LIMIT, node.members.access().ready())
        .await
        .expect("the view must become ready");
    assert!(
        has_boot_marker(&node).await,
        "the boot marker must be written"
    );
    node.shutdown().await;
}

fn slot_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mesh-wiring-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A slot directory with an invited peers file naming one host; returns the directory and the settings path.
fn slot_with_host(name: &str) -> (PathBuf, PathBuf, PeerId) {
    let dir = slot_dir(name);
    let slot_file = dir.join("config.json");
    let host = pid(7);
    save_invited_peers(&slot_file, &[PeerAddr::new(host)]).unwrap();
    (dir.clone(), slot_file, host)
}

fn invited_exists(dir: &Path) -> bool {
    dir.join("invited-peers.txt").exists()
}

#[test]
fn a_finished_boot_forgets_the_file() {
    for written in [0usize, 3] {
        let (dir, slot_file, _) = slot_with_host(&format!("done{written}"));
        forget_after_boot(&Ok::<usize, String>(written), Some(&slot_file));
        assert!(!invited_exists(&dir), "written = {written}");
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[test]
fn a_failed_boot_keeps_the_file() {
    let (dir, slot_file, _) = slot_with_host("failed");
    forget_after_boot(&Err::<usize, String>("no".into()), Some(&slot_file));
    assert!(invited_exists(&dir));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_failed_flush_keeps_the_file() {
    // given: a finished boot whose flush failed
    let (dir, slot_file, _) = slot_with_host("flush");
    let res = saved(
        &Ok::<usize, String>(1),
        Err::<(), String>("disk full".into()),
    );
    // when: the file is to be forgotten
    forget_after_boot(&res, Some(&slot_file));
    // then: the file stays for the next start; a flush that worked lets it go
    assert!(invited_exists(&dir));
    forget_after_boot(
        &saved(&Ok::<usize, String>(1), Ok::<(), String>(())),
        Some(&slot_file),
    );
    assert!(!invited_exists(&dir));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn no_path_means_nothing_to_forget() {
    let (dir, _, _) = slot_with_host("nopath");
    forget_after_boot(&Ok::<usize, String>(1), None);
    assert!(invited_exists(&dir));
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_live_list_boot_removes_the_invited_file() {
    // given: a live list and a slot whose invited peers file names the host
    let (dir, slot_file, host) = slot_with_host("live");
    let mut cfg = cfg_with_invite(&[host]);
    cfg.open_list = false;
    cfg.invited_peers_file = Some(slot_file);
    // when: the node starts and the boot finishes
    let node = crate::node::start_node(cfg).await.unwrap();
    tokio::time::timeout(LIMIT, node.members.access().ready())
        .await
        .expect("the view must become ready");
    // then: the file is gone and the boot marker is written
    assert!(!invited_exists(&dir), "the file must be removed");
    assert!(has_boot_marker(&node).await);
    node.shutdown().await;
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_open_list_leaves_the_invited_file() {
    // given: an open list and a slot with an invited peers file
    let (dir, slot_file, host) = slot_with_host("open");
    let mut cfg = cfg_with_invite(&[host]);
    cfg.invited_peers_file = Some(slot_file);
    // when: the node starts (no boot)
    let node = crate::node::start_node(cfg).await.unwrap();
    // then: the file stays
    assert!(invited_exists(&dir));
    node.shutdown().await;
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn access_start_carries_the_rules_of_the_net_kind() {
    // given: a peer network and a managed one
    let mut cfg = cfg_with_invite(&[]);
    let cache = PeersCache::load(None, "net", 1_000);
    let peer = access_start(&cfg, pid(1), &cache);
    cfg.net_kind = crate::members::network_key::NetKind::Managed { root: pid(7) };
    // when: the boot input is built for the managed one
    let managed = access_start(&cfg, pid(1), &cache);
    // then: the boot and the watcher get the same rules
    assert_eq!((peer.boot.rules, peer.rules), (Rules::Peer, Rules::Peer));
    let want = Rules::Managed { root: pid(7) };
    assert_eq!((managed.boot.rules, managed.rules), (want, want));
}
