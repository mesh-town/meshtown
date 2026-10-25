//! Tests for the reason a node refused to start in the poller: the proxy port check before launch and parsing the node log
//! after our own child exits (`start_fail.rs`).

use super::super::poller::test_support::*;
#[cfg(unix)]
use super::super::views::STOPPING_VIEW;
use super::*;

use std::time::Duration;

use crate::gui::tray::app::local_api::tests::temp_dir;
use crate::gui::tray::app::start_note::StartNote;
#[cfg(unix)]
use crate::gui::tray::app::view::{NodeState, PageState};
use crate::gui::tray::NodeCmd;

#[cfg(unix)]
const PORT_ERROR_LOG: &str =
    "Error: port 1080 for the browser proxy is in use by another program (Address already in use (os error 48)).\nClose it or start Mesh Town on another port: meshtown run --socks-port 1081\n";

#[cfg(unix)]
fn is_starting(v: &crate::gui::tray::app::view::View) -> bool {
    v.page == PageState::Starting && v.node == NodeState::Starting
}

/// The last reason the poller gave the tray (`None` means none).
fn last_note(rig: &mut Rig) -> Option<StartNote> {
    rig.probe.drain();
    rig.probe.history.iter().rev().find_map(|s| match s {
        Seen::StartFail(n) => Some(n.clone()),
        Seen::View(_) | Seen::Log(_) | Seen::Exits(_) | Seen::Invite(_) | Seen::Serve(_) => None,
    })
}

#[tokio::test]
async fn busy_proxy_port_sees_listener() {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    assert_eq!(
        busy_proxy_port(Some(port), ListenAccess::Localhost),
        Some(port)
    );
    drop(l);
    assert_eq!(busy_proxy_port(Some(port), ListenAccess::Localhost), None);
    assert_eq!(busy_proxy_port(None, ListenAccess::Localhost), None);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn exit_1_with_port_error_shows_reason() {
    let kids = Kids::default();
    let _reaper = Reaper(kids.clone());
    let dir = temp_dir("fail-port");
    let stub = StartStub::default();
    stub.set_log(Some(PORT_ERROR_LOG));
    let spawn = spawn_cmd(&kids, "sh", &["-c", "exit 1"]);
    let mut rig = Rig::start_with(
        refused_port().await,
        dir.join("web-token"),
        true,
        spawn,
        stub,
    );
    rig.probe.log("node stopped, code 1");
    rig.probe
        .start_note("port busy", |n| *n == StartNote::ProxyWasBusy(1080));
    rig.probe.view("Absent", is_absent);
    rig.quit();
    assert_eq!(rig.code().await, 0);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn exit_1_without_log_shows_code() {
    let kids = Kids::default();
    let _reaper = Reaper(kids.clone());
    let dir = temp_dir("fail-nolog");
    let stub = StartStub::default();
    let spawn = spawn_cmd(&kids, "sh", &["-c", "exit 1"]);
    let mut rig = Rig::start_with(
        refused_port().await,
        dir.join("web-token"),
        true,
        spawn,
        stub,
    );
    let note = rig.probe.start_note("code", |_| true);
    assert_eq!(
        note,
        StartNote::Failed {
            code: Some(1),
            has_node_log: false
        }
    );
    rig.quit();
    assert_eq!(rig.code().await, 0);

    let stub = StartStub::default();
    stub.set_log(Some("+ 0.1s [node] started\n"));
    let spawn = spawn_cmd(&kids, "sh", &["-c", "exit 1"]);
    let mut rig = Rig::start_with(
        refused_port().await,
        dir.join("web-token"),
        true,
        spawn,
        stub,
    );
    let note = rig.probe.start_note("code", |_| true);
    assert_eq!(
        note,
        StartNote::Failed {
            code: Some(1),
            has_node_log: true
        }
    );
    rig.quit();
    assert_eq!(rig.code().await, 0);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn exit_0_leaves_plain_stop() {
    let kids = Kids::default();
    let _reaper = Reaper(kids.clone());
    let dir = temp_dir("fail-exit0");
    let stub = StartStub::default();
    stub.set_log(Some(PORT_ERROR_LOG));
    let spawn = spawn_cmd(&kids, "sh", &["-c", "exit 0"]);
    let mut rig = Rig::start_with(
        refused_port().await,
        dir.join("web-token"),
        true,
        spawn,
        stub,
    );
    rig.probe.view("Absent", is_absent);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(last_note(&mut rig), None, "code 0 is not a refusal");
    rig.quit();
    assert_eq!(rig.code().await, 0);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn manual_stop_leaves_plain_stop() {
    let kids = Kids::default();
    let _reaper = Reaper(kids.clone());
    let dir = temp_dir("fail-stop");
    let stub = StartStub::default();
    stub.set_log(Some(PORT_ERROR_LOG));
    let spawn = spawn_cmd(&kids, "sleep", &["30"]);
    let mut rig = Rig::start_with(
        refused_port().await,
        dir.join("web-token"),
        true,
        spawn,
        stub,
    );
    rig.probe.log("node started, pid");
    rig.cmd(NodeCmd::Stop);
    rig.probe.view("Stopping", |v| *v == STOPPING_VIEW);
    rig.probe.view("Absent", is_absent);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let note = last_note(&mut rig);
    assert!(
        matches!(note, None | Some(StartNote::Idle)),
        "not a refusal: {note:?}"
    );
    rig.quit();
    assert_eq!(rig.code().await, 0);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn start_after_failure_clears_then_fails_again() {
    let kids = Kids::default();
    let _reaper = Reaper(kids.clone());
    let dir = temp_dir("fail-again");
    let stub = StartStub::default();
    stub.set_log(Some(PORT_ERROR_LOG));
    let spawn = spawn_cmd(&kids, "sh", &["-c", "exit 1"]);
    let mut rig = Rig::start_with(
        refused_port().await,
        dir.join("web-token"),
        true,
        spawn,
        stub,
    );
    rig.probe
        .start_note("first refusal", |n| *n == StartNote::ProxyWasBusy(1080));
    rig.probe.view("Absent", is_absent);
    rig.cmd(NodeCmd::Start);
    rig.probe.start_note("erased", |n| *n == StartNote::Idle);
    rig.probe.view("Starting", is_starting);
    rig.probe
        .start_note("second refusal", |n| *n == StartNote::ProxyWasBusy(1080));
    assert_eq!(kids.pids().len(), 2, "two launches");
    rig.quit();
    assert_eq!(rig.code().await, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn busy_proxy_blocks_first_spawn() {
    let dir = temp_dir("fail-busy");
    let stub = StartStub::default();
    stub.set_busy(Some(1080));
    let mut rig = Rig::start_with(
        refused_port().await,
        dir.join("web-token"),
        true,
        spawn_panics(),
        stub,
    );
    rig.probe.log("not starting the node");
    rig.probe
        .start_note("port busy", |n| *n == StartNote::ProxyBusy(1080));
    rig.probe.view("Absent", is_absent);
    rig.cmd(NodeCmd::Start);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(last_note(&mut rig), Some(StartNote::ProxyBusy(1080)));
    rig.quit();
    assert_eq!(
        rig.code().await,
        0,
        "spawn was not called, the poller is alive"
    );
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn freed_port_start_runs_and_clears() {
    let kids = Kids::default();
    let _reaper = Reaper(kids.clone());
    let dir = temp_dir("fail-freed");
    let stub = StartStub::default();
    stub.set_busy(Some(1080));
    let spawn = spawn_cmd(&kids, "sleep", &["30"]);
    let mut rig = Rig::start_with(
        refused_port().await,
        dir.join("web-token"),
        true,
        spawn,
        stub.clone(),
    );
    rig.probe
        .start_note("port busy", |n| *n == StartNote::ProxyBusy(1080));
    rig.probe.view("Absent", is_absent);
    stub.set_busy(None);
    rig.cmd(NodeCmd::Start);
    rig.probe.start_note("erased", |n| *n == StartNote::Idle);
    rig.probe.view("Starting", is_starting);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(last_note(&mut rig), Some(StartNote::Idle));
    assert_eq!(kids.pids().len(), 1, "the node is started");
    rig.quit();
    let _ = rig.code().await;
}

#[tokio::test]
async fn busy_proxy_port_localnet_probes_wildcard() {
    let l = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
    let port = l.local_addr().unwrap().port();
    assert_eq!(
        busy_proxy_port(Some(port), ListenAccess::Localnet),
        Some(port)
    );
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn error_line_in_log_gives_reason() {
    let kids = Kids::default();
    let _reaper = Reaper(kids.clone());
    let dir = temp_dir("fail-reason");
    let stub = StartStub::default();
    stub.set_log(Some("Error: x\n"));
    let spawn = spawn_cmd(&kids, "sh", &["-c", "exit 1"]);
    let mut rig = Rig::start_with(
        refused_port().await,
        dir.join("web-token"),
        true,
        spawn,
        stub,
    );
    let note = rig.probe.start_note("reason", |_| true);
    assert_eq!(note, StartNote::FailedWith("x".into()));
    rig.quit();
    assert_eq!(rig.code().await, 0);
}
