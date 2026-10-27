//! A tray menu command in the poller: dispatched by phase; outside the phase it needs, the command is
//! silently dropped.

use tokio::sync::mpsc::UnboundedReceiver;

use crate::gui::tray::NodeCmd;

use super::overview_poll::fetch_node_reply;
use super::poller::{Poller, QuitTriggers};
use super::poller_exit::ExitEdit;
use super::poller_phase::Phase;

/// Whether the command changes node settings — the menu view must converge to the fact without waiting for a tick.
pub(super) fn changes_node_state(cmd: &NodeCmd) -> bool {
    matches!(
        cmd,
        NodeCmd::ToggleExit { .. } | NodeCmd::ClearExits(_) | NodeCmd::SetExitServer { .. }
    )
}

/// Whether the command asks for a snapshot right away: node edits and an explicit refresh.
pub(super) fn wants_snapshot(cmd: &NodeCmd) -> bool {
    changes_node_state(cmd) || matches!(cmd, NodeCmd::Refresh)
}

/// A tray menu command: dispatched by phase.
pub(super) async fn on_cmd(
    poller: &mut Poller,
    phase: Phase,
    quit_triggers: &mut QuitTriggers,
    cmds: &mut UnboundedReceiver<NodeCmd>,
    cmd: NodeCmd,
) -> Result<Phase, i32> {
    let online = matches!(phase, Phase::OwnNode { .. } | Phase::ForeignNode { .. });
    match cmd {
        NodeCmd::Start if matches!(phase, Phase::Absent) => {
            let fetch =
                fetch_node_reply(poller.port, &poller.token_path, &mut poller.overview_gate);
            match quit_triggers.race_with_quit(fetch).await {
                Ok(reply) => Ok(poller.enter(reply, true, false)),
                Err(by) => Err(poller.quit_tray(by, phase).await),
            }
        }
        NodeCmd::Start => Ok(phase),
        // Commands that arrive during the procedure are dropped by `stop_button` — before the result view.
        NodeCmd::Stop if online => Ok(poller.stop_button(phase, cmds).await),
        NodeCmd::Stop => Ok(phase),
        NodeCmd::ToggleExit { slot, id } if online => {
            poller
                .send_exit_edit(slot.as_deref(), ExitEdit::Toggle(id))
                .await;
            Ok(phase)
        }
        NodeCmd::ToggleExit { .. } => Ok(phase),
        NodeCmd::ClearExits(slot) if online => {
            poller
                .send_exit_edit(slot.as_deref(), ExitEdit::Clear)
                .await;
            Ok(phase)
        }
        NodeCmd::ClearExits(_) => Ok(phase),
        NodeCmd::CopyInvite(slot) if online => {
            poller.copy_invite(slot.as_deref()).await;
            Ok(phase)
        }
        NodeCmd::CopyInvite(_) => Ok(phase),
        NodeCmd::SetExitServer { slot, on } if online => {
            poller.send_exit_server(slot.as_deref(), on).await;
            Ok(phase)
        }
        NodeCmd::SetExitServer { .. } => Ok(phase),
        // Only the tick reset in `step` matters: nothing to send, the phase stays.
        NodeCmd::Refresh => Ok(phase),
    }
}

#[cfg(test)]
#[path = "poller_cmd_tests.rs"]
mod tests;
