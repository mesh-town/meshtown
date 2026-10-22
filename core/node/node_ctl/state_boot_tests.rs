//! Tests of the cycle stamp in the slot (`NodeCtl::renew_boot_stamp`, `boot_id` in the snapshot and on the log page).

use super::*;
use crate::node::node_ctl::boot_stamp::BootStamp;
use crate::node::output::{log_summary, say};

async fn start_test_node() -> RunningNode {
    let key = NetworkKey::generate().unwrap();
    let mut cfg = NodeConfig::for_loopback_test(key);
    cfg.socks.enabled = false;
    crate::node::start_node(cfg).await.unwrap()
}

#[tokio::test]
async fn snapshot_carries_boot_id_before_first_cycle() {
    let ctl = NodeCtl::new_without_network(&Settings::default());
    let a = ctl.snapshot().boot_id;
    let b = ctl.snapshot().boot_id;
    assert_eq!(a.len(), 8);
    assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(a, b);
}

#[tokio::test]
async fn log_page_boot_id_equals_snapshot_boot_id() {
    let ctl = NodeCtl::new_without_network(&Settings::default());
    assert_eq!(ctl.log_after(0, 1).boot_id, ctl.snapshot().boot_id);
}

#[tokio::test]
async fn new_boot_changes_id_and_resets_uptime() {
    let ctl = NodeCtl::new_without_network(&Settings::default());
    ctl.ctl_shared
        .ctl_state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .boot = BootStamp::backdated(100);
    let old = ctl.snapshot();
    assert!(old.node.uptime_sec >= 100);
    ctl.renew_boot_stamp();
    let new = ctl.snapshot();
    assert_ne!(new.boot_id, old.boot_id);
    assert!(new.node.uptime_sec < 2);
    assert_eq!(ctl.log_after(0, 1).boot_id, new.boot_id);
}

#[tokio::test]
async fn new_boot_keeps_log_seq() {
    let ctl = NodeCtl::new_without_network(&Settings::default());
    say("tb1", "x");
    let last = log_summary(0).last_seq;
    ctl.renew_boot_stamp();
    assert!(log_summary(0).last_seq >= last);
}

#[tokio::test]
async fn attach_and_detach_keep_boot_stamp() {
    let ctl = NodeCtl::new(&Settings::default());
    let id = ctl.snapshot().boot_id;
    let node = start_test_node().await;
    ctl.attach(&node);
    assert_eq!(ctl.snapshot().boot_id, id);
    ctl.detach(NodeStatus::Stopping);
    assert_eq!(ctl.snapshot().boot_id, id);
    node.shutdown().await;
}
