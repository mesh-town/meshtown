//! Poller tests: a foreign node already running on the port — "Quit", the signal, "Stop Mesh Town", a node change (`poller.rs`).

use super::*;
use super::test_support::*;

use std::time::Duration;

use crate::app::local_api::{self, Reply};
use crate::app::local_api::tests::{start_fake, temp_dir, FAKE_TOKEN};

use super::super::run::read_token;
use super::super::views::STOPPING_VIEW;

async fn foreign_exit(signal: bool) {
    let dir = temp_dir("foreign-exit");
    let (handle, node) = start_fake(0, &dir).await;
    let port = handle.shared.port();
    let mut rig = Rig::start(port, dir.join("web-token"), true, spawn_panics());
    rig.probe.log("node is already running on the port");
    rig.probe.view("Ready+Running", is_ready);
    if signal {
        rig.signal();
        rig.probe.log("signal: quit");
    } else {
        rig.quit();
    }
    rig.probe.log("keeps running");
    assert_eq!(rig.code().await, 0);
    assert!(!node.stop_called(), "stop did not arrive");
    let token = read_token(&dir.join("web-token"));
    assert!(token == FAKE_TOKEN, "the fake's token is in place");
    assert!(matches!(
        local_api::get_state(port, &token).await,
        Ok(Reply::Ok(_))
    ));
    handle.shutdown(Duration::from_secs(2)).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn poller_foreign_quit_keeps_node() {
    foreign_exit(false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn poller_foreign_signal_keeps_node() {
    foreign_exit(true).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn poller_foreign_stop_cmd() {
    let dir = temp_dir("foreign-stop");
    let (handle, node) = start_fake(0, &dir).await;
    let port = handle.shared.port();
    let mut rig = Rig::start(port, dir.join("web-token"), true, spawn_panics());
    rig.probe.view("Ready+Running", is_ready);
    rig.cmd(NodeCmd::Stop);
    rig.probe.view("Stopping", |v| *v == STOPPING_VIEW);
    let deadline = std::time::Instant::now() + WAIT;
    while !node.stop_called() {
        assert!(std::time::Instant::now() < deadline, "stop did not arrive");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    handle.shutdown(Duration::from_millis(200)).await;
    rig.probe.log("node stopped via \"Stop Mesh Town\"");
    rig.probe.view("Absent", is_absent);
    rig.quit();
    assert_eq!(rig.code().await, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn poller_foreign_gone_then_new() {
    let dir = temp_dir("foreign-gone");
    let (handle, _node) = start_fake(0, &dir).await;
    let port = handle.shared.port();
    let mut rig = Rig::start(port, dir.join("web-token"), true, spawn_panics());
    rig.probe.view("Ready+Running", is_ready);
    handle.shutdown(Duration::from_millis(200)).await;
    rig.probe.log("node stopped");
    rig.probe.view("Absent", is_absent);
    assert!(!rig.task.is_finished(), "the task is alive");
    let (handle, _node) = start_fake(port, &dir).await;
    rig.probe.log("node is already running on the port");
    rig.probe.view("Ready+Running", is_ready);
    rig.quit();
    assert_eq!(rig.code().await, 0);
    handle.shutdown(Duration::from_secs(2)).await;
    let _ = std::fs::remove_dir_all(&dir);
}
