//! Poller tests: our own node started by the tray — exit, "Stop"/"Start", "Quit", the signal (`poller.rs`).

use super::test_support::*;
use super::*;

use std::time::Duration;

use crate::gui::tray::app::local_api::tests::temp_dir;

use super::super::views::STOPPING_VIEW;

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn poller_own_exit_keeps_icon() {
    let kids = Kids::default();
    let _reaper = Reaper(kids.clone());
    let dir = temp_dir("own-exit");
    let port = refused_port().await;
    let spawn = spawn_cmd(&kids, "sh", &["-c", "exit 3"]);
    let mut rig = Rig::start(port, dir.join("web-token"), true, spawn);
    rig.probe.log("node started, pid");
    rig.probe.log("node stopped, code 3");
    rig.probe.view("Absent", is_absent);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!rig.task.is_finished(), "the task is alive");
    assert_eq!(
        kids.pids().len(),
        1,
        "no spawn while polling without a node"
    );
    rig.quit();
    assert_eq!(rig.code().await, 0);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn poller_own_stop_cmd() {
    let kids = Kids::default();
    let _reaper = Reaper(kids.clone());
    let dir = temp_dir("own-stop");
    let port = refused_port().await;
    let spawn = spawn_cmd(&kids, "sleep", &["30"]);
    let mut rig = Rig::start(port, dir.join("web-token"), true, spawn);
    rig.probe.log("node started, pid");
    rig.cmd(NodeCmd::Stop);
    rig.probe.view("Stopping", |v| *v == STOPPING_VIEW);
    rig.probe.log("killing it");
    rig.probe
        .log("node stopped via \"Stop Mesh Town\", code signal");
    rig.probe.view("Absent", is_absent);
    let pid = kids.pids()[0];
    assert!(!alive(pid), "the child is dead");
    rig.quit();
    assert_eq!(rig.code().await, 0);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn poller_own_start_after_stop() {
    let kids = Kids::default();
    let _reaper = Reaper(kids.clone());
    let dir = temp_dir("own-restart");
    let port = refused_port().await;
    let spawn = spawn_cmd(&kids, "sleep", &["30"]);
    let mut rig = Rig::start(port, dir.join("web-token"), true, spawn);
    rig.probe.log("node started, pid");
    rig.cmd(NodeCmd::Stop);
    rig.probe.log("via \"Stop Mesh Town\"");
    rig.probe.view("Absent", is_absent);
    rig.cmd(NodeCmd::Start);
    rig.probe.log("node started, pid");
    rig.probe.view("Starting", |v| {
        v.page == PageState::Starting && v.node == NodeState::Starting
    });
    let pids = kids.pids();
    assert_eq!(pids.len(), 2, "second spawn call");
    assert_ne!(pids[0], pids[1], "new pid");
    rig.quit();
    assert_eq!(rig.code().await, 0, "the second one ended by SIGTERM");
    assert!(!alive(pids[0]) && !alive(pids[1]), "both are dead");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn poller_own_quit_stops() {
    let kids = Kids::default();
    let _reaper = Reaper(kids.clone());
    let dir = temp_dir("own-quit");
    let port = refused_port().await;
    let spawn = spawn_cmd(&kids, "sleep", &["30"]);
    let mut rig = Rig::start(port, dir.join("web-token"), true, spawn);
    rig.probe.log("node started, pid");
    rig.quit();
    rig.probe.log("node stopped via \"Quit\", code signal");
    assert!(!rig.probe.logged("signal: quit"));
    assert!(!rig.probe.logged("killing it"), "SIGTERM was enough");
    let code = rig.code().await;
    assert_eq!(code, 0);
    assert!(!alive(kids.pids()[0]), "the child is dead");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn poller_own_signal_stops() {
    let kids = Kids::default();
    let _reaper = Reaper(kids.clone());
    let dir = temp_dir("own-signal");
    let port = refused_port().await;
    let spawn = spawn_cmd(&kids, "sleep", &["30"]);
    let mut rig = Rig::start(port, dir.join("web-token"), true, spawn);
    rig.probe.log("node started, pid");
    rig.signal();
    rig.probe.log("signal: quit");
    rig.probe.log("node stopped via \"Quit\", code signal");
    assert!(!rig.probe.logged("killing it"), "SIGTERM was enough");
    assert_eq!(rig.code().await, 0);
    assert!(!alive(kids.pids()[0]), "the child is dead");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn poller_own_quit_kills_when_term_ignored() {
    let kids = Kids::default();
    let _reaper = Reaper(kids.clone());
    let dir = temp_dir("own-quit-ignore");
    let port = refused_port().await;
    let spawn = spawn_cmd(&kids, "sh", &["-c", "trap '' TERM; exec sleep 30"]);
    let mut rig = Rig::start(port, dir.join("web-token"), true, spawn);
    rig.probe.log("node started, pid");
    // Let the shell install the trap and exec before the signal arrives.
    tokio::time::sleep(Duration::from_millis(400)).await;
    rig.quit();
    rig.probe.log("killing it");
    rig.probe.log("node stopped via \"Quit\", code signal");
    assert_eq!(rig.code().await, 1);
    assert!(!alive(kids.pids()[0]), "the child is dead");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn poller_stop_drops_cmds() {
    let kids = Kids::default();
    let _reaper = Reaper(kids.clone());
    let dir = temp_dir("own-drop");
    let port = refused_port().await;
    let spawn = spawn_cmd(&kids, "sleep", &["30"]);
    let mut rig = Rig::start(port, dir.join("web-token"), true, spawn);
    rig.probe.log("node started, pid");
    rig.cmd(NodeCmd::Stop);
    rig.probe.view("Stopping", |v| *v == STOPPING_VIEW);
    rig.cmd(NodeCmd::Start);
    rig.probe.log("via \"Stop Mesh Town\"");
    rig.probe.view("Absent", is_absent);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        kids.pids().len(),
        1,
        "Start during the procedure was dropped"
    );
    rig.quit();
    assert_eq!(rig.code().await, 0);
}
