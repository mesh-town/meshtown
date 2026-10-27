//! Tests of which node commands reset the snapshot ticker (`poller_cmd.rs`).

use super::*;

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
