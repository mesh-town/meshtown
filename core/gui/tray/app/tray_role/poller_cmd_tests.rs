//! Tests of which node commands reset the snapshot ticker (`poller_cmd.rs`).

use super::*;

use crate::gui::tray::app::local_api::stub_node::{friends, overview_json, work, StubNode};
use crate::gui::tray::app::local_api::tests::{temp_dir, FAKE_TOKEN};
use crate::gui::tray::app::tray_role::poller::test_support::*;

#[test]
fn wants_snapshot_by_command() {
    let slot = || Some("net".to_string());
    let yes = [
        NodeCmd::Refresh,
        NodeCmd::ToggleExit {
            slot: slot(),
            id: "x".to_string(),
        },
        NodeCmd::ClearExits(slot()),
        NodeCmd::SetExitServer {
            slot: slot(),
            on: true,
        },
    ];
    let no = [NodeCmd::Start, NodeCmd::Stop, NodeCmd::CopyInvite(slot())];
    assert!(yes.iter().all(wants_snapshot));
    assert!(!no.iter().any(wants_snapshot));
}

#[tokio::test(flavor = "multi_thread")]
async fn refresh_keeps_the_phase_and_sends_nothing_but_snapshots() {
    // given
    let node = StubNode::start().await;
    node.reply(
        "GET",
        "/api/overview",
        200,
        &overview_json(&[friends(), work()]),
    );
    let dir = temp_dir("poller-refresh");
    std::fs::write(dir.join("web-token"), FAKE_TOKEN).expect("write the token file");
    let mut rig = Rig::start(node.port(), dir.join("web-token"), true, spawn_panics());
    rig.probe.view("a ready view", is_ready);
    let before = node.count("GET", "/api/overview");

    // when
    rig.cmd(NodeCmd::Refresh);
    node.wait_count("GET", "/api/overview", before + 1).await;

    // then
    assert_eq!(node.count("PUT", "/api/config"), 0);
    assert_eq!(node.count("POST", "/api/config"), 0);
    rig.quit();
    assert_eq!(rig.code().await, 0);
    let _ = std::fs::remove_dir_all(&dir);
}
